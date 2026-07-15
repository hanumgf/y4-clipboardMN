// Copyright (C) 2026 yukkkk1
// SPDX-License-Identifier: GPL-3.0-or-later

// src/cli/daemon.rs

use crate::storage::ClipboardDb;
use crate::daemon;
use crate::core::constants::*;
use crate::cli::utils::ArgContext;

/// Initialize and execute the background monitoring service.
pub fn run(args: &[String], db: ClipboardDb) {
    let ctx = ArgContext::parse(args);

    // Strict validation: 'daemon' permits only --verbose/-v and zero positional arguments
    if !ctx.unknown_flags.is_empty() || ctx.raw || ctx.full || ctx.force {
        eprintln!("{}command 'daemon' does not support specified options.", LOG_ERROR);
        return;
    }

    // Arity enforcement: ensure no positional arguments are provided
    if !ctx.positionals.is_empty() {
        eprintln!("{}command 'daemon' does not accept positional arguments.", LOG_ERROR);
        println!("usage: y1-clip daemon [--verbose | -v]");
        return;
    }

    // Notify initialization start
    println!("{}{}", LOG_INFO, MSG_DAEMON_START);
    
    if ctx.verbose {
        println!("{}extended event logging is active.", LOG_INFO);
    }

    // Transfer execution to the core monitor logic (src/daemon/mod.rs)
    // This blocks the current thread until the process is interrupted or a fatal error occurs.
    daemon::start_daemon(db, ctx.verbose);

    // Reaching this segment implies a collapse of the internal event loop
    eprintln!("{}{}", LOG_ERROR, MSG_DAEMON_STOP);
}
