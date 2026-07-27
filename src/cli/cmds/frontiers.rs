use crate::cli::responses::*;
use crate::cli::{resolve_db_path, Cli, FrontiersArgs, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn frontiers(args: &FrontiersArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::load_cfg_from_db;
    use crate::cfg::{compute_dominance_frontiers, DominatorTree};
    use crate::storage::resolve_function_or_semantic;

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Open database (honest open-error handling via shared helper)
    let db = super::open_db_or_exit(cli, &db_path);

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

    // Compute dominator tree
    let dom_tree = match DominatorTree::new(&cfg) {
        Some(tree) => tree,
        None => {
            output::error("Could not compute dominator tree (CFG may have no entry blocks)");
            std::process::exit(1);
        }
    };

    // Compute dominance frontiers
    let frontiers = compute_dominance_frontiers(&cfg, dom_tree);

    // Handle query modes based on args
    if args.iterated {
        // Show iterated dominance frontier
        let all_nodes: Vec<petgraph::graph::NodeIndex> = cfg.node_indices().collect();
        let iterated_frontier = frontiers.iterated_frontier(&all_nodes);
        let iterated_blocks: Vec<usize> = iterated_frontier.iter().map(|&n| cfg[n].id).collect();

        match cli.output {
            OutputFormat::Human => {
                println!("Function: {}", args.function);
                println!("Iterated Dominance Frontier:");
                println!("Count: {}", iterated_blocks.len());
                println!();
                if iterated_blocks.is_empty() {
                    output::info("No iterated dominance frontier (linear CFG)");
                } else {
                    println!("Blocks in iterated frontier:");
                    for id in &iterated_blocks {
                        println!("  - Block {}", id);
                    }
                }
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let response = IteratedFrontierResponse {
                    function: args.function.clone(),
                    iterated_frontier: iterated_blocks,
                };
                let wrapper = output::JsonResponse::new(response);
                match cli.output {
                    OutputFormat::Json => println!("{}", wrapper.to_json()),
                    OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                    _ => unreachable!(),
                }
            }
        }
    } else if let Some(node_id) = args.node {
        // Show frontier for specific node only
        let target_node = cfg.node_indices().find(|&n| cfg[n].id == node_id);

        let target_node = match target_node {
            Some(node) => node,
            None => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::block_not_found(node_id);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(1);
                } else {
                    output::error(&format!("Block {} not found in CFG", node_id));
                    std::process::exit(1);
                }
            }
        };

        let frontier = frontiers.frontier(target_node);
        let frontier_blocks: Vec<usize> = frontier.iter().map(|&n| cfg[n].id).collect();

        match cli.output {
            OutputFormat::Human => {
                println!("Function: {}", args.function);
                println!("Dominance Frontier for Block {}:", node_id);
                println!("Count: {}", frontier_blocks.len());
                println!();
                if frontier_blocks.is_empty() {
                    output::info(&format!("Block {} has empty dominance frontier", node_id));
                } else {
                    println!("Frontier blocks:");
                    for id in &frontier_blocks {
                        println!("  - Block {}", id);
                    }
                }
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let response = FrontiersResponse {
                    function: args.function.clone(),
                    nodes_with_frontiers: if frontier_blocks.is_empty() { 0 } else { 1 },
                    frontiers: vec![NodeFrontier {
                        node: node_id,
                        frontier_set: frontier_blocks,
                    }],
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
        // Show all nodes with non-empty frontiers
        let nodes_with_frontiers: Vec<NodeFrontier> = frontiers
            .nodes_with_frontiers()
            .map(|n| {
                let frontier = frontiers.frontier(n);
                NodeFrontier {
                    node: cfg[n].id,
                    frontier_set: frontier.iter().map(|&f| cfg[f].id).collect(),
                }
            })
            .collect();

        match cli.output {
            OutputFormat::Human => {
                println!("Function: {}", args.function);
                println!(
                    "Nodes with non-empty dominance frontiers: {}",
                    nodes_with_frontiers.len()
                );
                println!();

                if nodes_with_frontiers.is_empty() {
                    output::info("No dominance frontiers (linear CFG)");
                } else {
                    for node_info in &nodes_with_frontiers {
                        println!("Block {}:", node_info.node);
                        println!("  Frontier: {:?}", node_info.frontier_set);
                        println!();
                    }
                }
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let response = FrontiersResponse {
                    function: args.function.clone(),
                    nodes_with_frontiers: nodes_with_frontiers.len(),
                    frontiers: nodes_with_frontiers,
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
