// Copyright (C) 2026 yukkkk1
// SPDX-License-Identifier: GPL-3.0-or-later

// src/wayland/handlers/data_control/device.rs

use wayland_client::{Dispatch, Connection, QueueHandle, Proxy};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_offer_v1::ExtDataControlOfferV1,
};
use std::time::{SystemTime, UNIX_EPOCH};
use std::io::Read;
use std::os::fd::AsFd;
use std::sync::{Arc, Mutex};
use crate::wayland::state::{WaylandState, OfferData};
use crate::core::constants::*;
use super::{make_pipe, is_sensitive};

// --- ExtDataControlDeviceV1 ---

impl Dispatch<ExtDataControlDeviceV1, ()> for WaylandState {
    fn event(state: &mut Self, _: &ExtDataControlDeviceV1, ev: ext_data_control_device_v1::Event, _: &(), conn: &Connection, _: &QueueHandle<Self>) {
        if let ext_data_control_device_v1::Event::Selection { id } = ev {
            state.selection_received = true;

            // Reset provider flag to allow future ingestion of external copy events
            if state.provider_locks > 0 {
                state.provider_locks -= 1;
                return;
            }

            let Some(offer) = id else { return };

            // Isolate offered MIME types for this specific selection event
            let mimes: Vec<String> = offer
                .data::<OfferData>()
                .and_then(|d| d.mimes.lock().ok())
                .map(|g| g.clone())
                .unwrap_or_default();

            if mimes.is_empty() || is_sensitive(&mimes) { return; }

            // Determine optimal MIME type based on modern format availability
            let priority: &[&str] = &[
                "image/webp",
                "image/png",
                "image/jpeg",
                "image/gif",
                MIME_URI_LIST,
                "text/plain;charset=utf-8",
                "text/plain",
            ];

            let mime_to_get = priority.iter()
                .find_map(|&p| mimes.iter().find(|&m| m == p || m.starts_with(&format!("{};", p))))
                .cloned()
                .or_else(|| mimes.iter().find(|m| m.starts_with("image/")).cloned())
                .or_else(|| mimes.iter().find(|m| m.starts_with("text/")).cloned())
                .or_else(|| mimes.first().cloned())
                .unwrap_or_else(|| DEFAULT_MIME.to_string());

            let is_image = mime_to_get.starts_with("image/");
            let (read_file, write_fd) = match make_pipe(is_image) {
                Some(p) => p,
                None => return,
            };

            // Initiate asynchronous data transfer from the compositor
            offer.receive(mime_to_get.clone(), write_fd.as_fd());
            drop(write_fd); 
            let _ = conn.flush();

            // Safe database path extraction to prevent main loop panics on mutex poisoning
            let db_path = state.db.as_ref()
                .and_then(|mutex| mutex.lock().ok())
                .map(|db| db.path.clone());

            if let Some(path) = db_path {
                let is_verbose = state.verbose;

                std::thread::spawn(move || {
                    let mut hash_context = md5::Context::new();
                    let mut payload = Vec::with_capacity(1048576); 
                    let mut reader = read_file.take(268435456);
                    
                    // Optimization: Utilize RAII AlignedBuffer for all pipe I/O
                    let mut chunk_buffer = super::AlignedBuffer::new(65536, 4096);
                    let chunk = chunk_buffer.as_mut_slice();

                    // Ingest data from pipe using the aligned buffer
                    while let Ok(n) = reader.read(chunk) {
                        if n == 0 { break; }
                        let data = &chunk[..n];
                        hash_context.consume(data);
                        payload.extend_from_slice(data);
                    }

                    if payload.is_empty() { return; }
                    let mut final_mime = mime_to_get;

                    // Heuristic MIME identification via magic bytes
                    // Corrects misreported types from browsers or other applications
                    if payload.len() >= 4 {
                        let detected = match &payload[0..4] {
                            [0x89, 0x50, 0x4E, 0x47] => Some("image/png"),
                            [0xFF, 0xD8, 0xFF, _]    => Some("image/jpeg"),
                            [0x47, 0x49, 0x46, 0x38] => Some("image/gif"),
                            b"RIFF" if payload.len() >= 12 && &payload[8..12] == b"WEBP" => Some("image/webp"),
                            _ => None,
                        };

                        if let Some(m) = detected {
                            final_mime = m.to_string();
                            let mut new_ctx = md5::Context::new();
                            new_ctx.consume(&payload);
                            hash_context = new_ctx;
                        }
                    }

                    // Database persistence logic with hybrid storage (DB for text, FS for binary)
                    if let Ok(db_conn) = rusqlite::Connection::open(&path) {
                        db_conn.busy_timeout(std::time::Duration::from_millis(SQLITE_TIMEOUT_MS)).ok();
                        db_conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;").ok();

                        let hash = format!("{:x}", hash_context.finalize());
                        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64;

                        let existing: Option<i64> = db_conn.query_row(
                            "SELECT id FROM clipboard WHERE hash = ?1 LIMIT 1",
                            rusqlite::params![hash], |row| row.get(0)
                        ).ok();

                        let mut should_cleanup = false;

                        if let Some(id) = existing {
                            let _ = db_conn.execute("UPDATE clipboard SET timestamp = ?1 WHERE id = ?2", rusqlite::params![ts, id]);
                            should_cleanup = true;
                        } else {
                            let is_binary_ext = final_mime.starts_with("image/") || final_mime.contains("gif");
                            let mut cache_success = true;

                            if is_binary_ext {
                                let mut cache_path = crate::core::get_cache_dir();
                                cache_path.push(format!("{}.cache", hash));
                                if !cache_path.exists()
                                    && let Err(e) = std::fs::write(&cache_path, &payload) {
                                    eprintln!("{}failed to write cache file: {}", LOG_ERROR, e);
                                    cache_success = false;
                                }
                            }

                            if cache_success {
                                let is_textual = final_mime.contains("text") || final_mime == MIME_URI_LIST;
                                let preview = if is_textual {
                                    let s = String::from_utf8_lossy(&payload);
                                    Some(s.chars().take(PREVIEW_CHARS).collect::<String>().replace('\n', " "))
                                } else {
                                    None
                                };

                                let db_payload = if is_binary_ext { None } else { Some(&payload) };

                                let res = db_conn.execute(
                                    "INSERT INTO clipboard (timestamp, mime, size, preview, content, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", 
                                    rusqlite::params![ts, final_mime.clone(), payload.len() as i64, preview, db_payload, hash]
                                );

                                if res.is_ok() {
                                    should_cleanup = true;
                                    if is_verbose { println!("{}", log_save(&final_mime, payload.len())); }
                                }
                            }
                        }

                        if should_cleanup {
                            let expired_hashes: Vec<String> = db_conn.prepare(
                                "SELECT hash FROM clipboard WHERE id NOT IN (SELECT id FROM (SELECT id FROM clipboard ORDER BY timestamp DESC LIMIT ?1))"
                            ).and_then(|mut stmt| {
                                let rows = stmt.query_map(rusqlite::params![MAX_HISTORY as i64], |row| row.get::<_, String>(0))?;
                                Ok(rows.filter_map(|r| r.ok()).collect())
                            }).unwrap_or_default();

                            let _ = db_conn.execute(
                                "DELETE FROM clipboard WHERE id NOT IN (SELECT id FROM (SELECT id FROM clipboard ORDER BY timestamp DESC LIMIT ?1))", 
                                rusqlite::params![MAX_HISTORY as i64]
                            );

                            for h in expired_hashes {
                                let mut cache_path = crate::core::get_cache_dir();
                                cache_path.push(format!("{}.cache", h));
                                let _ = std::fs::remove_file(cache_path);
                            }
                        }
                    }
                    #[cfg(target_os = "linux")]
                    unsafe { libc::malloc_trim(0); }
                });
            } else {
                let mut buf = Vec::new();
                let mut reader = read_file.take(268435456);
                let _ = reader.read_to_end(&mut buf);
                state.rx_buf = buf;
            }
        }
    }
    wayland_client::event_created_child!(WaylandState, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, OfferData { mimes: Arc::new(Mutex::new(Vec::new())) })
    ]);
}
