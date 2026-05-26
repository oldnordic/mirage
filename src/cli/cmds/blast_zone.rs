use crate::cli::responses::*;
use crate::cli::{resolve_db_path, BlastZoneArgs, Cli, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn blast_zone(args: &BlastZoneArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::{find_reachable_from_block, load_cfg_from_db, resolve_function_name};
    use crate::storage::{compute_path_impact_from_db, get_function_name_db, MirageDb};
    use rusqlite::OptionalExtension;

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Open database (follows status command pattern for error handling)
    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(_e) => {
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

    // Determine query type: path-based or block-based
    if let Some(ref path_id) = args.path_id {
        // Path-based impact analysis requires SQLite backend (path caching)
        if !db.is_sqlite() {
            let msg = "Path-based blast-zone requires SQLite backend. Use block-based analysis with --function and --block-id instead.";
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error =
                    output::JsonError::new("UnsupportedBackend", msg, output::E_INVALID_INPUT);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_USAGE);
            } else {
                output::error(msg);
                output::info("retired binary backend backend does not support path caching. Use: mirage blast-zone --function <name> --block-id <id>");
                std::process::exit(output::EXIT_USAGE);
            }
        }

        // Path-based impact analysis
        let path_id_trimmed = path_id.trim();

        // Validate path_id format (basic BLAKE3 hex check)
        if path_id_trimmed.len() < 10 {
            let msg = format!("Invalid path_id format: '{}'", path_id_trimmed);
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new("InvalidInput", &msg, output::E_INVALID_INPUT);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_USAGE);
            } else {
                output::error(&msg);
                output::info("Path ID should be a BLAKE3 hash (64 hex characters)");
                std::process::exit(output::EXIT_USAGE);
            }
        }

        // Get path metadata to find function_id
        let (function_id, path_kind): (i64, String) = match db
            .conn()?
            .query_row(
                "SELECT function_id, path_kind FROM cfg_paths WHERE path_id = ?1",
                rusqlite::params![path_id_trimmed],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
        {
            Ok(Some(data)) => data,
            Ok(None) => {
                let msg = format!("Path '{}' not found in cache", path_id_trimmed);
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error =
                        output::JsonError::new("PathNotFound", &msg, output::E_PATH_NOT_FOUND);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_FILE_NOT_FOUND);
                } else {
                    output::error(&msg);
                    output::info("Hint: Run 'mirage paths' to enumerate paths first");
                    std::process::exit(output::EXIT_FILE_NOT_FOUND);
                }
            }
            Err(e) => {
                let msg = format!("Failed to query path: {}", e);
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error =
                        output::JsonError::new("DatabaseError", &msg, output::E_DATABASE_NOT_FOUND);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&msg);
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Filter by path_kind if include_errors is false
        if !args.include_errors && path_kind == "error" {
            let msg = format!(
                "Path '{}' is an error path (use --include-errors to analyze)",
                path_id_trimmed
            );
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error =
                    output::JsonError::new("ErrorPathExcluded", &msg, output::E_INVALID_INPUT);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_USAGE);
            } else {
                output::error(&msg);
                output::info("Use --include-errors to include error paths in analysis");
                std::process::exit(output::EXIT_USAGE);
            }
        }

        // Load CFG for the function
        let cfg = match load_cfg_from_db(&db, function_id) {
            Ok(cfg) => cfg,
            Err(_e) => {
                let msg = format!("Failed to load CFG for function_id {}", function_id);
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::new("CgfLoadError", &msg, output::E_CFG_ERROR);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&msg);
                    output::info("The function may be corrupted. Try re-running 'magellan watch'");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Get function name for display (backend-agnostic)
        let function_name = get_function_name_db(&db, function_id)
            .unwrap_or_else(|| format!("<function_{}>", function_id));

        // Compute path impact
        let max_depth = if args.max_depth == 100 {
            None
        } else {
            Some(args.max_depth)
        };
        let impact = match compute_path_impact_from_db(db.conn()?, path_id_trimmed, &cfg, max_depth)
        {
            Ok(impact) => impact,
            Err(e) => {
                let msg = format!("Failed to compute path impact: {}", e);
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::new("ImpactError", &msg, output::E_CFG_ERROR);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_ERROR);
                } else {
                    output::error(&msg);
                    std::process::exit(output::EXIT_ERROR);
                }
            }
        };

        // Compute call graph impact if requested
        let (forward_impact, backward_impact): (
            Option<Vec<CallGraphSymbol>>,
            Option<Vec<CallGraphSymbol>>,
        ) = if args.use_call_graph {
            use crate::analysis::MagellanBridge;
            match MagellanBridge::open(&db_path) {
                Ok(bridge) => {
                    // Use function name as symbol identifier
                    let symbol_id = function_name.as_str();
                    let forward: Option<Vec<CallGraphSymbol>> = bridge
                        .reachable_symbols(symbol_id)
                        .map(|symbols| {
                            symbols
                                .into_iter()
                                .map(|s| CallGraphSymbol {
                                    symbol_id: s.symbol_id,
                                    fqn: s.fqn,
                                    file_path: s.file_path,
                                    kind: s.kind,
                                })
                                .collect()
                        })
                        .ok();
                    let backward: Option<Vec<CallGraphSymbol>> = bridge
                        .reverse_reachable_symbols(symbol_id)
                        .map(|symbols| {
                            symbols
                                .into_iter()
                                .map(|s| CallGraphSymbol {
                                    symbol_id: s.symbol_id,
                                    fqn: s.fqn,
                                    file_path: s.file_path,
                                    kind: s.kind,
                                })
                                .collect()
                        })
                        .ok();
                    (forward, backward)
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Could not open Magellan database for call graph analysis: {}",
                        e
                    );
                    eprintln!("Note: --use-call-graph requires a Magellan code graph database");
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        // Output
        match cli.output {
            OutputFormat::Human => {
                println!("Path Impact Analysis");
                println!();
                println!("Path ID: {}", impact.path_id);
                println!("Function: {}", function_name);
                println!("Path kind: {}", path_kind);
                println!("Path length: {} blocks", impact.path_length);
                println!();

                // Show call graph impact if available
                if let Some(ref forward) = forward_impact {
                    println!("Inter-Procedural Impact (Call Graph):");
                    println!("  Forward Impact: {} functions reached", forward.len());
                    for sym in forward {
                        println!("    - {}", sym.fqn.as_deref().unwrap_or(&sym.file_path));
                    }
                }
                if let Some(ref backward) = backward_impact {
                    if !backward.is_empty() {
                        println!(
                            "  Backward Impact: {} functions can reach this",
                            backward.len()
                        );
                        for sym in backward {
                            println!("    - {}", sym.fqn.as_deref().unwrap_or(&sym.file_path));
                        }
                    }
                }
                println!();

                println!("Intra-Procedural Impact (CFG):");
                println!("  Unique blocks affected: {}", impact.impact_count);
                if impact.impact_count > 0 {
                    println!("  Affected blocks: {:?}", impact.unique_blocks_affected);
                } else {
                    println!("  Affected blocks: (none - path has no downstream impact)");
                }
                if let Some(depth) = max_depth {
                    println!("  Max depth: {}", depth);
                } else {
                    println!("  Max depth: unlimited");
                }
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let response = PathImpactResponse {
                    path_id: impact.path_id.clone(),
                    path_length: impact.path_length,
                    unique_blocks_affected: impact.unique_blocks_affected,
                    impact_count: impact.impact_count,
                    forward_impact: forward_impact.clone(),
                    backward_impact: backward_impact.clone(),
                };
                let wrapper = output::JsonResponse::new(response);
                match cli.output {
                    OutputFormat::Json => println!("{}", wrapper.to_json()),
                    OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                    _ => unreachable!(),
                }
            }
        }
    } else {
        // Block-based impact analysis
        // Get function from args
        let function_ref = args
            .function
            .as_ref()
            .expect("--function is required for block-based analysis");

        // Resolve function name/ID to function_id
        let function_id = match resolve_function_name(&db, function_ref) {
            Ok(id) => id,
            Err(_e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::function_not_found(function_ref);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!(
                        "Function '{}' not found in database",
                        function_ref
                    ));
                    output::info(&format!("Hint: {}", output::R_HINT_LIST_FUNCTIONS));
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Get function name for display (backend-agnostic)
        let function_name = get_function_name_db(&db, function_id)
            .unwrap_or_else(|| format!("<function_{}>", function_id));

        // Load CFG from database
        let cfg = match load_cfg_from_db(&db, function_id) {
            Ok(cfg) => cfg,
            Err(_e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::new(
                        "CgfLoadError",
                        &format!("Failed to load CFG for function '{}'", function_ref),
                        output::E_CFG_ERROR,
                    );
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!(
                        "Failed to load CFG for function '{}'",
                        function_ref
                    ));
                    output::info("The function may be corrupted. Try re-running 'magellan watch'");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Determine block ID (default to entry block 0)
        let block_id = args.block_id.unwrap_or(0);

        // Validate block_id exists in CFG
        let block_exists = cfg.node_indices().any(|n| cfg[n].id == block_id);
        if !block_exists {
            let valid_blocks: Vec<usize> = cfg.node_indices().map(|n| cfg[n].id).collect();
            let msg = format!(
                "Block {} not found in function '{}'. Valid blocks: {:?}",
                block_id, function_ref, valid_blocks
            );
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error =
                    output::JsonError::new("BlockNotFound", &msg, output::E_BLOCK_NOT_FOUND);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_VALIDATION);
            } else {
                output::error(&msg);
                std::process::exit(output::EXIT_VALIDATION);
            }
        }

        // Compute block impact
        let max_depth = if args.max_depth == 100 {
            None
        } else {
            Some(args.max_depth)
        };
        let impact = find_reachable_from_block(&cfg, block_id, max_depth);

        // Compute call graph impact if requested
        let (forward_impact, backward_impact): (
            Option<Vec<CallGraphSymbol>>,
            Option<Vec<CallGraphSymbol>>,
        ) = if args.use_call_graph {
            use crate::analysis::MagellanBridge;
            match MagellanBridge::open(&db_path) {
                Ok(bridge) => {
                    // Use function name as symbol identifier
                    let symbol_id = function_name.as_str();
                    let forward: Option<Vec<CallGraphSymbol>> = bridge
                        .reachable_symbols(symbol_id)
                        .map(|symbols| {
                            symbols
                                .into_iter()
                                .map(|s| CallGraphSymbol {
                                    symbol_id: s.symbol_id,
                                    fqn: s.fqn,
                                    file_path: s.file_path,
                                    kind: s.kind,
                                })
                                .collect()
                        })
                        .ok();
                    let backward: Option<Vec<CallGraphSymbol>> = bridge
                        .reverse_reachable_symbols(symbol_id)
                        .map(|symbols| {
                            symbols
                                .into_iter()
                                .map(|s| CallGraphSymbol {
                                    symbol_id: s.symbol_id,
                                    fqn: s.fqn,
                                    file_path: s.file_path,
                                    kind: s.kind,
                                })
                                .collect()
                        })
                        .ok();
                    (forward, backward)
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Could not open Magellan database for call graph analysis: {}",
                        e
                    );
                    eprintln!("Note: --use-call-graph requires a Magellan code graph database");
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        // Output
        match cli.output {
            OutputFormat::Human => {
                println!("Block Impact Analysis (Blast Zone)");
                println!();
                println!("Function: {}", function_name);
                println!("Source block: {}", impact.source_block_id);
                println!();

                // Show call graph impact if available
                if let Some(ref forward) = forward_impact {
                    println!("Inter-Procedural Impact (Call Graph):");
                    println!("  Forward Impact: {} functions reached", forward.len());
                    for sym in forward {
                        println!("    - {}", sym.fqn.as_deref().unwrap_or(&sym.file_path));
                    }
                }
                if let Some(ref backward) = backward_impact {
                    if !backward.is_empty() {
                        println!(
                            "  Backward Impact: {} functions can reach this",
                            backward.len()
                        );
                        for sym in backward {
                            println!("    - {}", sym.fqn.as_deref().unwrap_or(&sym.file_path));
                        }
                    }
                }
                println!();

                println!("Intra-Procedural Impact (CFG):");
                println!("  Reachable blocks: {}", impact.reachable_count);
                if impact.reachable_count > 0 {
                    println!("  Affected blocks: {:?}", impact.reachable_blocks);
                } else {
                    println!("  Affected blocks: (none - block has no downstream impact)");
                }
                println!("  Max depth reached: {}", impact.max_depth_reached);
                println!(
                    "  Contains cycles: {}",
                    if impact.has_cycles {
                        "yes (loop detected)"
                    } else {
                        "no"
                    }
                );
                if let Some(depth) = max_depth {
                    println!("  Depth limit: {}", depth);
                } else {
                    println!("  Depth limit: unlimited");
                }
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let response = BlockImpactResponse {
                    function: function_name,
                    block_id: impact.source_block_id,
                    reachable_blocks: impact.reachable_blocks,
                    reachable_count: impact.reachable_count,
                    max_depth: impact.max_depth_reached,
                    has_cycles: impact.has_cycles,
                    forward_impact: forward_impact.clone(),
                    backward_impact: backward_impact.clone(),
                };
                let wrapper = output::JsonResponse::new(response);
                match cli.output {
                    OutputFormat::Json => println!("{}", wrapper.to_json()),
                    OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                    _ => unreachable!(),
                }
            }
        }
    }

    Ok(())
}
