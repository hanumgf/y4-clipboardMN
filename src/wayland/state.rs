// Copyright (C) 2026 yukkkk1
// SPDX-License-Identifier: GPL-3.0-or-later

// src/wayland/state.rs

use crate::storage::ClipboardDb;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_source_v1::ExtDataControlSourceV1,
};
use std::sync::{Arc, Mutex};

pub struct OfferData {
    pub mimes: Arc<Mutex<Vec<String>>>,
}

pub struct SourceMetadata {
    pub mime: String,
    pub data: Vec<u8>,
}

pub struct WaylandState {
    pub manager: Option<ExtDataControlManagerV1>,
    pub manager_id: Option<u32>,
    pub seat: Option<WlSeat>,
    pub seat_id: Option<u32>,
    pub device: Option<ExtDataControlDeviceV1>,
    pub db: Option<Arc<Mutex<ClipboardDb>>>,
    pub verbose: bool,
    pub target_mime: String,
    pub rx_buf: Vec<u8>,
    pub last_data: Vec<u8>,
    pub provider_locks: u32,
    pub selection_received: bool,
    pub current_source: Option<ExtDataControlSourceV1>,
}

impl WaylandState {
    pub fn new_daemon(db: ClipboardDb, verbose: bool) -> Self {
        Self {
            manager: None,
            manager_id: None,
            seat: None,
            seat_id: None,
            device: None,
            db: Some(Arc::new(Mutex::new(db))),
            verbose,
            target_mime: String::new(),
            rx_buf: Vec::new(),
            last_data: Vec::new(),
            provider_locks: 0,
            selection_received: false,
            current_source: None,
        }
    }

    pub fn new_action(target_mime: String, verbose: bool) -> Self {
        Self {
            manager: None,
            manager_id: None,
            seat: None,
            seat_id: None,
            device: None,
            db: None,
            verbose,
            target_mime,
            rx_buf: Vec::new(),
            last_data: Vec::new(),
            provider_locks: 0,
            selection_received: false,
            current_source: None,
        }
    }
}
