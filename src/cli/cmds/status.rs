use crate::cli::{resolve_db_path, Cli, OutputFormat, StatusArgs};
use crate::output;
use anyhow::Result;

pub fn status(_args: &StatusArgs, cli: &Cli) -> Result<()> {
    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Open database (honest open-error handling via shared helper)
    let db = super::open_db_or_exit(cli, &db_path);

    // Query database statistics
    let status = db.status()?;

    // Output based on format
    // VERIFIED: All three output formats (human/json/pretty) are implemented correctly
    // and follow Magellan's JsonResponse wrapper pattern for JSON outputs.
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
            println!(
                "  cfg_paths: {} (use 'mirage paths --function <name>' to enumerate)",
                status.cfg_paths
            );
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
