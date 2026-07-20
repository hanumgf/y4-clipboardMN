// Copyright (C) 2026 yosana
// SPDX-License-Identifier: GPL-3.0-or-later

// src/cli/store.rs

use crate::storage::ClipboardDb;
use crate::core::constants::*;
use crate::cli::utils::ArgContext;
use std::io::{self, Read, Write};

/// Ingest data from stdin, persist to database, and synchronize to system clipboard.
pub fn run(args: &[String], db: &mut ClipboardDb) {
    let ctx = ArgContext::parse(args);

    // Strict validation: 'store' only permits --verbose/-v
    if !ctx.unknown_flags.is_empty() || ctx.raw || ctx.full || ctx.force {
        eprintln!("{}command 'store' does not support specified options.", LOG_ERROR);
        return;
    }

    // Arity enforcement: ensure no more than one positional (MIME) is provided
    if ctx.positionals.len() > 1 {
        eprintln!("{}command 'store' accepts at most one MIME type argument.", LOG_ERROR);
        return;
    }

    // Resolve target MIME from positional arguments or use system default
    let mime = ctx.positionals.first().map(|s| s.as_str()).unwrap_or(DEFAULT_MIME);

    // Read payload from standard input stream until EOF
    let mut buffer = Vec::new();
    if io::stdin().read_to_end(&mut buffer).is_err() {
        eprintln!("{}standard input stream read failure.", LOG_ERROR);
        return;
    }

    // Terminate processing for null or empty payloads
    if buffer.is_empty() {
        if ctx.verbose {
            println!("{}null payload detected; skipping storage.", LOG_INFO);
        }
        return;
    }

    // Execute atomic persistence with internal deduplication
    match db.insert_raw(mime, &buffer) {
        Ok(_) => {
            if ctx.verbose {
                println!("{}", log_save(mime, buffer.len()));
            }

            // Obtain the record identifier for the entry just processed
            let meta = db.fetch_metadata(1);
            if let Some(&(real_id, _, _, _, _)) = meta.first() {
                
                // IPC: Notify the running daemon to synchronize this new ID
                use std::os::unix::net::UnixStream;
                if let Ok(mut stream) = UnixStream::connect(crate::core::get_socket_path()) {
                    let mut payload = vec![IPC_CMD_RESTORE];
                    payload.extend_from_slice(real_id.to_string().as_bytes());
                    payload.push(IPC_DELIMITER);
                    let _ = stream.write_all(&payload);
                    let _ = stream.flush();
                }

                if ctx.verbose {
                    println!("{}system clipboard synchronized.", LOG_INFO);
                }
            }
        }
        Err(e) => {
            eprintln!("{}database transaction failure: {}", LOG_ERROR, e);
        }
    }
}
