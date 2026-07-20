// Copyright (C) 2026 yosana
// SPDX-License-Identifier: GPL-3.0-or-later

// src/cli/paste_from.rs

use crate::wayland;
use crate::core::constants::*;
use crate::cli::utils::ArgContext;
use std::io::{self, Write};

/// Retrieve system clipboard content and stream to standard output.
pub fn run(args: &[String]) {
    let ctx = ArgContext::parse(args);

    // Strict validation: reject all flags as paste-from is a direct stream operation
    if !ctx.unknown_flags.is_empty() || ctx.raw || ctx.full || ctx.force || ctx.verbose {
        eprintln!("{}command 'paste-from' does not support options.", LOG_ERROR);
        return;
    }

    // Arity enforcement: ensure no more than one positional (MIME) is provided
    if ctx.positionals.len() > 1 {
        eprintln!("{}command 'paste-from' accepts at most one MIME type argument.", LOG_ERROR);
        return;
    }

    // Resolve target MIME from the first positional argument or use system default
    let mime = ctx.positionals.first().map(|s| s.as_str()).unwrap_or(DEFAULT_MIME);

    // Synchronous data extraction from the Wayland compositor
    let raw = wayland::paste_from_os(mime);

    if raw.is_empty() {
        eprintln!("{}null or empty payload retrieved for MIME: {}", LOG_ERROR, mime);
        std::process::exit(1);
    }

    // Direct byte-stream output to the standard output buffer
    let mut stdout = io::stdout();
    if let Err(e) = stdout.write_all(&raw) {
        eprintln!("{}standard output stream failure: {}", LOG_ERROR, e);
        return;
    }
    
    // Explicit flush to ensure complete data transmission before process exit
    let _ = stdout.flush();
}
