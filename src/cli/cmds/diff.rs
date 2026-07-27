use crate::cli::{Cli, DiffArgs, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn diff(args: &DiffArgs, cli: &Cli) -> Result<()> {
    use crate::storage::Backend;

    let before = match Backend::detect_and_open(std::path::Path::new(&args.before_db)) {
        Ok(b) => b,
        Err(e) => super::db_open_error_exit(cli, &args.before_db, &e),
    };

    let after = match Backend::detect_and_open(std::path::Path::new(&args.after_db)) {
        Ok(b) => b,
        Err(e) => super::db_open_error_exit(cli, &args.after_db, &e),
    };

    let function_id = match resolve_function_id(&after, &args.function) {
        Ok(id) => id,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new("Database", &e.to_string(), "E001");
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to resolve function: {}", e));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    let diff = match crate::cfg::diff::compute_cfg_diff_with_sources(
        &before,
        &after,
        function_id,
        &args.before_db,
        &args.after_db,
    ) {
        Ok(diff) => diff,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new("Database", &e.to_string(), "E001");
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                return Err(e);
            }
        }
    };

    match cli.output {
        OutputFormat::Human => print_diff_human(&diff, args.show_edges, args.verbose),
        OutputFormat::Json => {
            let wrapper = output::JsonResponse::new(diff);
            println!("{}", wrapper.to_json());
        }
        OutputFormat::Pretty => {
            let wrapper = output::JsonResponse::new(diff);
            println!("{}", wrapper.to_pretty_json());
        }
    }

    Ok(())
}

fn resolve_function_id(backend: &crate::storage::Backend, ident: &str) -> Result<i64> {
    if let Ok(id) = ident.parse::<i64>() {
        if backend.get_entity(id).is_some() {
            return Ok(id);
        }
    }
    match backend {
        crate::storage::Backend::Sqlite(s) => {
            let rowid: i64 = s.conn().query_row(
                "SELECT id FROM graph_entities WHERE name = ?1 OR fqn = ?1 LIMIT 1",
                rusqlite::params![ident],
                |row| row.get(0),
            )?;
            Ok(rowid)
        }
    }
}
fn print_diff_human(diff: &crate::cfg::diff::CfgDiff, show_edges: bool, verbose: bool) {
    use crate::output::{info, success, warn};

    info(&format!("CFG Diff: {}", diff.function_name));
    println!("  Before: {}", diff.before_source);
    println!("  After: {}", diff.after_source);

    // Color-code similarity
    let similarity_pct = diff.structural_similarity * 100.0;
    if similarity_pct >= 90.0 {
        success(&format!("  Similarity: {:.1}%", similarity_pct));
    } else if similarity_pct >= 70.0 {
        println!("  Similarity: {:.1}%", similarity_pct);
    } else {
        warn(&format!("  Similarity: {:.1}%", similarity_pct));
    }

    if !diff.added_blocks.is_empty() {
        println!();
        info(&format!("Added blocks ({}):", diff.added_blocks.len()));
        for block in &diff.added_blocks {
            println!(
                "  + Block {}: {} @ {}",
                block.block_id, block.kind, block.source_location
            );
        }
    }

    if !diff.deleted_blocks.is_empty() {
        println!();
        info(&format!("Deleted blocks ({}):", diff.deleted_blocks.len()));
        for block in &diff.deleted_blocks {
            println!(
                "  - Block {}: {} @ {}",
                block.block_id, block.kind, block.source_location
            );
        }
    }

    if !diff.modified_blocks.is_empty() && verbose {
        println!();
        info(&format!(
            "Modified blocks ({}):",
            diff.modified_blocks.len()
        ));
        for change in &diff.modified_blocks {
            match &change.change_type {
                crate::cfg::diff::ChangeType::Terminator { before, after } => {
                    println!("  ~ Block {}: {} -> {}", change.block_id, before, after);
                }
                crate::cfg::diff::ChangeType::SourceLocation => {
                    println!("  ~ Block {}: location changed", change.block_id);
                }
                crate::cfg::diff::ChangeType::Both => {
                    println!(
                        "  ~ Block {}: terminator and location changed",
                        change.block_id
                    );
                }
                crate::cfg::diff::ChangeType::Edges => {
                    println!("  ~ Block {}: edges changed", change.block_id);
                }
            }
        }
    }

    if show_edges {
        if !diff.added_edges.is_empty() {
            println!();
            info(&format!("Added edges ({}):", diff.added_edges.len()));
            for edge in &diff.added_edges {
                println!(
                    "  + {} -> {} ({})",
                    edge.from_block, edge.to_block, edge.edge_type
                );
            }
        }
        if !diff.deleted_edges.is_empty() {
            println!();
            info(&format!("Deleted edges ({}):", diff.deleted_edges.len()));
            for edge in &diff.deleted_edges {
                println!(
                    "  - {} -> {} ({})",
                    edge.from_block, edge.to_block, edge.edge_type
                );
            }
        }
    }

    // Summary if no changes
    if diff.added_blocks.is_empty()
        && diff.deleted_blocks.is_empty()
        && diff.modified_blocks.is_empty()
        && diff.added_edges.is_empty()
        && diff.deleted_edges.is_empty()
    {
        println!();
        success("No changes detected");
    }
}
