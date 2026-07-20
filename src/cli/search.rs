// Copyright (C) 2026 yosana
// SPDX-License-Identifier: GPL-3.0-or-later

// src/cli/search.rs

use crate::storage::ClipboardDb;
use crate::core::constants::*;
use crate::cli::utils::ArgContext;
use super::list;

/// Search through metadata history and render results using strict argument validation.
pub fn run(args: &[String], db: &ClipboardDb) {
    let ctx = ArgContext::parse(args);

    // Strict validation: 'search' only supports --raw/-R and --verbose/-v
    if !ctx.unknown_flags.is_empty() || ctx.full || ctx.force {
        eprintln!("{}command 'search' does not support specified options.", LOG_ERROR);
        return;
    }

    // Arity enforcement: exactly one positional argument (keyword) required
    if ctx.positionals.is_empty() {
        eprintln!("{}missing required search keyword.", LOG_ERROR);
        println!("usage: y1-clip search <keyword> [--raw | -R]");
        return;
    }

    if ctx.positionals.len() > 1 {
        eprintln!("{}command 'search' accepts only one keyword.", LOG_ERROR);
        return;
    }

    let query = &ctx.positionals[0];

    // Execute metadata-level search via indexed SQLite query
    let results = db.search_metadata(query, MAX_HISTORY);
    let total_stored = db.get_total_count();

    if results.is_empty() {
        println!("{}no entries matching '{}' were found.", LOG_INFO, query);
        return;
    }

    // Prepare references and delegate rendering to the unified list module
    let refs: Vec<(usize, _)> = results
        .iter()
        .enumerate()
        .collect();

    let title = format!("search: '{}' ({} hits)", query, results.len());
    
    list::render_list(&title, &refs, total_stored, ctx.raw, ctx.use_id);
}
