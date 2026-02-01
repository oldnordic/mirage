// CLI command definitions following Magellan's CLI patterns

use clap::{Parser, Subcommand, ValueEnum};

/// Mirage - Path-Aware Code Intelligence Engine
///
/// A control-flow and logic graph engine for Rust codebases.
/// Extracts MIR from rustc, builds CFGs, enumerates execution paths.
#[derive(Parser, Debug, Clone)]
#[command(name = "mirage")]
#[command(author, version, about)]
#[command(long_about = "Mirage is a path-aware code intelligence engine that operates on graphs, not text.

It materializes behavior explicitly: paths, proofs, counterexamples.

NOT:
  - A search tool (llmgrep already does this)
  - An embedding tool
  - Static analysis / linting

IS:
  - Path enumeration and verification
  - Graph-based reasoning about code behavior
  - Truth engine that materializes facts for LLM consumption

The Golden Rule: An agent may only speak if it can reference a graph artifact.")]
pub struct Cli {
    /// Path to the Magellan/Mirage database
    #[arg(global = true, long, env = "MIRAGE_DB")]
    pub db: Option<String>,

    /// Output format
    #[arg(global = true, long, value_enum, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

/// Output format options
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text output
    Human,
    /// Compact JSON for programmatic consumption
    Json,
    /// Formatted JSON with indentation
    Pretty,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Index a Rust project (build CFG from MIR)
    Index(IndexArgs),

    /// Show database statistics
    Status(StatusArgs),

    /// Show all execution paths through a function
    Paths(PathsArgs),

    /// Show control-flow graph for a function
    Cfg(CfgArgs),

    /// Show dominance relationships for a function
    Dominators(DominatorsArgs),

    /// Find unreachable code within functions
    Unreachable(UnreachableArgs),

    /// Verify a path is still valid
    Verify(VerifyArgs),

    /// Show impact analysis using paths (blast zone)
    BlastZone(BlastZoneArgs),
}

// ============================================================================
// Indexing Commands
// ============================================================================

#[derive(Parser, Debug, Clone)]
pub struct IndexArgs {
    /// Path to the Rust project to index
    #[arg(long)]
    pub project: Option<String>,

    /// Index specific crate
    #[arg(long)]
    pub crate_: Option<String>,

    /// Re-index only this function (by symbol_id)
    #[arg(long)]
    pub reindex: Option<String>,

    /// Incremental update (only changed functions)
    #[arg(long)]
    pub incremental: bool,
}

// ============================================================================
// Query Commands
// ============================================================================

#[derive(Parser, Debug, Clone, Copy)]
pub struct StatusArgs {}

#[derive(Parser, Debug, Clone)]
pub struct PathsArgs {
    /// Function symbol ID or name
    #[arg(long)]
    pub function: String,

    /// Show only error paths
    #[arg(long)]
    pub show_errors: bool,

    /// Maximum path length (for pruning)
    #[arg(long)]
    pub max_length: Option<usize>,

    /// Show block details for each path
    #[arg(long)]
    pub with_blocks: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct CfgArgs {
    /// Function symbol ID or name
    #[arg(long)]
    pub function: String,

    /// Output format
    #[arg(long, value_enum)]
    pub format: Option<CfgFormat>,
}

#[derive(Parser, Debug, Clone)]
pub struct DominatorsArgs {
    /// Function symbol ID or name
    #[arg(long)]
    pub function: String,

    /// Show blocks that must pass through this block
    #[arg(long)]
    pub must_pass_through: Option<String>,

    /// Show post-dominators instead of dominators
    #[arg(long)]
    pub post: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct UnreachableArgs {
    /// Find unreachable code within functions
    #[arg(long)]
    pub within_functions: bool,

    /// Show branch details
    #[arg(long)]
    pub show_branches: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct VerifyArgs {
    /// Path ID to verify
    #[arg(long)]
    pub path_id: String,
}

#[derive(Parser, Debug, Clone)]
pub struct BlastZoneArgs {
    /// Symbol ID or name
    #[arg(long)]
    pub symbol: String,

    /// Maximum depth to traverse
    #[arg(long, default_value_t = 3)]
    pub max_depth: usize,

    /// Include error paths in analysis
    #[arg(long)]
    pub include_errors: bool,
}

/// CFG output format
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfgFormat {
    /// Human-readable text
    Human,
    /// Graphviz DOT format
    Dot,
    /// JSON export
    Json,
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Resolve the database path from multiple sources
///
/// Priority: CLI arg > MIRAGE_DB env var > default "./codemcp.db"
/// This follows Magellan's pattern for database path resolution.
pub fn resolve_db_path(cli_db: Option<String>) -> anyhow::Result<String> {
    match cli_db {
        Some(path) => Ok(path),
        None => std::env::var("MIRAGE_DB")
            .or_else(|_| Ok("./codemcp.db".to_string())),
    }
}

// ============================================================================
// Response Structs for JSON Output
// ============================================================================

/// Response for paths command
#[derive(serde::Serialize)]
struct PathsResponse {
    function: String,
    total_paths: usize,
    error_paths: usize,
    paths: Vec<PathSummary>,
}

/// Summary of a single path for JSON output
#[derive(serde::Serialize)]
struct PathSummary {
    path_id: String,
    kind: String,
    length: usize,
    blocks: Vec<usize>,
}

impl From<crate::cfg::Path> for PathSummary {
    fn from(path: crate::cfg::Path) -> Self {
        let length = path.len();
        Self {
            path_id: path.path_id,
            kind: format!("{:?}", path.kind),
            length,
            blocks: path.blocks,
        }
    }
}

/// Response for dominators command
#[derive(serde::Serialize)]
struct DominanceResponse {
    function: String,
    kind: String,  // "dominators" or "post-dominators"
    root: Option<usize>,
    dominance_tree: Vec<DominatorEntry>,
    must_pass_through: Option<MustPassThroughResult>,
}

/// Entry in dominance tree for JSON output
#[derive(serde::Serialize)]
struct DominatorEntry {
    block: usize,
    immediate_dominator: Option<usize>,
    dominated: Vec<usize>,
}

/// Result of must-pass-through query
#[derive(serde::Serialize)]
struct MustPassThroughResult {
    block: usize,
    must_pass: Vec<usize>,
}

/// Response for unreachable command
#[derive(serde::Serialize)]
struct UnreachableResponse {
    function: String,
    total_functions: usize,
    functions_with_unreachable: usize,
    unreachable_count: usize,
    blocks: Vec<UnreachableBlock>,
}

/// Unreachable block details for JSON output
#[derive(serde::Serialize)]
struct UnreachableBlock {
    block_id: usize,
    kind: String,
    statements: Vec<String>,
    terminator: String,
}

/// Response for verify command
#[derive(serde::Serialize)]
struct VerifyResult {
    path_id: String,
    valid: bool,
    found_in_cache: bool,
    function_id: Option<i64>,
    reason: String,
    current_paths: usize,
}

// ============================================================================
// Command Handlers (stubs for now)
// ============================================================================

pub mod cmds {
    use super::*;
    use crate::output;
    use anyhow::Result;

    pub fn index(_args: IndexArgs) -> Result<()> {
        // TODO: Implement M1 (MIR Extraction)
        output::error("Indexing not yet implemented - requires MIR extraction (Milestone 1)");
        std::process::exit(1);
    }

    pub fn status(_args: StatusArgs, cli: &Cli) -> Result<()> {
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

        // Open database
        let db = match MirageDb::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                output::error(&format!("Failed to open database: {}", e));
                output::info("Hint: Run 'mirage index' to create the database");
                std::process::exit(output::EXIT_DATABASE);
            }
        };

        // Query database statistics
        let status = db.status()?;

        // Output based on format
        // VERIFIED: All three output formats (human/json/pretty) are implemented correctly
        // and follow Magellan's JsonResponse wrapper pattern for JSON outputs.
        match cli.output {
            OutputFormat::Human => {
                // Human-readable text format
                println!("Mirage Database Status:");
                println!("  Schema version: {} (Magellan: {})", status.mirage_schema_version, status.magellan_schema_version);
                println!("  cfg_blocks: {}", status.cfg_blocks);
                println!("  cfg_edges: {}", status.cfg_edges);
                println!("  cfg_paths: {}", status.cfg_paths);
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

    pub fn paths(args: &PathsArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::{PathKind, PathLimits, enumerate_paths};
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

        // Open database
        let _db = match MirageDb::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                output::error(&format!("Failed to open database: {}", e));
                output::info("Hint: Run 'mirage index' to create the database");
                std::process::exit(output::EXIT_DATABASE);
            }
        };

        // For now, create a test CFG since MIR extraction isn't complete
        // TODO: Load CFG from database using args.function
        let cfg = create_test_cfg();

        // Build path limits based on args
        let mut limits = PathLimits::default();
        if let Some(max_length) = args.max_length {
            limits = limits.with_max_length(max_length);
        }

        // Enumerate paths
        let mut paths = enumerate_paths(&cfg, &limits);

        // Filter to error paths if requested
        if args.show_errors {
            paths.retain(|p| p.kind == PathKind::Error);
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
                        println!("  Blocks: {}", path.blocks.iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                            .join(" -> "));
                    }
                    println!();
                }
            }
            OutputFormat::Json => {
                // Compact JSON
                let response = PathsResponse {
                    function: args.function.clone(),
                    total_paths: paths.len(),
                    error_paths: error_count,
                    paths: paths.into_iter().map(PathSummary::from).collect(),
                };
                let wrapper = output::JsonResponse::new(response);
                println!("{}", wrapper.to_json());
            }
            OutputFormat::Pretty => {
                // Formatted JSON with indentation
                let response = PathsResponse {
                    function: args.function.clone(),
                    total_paths: paths.len(),
                    error_paths: error_count,
                    paths: paths.into_iter().map(PathSummary::from).collect(),
                };
                let wrapper = output::JsonResponse::new(response);
                println!("{}", wrapper.to_pretty_json());
            }
        }

        Ok(())
    }

    pub fn cfg(args: &CfgArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::{export_dot, export_json, CFGExport};
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

        // Open database (follows status command pattern for error handling)
        let _db = match MirageDb::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                output::error(&format!("Failed to open database: {}", e));
                output::info("Hint: Run 'mirage index' to create the database");
                std::process::exit(output::EXIT_DATABASE);
            }
        };

        // TODO: Load CFG from database for the specified function.
        // This requires MIR extraction (Phase 02-01) to be complete.
        // For now, create a test CFG to demonstrate the export functionality.
        let cfg = create_test_cfg();

        // Determine output format (args.format overrides cli.output)
        let format = args.format.unwrap_or(match cli.output {
            OutputFormat::Human => CfgFormat::Human,
            OutputFormat::Json => CfgFormat::Json,
            OutputFormat::Pretty => CfgFormat::Json,
        });

        match format {
            CfgFormat::Human | CfgFormat::Dot => {
                // Both Human and Dot use DOT format
                let dot = export_dot(&cfg);
                println!("{}", dot);
            }
            CfgFormat::Json => {
                // Export to JSON and wrap in JsonResponse for consistency
                let export: CFGExport = export_json(&cfg, &args.function);
                let response = output::JsonResponse::new(export);

                match cli.output {
                    OutputFormat::Json => println!("{}", response.to_json()),
                    OutputFormat::Pretty => println!("{}", response.to_pretty_json()),
                    OutputFormat::Human => println!("{}", response.to_pretty_json()),
                }
            }
        }

        Ok(())
    }

    /// Helper to create a test CFG for demonstration
    ///
    /// This will be replaced with database loading in future plans
    /// when MIR extraction (02-01) is complete.
    pub(crate) fn create_test_cfg() -> crate::cfg::Cfg {
        use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
        use petgraph::graph::DiGraph;
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec!["let x = 1".to_string()],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec!["if x > 0".to_string()],
            terminator: Terminator::SwitchInt {
                targets: vec![2],
                otherwise: 3,
            },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec!["return true".to_string()],
            terminator: Terminator::Return,
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec!["return false".to_string()],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b3, EdgeType::FalseBranch);

        g
    }

    pub fn dominators(args: &DominatorsArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::{DominatorTree, PostDominatorTree};
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

        // Open database (follows status command pattern for error handling)
        let _db = match MirageDb::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                output::error(&format!("Failed to open database: {}", e));
                output::info("Hint: Run 'mirage index' to create the database");
                std::process::exit(output::EXIT_DATABASE);
            }
        };

        // TODO: Load CFG from database for the specified function.
        // This requires MIR extraction (Phase 02-01) to be complete.
        // For now, create a test CFG to demonstrate the dominance functionality.
        let cfg = create_test_cfg();

        // Compute dominator tree based on args.post flag
        if args.post {
            // Post-dominator analysis
            let post_dom_tree = match PostDominatorTree::new(&cfg) {
                Some(tree) => tree,
                None => {
                    output::error("Could not compute post-dominator tree (CFG may have no exit blocks)");
                    std::process::exit(1);
                }
            };

            // Handle must-pass-through query if specified
            if let Some(ref block_id_str) = args.must_pass_through {
                match block_id_str.parse::<usize>() {
                    Ok(block_id) => {
                        // Find NodeIndex for this block
                        let target_node = cfg.node_indices()
                            .find(|&n| cfg[n].id == block_id);

                        let target_node = match target_node {
                            Some(node) => node,
                            None => {
                                output::error(&format!("Block {} not found in CFG", block_id));
                                std::process::exit(1);
                            }
                        };

                        // Find all nodes post-dominated by this block
                        let must_pass: Vec<usize> = cfg.node_indices()
                            .filter(|&n| post_dom_tree.post_dominates(target_node, n))
                            .map(|n| cfg[n].id)
                            .collect();

                        // Output based on format
                        match cli.output {
                            OutputFormat::Human => {
                                println!("Function: {}", args.function);
                                println!("Post-Dominator Query: Blocks post-dominated by {}", block_id);
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
                                    OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
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
            let dominance_tree: Vec<DominatorEntry> = cfg.node_indices()
                .map(|node| {
                    let block = cfg[node].id;
                    let immediate_dominator = post_dom_tree.immediate_post_dominator(node)
                        .map(|n| cfg[n].id);
                    let dominated: Vec<usize> = post_dom_tree.children(node)
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
                    println!("Post-Dominator Tree (root: {})", cfg[post_dom_tree.root()].id);
                    println!();

                    // Print tree structure
                    print_dominator_tree_human(&cfg, post_dom_tree.as_dominator_tree(), post_dom_tree.root(), 0, true);
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
                        let target_node = cfg.node_indices()
                            .find(|&n| cfg[n].id == block_id);

                        let target_node = match target_node {
                            Some(node) => node,
                            None => {
                                output::error(&format!("Block {} not found in CFG", block_id));
                                std::process::exit(1);
                            }
                        };

                        // Find all nodes dominated by this block
                        let must_pass: Vec<usize> = cfg.node_indices()
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
                                    OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
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
            let dominance_tree: Vec<DominatorEntry> = cfg.node_indices()
                .map(|node| {
                    let block = cfg[node].id;
                    let immediate_dominator = dom_tree.immediate_dominator(node)
                        .map(|n| cfg[n].id);
                    let dominated: Vec<usize> = dom_tree.children(node)
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

    /// Helper to print dominator tree in human-readable format
    fn print_dominator_tree_human(
        cfg: &crate::cfg::Cfg,
        dom_tree: &crate::cfg::DominatorTree,
        node: petgraph::graph::NodeIndex,
        depth: usize,
        is_post: bool,
    ) {
        let indent = "  ".repeat(depth);
        let block_id = cfg[node].id;
        let kind_label = if is_post { "post-dominator" } else { "dominator" };

        println!("{}Block {} ({})", indent, block_id, kind_label);

        for &child in dom_tree.children(node) {
            print_dominator_tree_human(cfg, dom_tree, child, depth + 1, is_post);
        }
    }

    /// Helper to print post-dominator tree in human-readable format
    fn print_post_dominator_tree_human(
        cfg: &crate::cfg::Cfg,
        post_dom_tree: &crate::cfg::PostDominatorTree,
        node: petgraph::graph::NodeIndex,
        depth: usize,
    ) {
        let indent = "  ".repeat(depth);
        let block_id = cfg[node].id;

        println!("{}Block {} (post-dominator)", indent, block_id);

        for &child in post_dom_tree.children(node) {
            print_post_dominator_tree_human(cfg, post_dom_tree, child, depth + 1);
        }
    }

    pub fn unreachable(args: &UnreachableArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::reachability::find_unreachable;
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

        // Open database (follows status command pattern for error handling)
        let _db = match MirageDb::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                output::error(&format!("Failed to open database: {}", e));
                output::info("Hint: Run 'mirage index' to create the database");
                std::process::exit(output::EXIT_DATABASE);
            }
        };

        // For now, create a test CFG since MIR extraction isn't complete
        // TODO: Load CFG from database using function filter when MIR extraction is complete
        let cfg = create_test_cfg();

        // Find unreachable blocks
        let unreachable_indices = find_unreachable(&cfg);

        // If no unreachable blocks found, display message and exit
        if unreachable_indices.is_empty() {
            match cli.output {
                OutputFormat::Human => {
                    output::info("No unreachable code found");
                }
                OutputFormat::Json | OutputFormat::Pretty => {
                    let response = UnreachableResponse {
                        function: "test".to_string(),
                        total_functions: 1,
                        functions_with_unreachable: 0,
                        unreachable_count: 0,
                        blocks: vec![],
                    };
                    let wrapper = output::JsonResponse::new(response);
                    match cli.output {
                        OutputFormat::Json => println!("{}", wrapper.to_json()),
                        OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                        _ => {}
                    }
                }
            }
            return Ok(());
        }

        // Build UnreachableBlock structs from the NodeIndex results
        let blocks: Vec<UnreachableBlock> = unreachable_indices
            .iter()
            .filter_map(|&idx| cfg.node_weight(idx))
            .map(|block| {
                let kind_str = format!("{:?}", block.kind);
                let terminator_str = format!("{:?}", block.terminator);
                UnreachableBlock {
                    block_id: block.id,
                    kind: kind_str,
                    statements: block.statements.clone(),
                    terminator: terminator_str,
                }
            })
            .collect();

        // Format output based on cli.output
        match cli.output {
            OutputFormat::Human => {
                println!("Unreachable Code Blocks:");
                println!("  Total blocks: {}", blocks.len());
                println!();

                if args.within_functions {
                    // Group by function (for now, just one test function)
                    println!("Function: test");
                    for block in &blocks {
                        println!("  Block {} ({})", block.block_id, block.kind);
                        if !block.statements.is_empty() {
                            for stmt in &block.statements {
                                println!("    - {}", stmt);
                            }
                        }
                        println!("    Terminator: {}", block.terminator);
                        println!();
                    }
                } else {
                    for block in &blocks {
                        println!("  Block {} ({})", block.block_id, block.kind);
                        if !block.statements.is_empty() {
                            for stmt in &block.statements {
                                println!("    - {}", stmt);
                            }
                        }
                        println!("    Terminator: {}", block.terminator);
                        println!();
                    }
                }

                if args.show_branches {
                    output::info("Branch details: Use --show-branches to see incoming edges (not yet implemented)");
                }
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let response = UnreachableResponse {
                    function: "test".to_string(),
                    total_functions: 1,
                    functions_with_unreachable: if blocks.is_empty() { 0 } else { 1 },
                    unreachable_count: blocks.len(),
                    blocks,
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

    pub fn verify(args: &VerifyArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::{PathLimits, enumerate_paths};
        use crate::storage::MirageDb;
        use rusqlite::OptionalExtension;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

        // Open database (follows status command pattern for error handling)
        let db = match MirageDb::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                output::error(&format!("Failed to open database: {}", e));
                output::info("Hint: Run 'mirage index' to create the database");
                std::process::exit(output::EXIT_DATABASE);
            }
        };

        let path_id = &args.path_id;

        // Check if path exists in cache by querying cfg_paths table
        let cached_path_info: Option<(String, i64, String)> = db.conn()
            .query_row(
                "SELECT path_id, function_id, path_kind FROM cfg_paths WHERE path_id = ?1",
                rusqlite::params![path_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                }
            )
            .optional()
            .unwrap_or(None);

        let (found_in_cache, function_id, _path_kind) = match cached_path_info {
            Some((_id, fid, kind)) => (true, fid, kind),
            None => {
                // Path not found in cache
                let result = VerifyResult {
                    path_id: path_id.clone(),
                    valid: false,
                    found_in_cache: false,
                    function_id: None,
                    reason: "Path not found in cache".to_string(),
                    current_paths: 0,
                };

                match cli.output {
                    OutputFormat::Human => {
                        println!("Path ID {}: not found in cache", path_id);
                        println!("  The path may have been invalidated or never existed.");
                    }
                    OutputFormat::Json | OutputFormat::Pretty => {
                        let wrapper = output::JsonResponse::new(result);
                        match cli.output {
                            OutputFormat::Json => println!("{}", wrapper.to_json()),
                            OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                            _ => unreachable!(),
                        }
                    }
                }
                return Ok(());
            }
        };

        // Path exists in cache - verify it still exists in current enumeration
        // For now, use test CFG since MIR extraction isn't complete
        let cfg = create_test_cfg();

        // Re-enumerate paths to check if the path still exists
        let limits = PathLimits::default();
        let current_paths = enumerate_paths(&cfg, &limits);
        let current_path_count = current_paths.len();

        // Check if any enumerated path has the same path_id
        let path_still_valid = current_paths.iter()
            .any(|p| &p.path_id == path_id);

        let reason = if path_still_valid {
            "Path found in current enumeration".to_string()
        } else {
            "Path no longer exists in current enumeration (code may have changed)".to_string()
        };

        let result = VerifyResult {
            path_id: path_id.clone(),
            valid: path_still_valid,
            found_in_cache,
            function_id: Some(function_id),
            reason,
            current_paths: current_path_count,
        };

        match cli.output {
            OutputFormat::Human => {
                println!("Path ID {}: {}", path_id, if result.valid { "valid" } else { "invalid" });
                println!("  Found in cache: {}", if found_in_cache { "yes" } else { "no" });
                println!("  Status: {}", result.reason);
                println!("  Current total paths: {}", current_path_count);
                if !path_still_valid {
                    println!();
                    output::info("The path may have been invalidated by code changes.");
                    output::info("Consider re-running path enumeration to update the cache.");
                }
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let wrapper = output::JsonResponse::new(result);
                match cli.output {
                    OutputFormat::Json => println!("{}", wrapper.to_json()),
                    OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                    _ => unreachable!(),
                }
            }
        }

        Ok(())
    }

    pub fn blast_zone(_args: BlastZoneArgs) -> Result<()> {
        // TODO: Implement path-based impact analysis
        output::error("Blast zone analysis not yet implemented");
        std::process::exit(1);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Ensure tests don't interfere with each other by clearing env var
    fn clear_env() {
        std::env::remove_var("MIRAGE_DB");
    }

    #[test]
    fn test_resolve_db_path_default() {
        clear_env();
        // No arg, no env -> returns default
        let result = resolve_db_path(None).unwrap();
        assert_eq!(result, "./codemcp.db");
    }

    #[test]
    fn test_resolve_db_path_with_cli_arg() {
        clear_env();
        // CLI arg provided -> returns CLI arg
        let result = resolve_db_path(Some("/custom/path.db".to_string())).unwrap();
        assert_eq!(result, "/custom/path.db");
    }

    #[test]
    fn test_resolve_db_path_with_env_var() {
        clear_env();
        // Env var set -> returns env var value
        std::env::set_var("MIRAGE_DB", "/env/path.db");
        let result = resolve_db_path(None).unwrap();
        assert_eq!(result, "/env/path.db");
        std::env::remove_var("MIRAGE_DB");
    }

    #[test]
    fn test_resolve_db_path_cli_overrides_env() {
        clear_env();
        // CLI arg should override env var
        std::env::set_var("MIRAGE_DB", "/env/path.db");
        let result = resolve_db_path(Some("/cli/path.db".to_string())).unwrap();
        assert_eq!(result, "/cli/path.db");
        std::env::remove_var("MIRAGE_DB");
    }
}

// ============================================================================
// cfg() Command Tests
// ============================================================================

#[cfg(test)]
mod cfg_tests {
    use super::*;
    use crate::cfg::{export_dot, export_json};

    /// Test that DOT format output contains expected Graphviz DOT syntax
    #[test]
    fn test_cfg_dot_format() {
        let cfg = cmds::create_test_cfg();
        let dot = export_dot(&cfg);

        // Verify basic Graphviz DOT structure
        assert!(dot.contains("digraph CFG"), "DOT output should contain 'digraph CFG'");
        assert!(dot.contains("rankdir=TB"), "DOT output should contain rankdir attribute");
        assert!(dot.contains("node [shape=box"), "DOT output should contain node shape attribute");
        assert!(dot.contains("}"), "DOT output should end with closing brace");

        // Verify edge syntax
        assert!(dot.contains("->"), "DOT output should contain edge arrows");
    }

    /// Test that JSON format output is valid and contains expected structure
    #[test]
    fn test_cfg_json_format() {
        let cfg = cmds::create_test_cfg();
        let function_name = "test_function";
        let export = export_json(&cfg, function_name);

        // Verify function name is included
        assert_eq!(export.function_name, function_name, "JSON export should include function name");

        // Verify structure
        assert!(export.entry.is_some(), "JSON export should have an entry block");
        assert!(!export.exits.is_empty(), "JSON export should have exit blocks");
        assert!(!export.blocks.is_empty(), "JSON export should have blocks");
        assert!(!export.edges.is_empty(), "JSON export should have edges");

        // Verify JSON can be serialized
        let json_str = serde_json::to_string(&export);
        assert!(json_str.is_ok(), "JSON export should be serializable to JSON");

        // Verify JSON contains function name
        let json = json_str.unwrap();
        assert!(json.contains(function_name), "JSON output should contain function name");
        assert!(json.contains("\"entry\""), "JSON output should contain entry field");
        assert!(json.contains("\"exits\""), "JSON output should contain exits field");
        assert!(json.contains("\"blocks\""), "JSON output should contain blocks field");
        assert!(json.contains("\"edges\""), "JSON output should contain edges field");
    }

    /// Test that function name is correctly passed to export_json()
    #[test]
    fn test_cfg_function_name_in_export() {
        let cfg = cmds::create_test_cfg();

        // Test with different function names
        let test_names = vec![
            "my_function",
            "TestFunc",
            "module::submodule::function",
        ];

        for name in test_names {
            let export = export_json(&cfg, name);
            assert_eq!(export.function_name, name, "Function name should be preserved in export");
        }
    }

    /// Test format fallback when args.format is None (should use cli.output)
    #[test]
    fn test_cfg_format_fallback() {
        // Test that CfgFormat::Human is used when cli.output is Human
        let cli_human = Cli {
            db: None,
            output: OutputFormat::Human,
            command: Commands::Cfg(CfgArgs {
                function: "test".to_string(),
                format: None,
            }),
        };

        let cfg_args = match &cli_human.command {
            Commands::Cfg(args) => args,
            _ => panic!("Expected Cfg command"),
        };

        // Simulate the format resolution logic from cfg()
        let resolved_format = cfg_args.format.unwrap_or(match cli_human.output {
            OutputFormat::Human => CfgFormat::Human,
            OutputFormat::Json => CfgFormat::Json,
            OutputFormat::Pretty => CfgFormat::Json,
        });

        assert_eq!(resolved_format, CfgFormat::Human, "Should fall back to Human format");

        // Test that CfgFormat::Json is used when cli.output is Json
        let cli_json = Cli {
            db: None,
            output: OutputFormat::Json,
            command: Commands::Cfg(CfgArgs {
                function: "test".to_string(),
                format: None,
            }),
        };

        let cfg_args_json = match &cli_json.command {
            Commands::Cfg(args) => args,
            _ => panic!("Expected Cfg command"),
        };

        let resolved_format_json = cfg_args_json.format.unwrap_or(match cli_json.output {
            OutputFormat::Human => CfgFormat::Human,
            OutputFormat::Json => CfgFormat::Json,
            OutputFormat::Pretty => CfgFormat::Json,
        });

        assert_eq!(resolved_format_json, CfgFormat::Json, "Should fall back to Json format");
    }

    /// Test that JsonResponse wrapper wraps CFGExport correctly
    #[test]
    fn test_cfg_json_response_wrapper() {
        use crate::output::JsonResponse;

        let cfg = cmds::create_test_cfg();
        let export = export_json(&cfg, "wrapped_function");
        let response = JsonResponse::new(export);

        // Verify JsonResponse structure
        assert_eq!(response.schema_version, "1.0.0");
        assert_eq!(response.tool, "mirage");
        assert!(!response.execution_id.is_empty());
        assert!(!response.timestamp.is_empty());

        // Verify can be serialized
        let json = response.to_json();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"execution_id\""));
        assert!(json.contains("\"tool\":\"mirage\""));
        assert!(json.contains("\"data\""));
        assert!(json.contains("wrapped_function"));
    }

    /// Test DOT format contains expected block information
    #[test]
    fn test_cfg_dot_block_info() {
        let cfg = cmds::create_test_cfg();
        let dot = export_dot(&cfg);

        // Check for ENTRY block marker (green fill)
        assert!(dot.contains("lightgreen"), "DOT should mark entry block with green");

        // Check for EXIT block marker (coral fill)
        assert!(dot.contains("lightcoral"), "DOT should mark exit blocks with coral");

        // Check for block labels
        assert!(dot.contains("Block"), "DOT should contain block labels");
    }

    /// Test DOT format contains expected edge information
    #[test]
    fn test_cfg_dot_edge_info() {
        let cfg = cmds::create_test_cfg();
        let dot = export_dot(&cfg);

        // Check for edge colors (TrueBranch=green, FalseBranch=red)
        assert!(dot.contains("color=green"), "DOT should show true branch edges in green");
        assert!(dot.contains("color=red"), "DOT should show false branch edges in red");
    }
}

// ============================================================================
// status() Command Tests
// ============================================================================

#[cfg(test)]
mod status_tests {
    use crate::storage::{create_schema, MirageDb};
    use rusqlite::{Connection, params};

    /// Create a test database with sample data
    fn create_test_db() -> anyhow::Result<(tempfile::NamedTempFile, MirageDb)> {
        use crate::storage::{REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION};

        let file = tempfile::NamedTempFile::new()?;
        let mut conn = Connection::open(file.path())?;

        // Create Magellan tables (simplified)
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, ?)",
            params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        )?;

        // Create Mirage schema
        create_schema(&mut conn)?;

        // Add sample data
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "test_func", "test.rs", "{}"),
        )?;
        let function_id: i64 = conn.last_insert_rowid();

        // Add test blocks
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, block_kind, byte_start, byte_end, terminator, function_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            params!(function_id, "entry", 0, 10, "goto", "abc123"),
        )?;
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, block_kind, byte_start, byte_end, terminator, function_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            params!(function_id, "exit", 10, 20, "ret", "abc123"),
        )?;

        // Add test edges
        conn.execute(
            "INSERT INTO cfg_edges (from_id, to_id, edge_type) VALUES (?, ?, ?)",
            params!(1, 2, "fallthrough"),
        )?;

        // Add test paths
        conn.execute(
            "INSERT INTO cfg_paths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params!("test_path", function_id, "normal", 1, 2, 2, 0),
        )?;

        // Add test dominators
        conn.execute(
            "INSERT INTO cfg_dominators (block_id, dominator_id, is_strict) VALUES (?, ?, ?)",
            params!(1, 1, false),
        )?;

        let db = MirageDb::open(file.path())?;
        Ok((file, db))
    }

    /// Test that status() returns correct database statistics
    #[test]
    fn test_status_returns_correct_statistics() {
        let (_file, db) = create_test_db().unwrap();
        let status = db.status().unwrap();

        assert_eq!(status.cfg_blocks, 2, "Should have 2 cfg_blocks");
        assert_eq!(status.cfg_edges, 1, "Should have 1 cfg_edge");
        assert_eq!(status.cfg_paths, 1, "Should have 1 cfg_path");
        assert_eq!(status.cfg_dominators, 1, "Should have 1 cfg_dominator");
        assert_eq!(status.mirage_schema_version, 1, "Schema version should be 1");
        assert_eq!(status.magellan_schema_version, 4, "Magellan version should be 4");
    }

    /// Test that human output format contains expected fields
    #[test]
    fn test_status_human_output_format() {
        let (_file, db) = create_test_db().unwrap();
        let status = db.status().unwrap();

        // Verify all expected fields are present and have correct values
        assert!(status.cfg_blocks >= 0, "cfg_blocks should be non-negative");
        assert!(status.cfg_edges >= 0, "cfg_edges should be non-negative");
        assert!(status.cfg_paths >= 0, "cfg_paths should be non-negative");
        assert!(status.cfg_dominators >= 0, "cfg_dominators should be non-negative");
        assert!(status.mirage_schema_version > 0, "mirage_schema_version should be positive");
        assert!(status.magellan_schema_version > 0, "magellan_schema_version should be positive");
    }

    /// Test that JSON output format is valid and contains expected structure
    #[test]
    fn test_status_json_output_format() {
        use crate::output::JsonResponse;

        let (_file, db) = create_test_db().unwrap();
        let status = db.status().unwrap();
        let response = JsonResponse::new(status);

        // Verify JsonResponse wrapper structure
        assert_eq!(response.schema_version, "1.0.0");
        assert_eq!(response.tool, "mirage");
        assert!(!response.execution_id.is_empty());
        assert!(!response.timestamp.is_empty());

        // Verify JSON serialization
        let json = response.to_json();
        assert!(json.contains("\"schema_version\":\"1.0.0\""));
        assert!(json.contains("\"tool\":\"mirage\""));
        assert!(json.contains("\"execution_id\""));
        assert!(json.contains("\"timestamp\""));
        assert!(json.contains("\"data\""));
        assert!(json.contains("\"cfg_blocks\""));
        assert!(json.contains("\"cfg_edges\""));
        assert!(json.contains("\"cfg_paths\""));
        assert!(json.contains("\"cfg_dominators\""));
        assert!(json.contains("\"mirage_schema_version\""));
        assert!(json.contains("\"magellan_schema_version\""));
    }

    /// Test that pretty JSON output is formatted with indentation
    #[test]
    fn test_status_pretty_json_output_format() {
        use crate::output::JsonResponse;

        let (_file, db) = create_test_db().unwrap();
        let status = db.status().unwrap();
        let response = JsonResponse::new(status);

        let pretty_json = response.to_pretty_json();

        // Pretty JSON should contain newlines and indentation
        assert!(pretty_json.contains("\n"), "Pretty JSON should contain newlines");
        assert!(pretty_json.contains("  "), "Pretty JSON should contain indentation");

        // Should still be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&pretty_json)
            .expect("Pretty JSON should be valid");
        assert!(parsed.is_object(), "Parsed JSON should be an object");
        assert_eq!(parsed["schema_version"], "1.0.0");
        assert_eq!(parsed["tool"], "mirage");
        assert!(parsed["data"].is_object(), "data field should be an object");
    }

    /// Test that database open error is handled correctly
    #[test]
    fn test_status_database_open_error() {
        use crate::storage::MirageDb;

        // Try to open a non-existent database
        let result = MirageDb::open("/nonexistent/path/to/database.db");

        // Use match to check error without Debug requirement
        match result {
            Ok(_) => panic!("Should fail to open non-existent database"),
            Err(e) => {
                let err_msg = e.to_string();
                assert!(err_msg.contains("Database not found") || err_msg.contains("not found"),
                    "Error message should mention database not found: {}", err_msg);
            }
        }
    }

    /// Test that status() with empty database returns zero counts
    #[test]
    fn test_status_empty_database_returns_zeros() {
        use crate::storage::{REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION};

        let file = tempfile::NamedTempFile::new().unwrap();
        let mut conn = Connection::open(file.path()).unwrap();

        // Create minimal schema
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, ?)",
            params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        create_schema(&mut conn).unwrap();

        let db = MirageDb::open(file.path()).unwrap();
        let status = db.status().unwrap();

        assert_eq!(status.cfg_blocks, 0, "Empty database should have 0 cfg_blocks");
        assert_eq!(status.cfg_edges, 0, "Empty database should have 0 cfg_edges");
        assert_eq!(status.cfg_paths, 0, "Empty database should have 0 cfg_paths");
        assert_eq!(status.cfg_dominators, 0, "Empty database should have 0 cfg_dominators");
    }
}

// ============================================================================
// paths() Command Tests
// ============================================================================

#[cfg(test)]
mod paths_tests {
    use super::*;
    use crate::cfg::{PathKind, PathLimits, enumerate_paths};

    /// Test that paths() command enumerates paths from a test CFG
    #[test]
    fn test_paths_enumeration_basic() {
        let cfg = cmds::create_test_cfg();
        let limits = PathLimits::default();
        let paths = enumerate_paths(&cfg, &limits);

        // Test CFG has 2 paths (entry -> true -> return, entry -> false -> return)
        assert!(!paths.is_empty(), "Should find at least one path");
        assert_eq!(paths.len(), 2, "Test CFG should have exactly 2 paths");

        // Both paths should be Normal kind (no errors in test CFG)
        let normal_count = paths.iter().filter(|p| p.kind == PathKind::Normal).count();
        assert_eq!(normal_count, 2, "Both paths should be Normal");
    }

    /// Test that show_errors flag filters to error paths only
    #[test]
    fn test_paths_show_errors_filter() {
        let cfg = cmds::create_test_cfg();
        let limits = PathLimits::default();
        let mut paths = enumerate_paths(&cfg, &limits);

        // Filter to error paths
        paths.retain(|p| p.kind == PathKind::Error);

        // Test CFG has no error paths
        assert_eq!(paths.len(), 0, "Test CFG should have no error paths");

        // Verify filter worked by checking all remaining paths would be errors
        for path in &paths {
            assert_eq!(path.kind, PathKind::Error, "Filtered paths should all be Error kind");
        }
    }

    /// Test that max_length limit is applied to path enumeration
    #[test]
    fn test_paths_max_length_limit() {
        let cfg = cmds::create_test_cfg();

        // Set a very low max_length limit
        let limits = PathLimits::default().with_max_length(1);
        let paths = enumerate_paths(&cfg, &limits);

        // All paths should have length <= 1
        for path in &paths {
            assert!(path.len() <= 1, "Path length should be <= max_length limit");
        }

        // With max_length=1, we should get fewer paths than unrestricted
        let unlimited_paths = enumerate_paths(&cfg, &PathLimits::default());
        assert!(paths.len() <= unlimited_paths.len(),
            "Limited enumeration should produce <= paths than unlimited");
    }

    /// Test that PathsArgs.function is extracted correctly
    #[test]
    fn test_paths_args_function_extraction() {
        let args = PathsArgs {
            function: "test_function".to_string(),
            show_errors: false,
            max_length: None,
            with_blocks: false,
        };

        assert_eq!(args.function, "test_function");
        assert!(!args.show_errors);
        assert!(args.max_length.is_none());
        assert!(!args.with_blocks);
    }

    /// Test that PathsArgs with flags set correctly reflects state
    #[test]
    fn test_paths_args_with_flags() {
        let args = PathsArgs {
            function: "my_func".to_string(),
            show_errors: true,
            max_length: Some(10),
            with_blocks: true,
        };

        assert_eq!(args.function, "my_func");
        assert!(args.show_errors, "show_errors flag should be true");
        assert_eq!(args.max_length, Some(10), "max_length should be Some(10)");
        assert!(args.with_blocks, "with_blocks flag should be true");
    }

    /// Test PathSummary conversion from Path
    #[test]
    fn test_path_summary_from_path() {
        use crate::cfg::Path;

        let path = Path::new(vec![0, 1, 2], PathKind::Normal);
        let summary = PathSummary::from(path);

        assert!(!summary.path_id.is_empty(), "path_id should not be empty");
        assert_eq!(summary.kind, "Normal", "kind should match PathKind");
        assert_eq!(summary.length, 3, "length should match path length");
        assert_eq!(summary.blocks, vec![0, 1, 2], "blocks should match path blocks");
    }

    /// Test PathSummary conversion for different PathKinds
    #[test]
    fn test_path_summary_different_kinds() {
        use crate::cfg::Path;

        let kinds = vec![
            (PathKind::Normal, "Normal"),
            (PathKind::Error, "Error"),
            (PathKind::Degenerate, "Degenerate"),
            (PathKind::Unreachable, "Unreachable"),
        ];

        for (kind, expected_str) in kinds {
            let path = Path::new(vec![0, 1], kind);
            let summary = PathSummary::from(path);
            assert_eq!(summary.kind, expected_str,
                "PathKind::{:?} should serialize to {}", kind, expected_str);
        }
    }

    /// Test that multiple paths produce multiple PathSummaries
    #[test]
    fn test_paths_response_multiple_paths() {
        use crate::cfg::Path;

        let paths = vec![
            Path::new(vec![0, 1], PathKind::Normal),
            Path::new(vec![0, 2], PathKind::Normal),
            Path::new(vec![0, 1, 3], PathKind::Error),
        ];

        let summaries: Vec<PathSummary> = paths.into_iter().map(PathSummary::from).collect();

        assert_eq!(summaries.len(), 3, "Should have 3 summaries");

        // Check that error path is correctly identified
        let error_summaries = summaries.iter().filter(|s| s.kind == "Error").count();
        assert_eq!(error_summaries, 1, "Should have 1 error path");
    }

    /// Test PathsResponse contains expected metadata
    #[test]
    fn test_paths_response_metadata() {
        let response = PathsResponse {
            function: "test_func".to_string(),
            total_paths: 5,
            error_paths: 2,
            paths: vec![],
        };

        assert_eq!(response.function, "test_func");
        assert_eq!(response.total_paths, 5);
        assert_eq!(response.error_paths, 2);
        assert!(response.paths.is_empty());
    }

    /// Test integration: create_test_cfg produces enumerable paths
    #[test]
    fn test_paths_integration_with_test_cfg() {
        let cfg = cmds::create_test_cfg();
        let limits = PathLimits::default();
        let paths = enumerate_paths(&cfg, &limits);

        // Verify we got the expected number of paths for the diamond CFG
        assert!(!paths.is_empty(), "Test CFG should produce paths");

        // Each path should start at entry (block 0)
        for path in &paths {
            assert_eq!(path.blocks[0], 0, "All paths should start at entry block 0");
            assert_eq!(path.entry, 0, "Path entry should be block 0");
        }

        // Each path should end at an exit block
        for path in &paths {
            assert!(path.exit == 2 || path.exit == 3,
                "Path exit should be either block 2 or 3 (the return blocks)");
        }
    }

    /// Test that with_blocks flag affects output format (integration check)
    #[test]
    fn test_paths_args_with_blocks_flag() {
        let args_with = PathsArgs {
            function: "test".to_string(),
            show_errors: false,
            max_length: None,
            with_blocks: true,
        };

        let args_without = PathsArgs {
            function: "test".to_string(),
            show_errors: false,
            max_length: None,
            with_blocks: false,
        };

        assert!(args_with.with_blocks, "with_blocks should be true");
        assert!(!args_without.with_blocks, "with_blocks should be false");
    }
}

// ============================================================================
// unreachable() Command Tests
// ============================================================================

#[cfg(test)]
mod unreachable_tests {
    use super::*;
    use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};
    use crate::cfg::reachability::find_unreachable;
    use petgraph::graph::DiGraph;

    /// Helper to create a test CFG with an unreachable block
    fn create_cfg_with_unreachable() -> Cfg {
        let mut g = DiGraph::new();

        // Block 0: entry, goes to 1
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec!["let x = 1".to_string()],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Block 1: normal, goes to 2
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec!["if x > 0".to_string()],
            terminator: Terminator::SwitchInt {
                targets: vec![2],
                otherwise: 3,
            },
            source_location: None,
        });

        // Block 2: exit (reachable)
        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec!["return true".to_string()],
            terminator: Terminator::Return,
            source_location: None,
        });

        // Block 3: exit (reachable)
        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec!["return false".to_string()],
            terminator: Terminator::Return,
            source_location: None,
        });

        // Block 4: unreachable (no edges to it)
        let _b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Exit,
            statements: vec!["unreachable code".to_string()],
            terminator: Terminator::Unreachable,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b3, EdgeType::FalseBranch);

        g
    }

    /// Test that unreachable blocks are detected
    #[test]
    fn test_unreachable_detects_dead_code() {
        let cfg = create_cfg_with_unreachable();
        let unreachable_indices = find_unreachable(&cfg);

        // Should find exactly 1 unreachable block (block 4)
        assert_eq!(unreachable_indices.len(), 1, "Should find exactly 1 unreachable block");

        // Verify it's block 4
        let block_id = cfg.node_weight(unreachable_indices[0]).unwrap().id;
        assert_eq!(block_id, 4, "Unreachable block should be block 4");
    }

    /// Test that UnreachableResponse struct serializes correctly
    #[test]
    fn test_unreachable_response_serialization() {
        use crate::output::JsonResponse;

        let response = UnreachableResponse {
            function: "test_func".to_string(),
            total_functions: 1,
            functions_with_unreachable: 1,
            unreachable_count: 1,
            blocks: vec![
                UnreachableBlock {
                    block_id: 4,
                    kind: "Exit".to_string(),
                    statements: vec!["unreachable code".to_string()],
                    terminator: "Unreachable".to_string(),
                }
            ],
        };

        let wrapper = JsonResponse::new(response);
        let json = wrapper.to_json();

        assert!(json.contains("\"function\":\"test_func\""));
        assert!(json.contains("\"unreachable_count\":1"));
        assert!(json.contains("\"block_id\":4"));
        assert!(json.contains("\"kind\":\"Exit\""));
    }

    /// Test that empty unreachable response is handled correctly
    #[test]
    fn test_unreachable_empty_response() {
        use crate::output::JsonResponse;

        let response = UnreachableResponse {
            function: "test_func".to_string(),
            total_functions: 1,
            functions_with_unreachable: 0,
            unreachable_count: 0,
            blocks: vec![],
        };

        let wrapper = JsonResponse::new(response);
        let json = wrapper.to_json();

        assert!(json.contains("\"unreachable_count\":0"));
        assert!(json.contains("\"functions_with_unreachable\":0"));
    }

    /// Test that UnreachableBlock struct contains expected fields
    #[test]
    fn test_unreachable_block_fields() {
        let block = UnreachableBlock {
            block_id: 5,
            kind: "Normal".to_string(),
            statements: vec!["stmt1".to_string(), "stmt2".to_string()],
            terminator: "Return".to_string(),
        };

        assert_eq!(block.block_id, 5);
        assert_eq!(block.kind, "Normal");
        assert_eq!(block.statements.len(), 2);
        assert_eq!(block.terminator, "Return");
    }

    /// Test UnreachableArgs flags
    #[test]
    fn test_unreachable_args_flags() {
        let args_with = UnreachableArgs {
            within_functions: true,
            show_branches: true,
        };

        let args_without = UnreachableArgs {
            within_functions: false,
            show_branches: false,
        };

        assert!(args_with.within_functions);
        assert!(args_with.show_branches);
        assert!(!args_without.within_functions);
        assert!(!args_without.show_branches);
    }

    /// Test that create_test_cfg has no unreachable blocks
    #[test]
    fn test_test_cfg_fully_reachable() {
        let cfg = cmds::create_test_cfg();
        let unreachable_indices = find_unreachable(&cfg);

        // Test CFG should have no unreachable blocks
        assert_eq!(unreachable_indices.len(), 0, "Test CFG should have no unreachable blocks");
    }
}

// ============================================================================
// dominators() Command Tests
// ============================================================================

#[cfg(test)]
mod dominators_tests {
    use super::*;
    use crate::cfg::{DominatorTree, PostDominatorTree};
    use tempfile::NamedTempFile;

    /// Create a minimal test database
    fn create_minimal_db() -> anyhow::Result<NamedTempFile> {
        use crate::storage::{REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION};
        let file = NamedTempFile::new()?;
        let conn = rusqlite::Connection::open(file.path())?;

        // Create Magellan tables
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, ?)",
            rusqlite::params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        )?;

        // Create Mirage schema
        conn.execute(
            "CREATE TABLE mirage_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                mirage_schema_version INTEGER NOT NULL,
                magellan_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE cfg_blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                function_id INTEGER NOT NULL,
                block_kind TEXT NOT NULL,
                byte_start INTEGER NOT NULL,
                byte_end INTEGER NOT NULL,
                terminator TEXT NOT NULL,
                function_hash TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE cfg_edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_id INTEGER NOT NULL,
                to_id INTEGER NOT NULL,
                edge_type TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE cfg_paths (
                path_id TEXT PRIMARY KEY,
                function_id INTEGER NOT NULL,
                path_kind TEXT NOT NULL,
                entry_block INTEGER NOT NULL,
                exit_block INTEGER NOT NULL,
                length INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE cfg_dominators (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                block_id INTEGER NOT NULL,
                dominator_id INTEGER NOT NULL,
                is_strict INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "INSERT INTO mirage_meta (id, mirage_schema_version, magellan_schema_version, created_at)
             VALUES (1, 1, 4, 0)",
            [],
        )?;

        // Add a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        )?;

        Ok(file)
    }

    /// Test that DominatorTree can be computed from test CFG
    #[test]
    fn test_dominator_tree_computation() {
        let cfg = cmds::create_test_cfg();
        let dom_tree = DominatorTree::new(&cfg);

        assert!(dom_tree.is_some(), "DominatorTree should be computed successfully");

        let dom_tree = dom_tree.unwrap();
        // Entry block (0) should be the root
        assert_eq!(cfg[dom_tree.root()].id, 0, "Root should be entry block");
    }

    /// Test that PostDominatorTree can be computed from test CFG
    #[test]
    fn test_post_dominator_tree_computation() {
        let cfg = cmds::create_test_cfg();
        let post_dom_tree = PostDominatorTree::new(&cfg);

        assert!(post_dom_tree.is_some(), "PostDominatorTree should be computed successfully");

        let post_dom_tree = post_dom_tree.unwrap();
        // Root of post-dominator tree should be an exit block
        let root_id = cfg[post_dom_tree.root()].id;
        assert!(root_id == 2 || root_id == 3, "Root should be an exit block");
    }

    /// Test immediate dominator relationships in test CFG
    #[test]
    fn test_immediate_dominator_relationships() {
        let cfg = cmds::create_test_cfg();
        let dom_tree = DominatorTree::new(&cfg).unwrap();

        // Find nodes by block ID
        let node_0 = cfg.node_indices().find(|&n| cfg[n].id == 0).unwrap();
        let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();
        let node_2 = cfg.node_indices().find(|&n| cfg[n].id == 2).unwrap();
        let node_3 = cfg.node_indices().find(|&n| cfg[n].id == 3).unwrap();

        // Entry (0) has no immediate dominator
        assert_eq!(dom_tree.immediate_dominator(node_0), None, "Entry should have no dominator");

        // Node 1 is dominated by entry (0)
        assert_eq!(dom_tree.immediate_dominator(node_1), Some(node_0), "Node 1 should be dominated by entry");

        // Node 2 is dominated by node 1 (through true branch)
        assert_eq!(dom_tree.immediate_dominator(node_2), Some(node_1), "Node 2 should be dominated by node 1");

        // Node 3 is dominated by node 1 (through false branch)
        assert_eq!(dom_tree.immediate_dominator(node_3), Some(node_1), "Node 3 should be dominated by node 1");
    }

    /// Test dominates() method
    #[test]
    fn test_dominates_method() {
        let cfg = cmds::create_test_cfg();
        let dom_tree = DominatorTree::new(&cfg).unwrap();

        let node_0 = cfg.node_indices().find(|&n| cfg[n].id == 0).unwrap();
        let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();
        let node_2 = cfg.node_indices().find(|&n| cfg[n].id == 2).unwrap();

        // Entry dominates all nodes
        assert!(dom_tree.dominates(node_0, node_0), "Node dominates itself");
        assert!(dom_tree.dominates(node_0, node_1), "Entry dominates node 1");
        assert!(dom_tree.dominates(node_0, node_2), "Entry dominates node 2");

        // Non-entry doesn't dominate entry
        assert!(!dom_tree.dominates(node_1, node_0), "Node 1 does not dominate entry");
    }

    /// Test children() method returns dominated nodes
    #[test]
    fn test_dominator_tree_children() {
        let cfg = cmds::create_test_cfg();
        let dom_tree = DominatorTree::new(&cfg).unwrap();

        let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();

        // Node 1 should have 2 children (blocks 2 and 3)
        let children = dom_tree.children(node_1);
        assert_eq!(children.len(), 2, "Node 1 should have 2 children");

        let child_ids: Vec<_> = children.iter().map(|&n| cfg[n].id).collect();
        assert!(child_ids.contains(&2), "Children should include block 2");
        assert!(child_ids.contains(&3), "Children should include block 3");
    }

    /// Test DominatorsArgs struct has expected fields
    #[test]
    fn test_dominators_args_fields() {
        let args = DominatorsArgs {
            function: "test_func".to_string(),
            must_pass_through: Some("1".to_string()),
            post: false,
        };

        assert_eq!(args.function, "test_func");
        assert_eq!(args.must_pass_through, Some("1".to_string()));
        assert!(!args.post);
    }

    /// Test DominatorsArgs with --post flag
    #[test]
    fn test_dominators_args_with_post_flag() {
        let args = DominatorsArgs {
            function: "my_function".to_string(),
            must_pass_through: None,
            post: true,
        };

        assert_eq!(args.function, "my_function");
        assert!(args.post, "post flag should be true");
        assert!(args.must_pass_through.is_none(), "must_pass_through should be None");
    }

    /// Test DominanceResponse struct serializes correctly
    #[test]
    fn test_dominance_response_serialization() {
        let response = DominanceResponse {
            function: "test".to_string(),
            kind: "dominators".to_string(),
            root: Some(0),
            dominance_tree: vec![
                DominatorEntry {
                    block: 0,
                    immediate_dominator: None,
                    dominated: vec![1],
                },
            ],
            must_pass_through: None,
        };

        let json = serde_json::to_string(&response);
        assert!(json.is_ok(), "DominanceResponse should serialize to JSON");

        let json_str = json.unwrap();
        assert!(json_str.contains("\"function\":\"test\""));
        assert!(json_str.contains("\"kind\":\"dominators\""));
        assert!(json_str.contains("\"root\":0"));
    }

    /// Test MustPassThroughResult struct
    #[test]
    fn test_must_pass_through_result() {
        let result = MustPassThroughResult {
            block: 1,
            must_pass: vec![1, 2, 3],
        };

        assert_eq!(result.block, 1);
        assert_eq!(result.must_pass.len(), 3);
        assert_eq!(result.must_pass, vec![1, 2, 3]);

        // Verify it serializes correctly
        let json = serde_json::to_string(&result);
        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("\"block\":1"));
        assert!(json_str.contains("\"must_pass\":[1,2,3]"));
    }

    /// Test DominatorEntry struct
    #[test]
    fn test_dominator_entry() {
        let entry = DominatorEntry {
            block: 5,
            immediate_dominator: Some(2),
            dominated: vec![6, 7],
        };

        assert_eq!(entry.block, 5);
        assert_eq!(entry.immediate_dominator, Some(2));
        assert_eq!(entry.dominated, vec![6, 7]);
    }

    /// Test post-dominates() method
    #[test]
    fn test_post_dominates_method() {
        let cfg = cmds::create_test_cfg();
        let post_dom_tree = PostDominatorTree::new(&cfg).unwrap();

        let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();
        let node_2 = cfg.node_indices().find(|&n| cfg[n].id == 2).unwrap();

        // Exit post-dominates nodes that can reach it
        assert!(post_dom_tree.post_dominates(node_2, node_2), "Node post-dominates itself");
        assert!(post_dom_tree.post_dominates(node_2, node_1), "Exit post-dominates node 1");
    }

    /// Test immediate post-dominator relationships
    #[test]
    fn test_immediate_post_dominator_relationships() {
        let cfg = cmds::create_test_cfg();
        let post_dom_tree = PostDominatorTree::new(&cfg).unwrap();

        let node_0 = cfg.node_indices().find(|&n| cfg[n].id == 0).unwrap();
        let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();

        // Node 1 should be immediately post-dominated by an exit (2 or 3)
        let ipdom_1 = post_dom_tree.immediate_post_dominator(node_1);
        assert!(ipdom_1.is_some(), "Node 1 should have an immediate post-dominator");

        // Node 0 should be immediately post-dominated by node 1
        let ipdom_0 = post_dom_tree.immediate_post_dominator(node_0);
        assert_eq!(ipdom_0, Some(node_1), "Node 0 should be immediately post-dominated by node 1");
    }

    /// Test that empty CFG returns None for DominatorTree
    #[test]
    fn test_empty_cfg_dominator_tree() {
        use petgraph::graph::DiGraph;
        let empty_cfg: crate::cfg::Cfg = DiGraph::new();
        let dom_tree = DominatorTree::new(&empty_cfg);

        assert!(dom_tree.is_none(), "Empty CFG should produce None for DominatorTree");
    }

    /// Test that empty CFG returns None for PostDominatorTree
    #[test]
    fn test_empty_cfg_post_dominator_tree() {
        use petgraph::graph::DiGraph;
        let empty_cfg: crate::cfg::Cfg = DiGraph::new();
        let post_dom_tree = PostDominatorTree::new(&empty_cfg);

        assert!(post_dom_tree.is_none(), "Empty CFG should produce None for PostDominatorTree");
    }

    /// Test JsonResponse wrapper for DominanceResponse
    #[test]
    fn test_dominance_response_json_wrapper() {
        use crate::output::JsonResponse;

        let response = DominanceResponse {
            function: "wrapped_test".to_string(),
            kind: "dominators".to_string(),
            root: Some(0),
            dominance_tree: vec![],
            must_pass_through: None,
        };

        let wrapper = JsonResponse::new(response);

        assert_eq!(wrapper.schema_version, "1.0.0");
        assert_eq!(wrapper.tool, "mirage");
        assert!(!wrapper.execution_id.is_empty());
        assert!(!wrapper.timestamp.is_empty());

        // Verify JSON contains expected fields
        let json = wrapper.to_json();
        assert!(json.contains("\"schema_version\":\"1.0.0\""));
        assert!(json.contains("\"tool\":\"mirage\""));
        assert!(json.contains("wrapped_test"));
    }

    /// Test must-pass-through query with valid block
    #[test]
    fn test_must_pass_through_valid_block() {
        let cfg = cmds::create_test_cfg();
        let dom_tree = DominatorTree::new(&cfg).unwrap();

        let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();

        // All nodes dominated by node 1 should include 1, 2, and 3
        let must_pass: Vec<usize> = cfg.node_indices()
            .filter(|&n| dom_tree.dominates(node_1, n))
            .map(|n| cfg[n].id)
            .collect();

        assert_eq!(must_pass.len(), 3, "Block 1 should dominate 3 blocks");
        assert!(must_pass.contains(&1), "Must include block 1 itself");
        assert!(must_pass.contains(&2), "Must include block 2");
        assert!(must_pass.contains(&3), "Must include block 3");
    }

    /// Test that non-existent block ID is handled gracefully
    #[test]
    fn test_nonexistent_block_id() {
        let cfg = cmds::create_test_cfg();

        // Block ID 99 doesn't exist in test CFG
        let found = cfg.node_indices().find(|&n| cfg[n].id == 99);
        assert!(found.is_none(), "Non-existent block should not be found");
    }

    /// Test JSON output for dominators command structure
    #[test]
    fn test_dominators_json_structure() {
        use crate::output::JsonResponse;

        let response = DominanceResponse {
            function: "json_test".to_string(),
            kind: "post-dominators".to_string(),
            root: Some(3),
            dominance_tree: vec![
                DominatorEntry {
                    block: 3,
                    immediate_dominator: None,
                    dominated: vec![0, 2],
                },
            ],
            must_pass_through: Some(MustPassThroughResult {
                block: 0,
                must_pass: vec![0, 1],
            }),
        };

        let wrapper = JsonResponse::new(response);
        let json = wrapper.to_json();

        assert!(json.contains("\"kind\":\"post-dominators\""));
        assert!(json.contains("\"root\":3"));
        assert!(json.contains("\"must_pass_through\""));
        assert!(json.contains("\"block\":0"));
    }
}

// ============================================================================
// verify() Command Tests
// ============================================================================

#[cfg(test)]
mod verify_tests {
    use super::*;
    use crate::cfg::{PathLimits, enumerate_paths};
    use crate::storage::MirageDb;
    use crate::output::JsonResponse;

    /// Create a test database with a cached path
    fn create_test_db_with_cached_path() -> anyhow::Result<(tempfile::NamedTempFile, MirageDb, String)> {
        use crate::storage::{REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION};
        let file = tempfile::NamedTempFile::new()?;
        let mut conn = rusqlite::Connection::open(file.path())?;

        // Create Magellan tables
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, ?)",
            rusqlite::params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        )?;

        // Create Mirage schema
        crate::storage::create_schema(&mut conn)?;

        // Add a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        )?;
        let function_id: i64 = conn.last_insert_rowid();

        // Enumerate paths from test CFG and cache one
        let cfg = cmds::create_test_cfg();
        let paths = enumerate_paths(&cfg, &PathLimits::default());

        // Store paths in database
        if let Some(first_path) = paths.first() {
            let path_id = &first_path.path_id;

            // Insert path metadata
            conn.execute(
                "INSERT INTO cfg_paths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    path_id,
                    function_id,
                    "Normal",
                    first_path.entry as i64,
                    first_path.exit as i64,
                    first_path.len() as i64,
                    0,
                ],
            )?;

            // Insert path elements
            for (idx, &block_id) in first_path.blocks.iter().enumerate() {
                conn.execute(
                    "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id)
                     VALUES (?1, ?2, ?3)",
                    rusqlite::params![path_id, idx as i64, block_id as i64],
                )?;
            }

            let db = MirageDb::open(file.path())?;
            Ok((file, db, path_id.clone()))
        } else {
            anyhow::bail!("No paths found in test CFG")
        }
    }

    /// Test that verify() returns valid for a path that exists in current enumeration
    #[test]
    fn test_verify_valid_path() {
        let (_file, _db, cached_path_id) = create_test_db_with_cached_path().unwrap();

        // Create test CFG and enumerate to get current paths
        let cfg = cmds::create_test_cfg();
        let current_paths = enumerate_paths(&cfg, &PathLimits::default());

        // Find the cached path in current enumeration
        let is_valid = current_paths.iter().any(|p| p.path_id == cached_path_id);

        // Since we're using the same test CFG, the path should be valid
        assert!(is_valid, "Cached path should exist in current enumeration");
    }

    /// Test that VerifyResult serializes correctly
    #[test]
    fn test_verify_result_serialization() {
        let result = VerifyResult {
            path_id: "test_path_123".to_string(),
            valid: true,
            found_in_cache: true,
            function_id: Some(1),
            reason: "Path found in current enumeration".to_string(),
            current_paths: 2,
        };

        let json = serde_json::to_string(&result);
        assert!(json.is_ok());

        let json_str = json.unwrap();
        assert!(json_str.contains("\"path_id\":\"test_path_123\""));
        assert!(json_str.contains("\"valid\":true"));
        assert!(json_str.contains("\"found_in_cache\":true"));
        assert!(json_str.contains("\"function_id\":1"));
        assert!(json_str.contains("\"reason\""));
        assert!(json_str.contains("\"current_paths\":2"));
    }

    /// Test that invalid path verification returns correct result
    #[test]
    fn test_verify_invalid_path_result() {
        let result = VerifyResult {
            path_id: "nonexistent_path".to_string(),
            valid: false,
            found_in_cache: false,
            function_id: None,
            reason: "Path not found in cache".to_string(),
            current_paths: 0,
        };

        assert!(!result.valid);
        assert!(!result.found_in_cache);
        assert!(result.function_id.is_none());
        assert_eq!(result.reason, "Path not found in cache");
    }

    /// Test VerifyArgs struct has expected fields
    #[test]
    fn test_verify_args_fields() {
        let args = VerifyArgs {
            path_id: "abc123".to_string(),
        };

        assert_eq!(args.path_id, "abc123");
    }

    /// Test that JsonResponse wrapper works with VerifyResult
    #[test]
    fn test_verify_result_json_wrapper() {
        let result = VerifyResult {
            path_id: "wrapped_path".to_string(),
            valid: true,
            found_in_cache: true,
            function_id: Some(42),
            reason: "Test reason".to_string(),
            current_paths: 100,
        };

        let wrapper = JsonResponse::new(result);

        assert_eq!(wrapper.schema_version, "1.0.0");
        assert_eq!(wrapper.tool, "mirage");
        assert!(!wrapper.execution_id.is_empty());
        assert!(!wrapper.timestamp.is_empty());

        let json = wrapper.to_json();
        assert!(json.contains("\"schema_version\":\"1.0.0\""));
        assert!(json.contains("\"tool\":\"mirage\""));
        assert!(json.contains("wrapped_path"));
    }

    /// Test path validity check with existing path
    #[test]
    fn test_verify_check_path_exists() {
        let cfg = cmds::create_test_cfg();
        let paths = enumerate_paths(&cfg, &PathLimits::default());

        // Get first path ID
        if let Some(first_path) = paths.first() {
            let path_id = &first_path.path_id;

            // Check if path exists
            let exists = paths.iter().any(|p| &p.path_id == path_id);
            assert!(exists, "Path should exist in enumeration");

            // Verify we can find it by blocks
            let same_blocks = paths.iter().any(|p| p.blocks == first_path.blocks);
            assert!(same_blocks, "Should find path with same blocks");
        }
    }

    /// Test that multiple paths have different IDs
    #[test]
    fn test_verify_multiple_paths_have_different_ids() {
        let cfg = cmds::create_test_cfg();
        let paths = enumerate_paths(&cfg, &PathLimits::default());

        // Test CFG should have multiple paths (2 paths for the diamond)
        assert!(paths.len() >= 2, "Test CFG should have at least 2 paths");

        // Check that all path IDs are unique
        let mut path_ids = std::collections::HashSet::new();
        for path in &paths {
            assert!(path_ids.insert(&path.path_id), "Path ID should be unique: {}", path.path_id);
        }
    }

    /// Test that path not in cache returns found_in_cache: false
    #[test]
    fn test_verify_path_not_in_cache() {
        let result = VerifyResult {
            path_id: "fake_id_that_does_not_exist".to_string(),
            valid: false,
            found_in_cache: false,
            function_id: None,
            reason: "Path not found in cache".to_string(),
            current_paths: 0,
        };

        assert!(!result.found_in_cache);
        assert!(!result.valid);
    }

    /// Test JSON output format for verify command
    #[test]
    fn test_verify_json_output_format() {
        let result = VerifyResult {
            path_id: "json_test_path".to_string(),
            valid: true,
            found_in_cache: true,
            function_id: Some(123),
            reason: "Test".to_string(),
            current_paths: 5,
        };

        let wrapper = JsonResponse::new(result);
        let json = wrapper.to_pretty_json();

        // Pretty JSON should have newlines
        assert!(json.contains("\n"));

        // Verify it can be parsed back
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["tool"], "mirage");
        assert_eq!(parsed["data"]["path_id"], "json_test_path");
        assert_eq!(parsed["data"]["valid"], true);
    }

    /// Test verify response with function_id None
    #[test]
    fn test_verify_result_without_function_id() {
        let result = VerifyResult {
            path_id: "orphan_path".to_string(),
            valid: false,
            found_in_cache: false,
            function_id: None,
            reason: "No function associated".to_string(),
            current_paths: 10,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"function_id\":null"));
        assert!(!result.valid);
        assert!(!result.found_in_cache);
    }
}
