use crate::cli::responses::*;
use crate::cli::{resolve_db_path, Cli, OutputFormat, UnreachableArgs};
use crate::output;
use anyhow::Result;

pub fn unreachable(args: &UnreachableArgs, cli: &Cli) -> Result<()> {
    use crate::analysis::DeadSymbolJson;
    use crate::analysis::MagellanBridge;
    use crate::cfg::load_cfg_from_db;
    use crate::cfg::reachability::find_unreachable;
    use crate::storage::MirageDb;
    use petgraph::visit::EdgeRef;

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // For --include-uncalled, also open Magellan database
    let uncalled_functions: Option<Vec<DeadSymbolJson>> = if args.include_uncalled {
        match MagellanBridge::open(&db_path) {
            Ok(bridge) => {
                match bridge.dead_symbols("main") {
                    Ok(dead) => {
                        let json_symbols: Vec<DeadSymbolJson> =
                            dead.iter().map(|d| d.into()).collect();
                        Some(json_symbols)
                    }
                    Err(e) => {
                        // Log but continue with intra-procedural analysis
                        eprintln!("Warning: Failed to detect uncalled functions: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                // Magellan database not available - warn but continue
                eprintln!(
                    "Warning: Could not open Magellan database for --include-uncalled: {}",
                    e
                );
                eprintln!("Note: --include-uncalled requires a Magellan code graph database");
                None
            }
        }
    } else {
        None
    };

    // Open database (follows status command pattern for error handling)
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

    // Struct to hold unreachable results per function
    struct FunctionUnreachable {
        function_name: String,
        function_id: i64,
        blocks: Vec<UnreachableBlock>,
    }

    // Query all functions from the database
    // Note: Requires SQLite backend for full table scan
    if !db.is_sqlite() {
        output::error("The 'unreachable' command currently requires SQLite backend.");
        output::info("Use SQLite backend or run with --help for alternatives.");
        std::process::exit(output::EXIT_USAGE);
    }

    // Use prepare and execute to handle multiple rows properly
    let mut function_rows: Vec<(String, i64)> = Vec::new();
    // Magellan v7 stores functions as kind='Symbol' with data.kind='Function'
    let mut stmt = match db.conn()?.prepare(
        "SELECT name, id FROM graph_entities WHERE kind = 'Symbol' AND json_extract(data, '$.kind') = 'Function'") {
        Ok(stmt) => stmt,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "QueryError",
                    &format!("Failed to query functions: {}", e),
                    output::E_DATABASE_NOT_FOUND,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to query functions: {}", e));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    let rows_result = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    });

    match rows_result {
        Ok(rows) => {
            for row in rows {
                match row {
                    Ok((name, id)) => function_rows.push((name, id)),
                    Err(e) => {
                        if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                            let error = output::JsonError::new(
                                "QueryError",
                                &format!("Failed to read function row: {}", e),
                                output::E_DATABASE_NOT_FOUND,
                            );
                            let wrapper = output::JsonResponse::new(error);
                            println!("{}", wrapper.to_json());
                            std::process::exit(output::EXIT_DATABASE);
                        } else {
                            output::error(&format!("Failed to read function row: {}", e));
                            std::process::exit(output::EXIT_DATABASE);
                        }
                    }
                }
            }
        }
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "QueryError",
                    &format!("Failed to execute query: {}", e),
                    output::E_DATABASE_NOT_FOUND,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to execute query: {}", e));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    }

    // Load CFG for each function and find unreachable blocks
    let mut all_results = Vec::new();
    for (function_name, function_id) in function_rows {
        match load_cfg_from_db(&db, function_id) {
            Ok(cfg) => {
                let unreachable_indices = find_unreachable(&cfg);
                if !unreachable_indices.is_empty() {
                    let blocks: Vec<UnreachableBlock> = unreachable_indices
                        .iter()
                        .map(|&idx| {
                            let block = &cfg[idx];
                            let kind_str = format!("{:?}", block.kind);
                            let terminator_str = format!("{:?}", block.terminator);

                            let incoming_edges = if args.show_branches {
                                cfg.edge_references()
                                    .filter(|edge| edge.target() == idx)
                                    .filter_map(|edge| {
                                        let source_block = &cfg[edge.source()];
                                        cfg.edge_weight(edge.id()).map(|edge_type| IncomingEdge {
                                            from_block: source_block.id,
                                            edge_type: format!("{:?}", edge_type),
                                        })
                                    })
                                    .collect()
                            } else {
                                vec![]
                            };

                            UnreachableBlock {
                                block_id: block.id,
                                kind: kind_str,
                                statements: block.statements.clone(),
                                terminator: terminator_str,
                                incoming_edges,
                            }
                        })
                        .collect();

                    all_results.push(FunctionUnreachable {
                        function_name,
                        function_id,
                        blocks,
                    });
                }
            }
            Err(_) => {
                // Skip functions that fail to load
                continue;
            }
        }
    }

    // Calculate totals
    let total_functions = all_results.len();
    let functions_with_unreachable = all_results.iter().filter(|r| !r.blocks.is_empty()).count();
    let total_blocks: usize = all_results.iter().map(|r| r.blocks.len()).sum();

    // Format output based on cli.output
    match cli.output {
        OutputFormat::Human => {
            // Show uncalled functions first if available
            if let Some(ref uncalled) = uncalled_functions {
                println!("Uncalled Functions ({}):", uncalled.len());
                for dead in uncalled {
                    let name = dead.fqn.as_deref().unwrap_or("?");
                    println!("  - {} ({})", name, dead.kind);
                    println!("    File: {}", dead.file_path);
                    println!("    Reason: {}", dead.reason);
                }
                println!();
            }

            // Show unreachable blocks
            if total_blocks == 0 {
                if uncalled_functions.is_none()
                    || uncalled_functions
                        .as_ref()
                        .map(|v| v.is_empty())
                        .unwrap_or(false)
                {
                    output::info("No unreachable code found");
                }
                return Ok(());
            }

            println!("Unreachable Code Blocks:");
            println!("  Total blocks: {}", total_blocks);
            println!(
                "  Functions with unreachable: {}/{}",
                functions_with_unreachable, total_functions
            );
            println!();

            for result in &all_results {
                if result.blocks.is_empty() {
                    continue;
                }

                println!("Function: {}", result.function_name);

                for block in &result.blocks {
                    println!("  Block {} ({})", block.block_id, block.kind);
                    if !block.statements.is_empty() {
                        for stmt in &block.statements {
                            println!("    - {}", stmt);
                        }
                    }
                    println!("    Terminator: {}", block.terminator);
                    println!();
                }

                if args.show_branches {
                    println!("  Incoming Edges:");
                    for block in &result.blocks {
                        if block.incoming_edges.is_empty() {
                            println!(
                                "    Block {} has no incoming edges (entry or isolated)",
                                block.block_id
                            );
                        } else {
                            println!("    Block {} incoming edges:", block.block_id);
                            for edge in &block.incoming_edges {
                                println!(
                                    "      from block {} ({})",
                                    edge.from_block, edge.edge_type
                                );
                            }
                        }
                    }
                    println!();
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            // For multi-function mode, flatten blocks across all functions
            let all_blocks: Vec<UnreachableBlock> =
                all_results.iter().flat_map(|r| r.blocks.clone()).collect();

            let response = UnreachableResponse {
                function: "all".to_string(),
                total_functions,
                functions_with_unreachable,
                unreachable_count: total_blocks,
                blocks: all_blocks,
                uncalled_functions,
            };
            let wrapper = output::JsonResponse::new(response);

            match cli.output {
                OutputFormat::Json => println!("{}", wrapper.to_json()),
                OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                _ => {}
            }
        }
    }

    Ok(())
}
