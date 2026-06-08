use crate::cli::responses::*;
use crate::cli::{detect_repo_path, resolve_db_path, Cli, OutputFormat, PathsArgs};
use crate::output;
use anyhow::Result;

pub fn paths(args: &PathsArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::load_cfg_from_db;
    use crate::cfg::{enumerate_paths_incremental, get_or_enumerate_paths, PathKind, PathLimits};
    use crate::storage::resolve_function_or_semantic;
    use crate::storage::{get_function_hash_db, MirageDb};

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Detect repository path for incremental mode
    let repo_path = detect_repo_path(&db_path);

    // Handle incremental mode
    if args.incremental {
        let since = args
            .since
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("--since required with --incremental"))?;

        // Open database for incremental mode
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

        // Run incremental path enumeration
        let result = match enumerate_paths_incremental(
            &args.function,
            &db,
            &repo_path,
            since,
            args.max_length,
        ) {
            Ok(r) => r,
            Err(e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::new(
                        "IncrementalAnalysisError",
                        &format!("Incremental analysis failed: {}", e),
                        output::E_CFG_ERROR,
                    );
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!("Incremental analysis failed: {}", e));
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Output results
        match cli.output {
            OutputFormat::Human => {
                println!("Incremental path enumeration (since {}):", since);
                println!("  Analyzed functions: {}", result.analyzed_functions);
                println!("  Total paths: {}", result.paths.len());

                if args.show_errors {
                    let error_count = result
                        .paths
                        .iter()
                        .filter(|p| matches!(p.kind, PathKind::Error))
                        .count();
                    println!("  Error paths: {}", error_count);
                }

                if !result.paths.is_empty() {
                    println!("\nPaths:");
                    for path in &result.paths {
                        if args.show_errors || !matches!(path.kind, PathKind::Error) {
                            println!("  {}", path);
                        }
                    }
                }
            }
            OutputFormat::Json => {
                let response = serde_json::json!({
                    "incremental": true,
                    "since": since,
                    "analyzed_functions": result.analyzed_functions,
                    "skipped_functions": result.skipped_functions,
                    "total_paths": result.paths.len(),
                    "paths": result.paths,
                });
                println!("{}", serde_json::to_string(&response)?);
            }
            OutputFormat::Pretty => {
                let response = serde_json::json!({
                    "incremental": true,
                    "since": since,
                    "analyzed_functions": result.analyzed_functions,
                    "skipped_functions": result.skipped_functions,
                    "total_paths": result.paths.len(),
                    "paths": result.paths,
                });
                println!("{}", serde_json::to_string_pretty(&response)?);
            }
        }

        return Ok(());
    }

    // Standard path enumeration (non-incremental)
    // Open database
    let mut db = match MirageDb::open(&db_path) {
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

    // Resolve function name/ID or semantic query to function_id (with optional file filter)
    let function_id = match resolve_function_or_semantic(
        &db,
        &args.function,
        args.semantic_query.as_deref(),
        args.file.as_deref(),
    ) {
        Ok(id) => id,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "FunctionNotFound",
                    &format!("{}", e),
                    output::E_CFG_ERROR,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("{}", e));
                output::info(&format!("Hint: {}", output::R_HINT_LIST_FUNCTIONS));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Load CFG from database
    let cfg = match load_cfg_from_db(&db, function_id) {
        Ok(cfg) => cfg,
        Err(_e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "CgfLoadError",
                    &format!("Failed to load CFG for function '{}'", args.function),
                    output::E_CFG_ERROR,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!(
                    "Failed to load CFG for function '{}'",
                    args.function
                ));
                output::info("The function may be corrupted. Try re-running 'magellan watch'");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Build path limits based on args
    let mut limits = PathLimits::default();
    if let Some(max_length) = args.max_length {
        limits = limits.with_max_length(max_length);
    }

    // Enumerate paths (backend-agnostic)
    // For SQLite backend: use get_or_enumerate_paths for caching
    let mut paths = if db.is_sqlite() {
        // SQLite backend: use caching layer
        let function_hash = match get_function_hash_db(&db, function_id) {
            Some(hash) => hash,
            None => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::new(
                        "HashNotFound",
                        &format!("Function hash not found for '{}'", args.function),
                        output::E_CFG_ERROR,
                    );
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!("Function hash not found for '{}'", args.function));
                    output::info(
                        "The function data may be incomplete. Try re-running 'magellan watch'",
                    );
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        get_or_enumerate_paths(&cfg, function_id, &function_hash, &limits, db.conn_mut()?)
            .map_err(|e| anyhow::anyhow!("Path enumeration failed: {}", e))?
    } else {
        // Native-v3 backend: enumerate directly without caching
        // Magellan manages its own caching
        crate::cfg::enumerate_paths(&cfg, &limits)
    };

    // Filter to error paths if requested
    if args.show_errors {
        paths.retain(|p| p.kind == PathKind::Error);
    }

    // Sort by coverage if requested (highest total hit count first)
    if args.by_coverage {
        let coverage_map: std::collections::HashMap<i64, i64> = db
            .conn()
            .ok()
            .and_then(|conn| {
                let sql = "SELECT block_id, hit_count FROM cfg_block_coverage \
                       WHERE block_id IN (SELECT id FROM cfg_blocks WHERE function_id = ?1)";
                let mut stmt = conn.prepare(sql).ok()?;
                let rows = stmt.query_map([function_id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                });
                let mut map = std::collections::HashMap::new();
                if let Ok(iter) = rows {
                    for (block_id, hit_count) in iter.flatten() {
                        map.insert(block_id, hit_count);
                    }
                }
                if map.is_empty() {
                    None
                } else {
                    Some(map)
                }
            })
            .unwrap_or_default();

        // Build graph node index -> hit_count lookup via db_id
        let node_hits: std::collections::HashMap<usize, i64> = cfg
            .node_indices()
            .filter_map(|idx| {
                cfg.node_weight(idx).and_then(|b| {
                    b.db_id
                        .and_then(|db_id| coverage_map.get(&db_id).copied())
                        .map(|hits| (b.id, hits))
                })
            })
            .collect();

        paths.sort_by(|a, b| {
            let total_a: i64 = a
                .blocks
                .iter()
                .map(|bid| node_hits.get(bid).copied().unwrap_or(0))
                .sum();
            let total_b: i64 = b
                .blocks
                .iter()
                .map(|bid| node_hits.get(bid).copied().unwrap_or(0))
                .sum();
            total_b.cmp(&total_a) // descending
        });
    }

    // Count error paths for reporting
    let error_count = paths.iter().filter(|p| p.kind == PathKind::Error).count();

    // Format output based on cli.output
    match cli.output {
        OutputFormat::Human => {
            // Human-readable text format
            println!("Function: {}", args.function);
            println!("Total paths: {}", paths.len());
            if args.show_errors {
                println!("(Showing error paths only)");
            } else {
                println!("Error paths: {}", error_count);
            }
            println!();

            if paths.is_empty() {
                output::info("No paths found");
                return Ok(());
            }

            for (i, path) in paths.iter().enumerate() {
                println!("Path {}: {}", i + 1, path.path_id);
                println!("  Kind: {:?}", path.kind);
                println!("  Length: {} blocks", path.len());
                if args.with_blocks {
                    println!(
                        "  Blocks: {}",
                        path.blocks
                            .iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                            .join(" -> ")
                    );
                }
                println!();
            }
        }
        OutputFormat::Json => {
            // Compact JSON with source locations from CFG
            let response = PathsResponse {
                function: args.function.clone(),
                total_paths: paths.len(),
                error_paths: error_count,
                paths: paths
                    .iter()
                    .map(|p| PathSummary::from_with_cfg(p.clone(), &cfg))
                    .collect(),
            };
            let wrapper = output::JsonResponse::new(response);
            println!("{}", wrapper.to_json());
        }
        OutputFormat::Pretty => {
            // Formatted JSON with indentation and source locations from CFG
            let response = PathsResponse {
                function: args.function.clone(),
                total_paths: paths.len(),
                error_paths: error_count,
                paths: paths
                    .iter()
                    .map(|p| PathSummary::from_with_cfg(p.clone(), &cfg))
                    .collect(),
            };
            let wrapper = output::JsonResponse::new(response);
            println!("{}", wrapper.to_pretty_json());
        }
    }

    Ok(())
}
