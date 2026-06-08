use crate::cli::responses::*;
use crate::cli::{resolve_db_path, Cli, DominatorsArgs, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn dominators(args: &DominatorsArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::load_cfg_from_db;
    use crate::cfg::{DominatorTree, PostDominatorTree};
    use crate::storage::resolve_function_or_semantic;
    use crate::storage::MirageDb;

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Handle inter-procedural mode using call graph dominance
    if args.inter_procedural {
        return inter_procedural_dominators(args, cli, &db_path);
    }

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

    // Compute dominator tree based on args.post flag
    if args.post {
        // Post-dominator analysis
        let post_dom_tree = match PostDominatorTree::new(&cfg) {
            Some(tree) => tree,
            None => {
                output::error(
                    "Could not compute post-dominator tree (CFG may have no exit blocks)",
                );
                std::process::exit(1);
            }
        };

        // Handle must-pass-through query if specified
        if let Some(ref block_id_str) = args.must_pass_through {
            match block_id_str.parse::<usize>() {
                Ok(block_id) => {
                    // Find NodeIndex for this block
                    let target_node = cfg.node_indices().find(|&n| cfg[n].id == block_id);

                    let target_node = match target_node {
                        Some(node) => node,
                        None => {
                            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                                let error = output::JsonError::block_not_found(block_id);
                                let wrapper = output::JsonResponse::new(error);
                                println!("{}", wrapper.to_json());
                                std::process::exit(1);
                            } else {
                                output::error(&format!("Block {} not found in CFG", block_id));
                                std::process::exit(1);
                            }
                        }
                    };

                    // Find all nodes post-dominated by this block
                    let must_pass: Vec<usize> = cfg
                        .node_indices()
                        .filter(|&n| post_dom_tree.post_dominates(target_node, n))
                        .map(|n| cfg[n].id)
                        .collect();

                    // Output based on format
                    match cli.output {
                        OutputFormat::Human => {
                            println!("Function: {}", args.function);
                            println!(
                                "Post-Dominator Query: Blocks post-dominated by {}",
                                block_id
                            );
                            println!("Count: {}", must_pass.len());
                            println!();
                            if must_pass.is_empty() {
                                output::info("No blocks are post-dominated by this block");
                            } else {
                                println!("Blocks that must pass through {}:", block_id);
                                for id in &must_pass {
                                    println!("  - Block {}", id);
                                }
                            }
                        }
                        OutputFormat::Json | OutputFormat::Pretty => {
                            let response = DominanceResponse {
                                function: args.function.clone(),
                                kind: "post-dominators".to_string(),
                                root: Some(cfg[post_dom_tree.root()].id),
                                dominance_tree: vec![],
                                must_pass_through: Some(MustPassThroughResult {
                                    block: block_id,
                                    must_pass,
                                }),
                            };
                            let wrapper = output::JsonResponse::new(response);
                            match cli.output {
                                OutputFormat::Json => println!("{}", wrapper.to_json()),
                                OutputFormat::Pretty => {
                                    println!("{}", wrapper.to_pretty_json())
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                    return Ok(());
                }
                Err(_) => {
                    output::error(&format!("Invalid block ID: {}", block_id_str));
                    std::process::exit(1);
                }
            }
        }

        // Build dominance tree for output
        let dominance_tree: Vec<DominatorEntry> = cfg
            .node_indices()
            .map(|node| {
                let block = cfg[node].id;
                let immediate_dominator = post_dom_tree
                    .immediate_post_dominator(node)
                    .map(|n| cfg[n].id);
                let dominated: Vec<usize> = post_dom_tree
                    .children(node)
                    .iter()
                    .map(|&n| cfg[n].id)
                    .collect();
                DominatorEntry {
                    block,
                    immediate_dominator,
                    dominated,
                }
            })
            .collect();

        // Format output
        match cli.output {
            OutputFormat::Human => {
                println!("Function: {}", args.function);
                println!(
                    "Post-Dominator Tree (root: {})",
                    cfg[post_dom_tree.root()].id
                );
                println!();

                // Print tree structure
                print_dominator_tree_human(
                    &cfg,
                    post_dom_tree.as_dominator_tree(),
                    post_dom_tree.root(),
                    0,
                    true,
                );
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let response = DominanceResponse {
                    function: args.function.clone(),
                    kind: "post-dominators".to_string(),
                    root: Some(cfg[post_dom_tree.root()].id),
                    dominance_tree,
                    must_pass_through: None,
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
        // Regular dominator analysis
        let dom_tree = match DominatorTree::new(&cfg) {
            Some(tree) => tree,
            None => {
                output::error("Could not compute dominator tree (CFG may have no entry block)");
                std::process::exit(1);
            }
        };

        // Handle must-pass-through query if specified
        if let Some(ref block_id_str) = args.must_pass_through {
            match block_id_str.parse::<usize>() {
                Ok(block_id) => {
                    // Find NodeIndex for this block
                    let target_node = cfg.node_indices().find(|&n| cfg[n].id == block_id);

                    let target_node = match target_node {
                        Some(node) => node,
                        None => {
                            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                                let error = output::JsonError::block_not_found(block_id);
                                let wrapper = output::JsonResponse::new(error);
                                println!("{}", wrapper.to_json());
                                std::process::exit(1);
                            } else {
                                output::error(&format!("Block {} not found in CFG", block_id));
                                std::process::exit(1);
                            }
                        }
                    };

                    // Find all nodes dominated by this block
                    let must_pass: Vec<usize> = cfg
                        .node_indices()
                        .filter(|&n| dom_tree.dominates(target_node, n))
                        .map(|n| cfg[n].id)
                        .collect();

                    // Output based on format
                    match cli.output {
                        OutputFormat::Human => {
                            println!("Function: {}", args.function);
                            println!("Dominator Query: Blocks dominated by {}", block_id);
                            println!("Count: {}", must_pass.len());
                            println!();
                            if must_pass.is_empty() {
                                output::info("No blocks are dominated by this block");
                            } else {
                                println!("Blocks that must pass through {}:", block_id);
                                for id in &must_pass {
                                    println!("  - Block {}", id);
                                }
                            }
                        }
                        OutputFormat::Json | OutputFormat::Pretty => {
                            let response = DominanceResponse {
                                function: args.function.clone(),
                                kind: "dominators".to_string(),
                                root: Some(cfg[dom_tree.root()].id),
                                dominance_tree: vec![],
                                must_pass_through: Some(MustPassThroughResult {
                                    block: block_id,
                                    must_pass,
                                }),
                            };
                            let wrapper = output::JsonResponse::new(response);
                            match cli.output {
                                OutputFormat::Json => println!("{}", wrapper.to_json()),
                                OutputFormat::Pretty => {
                                    println!("{}", wrapper.to_pretty_json())
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                    return Ok(());
                }
                Err(_) => {
                    output::error(&format!("Invalid block ID: {}", block_id_str));
                    std::process::exit(1);
                }
            }
        }

        // Build dominance tree for output
        let dominance_tree: Vec<DominatorEntry> = cfg
            .node_indices()
            .map(|node| {
                let block = cfg[node].id;
                let immediate_dominator = dom_tree.immediate_dominator(node).map(|n| cfg[n].id);
                let dominated: Vec<usize> =
                    dom_tree.children(node).iter().map(|&n| cfg[n].id).collect();
                DominatorEntry {
                    block,
                    immediate_dominator,
                    dominated,
                }
            })
            .collect();

        // Format output
        match cli.output {
            OutputFormat::Human => {
                println!("Function: {}", args.function);
                println!("Dominator Tree (root: {})", cfg[dom_tree.root()].id);
                println!();

                // Print tree structure
                print_dominator_tree_human(&cfg, &dom_tree, dom_tree.root(), 0, false);
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let response = DominanceResponse {
                    function: args.function.clone(),
                    kind: "dominators".to_string(),
                    root: Some(cfg[dom_tree.root()].id),
                    dominance_tree,
                    must_pass_through: None,
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
fn print_dominator_tree_human(
    cfg: &crate::cfg::Cfg,
    dom_tree: &crate::cfg::DominatorTree,
    node: petgraph::graph::NodeIndex,
    depth: usize,
    is_post: bool,
) {
    let indent = "  ".repeat(depth);
    let block_id = cfg[node].id;
    let kind_label = if is_post {
        "post-dominator"
    } else {
        "dominator"
    };

    println!("{}Block {} ({})", indent, block_id, kind_label);

    for &child in dom_tree.children(node) {
        print_dominator_tree_human(cfg, dom_tree, child, depth + 1, is_post);
    }
}
fn inter_procedural_dominators(args: &DominatorsArgs, cli: &Cli, db_path: &str) -> Result<()> {
    use crate::analysis::MagellanBridge;
    use std::collections::{HashMap, HashSet};

    // Try to open Magellan database
    let bridge = match MagellanBridge::open(db_path) {
        Ok(b) => b,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "MagellanUnavailable",
                    &format!("Magellan database not available: {}", e),
                    "Run 'magellan watch' to build the call graph",
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Magellan database not available: {}", e));
                output::info("Hint: Run 'magellan watch' to build the call graph");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Condense the call graph to get a DAG of SCCs
    let condensed = match bridge.condense_call_graph() {
        Ok(c) => c,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "CondensationError",
                    &format!("Failed to condense call graph: {}", e),
                    "Ensure the call graph is properly built",
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to condense call graph: {}", e));
                output::info("Hint: Ensure the call graph is properly built");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Build adjacency list from condensation edges (for reachability analysis)
    let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();
    for &(from_id, to_id) in &condensed.graph.edges {
        adjacency.entry(from_id).or_default().push(to_id);
    }

    // Map symbols to their SCC IDs
    let mut symbol_to_scc: HashMap<String, i64> = HashMap::new();
    let mut scc_members: HashMap<i64, Vec<String>> = HashMap::new();

    for supernode in &condensed.graph.supernodes {
        let scc_id = supernode.id;
        for member in &supernode.members {
            if let Some(fqn) = &member.fqn {
                symbol_to_scc.insert(fqn.clone(), scc_id);
                scc_members.entry(scc_id).or_default().push(fqn.clone());
            }
        }
    }

    // Find all functions that dominate the target function
    // In a DAG, functions in upstream SCCs dominate functions in downstream SCCs
    let mut dominating_functions: Vec<String> = Vec::new();

    if let Some(&target_scc_id) = symbol_to_scc.get(&args.function) {
        // Find all SCCs that can reach the target SCC
        for &scc_id in scc_members.keys() {
            if scc_id != target_scc_id {
                let mut visited = HashSet::new();
                if can_reach_scc(scc_id, target_scc_id, &adjacency, &mut visited) {
                    // Add all members of this SCC as dominators
                    if let Some(members) = scc_members.get(&scc_id) {
                        dominating_functions.extend(members.clone());
                    }
                }
            }
        }
    }

    // Sort for consistent output
    dominating_functions.sort();

    // Format output
    match cli.output {
        OutputFormat::Human => {
            output::header(&format!("Inter-procedural Dominators: {}", args.function));
            output::info("Functions that must execute before this function can be reached");
            println!();

            if dominating_functions.is_empty() {
                println!("No dominators found (this may be an entry point or not in call graph)");
            } else {
                println!(
                    "Found {} dominating function(s):",
                    dominating_functions.len()
                );
                println!();
                for (i, dominator) in dominating_functions.iter().enumerate() {
                    println!("{}. {}", i + 1, dominator);
                }
                println!();
                output::info("These functions are on all call paths to the target");
            }
        }
        OutputFormat::Json => {
            let response = InterProceduralDominanceResponse {
                function: args.function.clone(),
                kind: "inter-procedural-dominators".to_string(),
                dominator_count: dominating_functions.len(),
                dominators: dominating_functions.clone(),
            };
            let wrapper = output::JsonResponse::new(response);
            println!("{}", wrapper.to_json());
        }
        OutputFormat::Pretty => {
            let response = InterProceduralDominanceResponse {
                function: args.function.clone(),
                kind: "inter-procedural-dominators".to_string(),
                dominator_count: dominating_functions.len(),
                dominators: dominating_functions.clone(),
            };
            let wrapper = output::JsonResponse::new(response);
            println!("{}", wrapper.to_pretty_json());
        }
    }

    Ok(())
}
fn can_reach_scc(
    from: i64,
    to: i64,
    adjacency: &std::collections::HashMap<i64, Vec<i64>>,
    visited: &mut std::collections::HashSet<i64>,
) -> bool {
    if from == to {
        return true;
    }
    if visited.contains(&from) {
        return false;
    }
    visited.insert(from);

    if let Some(neighbors) = adjacency.get(&from) {
        for &neighbor in neighbors {
            if can_reach_scc(neighbor, to, adjacency, visited) {
                return true;
            }
        }
    }
    false
}
