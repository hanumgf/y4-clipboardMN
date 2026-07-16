// Copyright (C) 2026 yukkkk1
// SPDX-License-Identifier: GPL-3.0-or-later

// src/storage/mod.rs

use rusqlite::{params, Connection, Result};
use std::time::{SystemTime, UNIX_EPOCH};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use crate::core::constants::*;

pub struct ClipboardDb {
    pub path: String,
    conn: Connection,
}

impl ClipboardDb {
    /// Open the database with optimized configurations. 
    /// Returns Result to allow the caller to handle connection failures gracefully.
    pub fn open() -> Result<Self, String> {
        let db_path = crate::core::get_db_path();
        let _ = crate::core::get_cache_dir(); 

        // Attempt connection with error mapping
        let conn = Connection::open(&db_path)
            .map_err(|e| format!("sqlite connection failed: {}", e))?;

        // Secure file permissions
        if let Ok(metadata) = fs::metadata(&db_path) {
            let mut perms = metadata.permissions();
            if perms.mode() != 0o600 {
                perms.set_mode(0o600);
                let _ = fs::set_permissions(&db_path, perms);
            }
        }

        // Apply PRAGMA settings
        conn.busy_timeout(std::time::Duration::from_millis(SQLITE_TIMEOUT_MS)).ok();
        let _ = conn.execute_batch("
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
            PRAGMA mmap_size = 268435456;
            PRAGMA cache_size = -64000;
        ");

        // Schema initialization
        conn.execute(
            "CREATE TABLE IF NOT EXISTS clipboard (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                mime TEXT NOT NULL,
                size INTEGER NOT NULL,
                preview TEXT,
                content BLOB,
                hash TEXT UNIQUE
            )", [],
        ).map_err(|e| format!("schema initialization failed: {}", e))?;

        conn.execute("CREATE INDEX IF NOT EXISTS idx_ts ON clipboard(timestamp)", []).ok();

        Ok(Self { 
            path: db_path.to_string_lossy().into_owned(), 
            conn 
        })
    }

    /// Public wrapper for raw data insertion.
    pub fn insert_raw(&mut self, mime: &str, data: &[u8]) -> Result<()> {
        let hash = format!("{:x}", md5::compute(data));
        self.insert_with_hash(mime, data, &hash)
    }

    /// Optimized insertion utilizing a pre-computed hash and atomic transactions.
    pub fn insert_with_hash(&mut self, mime: &str, data: &[u8], hash: &str) -> Result<()> {
        if data.is_empty() { return Ok(()); }
        if SENSITIVE_MIME_HINTS.iter().any(|&hint| mime.contains(hint)) { return Ok(()); }

        let is_image = mime.starts_with("image/") || mime.contains("gif");
        
        if is_image {
            let mut cache_path = crate::core::get_cache_dir();
            cache_path.push(format!("{}.cache", hash));
            if !cache_path.exists() {
                let _ = fs::write(cache_path, data);
            }
        }

        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
        let tx = self.conn.transaction()?;

        let existing: Option<i64> = tx.query_row(
            "SELECT id FROM clipboard WHERE hash = ?1 LIMIT 1",
            params![hash], |row| row.get(0)
        ).ok();

        if let Some(id) = existing {
            tx.execute("UPDATE clipboard SET timestamp = ?1 WHERE id = ?2", params![ts, id])?;
        } else {
            let preview = if mime.contains("text") || mime.contains("uri-list") {
                let s = String::from_utf8_lossy(data);
                Some(s.chars().take(PREVIEW_CHARS).collect::<String>().replace('\n', " "))
            } else { None };

            let db_content = if is_image { None } else { Some(data) };

            tx.execute(
                "INSERT INTO clipboard (timestamp, mime, size, preview, content, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![ts, mime, data.len() as i64, preview, db_content, hash],
            )?;
        }

        let expired_hashes: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT hash FROM clipboard WHERE id NOT IN (SELECT id FROM clipboard ORDER BY timestamp DESC LIMIT ?1)"
            )?;
            let rows = stmt.query_map(params![MAX_HISTORY as i64], |row| row.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };

        tx.execute(
            "DELETE FROM clipboard WHERE id NOT IN (SELECT id FROM clipboard ORDER BY timestamp DESC LIMIT ?1)",
            params![MAX_HISTORY as i64]
        )?;

        tx.commit()?;

        for h in expired_hashes {
            let mut cache_path = crate::core::get_cache_dir();
            cache_path.push(format!("{}.cache", h));
            let _ = fs::remove_file(cache_path);
        }

        Ok(())
    }

    /// Search metadata with protection against full BLOB scans.
    /// Optimized to only scan content when mime is text-based and preview is insufficient.
    pub fn search_metadata(&self, query: &str, limit: usize) -> Vec<(i64, i64, String, i64, Option<String>)> {
        let mut stmt = match self.conn.prepare(
            "SELECT id, timestamp, mime, size, preview FROM clipboard 
             WHERE (mime LIKE '%text%' OR mime LIKE '%UTF8%') 
             AND (preview LIKE ?1 OR (preview IS NULL AND CAST(content AS TEXT) LIKE ?1))
             ORDER BY timestamp DESC LIMIT ?2"
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let query_param = format!("%{}%", query);
        let rows = stmt.query_map(params![query_param, limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        }).unwrap();

        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn fetch_metadata(&self, limit: usize) -> Vec<(i64, i64, String, i64, Option<String>)> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, mime, size, preview FROM clipboard ORDER BY timestamp DESC LIMIT ?1"
        ).unwrap();
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        }).unwrap();
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn get_content_by_id(&self, id: i64) -> Option<(String, Vec<u8>)> {
        let (mime, db_content, hash): (String, Option<Vec<u8>>, String) = self.conn.query_row(
            "SELECT mime, content, hash FROM clipboard WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        ).ok()?;

        if let Some(data) = db_content {
            Some((mime, data))
        } else {
            let mut cache_path = crate::core::get_cache_dir();
            cache_path.push(format!("{}.cache", hash));
            let data = fs::read(cache_path).ok()?;
            Some((mime, data))
        }
    }

    pub fn get_latest_data(&self) -> Option<Vec<u8>> {
        // Reuse get_content_by_id logic for consistency
        let id: i64 = self.conn.query_row(
            "SELECT id FROM clipboard ORDER BY timestamp DESC LIMIT 1",
            [], |row| row.get(0)
        ).ok()?;
        self.get_content_by_id(id).map(|(_, data)| data)
    }

    /// Update record timestamp. Standardized to &mut self for state consistency.
    pub fn update_timestamp(&mut self, id: i64) -> Result<()> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
        self.conn.execute("UPDATE clipboard SET timestamp = ?1 WHERE id = ?2", params![ts, id])?;
        Ok(())
    }

    /// Remove record by ID. Standardized to &mut self for state consistency.
    pub fn delete_by_id(&mut self, id: i64) -> Result<bool> {
        let hash: Option<String> = self.conn.query_row(
            "SELECT hash FROM clipboard WHERE id = ?1",
            params![id], |row| row.get(0)
        ).ok();

        let res = self.conn.execute("DELETE FROM clipboard WHERE id = ?1", params![id])?;
        
        if let Some(h) = hash {
            let mut cache_path = crate::core::get_cache_dir();
            cache_path.push(format!("{}.cache", h));
            let _ = fs::remove_file(cache_path);
        }

        Ok(res > 0)
    }

    /// Clear all history and reclaim disk space. Standardized to &mut self.
    pub fn wipe(&mut self) -> Result<()> {
        self.conn.execute("DELETE FROM clipboard", [])?;

        let cache_dir = crate::core::get_cache_dir();
        let _ = fs::remove_dir_all(&cache_dir);
        let _ = fs::create_dir_all(&cache_dir);

        let _ = self.conn.execute_batch("
            PRAGMA journal_mode = DELETE;
            VACUUM;
            PRAGMA journal_mode = WAL;
        ");

        Ok(())
    }

    /// Safely retrieve total record count. Removed unwrap() to prevent daemon panics.
    pub fn get_total_count(&self) -> usize {
        self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard",
            [],
            |row| row.get::<_, i64>(0).map(|val| val as usize)
        ).unwrap_or(0)
    }
}
