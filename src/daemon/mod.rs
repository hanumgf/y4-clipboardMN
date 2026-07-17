// Copyright (C) 2026 yukkkk1
// SPDX-License-Identifier: GPL-3.0-or-later

// src/daemon/mod.rs

use crate::storage::ClipboardDb;
use crate::wayland;
use crate::wayland::state::{WaylandState, ClipboardJob};
use crate::core::constants::*;
use crate::core::SocketGuard;
use std::os::unix::net::{UnixListener, UnixStream};
use std::io::{BufRead, Write};
use std::fs;
use std::os::fd::{AsFd, AsRawFd};
use std::time::Duration;
use std::sync::mpsc;

/// Initialize and run the clipboard daemon with a unified, high-performance event loop.
pub fn start_daemon(mut db: ClipboardDb, verbose: bool) {
    let socket_path = crate::core::get_socket_path();

    if let Ok(mut stream) = UnixStream::connect(&socket_path) {
        let _ = stream.write_all(&[IPC_CMD_EXIT]);
        std::thread::sleep(Duration::from_millis(RECONNECT_DELAY_MS));
    }
    
    let _ = fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path).expect("failed to bind IPC socket");
    let _ = listener.set_nonblocking(true);
    let _guard = SocketGuard::new(socket_path);

    // Initialize Database Worker Thread
    let (job_tx, job_rx) = mpsc::channel::<ClipboardJob>();
    let is_verbose = verbose;

    std::thread::spawn(move || {
        // The worker thread owns the mutable reference to the database.
        while let Ok(job) = job_rx.recv() {
            if let Err(e) = db.insert_with_hash(&job.mime, &job.data, &job.hash) {
                eprintln!("error: worker failed to persist data: {}", e);
            } else if is_verbose {
                println!("{}", log_save(&job.mime, job.data.len()));
            }
            #[cfg(target_os = "linux")]
            unsafe { libc::malloc_trim(0); }
        }
    });

    let (conn, mut event_queue) = wayland::create_connection();
    let qh = event_queue.handle();
    let _registry = conn.display().get_registry(&qh, ());

    // Initialize state with the job sender and a secondary DB handle for reads.
    let read_db = ClipboardDb::open().expect("failed to open read-only database handle");
    let mut state = WaylandState::new_daemon(read_db, job_tx, verbose);
    
    // Pre-load last data for deduplication.
    state.last_data = state.db.as_ref().and_then(|d| d.lock().ok()).and_then(|d| d.get_latest_data()).unwrap_or_default();
    state.target_mime = DEFAULT_MIME.to_string();

    if event_queue.roundtrip(&mut state).is_err() { return; }
    if let (Some(manager), Some(seat)) = (&state.manager, &state.seat) {
        state.device = Some(manager.get_data_device(seat, &qh, ()));
        let _ = conn.flush();
    } else { return; }

    println!("{}{}", LOG_INFO, MSG_DAEMON_START);

    while !crate::core::is_exiting() {
        let _ = event_queue.dispatch_pending(&mut state);
        let _ = conn.flush();

        let mut poll_fds = [
            libc::pollfd { fd: conn.as_fd().as_raw_fd(), events: libc::POLLIN, revents: 0 },
            libc::pollfd { fd: listener.as_fd().as_raw_fd(),  events: libc::POLLIN, revents: 0 },
        ];

        if unsafe { libc::poll(poll_fds.as_mut_ptr(), 2, 500) } < 0 { continue; }

        // 3. IPC Ingress Handling
        if poll_fds[1].revents & libc::POLLIN != 0
            && let Ok((stream, _)) = listener.accept() {
                let mut reader = std::io::BufReader::new(stream);
                let mut buf = Vec::new();

                if reader.read_until(IPC_DELIMITER, &mut buf).is_ok()
                    && buf.len() > 1 {
                        let n = buf.len() - 1; 
                        match buf[0] {
                            IPC_CMD_EXIT => crate::core::request_exit(),
                            IPC_CMD_RESTORE => {
                                let id_str = String::from_utf8_lossy(&buf[1..n]);
                                if let Ok(real_id) = id_str.trim().parse::<i64>() {
                                    handle_restore_request(&mut state, &qh, real_id, &conn);
                                }
                            }
                            _ => {}
                        }
                }
        }

        if poll_fds[0].revents & (libc::POLLHUP | libc::POLLERR) != 0 { break; }
        if poll_fds[0].revents & libc::POLLIN != 0
        && let Some(guard) = event_queue.prepare_read() {
            let _ = guard.read();
        }
    }
}

/// Serve a historical record with narrow lock scope and broad MIME compatibility.
fn handle_restore_request(state: &mut WaylandState, qh: &wayland_client::QueueHandle<WaylandState>, real_id: i64, conn: &wayland_client::Connection) {
    let db_payload = {
        if let Some(ref db_mutex) = state.db {
            if let Ok(db) = db_mutex.lock() {
                db.get_content_by_id(real_id)
            } else { None }
        } else { None }
    };

    if let Some((mime, val)) = db_payload
        && let Some(ref manager) = state.manager {
        state.provider_locks += 1;

        let meta = crate::wayland::state::SourceMetadata {
            mime: mime.clone(),
            data: val,
        };

        let source = manager.create_data_source(qh, meta);
        
        // Broadcaster Strategy: Advertise multiple compatible MIMEs
        source.offer(mime.clone());

        if mime.starts_with("image/") {
            let image_alts = ["image/png", "image/jpeg", "image/webp", "image/gif"];
            for alt in image_alts {
                if *alt != mime { source.offer(alt.to_string()); }
            }
        } else if mime.contains("text") || mime == MIME_URI_LIST {
            for alt in TEXT_MIME_ALTS {
                if *alt != mime { source.offer(alt.to_string()); }
            }
            // Ensure URI lists can be consumed by standard text editors
            if mime == MIME_URI_LIST {
                source.offer("text/plain".to_string());
            }
        }

        if let Some(ref device) = state.device {
            device.set_selection(Some(&source));
            let _ = conn.flush();
        }

        state.current_source = Some(source);
        if state.verbose { 
            println!("{}", log_restore(real_id as usize)); 
        }
    }
}
