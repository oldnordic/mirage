use crate::cli::responses::*;
use crate::cli::{detect_repo_path, resolve_db_path, Cli, OutputFormat, PathsArgs};
use crate::output;
use anyhow::Result;

pub fn paths(args: &PathsArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::icfg::{build_icfg, enumerate_icfg_paths, project_icfg_to_cfg, IcfgOptions};
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
                let mut output = String::new();
                output.push_str(&format!(
                    "Incremental path enumeration (since {}):\n",
                    since
                ));
                output.push_str(&format!(
                    "  Analyzed functions: {}\n",
                    result.analyzed_functions
                ));
                output.push_str(&format!("  Total paths: {}\n", result.paths.len()));

                if args.show_errors {
                    let error_count = result
                        .paths
                        .iter()
                        .filter(|p| matches!(p.kind, PathKind::Error))
                        .count();
                    output.push_str(&format!("  Error paths: {}\n", error_count));
                }

                if !result.paths.is_empty() {
                    output.push_str("\nPaths:\n");
                    for path in &result.paths {
                        if args.show_errors || !matches!(path.kind, PathKind::Error) {
                            output.push_str(&format!("  {}\n", path));
                        }
                    }
                }
                let processed = output::apply_token_budget(output, args.tokens);
                print!("{}", processed);
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
                let json_str = serde_json::to_string(&response).unwrap_or_default();
                let processed = output::apply_token_budget(json_str, args.tokens);
                let tokens_est = processed.len() / 4;
                let truncated = args.tokens.map_or(false, |t| t > 0 && tokens_est > t);
                let wrapper = output::JsonResponse::new(response)
                    .with_tokens(tokens_est)
                    .with_truncated(truncated);
                println!("{}", wrapper.to_json());
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
                let json_str = serde_json::to_string_pretty(&response).unwrap_or_default();
                let processed = output::apply_token_budget(json_str, args.tokens);
                let tokens_est = processed.len() / 4;
                let truncated = args.tokens.map_or(false, |t| t > 0 && tokens_est > t);
                let wrapper = output::JsonResponse::new(response)
                    .with_tokens(tokens_est)
                    .with_truncated(truncated);
                println!("{}", wrapper.to_pretty_json());
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

    // Build path limits based on args
    let mut limits = PathLimits::default();
    if let Some(max_length) = args.max_length {
        limits = limits.with_max_length(max_length);
    }

    let mut projected_icfg = None;

    let mut paths = if args.inter_procedural {
        let icfg = build_icfg(
            db.storage(),
            db.backend(),
            db.path(),
            function_id,
            IcfgOptions {
                max_depth: limits.max_length,
                include_return_edges: true,
            },
        )
        .map_err(|e| anyhow::anyhow!("ICFG path enumeration failed: {}", e))?;
        let synthetic_cfg = project_icfg_to_cfg(&icfg);
        let paths = enumerate_icfg_paths(&icfg, &limits);
        projected_icfg = Some((icfg, synthetic_cfg));
        paths
    } else {
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

        if db.is_sqlite() {
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
            crate::cfg::enumerate_paths(&cfg, &limits)
        }
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
        if let Some((_, synthetic_cfg)) = projected_icfg.as_ref() {
            let node_hits: std::collections::HashMap<usize, i64> = synthetic_cfg
                .node_indices()
                .filter_map(|idx| synthetic_cfg.node_weight(idx).map(|b| (b.id, 0)))
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
                total_b.cmp(&total_a)
            });
        } else {
            let cfg = load_cfg_from_db(&db, function_id)
                .map_err(|e| anyhow::anyhow!("Failed to reload CFG for coverage sorting: {}", e))?;
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
                total_b.cmp(&total_a)
            });
        }
    }

    // Count error paths for reporting
    let error_count = paths.iter().filter(|p| p.kind == PathKind::Error).count();

    // Format output based on cli.output
    match cli.output {
        OutputFormat::Human => {
            // Human-readable text format
            let mut output = String::new();
            output.push_str(&format!("Function: {}\n", args.function));
            output.push_str(&format!("Total paths: {}\n", paths.len()));
            if args.show_errors {
                output.push_str("(Showing error paths only)\n");
            } else {
                output.push_str(&format!("Error paths: {}\n", error_count));
            }
            output.push_str("\n");

            if paths.is_empty() {
                let processed = output::apply_token_budget(output, args.tokens);
                print!("{}", processed);
                output::info("No paths found");
                return Ok(());
            }

            for (i, path) in paths.iter().enumerate() {
                output.push_str(&format!("Path {}: {}\n", i + 1, path.path_id));
                output.push_str(&format!("  Kind: {:?}\n", path.kind));
                output.push_str(&format!("  Length: {} blocks\n", path.len()));
                if args.with_blocks {
                    let rendered_blocks = if let Some((icfg, _)) = projected_icfg.as_ref() {
                        path.blocks
                            .iter()
                            .map(|id| {
                                let node = &icfg.graph[petgraph::graph::NodeIndex::new(*id)];
                                match (&node.function_name, node.block_id) {
                                    (Some(function_name), block_id) if block_id >= 0 => {
                                        format!("{}:{}", function_name, block_id)
                                    }
                                    (Some(function_name), -1) => format!("{}:entry", function_name),
                                    (Some(function_name), -2) => format!("{}:exit", function_name),
                                    (Some(function_name), block_id) => {
                                        format!("{}:{}", function_name, block_id)
                                    }
                                    (None, block_id) => block_id.to_string(),
                                }
                            })
                            .collect::<Vec<_>>()
                    } else {
                        path.blocks
                            .iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                    };
                    output.push_str(&format!("  Blocks: {}\n", rendered_blocks.join(" -> ")));
                }
                output.push_str("\n");
            }
            let processed = output::apply_token_budget(output, args.tokens);
            print!("{}", processed);
        }
        OutputFormat::Json => {
            let response = PathsResponse {
                function: args.function.clone(),
                total_paths: paths.len(),
                error_paths: error_count,
                paths: if let Some((icfg, synthetic_cfg)) = projected_icfg.as_ref() {
                    paths
                        .iter()
                        .map(|p| PathSummary::from_icfg_path(p.clone(), synthetic_cfg, icfg))
                        .collect()
                } else {
                    let cfg = load_cfg_from_db(&db, function_id).map_err(|e| {
                        anyhow::anyhow!("Failed to reload CFG for JSON path output: {}", e)
                    })?;
                    paths
                        .iter()
                        .map(|p| PathSummary::from_with_cfg(p.clone(), &cfg))
                        .collect()
                },
            };
            let json_str = serde_json::to_string(&response).unwrap_or_default();
            let processed = output::apply_token_budget(json_str, args.tokens);
            let tokens_est = processed.len() / 4;
            let truncated = args.tokens.map_or(false, |t| t > 0 && tokens_est > t);
            let wrapper = output::JsonResponse::new(response)
                .with_tokens(tokens_est)
                .with_truncated(truncated);
            println!("{}", wrapper.to_json());
        }
        OutputFormat::Pretty => {
            let response = PathsResponse {
                function: args.function.clone(),
                total_paths: paths.len(),
                error_paths: error_count,
                paths: if let Some((icfg, synthetic_cfg)) = projected_icfg.as_ref() {
                    paths
                        .iter()
                        .map(|p| PathSummary::from_icfg_path(p.clone(), synthetic_cfg, icfg))
                        .collect()
                } else {
                    let cfg = load_cfg_from_db(&db, function_id).map_err(|e| {
                        anyhow::anyhow!("Failed to reload CFG for pretty path output: {}", e)
                    })?;
                    paths
                        .iter()
                        .map(|p| PathSummary::from_with_cfg(p.clone(), &cfg))
                        .collect()
                },
            };
            let json_str = serde_json::to_string_pretty(&response).unwrap_or_default();
            let processed = output::apply_token_budget(json_str, args.tokens);
            let tokens_est = processed.len() / 4;
            let truncated = args.tokens.map_or(false, |t| t > 0 && tokens_est > t);
            let wrapper = output::JsonResponse::new(response)
                .with_tokens(tokens_est)
                .with_truncated(truncated);
            println!("{}", wrapper.to_pretty_json());
        }
    }

    Ok(())
}
