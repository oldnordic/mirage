//! Status command - Show database statistics

use crate::cli::{Cli, OutputFormat, StatusArgs};
use crate::output;
use crate::storage::MirageDb;
use anyhow::Result;

/// Show database statistics
pub fn status(_args: &StatusArgs, cli: &Cli) -> Result<()> {
    // Resolve database path
    let db_path = crate::cli::resolve_db_path(cli.db.clone())?;

    // Open database
    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(_e) => {
            // JSON-aware error handling with remediation
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::database_not_found(&db_path);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to open database: {}", db_path));
                output::info("Hint: Run 'magellan watch' to create the database");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Query database statistics
    let status = db.status()?;

    // Output based on format
    match cli.output {
        OutputFormat::Human => {
            // Human-readable text format
            println!("Mirage Database Status:");
            println!(
                "  Schema version: {} (Magellan: {})",
                status.mirage_schema_version, status.magellan_schema_version
            );
            println!("  cfg_blocks: {}", status.cfg_blocks);
            // cfg_edges are computed in memory from terminators, not stored
            // cfg_paths requires explicit enumeration via 'mirage paths --function <name>'
            println!("  cfg_paths: {} (use 'mirage paths --function <name>' to enumerate)", status.cfg_paths);
            println!("  cfg_dominators: {}", status.cfg_dominators);
        }
        OutputFormat::Json => {
            // Compact JSON
            let response = output::JsonResponse::new(status);
            println!("{}", response.to_json());
        }
        OutputFormat::Pretty => {
            // Formatted JSON with indentation
            let response = output::JsonResponse::new(status);
            println!("{}", response.to_pretty_json());
        }
    }

    Ok(())
}
