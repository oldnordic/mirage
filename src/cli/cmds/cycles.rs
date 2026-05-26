use crate::cli::{resolve_db_path, Cli, CycleTypeArg, CyclesArgs, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn cycles(args: &CyclesArgs, cli: &Cli) -> Result<()> {
    use crate::analysis::{CycleInfo, EnhancedCycles, LoopInfo, MagellanBridge};
    use crate::cfg::detect_natural_loops;
    use crate::cfg::load_cfg_from_db;
    use crate::storage::MirageDb;

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Default: show both types if no flag specified
    let show_call_graph = args.call_graph || args.both || !args.function_loops;
    let show_function_loops = args.function_loops || args.both || !args.call_graph;

    // Detect call graph cycles if requested
    let mut call_graph_cycles: Vec<CycleInfo> = if show_call_graph {
        match MagellanBridge::open(&db_path) {
            Ok(bridge) => match bridge.detect_cycles() {
                Ok(report) => report.cycles.iter().map(|c| c.into()).collect(),
                Err(e) => {
                    eprintln!("Warning: Failed to detect call graph cycles: {}", e);
                    vec![]
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: Could not open Magellan database for call graph cycles: {}",
                    e
                );
                eprintln!("Note: Call graph cycles require a Magellan code graph database");
                vec![]
            }
        }
    } else {
        vec![]
    };

    // Filter call graph cycles by type
    call_graph_cycles.retain(|c| match args.cycle_type {
        CycleTypeArg::All => true,
        CycleTypeArg::InterFunction => c.cycle_type == "MutualRecursion",
        CycleTypeArg::SelfLoop => c.cycle_type == "SelfLoop",
    });

    // Detect function loops if requested
    let mut function_loops_map: std::collections::HashMap<String, Vec<LoopInfo>> =
        std::collections::HashMap::new();

    if show_function_loops {
        // Open Mirage database
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

        // Query all functions from the database
        // Note: Requires SQLite backend for SQL queries
        if !db.is_sqlite() {
            output::error("The 'cycles' command with --function-loops requires SQLite backend.");
            output::info("retired binary backend backend is not yet supported for this feature.");
            std::process::exit(output::EXIT_USAGE);
        }

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
                for (function_name, function_id) in rows.flatten() {
                    // Load CFG for this function
                    if let Ok(cfg) = load_cfg_from_db(&db, function_id) {
                        // Detect natural loops
                        let natural_loops = detect_natural_loops(&cfg);

                        if !natural_loops.is_empty() {
                            let loop_infos: Vec<LoopInfo> = natural_loops
                                .iter()
                                .map(|loop_| {
                                    let nesting_level = loop_.nesting_level(&natural_loops);
                                    let body_blocks: Vec<usize> =
                                        loop_.body.iter().map(|&node| cfg[node].id).collect();
                                    LoopInfo {
                                        header: cfg[loop_.header].id,
                                        back_edge_from: cfg[loop_.back_edge.0].id,
                                        body_size: loop_.size(),
                                        nesting_level,
                                        body_blocks,
                                    }
                                })
                                .collect();

                            function_loops_map.insert(function_name, loop_infos);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to execute query: {}", e);
            }
        }
    }

    // Combine results
    let total_cycles =
        call_graph_cycles.len() + function_loops_map.values().map(|v| v.len()).sum::<usize>();

    let enhanced_cycles = EnhancedCycles {
        call_graph_cycles,
        function_loops: function_loops_map.clone(),
        total_cycles,
    };

    // Output based on format
    match cli.output {
        OutputFormat::Human => {
            println!("Cycle Detection Report");
            println!();

            if show_call_graph {
                println!(
                    "Call Graph Cycles (Inter-procedural): {}",
                    enhanced_cycles.call_graph_cycles.len()
                );
                if enhanced_cycles.call_graph_cycles.is_empty() {
                    println!("  No call graph cycles detected");
                } else {
                    for (i, cycle) in enhanced_cycles.call_graph_cycles.iter().enumerate() {
                        println!("  Cycle {}:", i + 1);
                        println!("    Type: {}", cycle.cycle_type);
                        println!("    Size: {} symbols", cycle.size);
                        if args.verbose {
                            println!("    Members:");
                            for member in &cycle.members {
                                println!("      - {}", member);
                            }
                        }
                    }
                }
                println!();
            }

            if show_function_loops {
                println!(
                    "Function Loops (Intra-procedural): {} functions with loops",
                    enhanced_cycles.function_loops.len()
                );
                if enhanced_cycles.function_loops.is_empty() {
                    println!("  No natural loops detected in any function");
                } else {
                    for (function_name, loops) in &enhanced_cycles.function_loops {
                        println!("  Function: {} ({} loops)", function_name, loops.len());
                        if args.verbose {
                            for (i, loop_info) in loops.iter().enumerate() {
                                println!("    Loop {}:", i + 1);
                                println!("      Header: Block {}", loop_info.header);
                                println!(
                                    "      Back edge from: Block {}",
                                    loop_info.back_edge_from
                                );
                                println!("      Body size: {} blocks", loop_info.body_size);
                                println!("      Nesting level: {}", loop_info.nesting_level);
                                println!("      Body blocks: {:?}", loop_info.body_blocks);
                            }
                        }
                    }
                }
                println!();
            }

            println!("Total cycles: {}", total_cycles);
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let wrapper = output::JsonResponse::new(enhanced_cycles);
            match cli.output {
                OutputFormat::Json => println!("{}", wrapper.to_json()),
                OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}
