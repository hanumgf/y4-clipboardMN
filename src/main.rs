/*
 * y1-clipboardMN: A Wayland clipboard manager for power users.
 * Copyright (C) 2026  yukkkk1
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

// src/main.rs

#[cfg(target_os = "linux")]
fn trim_memory() {
    unsafe {
        libc::malloc_trim(0);
    }
}

mod core;
mod storage;
mod wayland;
mod daemon;
mod cli;

use crate::core::constants::*;
use std::os::unix::net::UnixStream;
use std::io::Write;

fn main() {
    #[cfg(target_os = "linux")]
    trim_memory();

    // Security & Stability: Ignore SIGPIPE to prevent the daemon from being
    // terminated by the OS when a pipe is closed prematurely by a receiver.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN); }

    // 1. Initialize signal handler for graceful shutdown
    ctrlc::set_handler(move || {
        if crate::core::is_exiting() {
            // Emergency Exit: Force terminate if Ctrl+C is pressed again
            eprintln!("\n{}forceful termination initiated.", LOG_ERROR);
            std::process::exit(1);
        }

        // Signal the primary loop to wrap up operations
        crate::core::request_exit();

        if let Ok(mut stream) = UnixStream::connect(crate::core::get_socket_path()) {
            let _ = stream.write_all(&[IPC_CMD_EXIT]);
        }

        // Provide immediate feedback to the user
        eprintln!("\n{}termination signal received. closing storage safely...", LOG_INFO);
    }).expect("failed to set signal handler");

    let args: Vec<String> = std::env::args().collect();
    let db = storage::ClipboardDb::open();
    cli::handle_command(&args, db);
}
