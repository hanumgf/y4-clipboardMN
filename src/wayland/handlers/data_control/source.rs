// Copyright (C) 2026 yosana
// SPDX-License-Identifier: GPL-3.0-or-later

// src/wayland/handlers/data_control/source.rs

use wayland_client::{Dispatch, Connection, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_source_v1::{self, ExtDataControlSourceV1};
use std::io::Write;
use std::os::fd::AsRawFd;
use crate::wayland::state::{WaylandState, SourceMetadata};
use super::mime_is_compatible;
use crate::core::constants::*;

// --- ExtDataControlSourceV1 ---

impl Dispatch<ExtDataControlSourceV1, SourceMetadata> for WaylandState {
    fn event(state: &mut Self, _source: &ExtDataControlSourceV1, ev: ext_data_control_source_v1::Event, meta: &SourceMetadata, _: &Connection, _: &QueueHandle<Self>) {
        match ev {
            ext_data_control_source_v1::Event::Send { mime_type, fd } => {
                if mime_is_compatible(&mime_type, &meta.mime) {
                    unsafe {
                        let raw = fd.as_raw_fd();
                        let flags = libc::fcntl(raw, libc::F_GETFL, 0);
                        if flags >= 0 {
                            libc::fcntl(raw, libc::F_SETFL, flags & !libc::O_NONBLOCK);
                        }
                    }

                    let mut file = std::fs::File::from(fd);
                    
                    let mut data_to_send = meta.data.clone();

                    // If source is a URI list but the target specifically requested plain text,
                    if meta.mime == MIME_URI_LIST && mime_type.contains("text/plain") {
                        let content = String::from_utf8_lossy(&meta.data);
                        let stripped: Vec<String> = content.lines()
                            .map(|l| l.trim_start_matches("file://").to_string())
                            .collect();
                        data_to_send = stripped.join("\n").into_bytes();
                    }

                    std::thread::spawn(move || {
                        if let Err(e) = file.write_all(&data_to_send) {
                            eprintln!("{}egress transmission failure: {}", LOG_ERROR, e);
                        }
                        let _ = file.flush();
                    });
                } else {
                    drop(std::fs::File::from(fd));
                }
            }
            ext_data_control_source_v1::Event::Cancelled => {
                state.current_source = None;
                
                if state.verbose {
                    println!("{}clipboard ownership relinquished.", LOG_INFO);
                }
            }
            _ => {}
        }
    }
}
