use crate::cli::responses::*;
use crate::cli::{resolve_db_path, Cli, HotspotsArgs, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn hotspots(args: &HotspotsArgs, cli: &Cli) -> Result<()> {
    use crate::analysis::MagellanBridge;
    #[cfg(feature = "sqlite")]
    use crate::cfg::{
        enumerate_paths_with_context, load_cfg_from_db_with_conn, EnumerationContext, PathLimits,
    };
    #[cfg(feature = "sqlite")]
    use crate::storage::MirageDb;
    use std::collections::HashMap;

    let db_path = resolve_db_path(cli.db.clone())?;

    // Open Mirage database for intra-procedural analysis
    // (honest open-error handling via shared helper)
    #[cfg(feature = "sqlite")]
    let mut db = super::open_db_or_exit(cli, &db_path);

    let mut hotspots: Vec<HotspotEntry> = Vec::new();
    let mut function_count = 0;

    if args.inter_procedural {
        // Inter-procedural: Use Magellan for call graph analysis
        match MagellanBridge::open(&db_path) {
            Ok(bridge) => {
                // Get path enumeration from entry point
                let path_result = bridge.enumerate_paths(&args.entry, None, 50, args.top * 10);

                if let Ok(paths) = path_result {
                    // Count paths through each function
                    let mut path_counts: HashMap<String, usize> = HashMap::new();

                    for path in &paths.paths {
                        for symbol in &path.symbols {
                            if let Some(fqn) = &symbol.fqn {
                                *path_counts.entry(fqn.clone()).or_insert(0) += 1;
                            }
                        }
                    }

                    // Get condensation for dominance (SCC size indicates coupling)
                    let condensed = bridge.condense_call_graph();
                    if let Ok(condensed) = condensed {
                        let mut scc_sizes: HashMap<String, f64> = HashMap::new();

                        for supernode in &condensed.graph.supernodes {
                            let size = supernode.members.len() as f64;
                            for member in &supernode.members {
                                if let Some(fqn) = &member.fqn {
                                    scc_sizes.insert(fqn.clone(), size);
                                }
                            }
                        }

                        // Combine metrics for hotspot scoring
                        for (fqn, path_count) in &path_counts {
                            if *path_count >= args.min_paths.unwrap_or(1) {
                                let dominance = scc_sizes.get(fqn).copied().unwrap_or(1.0);
                                let risk_score = (*path_count as f64) * 1.0 + dominance * 2.0;

                                hotspots.push(HotspotEntry {
                                    function: fqn.clone(),
                                    risk_score,
                                    path_count: *path_count,
                                    dominance_factor: dominance,
                                    complexity: 0, // Would need CFG for this
                                    file_path: "".to_string(),
                                });
                            }
                        }

                        // Count total functions found in inter-procedural analysis
                        function_count = path_counts.len();
                    }
                }
            }
            Err(_) => {
                output::warn("Magellan database not available, using intra-procedural analysis");
            }
        }
    }

    // Fallback to intra-procedural if no hotspots found or inter-procedural failed
    #[cfg(feature = "sqlite")]
    if hotspots.is_empty() && db.is_sqlite() {
        // Get all functions from database by joining with graph_entities
        let conn = db.conn_mut()?;

        let query = "SELECT DISTINCT cb.function_id, ge.name, ge.file_path
                    FROM cfg_blocks cb
                    JOIN graph_entities ge ON cb.function_id = ge.id";
        let mut stmt = conn.prepare(query)?;

        let function_rows = stmt.query_map([], |row: &rusqlite::Row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        for func_result in function_rows {
            let (func_id, func_name, file_path) = func_result?;
            function_count += 1;

            if let Ok(cfg) = load_cfg_from_db_with_conn(conn, func_id) {
                let ctx = EnumerationContext::new(&cfg);
                let limits = PathLimits::quick_analysis();
                let paths = enumerate_paths_with_context(&cfg, &limits, &ctx);

                let path_count = paths.len();
                if path_count < args.min_paths.unwrap_or(1) {
                    continue;
                }

                let complexity = cfg.node_count();
                let dominance = 1.0;
                let risk_score = path_count as f64 * 0.5 + complexity as f64 * 0.1;

                hotspots.push(HotspotEntry {
                    function: func_name.clone(),
                    risk_score,
                    path_count,
                    dominance_factor: dominance,
                    complexity,
                    file_path,
                });
            }
        }
    }
    // Sort by risk score (descending) - use total_cmp for f64 comparison
    hotspots.sort_by(|a, b| b.risk_score.total_cmp(&a.risk_score));

    // Limit to top N
    hotspots.truncate(args.top);

    let response = HotspotsResponse {
        entry_point: args.entry.clone(),
        total_functions: function_count,
        hotspots: hotspots.clone(),
        mode: if args.inter_procedural {
            "inter-procedural"
        } else {
            "intra-procedural"
        }
        .to_string(),
    };

    match cli.output {
        OutputFormat::Human => {
            let mut output = String::new();
            output.push_str(&format!(
                "Hotspots Analysis (entry: {})\n",
                response.entry_point
            ));
            output.push('\n');

            // Add helpful hint if 0 functions found with intra-procedural mode
            if response.total_functions == 0 && response.mode == "intra-procedural" {
                output.push_str("No functions found. This may be because:\n");
                output.push_str("  1. The database hasn't been indexed yet\n");
                output.push_str("  2. You need to run: magellan watch --db <path>\n");
                output.push_str("  3. Try --inter-procedural for call-graph-based analysis\n");
                output.push('\n');
            }

            output.push_str(&format!(
                "Found {} hotspots out of {} functions\n",
                hotspots.len(),
                response.total_functions
            ));
            output.push('\n');

            for (i, hotspot) in hotspots.iter().enumerate() {
                output.push_str(&format!(
                    "{}. {} (risk: {:.1})\n",
                    i + 1,
                    hotspot.function,
                    hotspot.risk_score
                ));
                if args.verbose {
                    output.push_str(&format!("   Paths: {}\n", hotspot.path_count));
                    output.push_str(&format!("   Dominance: {:.1}\n", hotspot.dominance_factor));
                    output.push_str(&format!("   Complexity: {}\n", hotspot.complexity));
                }
            }
            let processed = output::apply_token_budget(output, args.tokens);
            print!("{}", processed);
        }
        OutputFormat::Json => {
            let json_str = serde_json::to_string(&response).unwrap_or_default();
            let processed = output::apply_token_budget(json_str, args.tokens);
            let tokens_est = processed.len() / 4;
            let truncated = args.tokens.is_some_and(|t| t > 0 && tokens_est > t);
            let wrapper = output::JsonResponse::new(response)
                .with_tokens(tokens_est)
                .with_truncated(truncated);
            println!("{}", wrapper.to_json());
        }
        OutputFormat::Pretty => {
            let json_str = serde_json::to_string_pretty(&response).unwrap_or_default();
            let processed = output::apply_token_budget(json_str, args.tokens);
            let tokens_est = processed.len() / 4;
            let truncated = args.tokens.is_some_and(|t| t > 0 && tokens_est > t);
            let wrapper = output::JsonResponse::new(response)
                .with_tokens(tokens_est)
                .with_truncated(truncated);
            println!("{}", wrapper.to_pretty_json());
        }
    }

    Ok(())
}
