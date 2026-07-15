// Copyright (C) 2026 yukkkk1
// SPDX-License-Identifier: GPL-3.0-or-later

// src/daemon/mod.rs

use crate::storage::ClipboardDb;
use crate::wayland;
use crate::wayland::state::WaylandState;
use crate::core::constants::*;
use crate::core::SocketGuard;
use std::os::unix::net::{UnixListener, UnixStream};
use std::io::{Read, Write};
use std::fs;
use std::os::fd::{AsFd, AsRawFd};
use std::time::Duration;

/// Initialize and run the clipboard daemon with a unified, high-performance event loop.
pub fn start_daemon(db: ClipboardDb, verbose: bool) {
    let socket_path = crate::core::get_socket_path();

    // Protocol: Signal handover if an existing instance is found.
    if let Ok(mut stream) = UnixStream::connect(&socket_path) {
        if verbose { println!("{}handover requested from existing instance.", LOG_INFO); }
        let _ = stream.write_all(&[IPC_CMD_EXIT]);
        std::thread::sleep(Duration::from_millis(RECONNECT_DELAY_MS));
    }
    
    let _ = fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path).expect("failed to bind IPC socket");
    listener.set_nonblocking(true).expect("failed to set non-blocking mode");
    
    // RAII Guard: Automated filesystem cleanup.
    let _guard = SocketGuard::new(socket_path);

    let (conn, mut event_queue) = wayland::create_connection();
    let qh = event_queue.handle();
    let _registry = conn.display().get_registry(&qh, ());

    let mut state = WaylandState::new_daemon(db, verbose);
    state.last_data = state.db.as_ref()
        .and_then(|d| d.lock().unwrap().get_latest_data())
        .unwrap_or_default();
    state.target_mime = DEFAULT_MIME.to_string();

    if event_queue.roundtrip(&mut state).is_err() { return; }
    if let (Some(manager), Some(seat)) = (&state.manager, &state.seat) {
        state.device = Some(manager.get_data_device(seat, &qh, ()));
        let _ = conn.flush();
    } else {
        return;
    }

    println!("{}{}", LOG_INFO, MSG_DAEMON_START);

    // --- Main Integrated Loop ---
    while !crate::core::is_exiting() {
        // 1. Consolidated Dispatch: Single point of event processing for user-space buffer.
        // This ensures events are handled before kernel polling to prevent stalls.
        let _ = event_queue.dispatch_pending(&mut state);
        let _ = conn.flush();

        let wayland_fd = conn.as_fd().as_raw_fd();
        let socket_fd = listener.as_fd().as_raw_fd();

        let mut poll_fds = [
            libc::pollfd { fd: wayland_fd, events: libc::POLLIN, revents: 0 },
            libc::pollfd { fd: socket_fd,  events: libc::POLLIN, revents: 0 },
        ];

        // 2. Kernel Polling: Efficient wait for FD activity.
        let poll_res = unsafe { libc::poll(poll_fds.as_mut_ptr(), 2, 500) };
        
        if poll_res < 0 {
            if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted { break; }
            continue;
        }

        // 3. IPC Ingress Handling
        if poll_fds[1].revents & libc::POLLIN != 0
            && let Ok((mut stream, _)) = listener.accept() {
            const IPC_BUF_SIZE: usize = 1 + 20;
            let mut buf = [0u8; IPC_BUF_SIZE]; 
            if let Ok(n) = stream.read(&mut buf) && n > 0 {
                match buf[0] {
                    IPC_CMD_EXIT => {
                        if verbose { println!("{}termination requested via IPC.", LOG_INFO); }
                        crate::core::request_exit();
                    }
                    IPC_CMD_RESTORE => {
                        if n > 1 
                            && let id_str = String::from_utf8_lossy(&buf[1..n])
                            && let Ok(real_id) = id_str.trim().parse::<i64>() {
                            handle_restore_request(&mut state, &qh, real_id, &conn);
                        }
                    }
                    _ => {
                        if verbose { eprintln!("{}unknown IPC command byte: 0x{:02x}", LOG_ERROR, buf[0]); }
                    }
                }
            }
        }

        // 4. Wayland Egress/Ingress Handling
        let wayland_revents = poll_fds[0].revents;
        if wayland_revents & (libc::POLLHUP | libc::POLLERR) != 0 {
            if !crate::core::is_exiting() { 
                eprintln!("{}wayland link severed.", LOG_ERROR); 
            }
            break;
        }

        if wayland_revents & libc::POLLIN != 0 {
            // Read from kernel into library buffer. Actual dispatch occurs at the loop start.
            if let Some(guard) = event_queue.prepare_read() {
                match guard.read() {
                    Ok(_) => {},
                    Err(wayland_client::backend::WaylandError::Io(ie)) if ie.kind() == std::io::ErrorKind::WouldBlock => {},
                    Err(_) => break,
                }
            }
        }
    }
    // Finalization: SocketGuard handles filesystem cleanup on scope exit.
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

