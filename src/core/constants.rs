// Copyright (C) 2026 yosana
// SPDX-License-Identifier: GPL-3.0-or-later

// src/core/constants.rs

// --- System & Storage Configuration ---
pub const DB_DIR_NAME:  &str = "y4-clipboard";
pub const DB_FILE_NAME: &str = "y4_clipboard.sqlite";
pub const MAX_HISTORY: usize = 256;
pub const SQLITE_TIMEOUT_MS: u64 = 5000;

// --- IPC Protocol ---
pub const IPC_CMD_RESTORE: u8 = 0x01;
pub const IPC_CMD_EXIT:    u8 = 0x02;
pub const IPC_DELIMITER:   u8 = b'\n';

pub const RECONNECT_DELAY_MS: u64 = 500;

// --- Security & Privacy Configuration ---
// Clipboard security: MIME types to exclude from persistent storage
pub const SENSITIVE_MIME_HINTS: &[&str] = &[
    "x-kde-passwordManagerHint", 
    "password", 
    "secret",
    "x-gnome-cliptrace"
];

// --- Clipboard & Preview Settings ---
pub const DEFAULT_MIME: &str = "text/plain;charset=utf-8";
pub const PREVIEW_CHARS: usize = 100;
pub const TEXT_MIME_ALTS: &[&str] = &[
    "text/plain;charset=utf-8",
    "text/plain",
    "UTF8_STRING",
    "STRING",
    "TEXT",
];

pub const MIME_URI_LIST: &str = "text/uri-list";

// --- UI Layout & Formatting Settings ---
pub const WIDTH_ID: usize      = 6;
pub const WIDTH_WHEN: usize    = 8;
pub const WIDTH_SIZE: usize    = 11;
pub const PREVIEW_WIDTH: usize = 42;
pub const ELLIPSIS: &str       = "...";
pub const TABLE_SEP: &str       = " | ";
pub const TABLE_LINE_CHAR: &str = "-";

// --- UI Labels & Headers ---
pub const LABEL_IMAGE: &str = "[IMG]";
pub const LABEL_TEXT:  &str = "[TXT]";
pub const LABEL_DATA:  &str = "[BIN]";
pub const LABEL_FILE:  &str = "[FIL]";

pub const LIST_HEADER_ID: &str      = "ID";
pub const LIST_HEADER_WHEN: &str    = "WHEN";
pub const LIST_HEADER_SIZE: &str    = "SIZE";
pub const LIST_HEADER_CONTENT: &str = "CONTENT";

pub const TIME_UNIT_SEC:  &str = "s ago";
pub const TIME_UNIT_MIN:  &str = "m ago";
pub const TIME_UNIT_HOUR: &str = "h ago";

// --- Wayland Protocol Configuration ---
pub const INTERFACE_MANAGER: &str = "ext_data_control_manager_v1";
pub const INTERFACE_SEAT:    &str = "wl_seat";

// --- Logging & Notification Messages ---
pub const LOG_INFO:  &str = "info: ";
pub const LOG_ERROR: &str = "error: ";

pub const MSG_DAEMON_START: &str = "starting y4-clipboard daemon...";
pub const MSG_DAEMON_STOP:  &str = "daemon process terminated.";
pub const MSG_WAYLAND_CONN_FAIL: &str = "failed to connect to wayland compositor. is DISPLAY/WAYLAND_DISPLAY set?";

pub fn log_save(mime: &str, size: usize) -> String {
    format!("{}saved: {} ({} bytes)", LOG_INFO, mime, size)
}

pub fn log_restore(idx: usize) -> String {
    format!("{}restored ID [{}] to clipboard", LOG_INFO, idx)
}

pub fn log_seat_detected(name: &str, caps: &str) -> String {
    format!("{}wayland seat detected: {} (capabilities: {})", LOG_INFO, name, caps)
}

pub fn log_protocol_bound(interface: &str) -> String {
    format!("{}bound to wayland interface: {}", LOG_INFO, interface)
}

/// Path to the Unix Domain Socket for Inter-Process Communication.
pub const SOCKET_PATH: &str = "/tmp/y4-clipboard.sock";

/// Returns the user-specific socket path to prevent multi-user conflicts.
pub fn get_socket_path() -> String {
    format!("{}.{}.sock", SOCKET_PATH, unsafe { libc::getuid() })
}
