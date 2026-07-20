/*
 * y4-clipboardMN: A Wayland clipboard manager for power users.
 * Copyright (C) 2026  yosana
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

    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN); }

    ctrlc::set_handler(move || {
        if crate::core::is_exiting() {
            std::process::exit(1);
        }
        crate::core::request_exit();
        if let Ok(mut stream) = UnixStream::connect(crate::core::get_socket_path()) {
            let _ = stream.write_all(&[IPC_CMD_EXIT]);
        }
    }).expect("failed to set signal handler");

    let args: Vec<String> = std::env::args().collect();

    // Robustness: Handle database open errors without panicking.
    let db = match storage::ClipboardDb::open() {
        Ok(database) => database,
        Err(e) => {
            eprintln!("{}critical: {}", LOG_ERROR, e);
            std::process::exit(1);
        }
    };

    cli::handle_command(&args, db);
}
