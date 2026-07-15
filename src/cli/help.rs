// Copyright (C) 2026 yukkkk1
// SPDX-License-Identifier: GPL-3.0-or-later

// src/cli/help.rs

/// Display the application version and primary system description.
pub fn print_version() {
    println!("y1-clipboard v1.0.0");
    println!("Unified Wayland Clipboard Infrastructure.");
}

/// Render structured usage instructions, command definitions, and technical examples.
pub fn print_help() {
    print_version();

    println!("\nUSAGE:");
    println!("    y1-clip <COMMAND> [ARGS] [OPTIONS]");

    println!("\nCORE COMMANDS:");
    println!("    daemon             - Initialize background monitor and IPC socket listener.");
    println!("                         Flags: --verbose (-v).");
    
    println!("    list [range]       - Display history metadata. Supports range (e.g., 0-50).");
    println!("                         Flags: --raw (-R), --full (-A), --id (-i).");
    
    println!("    search <query>     - Keyword scan metadata using SQLite indexing.");
    println!("                         Flags: --raw (-R), --id (-i).");
    
    println!("    copy-to <target>   - Restore record to clipboard via IPC synchronization.");
    println!("                         Accepts index or stable ID (via --id flag).");
    println!("                         Flags: --id (-i), --verbose (-v).");

    println!("\nDATA OPERATIONS:");
    println!("    show <target>      - Inspect record content and metadata.");
    println!("                         Flags: --raw (-R), --id (-i).");

    println!("    store [mime]       - Ingest stdin to storage and sync with active daemon.");
    println!("                         Flags: --verbose (-v).");
    
    println!("    paste-from [mime]  - Access system clipboard directly. Bypasses database.");

    println!("\nMANAGEMENT:");
    println!("    delete <target>    - Physically remove a specific record from persistent storage.");
    println!("                         Flags: --id (-i).");
    
    println!("    wipe               - Purge all history and execute SQLite VACUUM.");
    println!("                         Flags: --force (-f).");

    println!("\nGLOBAL OPTIONS:");
    println!("    -h, --help         - Show this help information.");
    println!("    -V, --version      - Show version information.");
    println!("    -v, --verbose      - Enable detailed system and transfer logging.");


    println!("\nPRACTICAL EXAMPLES:");
    println!("    # 1. High-speed selection with fzf using Stable IDs:");
    println!("    $ y1-clip list 0-100 --raw --id | fzf | awk '{{print $1}}' | xargs -r y1-clip copy-to --id");
    
    println!("\n    # 2. Extracting binary content from history:");
    println!("    $ y1-clip show 12 --id --raw > recovered_asset.webp");
    
    println!("\n    # 3. Manual ingestion with custom MIME:");
    println!("    $ cat data.json | y1-clip store application/json");

    println!("\nTECHNICAL NOTES:");
    println!("    - Storage: Secured at ~/.local/share/y1-clipboard/ (mode 600).");
    println!("    - IPC: Communication via /tmp/y1-clipboard.<uid>.sock.");
    println!("    - Engine: SQLite WAL mode with MD5-based deduplication.");
    println!();
}
