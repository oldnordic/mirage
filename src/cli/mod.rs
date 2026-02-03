// CLI command definitions following Magellan's CLI patterns

use clap::{Parser, Subcommand, ValueEnum};

// Re-export for CLI use
pub use crate::analysis::DeadSymbolJson;

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

    /// Show natural loops in CFG
    Loops(LoopsArgs),

    /// Find unreachable code within functions
    Unreachable(UnreachableArgs),

    /// Show branching patterns (if/else, match) in CFG
    Patterns(PatternsArgs),

    /// Show dominance frontiers in CFG
    Frontiers(FrontiersArgs),

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
pub struct LoopsArgs {
    /// Function to analyze for loops
    #[arg(long)]
    pub function: String,

    /// Show detailed loop body blocks
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct UnreachableArgs {
    /// Find unreachable code within functions
    #[arg(long)]
    pub within_functions: bool,

    /// Show branch details
    #[arg(long)]
    pub show_branches: bool,

    /// Include uncalled functions (requires Magellan call graph)
    #[arg(long)]
    pub include_uncalled: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct PatternsArgs {
    /// Function to analyze for branching patterns
    #[arg(long)]
    pub function: String,

    /// Show only if/else patterns
    #[arg(long)]
    pub if_else: bool,

    /// Show only match patterns
    #[arg(long)]
    pub r#match: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct FrontiersArgs {
    /// Function to analyze for dominance frontiers
    #[arg(long)]
    pub function: String,

    /// Show iterated dominance frontier (for phi placement)
    #[arg(long)]
    pub iterated: bool,

    /// Show frontiers for specific node only
    #[arg(long)]
    pub node: Option<usize>,
}

#[derive(Parser, Debug, Clone)]
pub struct VerifyArgs {
    /// Path ID to verify
    #[arg(long)]
    pub path_id: String,
}

#[derive(Parser, Debug, Clone)]
pub struct BlastZoneArgs {
    /// Function symbol ID or name (for block-based analysis)
    #[arg(long)]
    pub function: Option<String>,

    /// Block ID to analyze impact from (default: entry block 0)
    #[arg(long)]
    pub block_id: Option<usize>,

    /// Path ID to analyze impact for
    #[arg(long)]
    pub path_id: Option<String>,

    /// Maximum depth to traverse
    #[arg(long, default_value_t = 100)]
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
/// Priority: CLI arg > MIRAGE_DB env var > default ".codemcp/codegraph.db"
/// This follows Magellan/llmgrep's pattern for database path resolution.
pub fn resolve_db_path(cli_db: Option<String>) -> anyhow::Result<String> {
    match cli_db {
        Some(path) => Ok(path),
        None => std::env::var("MIRAGE_DB")
            .or_else(|_| Ok(".codemcp/codegraph.db".to_string())),
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

/// LLM-optimized block representation with metadata
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct PathBlock {
    block_id: usize,
    terminator: String,
}

/// Source location range for a path (to be populated in plan 07-02)
#[derive(serde::Serialize)]
struct SourceRange {
    file_path: String,
    start_line: usize,
    end_line: usize,
}

/// Summary of a single path for JSON output (LLM-optimized)
#[derive(serde::Serialize)]
struct PathSummary {
    path_id: String,
    kind: String,
    length: usize,
    blocks: Vec<PathBlock>,
    /// Human-readable summary (to be populated in plan 07-04)
    summary: Option<String>,
    /// Source range for the entire path (to be populated in plan 07-02)
    source_range: Option<SourceRange>,
}

impl From<crate::cfg::Path> for PathSummary {
    fn from(path: crate::cfg::Path) -> Self {
        let length = path.len();
        // Convert Vec<usize> block IDs to Vec<PathBlock> with placeholder terminator
        // Full terminator info will be added in plan 07-02 when source locations are integrated
        let blocks: Vec<PathBlock> = path.blocks
            .into_iter()
            .map(|block_id| PathBlock {
                block_id,
                terminator: "Unknown".to_string(),
            })
            .collect();

        Self {
            path_id: path.path_id,
            kind: format!("{:?}", path.kind),
            length,
            blocks,
            summary: None,  // To be populated in plan 07-04
            source_range: None,  // To be populated in plan 07-02
        }
    }
}

impl PathSummary {
    /// Create PathSummary with CFG data for source locations
    /// This provides actual terminator types and source range information
    pub fn from_with_cfg(path: crate::cfg::Path, cfg: &crate::cfg::Cfg) -> Self {
        use crate::cfg::summarize_path;

        // Generate natural language summary
        let summary = Some(summarize_path(cfg, &path));

        // Build PathBlock list with actual terminator types from CFG
        let blocks: Vec<PathBlock> = path.blocks.iter().map(|&block_id| {
            // Find the node in the CFG
            let node_idx = cfg.node_indices()
                .find(|&n| cfg[n].id == block_id);

            let terminator = match node_idx {
                Some(idx) => format!("{:?}", cfg[idx].terminator),
                None => "Unknown".to_string(),
            };

            PathBlock {
                block_id,
                terminator,
            }
        }).collect();

        // Calculate source range from first and last blocks
        let source_range = Self::calculate_source_range(&path, cfg);

        let length = path.len();

        Self {
            path_id: path.path_id,
            kind: format!("{:?}", path.kind),
            length,
            summary,
            source_range,
            blocks,
        }
    }

    /// Calculate overall source range for a path
    fn calculate_source_range(path: &crate::cfg::Path, cfg: &crate::cfg::Cfg) -> Option<SourceRange> {
        let first_loc = path.blocks.first()
            .and_then(|&bid| cfg.node_indices().find(|&n| cfg[n].id == bid))
            .and_then(|idx| cfg[idx].source_location.clone());

        let last_loc = path.blocks.last()
            .and_then(|&bid| cfg.node_indices().find(|&n| cfg[n].id == bid))
            .and_then(|idx| cfg[idx].source_location.clone());

        match (first_loc, last_loc) {
            (Some(first), Some(last)) => {
                // Use first file_path, combine line ranges
                Some(SourceRange {
                    file_path: first.file_path.to_string_lossy().to_string(),
                    start_line: first.start_line,
                    end_line: last.end_line,
                })
            }
            _ => None,
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
    /// Uncalled functions (only populated when --include-uncalled is set)
    #[serde(skip_serializing_if = "Option::is_none")]
    uncalled_functions: Option<Vec<DeadSymbolJson>>,
}

/// Incoming edge information for unreachable blocks
#[derive(serde::Serialize, Clone)]
struct IncomingEdge {
    from_block: usize,
    edge_type: String,
}

/// Unreachable block details for JSON output
#[derive(serde::Serialize, Clone)]
struct UnreachableBlock {
    block_id: usize,
    kind: String,
    statements: Vec<String>,
    terminator: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    incoming_edges: Vec<IncomingEdge>,
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

/// Response for loops command
#[derive(serde::Serialize)]
struct LoopsResponse {
    function: String,
    loop_count: usize,
    loops: Vec<LoopInfo>,
}

/// Information about a single natural loop
#[derive(serde::Serialize)]
struct LoopInfo {
    header: usize,
    back_edge_from: usize,
    body_size: usize,
    nesting_level: usize,
    body_blocks: Vec<usize>,
}

/// Response for patterns command
#[derive(serde::Serialize)]
struct PatternsResponse {
    function: String,
    if_else_count: usize,
    match_count: usize,
    if_else_patterns: Vec<IfElseInfo>,
    match_patterns: Vec<MatchInfo>,
}

/// Information about a single if/else pattern
#[derive(serde::Serialize)]
struct IfElseInfo {
    condition_block: usize,
    true_branch: usize,
    false_branch: usize,
    merge_point: Option<usize>,
    has_else: bool,
}

/// Information about a single match pattern
#[derive(serde::Serialize)]
struct MatchInfo {
    switch_block: usize,
    branch_count: usize,
    targets: Vec<usize>,
    otherwise: usize,
}

/// Response for frontiers command
#[derive(serde::Serialize)]
struct FrontiersResponse {
    function: String,
    nodes_with_frontiers: usize,
    frontiers: Vec<NodeFrontier>,
}

/// Information about a single node's dominance frontier
#[derive(serde::Serialize)]
struct NodeFrontier {
    node: usize,
    frontier_set: Vec<usize>,
}

/// Response for iterated frontier command
#[derive(serde::Serialize)]
struct IteratedFrontierResponse {
    function: String,
    iterated_frontier: Vec<usize>,
}

/// Response for block impact analysis (blast zone)
#[derive(serde::Serialize)]
struct BlockImpactResponse {
    function: String,
    block_id: usize,
    reachable_blocks: Vec<usize>,
    reachable_count: usize,
    max_depth: usize,
    has_cycles: bool,
}

/// Response for path impact analysis (blast zone)
#[derive(serde::Serialize)]
struct PathImpactResponse {
    path_id: String,
    path_length: usize,
    unique_blocks_affected: Vec<usize>,
    impact_count: usize,
}

// ============================================================================
// Command Handlers (stubs for now)
// ============================================================================

pub mod cmds {
    use super::*;
    use crate::output;
    use anyhow::{Context, Result};

    /// Response for index command
    #[derive(serde::Serialize)]
    struct IndexResult {
        crate_name: String,
        total_functions: usize,
        updated_functions: usize,
        skipped_functions: usize,
        errors: Vec<String>,
    }

    pub fn index(args: &IndexArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::ullbc_to_cfg;
        use crate::mir::{run_charon, parse_ullbc};
        use crate::mir::charon::UllbcFunction;
        use crate::storage::{MirageDb, create_minimal_database, store_cfg, get_function_hash};
        use rusqlite::OptionalExtension;
        use std::path::{Path, PathBuf};

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

        // Create database if it doesn't exist
        if !Path::new(&db_path).exists() {
            output::info(&format!("Creating database at {}", db_path));
            create_minimal_database(&db_path)
                .with_context(|| format!("Failed to create database at {}", db_path))?;
            output::success("Database created successfully");
        }

        // Open database
        let mut db = match MirageDb::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                // JSON-aware error handling with remediation
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::new(
                        "DatabaseError",
                        &format!("Failed to open database: {}", e),
                        output::E_DATABASE_NOT_FOUND
                    );
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!("Failed to open database: {}", e));
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Determine project path
        let project_path = determine_project_path(&args)?;
        let cargo_toml = project_path.join("Cargo.toml");

        // Verify Cargo.toml exists
        if !cargo_toml.exists() {
            let msg = format!("Cargo.toml not found at {}", cargo_toml.display());
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "ProjectNotFound",
                    &msg,
                    output::E_INVALID_INPUT
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_FILE_NOT_FOUND);
            } else {
                output::error(&msg);
                output::info("Hint: --project should point to a Rust project directory");
                std::process::exit(output::EXIT_FILE_NOT_FOUND);
            }
        }

        output::header(&format!("Indexing {}", project_path.display()));

        // Run Charon (with auto-install prompt if missing)
        output::cmd("Running Charon to extract MIR...");

        // Check if Charon binary exists before attempting to run
        let charon_exists = std::path::Path::new("charon").exists()
            || std::process::Command::new("charon")
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

        let ullbc_json = if !charon_exists {
            // Charon not found - offer to auto-install
            let msg = "Charon binary not found";
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "CharonNotFound",
                    msg,
                    output::E_INVALID_INPUT
                ).with_remediation("Install Charon: cargo install --git https://github.com/AeneasVerif/charon charon");
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_ERROR);
            }

            // Human mode: prompt for auto-install
            output::error(msg);
            output::info("Charon is required for MIR extraction.");
            print!("\nInstall Charon now? [Y/n] ");
            use std::io::Write;
            std::io::stdout().flush().ok();

            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            let input = input.trim().to_lowercase();

            if input == "n" || input == "no" {
                output::info("Install manually: cargo install --git https://github.com/AeneasVerif/charon charon");
                output::info("Or download from: https://github.com/AeneasVerif/charon");
                std::process::exit(output::EXIT_ERROR);
            }

            // Attempt auto-install
            output::info("Installing Charon from GitHub...");
            output::cmd("cargo install --git https://github.com/AeneasVerif/charon charon");

            let install_status = std::process::Command::new("cargo")
                .args(["install", "--git", "https://github.com/AeneasVerif/charon", "charon"])
                .status()
                .context("Failed to run cargo install")?;

            if !install_status.success() {
                output::error("Charon installation failed");
                output::info("Try installing manually: cargo install --git https://github.com/AeneasVerif/charon charon");
                std::process::exit(output::EXIT_ERROR);
            }

            output::success("Charon installed successfully");

            // Now run Charon
            match run_charon(&project_path) {
                Ok(json) => json,
                Err(e) => {
                    output::error(&format!("Failed to run Charon after installation: {}", e));
                    return Err(e.context("Failed to run Charon"));
                }
            }
        } else {
            // Charon exists - run it normally
            match run_charon(&project_path) {
                Ok(json) => json,
                Err(e) => {
                    // Check if the error is about binary not found (path issue)
                    if e.to_string().contains("No such file") || e.to_string().contains("not found") {
                        let msg = "Charon binary not found in PATH";
                        if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                            let error = output::JsonError::new(
                                "CharonNotFound",
                                msg,
                                output::E_INVALID_INPUT
                            ).with_remediation("Install Charon: cargo install --git https://github.com/AeneasVerif/charon charon");
                            let wrapper = output::JsonResponse::new(error);
                            println!("{}", wrapper.to_json());
                            std::process::exit(output::EXIT_ERROR);
                        } else {
                            output::error(msg);
                            output::info("Install Charon: cargo install --git https://github.com/AeneasVerif/charon charon");
                            output::info("Or download from: https://github.com/AeneasVerif/charon");
                            std::process::exit(output::EXIT_ERROR);
                        }
                    }
                    return Err(e.context("Failed to run Charon"));
                }
            }
        };

        // Parse ULLBC
        let ullbc_data = match parse_ullbc(&ullbc_json) {
            Ok(data) => data,
            Err(e) => {
                output::error(&format!("Failed to parse Charon output: {}", e));
                output::info("Hint: Ensure Charon output format is JSON");
                return Err(e);
            }
        };

        let crate_name = &ullbc_data.crate_name;
        let functions = ullbc_data.functions;

        if functions.is_empty() {
            output::warn("No functions found in ULLBC output");
            let result = IndexResult {
                crate_name: crate_name.clone(),
                total_functions: 0,
                updated_functions: 0,
                skipped_functions: 0,
                errors: vec!["No functions found".to_string()],
            };

            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let wrapper = output::JsonResponse::new(result);
                println!("{}", wrapper.to_json());
            } else {
                println!("\nNo functions found to index.");
            }
            return Ok(());
        }

        // Process each function
        let mut updated = 0;
        let mut skipped = 0;
        let mut errors = Vec::new();
        let conn = db.conn_mut();

        for func in &functions {
            let func_name = &func.name;

            // Skip functions without body
            if func.body.is_none() {
                continue;
            }

            // Compute function hash for incremental detection
            let body = func.body.as_ref().unwrap();
            let function_hash = compute_function_hash(func);

            // Check if we should skip (incremental mode)
            if args.incremental {
                // Check if function already exists with same hash
                // First we need to find the function_id in graph_entities
                let existing_func_id: Option<i64> = conn.query_row(
                    "SELECT id FROM graph_entities WHERE kind = 'function' AND name = ? LIMIT 1",
                    rusqlite::params![func_name],
                    |row| row.get(0)
                ).optional().ok().flatten();

                if let Some(func_id) = existing_func_id {
                    if let Some(stored_hash) = get_function_hash(conn, func_id) {
                        if stored_hash == function_hash {
                            skipped += 1;
                            continue;
                        }
                    }
                }
            }

            // Convert ULLBC to CFG
            let cfg = ullbc_to_cfg(body);

            // Find or create graph_entities entry for the function
            let function_id: i64 = match conn.query_row(
                "SELECT id FROM graph_entities WHERE kind = 'function' AND name = ? LIMIT 1",
                rusqlite::params![func_name],
                |row| row.get(0)
            ).optional() {
                Ok(Some(id)) => id,
                Ok(None) => {
                    // Create new function entity
                    conn.execute(
                        "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
                        rusqlite::params!("function", func_name, "", "{}"),
                    ).context("Failed to insert function entity")?;
                    conn.last_insert_rowid()
                }
                Err(e) => {
                    errors.push(format!("{}: {}", func_name, e));
                    continue;
                }
            };

            // Store CFG
            if let Err(e) = store_cfg(conn, function_id, &function_hash, &cfg) {
                errors.push(format!("{}: {}", func_name, e));
            } else {
                updated += 1;

                // Print progress for human output
                if !matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let block_count = cfg.node_count();
                    let edge_count = cfg.edge_count();
                    println!("  Indexed: {} ({} blocks, {} edges)", func_name, block_count, edge_count);
                }
            }
        }

        // Prepare result
        let result = IndexResult {
            crate_name: crate_name.clone(),
            total_functions: functions.len(),
            updated_functions: updated,
            skipped_functions: skipped,
            errors: errors.clone(),
        };

        // Output based on format
        match cli.output {
            OutputFormat::Human => {
                println!();
                output::success(&format!("Indexing complete for {}", crate_name));
                println!("  Total functions: {}", result.total_functions);
                println!("  Updated: {}", result.updated_functions);
                println!("  Skipped: {}", result.skipped_functions);
                if !errors.is_empty() {
                    println!("  Errors: {}", errors.len());
                    for err in &errors {
                        println!("    - {}", err);
                    }
                }
            }
            OutputFormat::Json => {
                let wrapper = output::JsonResponse::new(result);
                println!("{}", wrapper.to_json());
            }
            OutputFormat::Pretty => {
                let wrapper = output::JsonResponse::new(result);
                println!("{}", wrapper.to_pretty_json());
            }
        }

        Ok(())
    }

    /// Determine the project path from arguments
    fn determine_project_path(args: &IndexArgs) -> anyhow::Result<std::path::PathBuf> {
        use std::path::PathBuf;

        if let Some(ref project) = args.project {
            Ok(PathBuf::from(project))
        } else if args.crate_.is_some() {
            // Use current directory
            std::env::current_dir()
                .context("Failed to get current directory")
        } else {
            // Default: current directory
            std::env::current_dir()
                .context("Failed to get current directory")
        }
    }

    /// Compute a BLAKE3 hash of a function for incremental detection
    fn compute_function_hash(func: &crate::mir::charon::UllbcFunction) -> String {
        let serialized = serde_json::to_string(func).unwrap_or_default();
        let hash = blake3::hash(serialized.as_bytes());
        hash.to_string()
    }

    pub fn status(_args: StatusArgs, cli: &Cli) -> Result<()> {
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

        // Open database
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
                    output::info("Hint: Run 'mirage index' to create the database");
                    std::process::exit(output::EXIT_DATABASE);
                }
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
        use crate::cfg::{PathKind, PathLimits, get_or_enumerate_paths};
        use crate::cfg::{resolve_function_name, load_cfg_from_db};
        use crate::storage::{MirageDb, get_function_hash};

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

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
                    output::info("Hint: Run 'mirage index' to create the database");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Resolve function name/ID to function_id
        let function_id = match resolve_function_name(db.conn(), &args.function) {
            Ok(id) => id,
            Err(_e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::function_not_found(&args.function);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!("Function '{}' not found in database", args.function));
                    output::info("Hint: Run 'mirage index' to index your code");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Load CFG from database
        let cfg = match load_cfg_from_db(db.conn(), function_id) {
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
                    output::error(&format!("Failed to load CFG for function '{}'", args.function));
                    output::info("The function may be corrupted. Try re-running 'mirage index'");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Build path limits based on args
        let mut limits = PathLimits::default();
        if let Some(max_length) = args.max_length {
            limits = limits.with_max_length(max_length);
        }

        // Get function hash for path caching
        let function_hash = match get_function_hash(db.conn(), function_id) {
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
                    output::info("The function data may be incomplete. Try re-running 'mirage index'");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        let mut paths = get_or_enumerate_paths(
            &cfg,
            function_id,
            &function_hash,
            &limits,
            db.conn_mut(),
        ).map_err(|e| anyhow::anyhow!("Path enumeration failed: {}", e))?;

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
                // Compact JSON with source locations from CFG
                let response = PathsResponse {
                    function: args.function.clone(),
                    total_paths: paths.len(),
                    error_paths: error_count,
                    paths: paths.iter().map(|p| PathSummary::from_with_cfg(p.clone(), &cfg)).collect(),
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
                    paths: paths.iter().map(|p| PathSummary::from_with_cfg(p.clone(), &cfg)).collect(),
                };
                let wrapper = output::JsonResponse::new(response);
                println!("{}", wrapper.to_pretty_json());
            }
        }

        Ok(())
    }

    pub fn cfg(args: &CfgArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::{export_dot, export_json, CFGExport};
        use crate::cfg::{resolve_function_name, load_cfg_from_db};
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

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
                    output::info("Hint: Run 'mirage index' to create the database");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Resolve function name/ID to function_id
        let function_id = match resolve_function_name(db.conn(), &args.function) {
            Ok(id) => id,
            Err(_e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::function_not_found(&args.function);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!("Function '{}' not found in database", args.function));
                    output::info("Hint: Run 'mirage index' to index your code");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Load CFG from database
        let cfg = match load_cfg_from_db(db.conn(), function_id) {
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
                    output::error(&format!("Failed to load CFG for function '{}'", args.function));
                    output::info("The function may be corrupted. Try re-running 'mirage index'");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

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
        use crate::cfg::{resolve_function_name, load_cfg_from_db};
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

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
                    output::info("Hint: Run 'mirage index' to create the database");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Resolve function name/ID to function_id
        let function_id = match resolve_function_name(db.conn(), &args.function) {
            Ok(id) => id,
            Err(_e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::function_not_found(&args.function);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!("Function '{}' not found in database", args.function));
                    output::info("Hint: Run 'mirage index' to index your code");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Load CFG from database
        let cfg = match load_cfg_from_db(db.conn(), function_id) {
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
                    output::error(&format!("Failed to load CFG for function '{}'", args.function));
                    output::info("The function may be corrupted. Try re-running 'mirage index'");
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

    pub fn loops(args: &LoopsArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::detect_natural_loops;
        use crate::cfg::{resolve_function_name, load_cfg_from_db};
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

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
                    output::info("Hint: Run 'mirage index' to create the database");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Resolve function name/ID to function_id
        let function_id = match resolve_function_name(db.conn(), &args.function) {
            Ok(id) => id,
            Err(_e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::function_not_found(&args.function);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!("Function '{}' not found in database", args.function));
                    output::info("Hint: Run 'mirage index' to index your code");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Load CFG from database
        let cfg = match load_cfg_from_db(db.conn(), function_id) {
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
                    output::error(&format!("Failed to load CFG for function '{}'", args.function));
                    output::info("The function may be corrupted. Try re-running 'mirage index'");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Detect natural loops
        let natural_loops = detect_natural_loops(&cfg);

        // Compute nesting levels for each loop
        let loop_infos: Vec<LoopInfo> = natural_loops.iter().map(|loop_| {
            let nesting_level = loop_.nesting_level(&natural_loops);
            let body_blocks: Vec<usize> = loop_.body.iter()
                .map(|&node| cfg[node].id)
                .collect();
            LoopInfo {
                header: cfg[loop_.header].id,
                back_edge_from: cfg[loop_.back_edge.0].id,
                body_size: loop_.size(),
                nesting_level,
                body_blocks,
            }
        }).collect();

        // Output based on format
        match cli.output {
            OutputFormat::Human => {
                println!("Function: {}", args.function);
                println!("Natural Loops: {}", natural_loops.len());
                println!();

                if natural_loops.is_empty() {
                    output::info("No natural loops detected in this function");
                } else {
                    for (i, loop_info) in loop_infos.iter().enumerate() {
                        println!("Loop {}:", i + 1);
                        println!("  Header: Block {}", loop_info.header);
                        println!("  Back edge from: Block {}", loop_info.back_edge_from);
                        println!("  Body size: {} blocks", loop_info.body_size);
                        println!("  Nesting level: {}", loop_info.nesting_level);

                        if args.verbose {
                            println!("  Body blocks: {:?}", loop_info.body_blocks);
                        }
                        println!();
                    }
                }
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let response = LoopsResponse {
                    function: args.function.clone(),
                    loop_count: natural_loops.len(),
                    loops: loop_infos,
                };
                let wrapper = output::JsonResponse::new(response);
                match cli.output {
                    OutputFormat::Json => println!("{}", wrapper.to_json()),
                    OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                    _ => unreachable!(),
                }
            }
        }

        Ok(())
    }

    pub fn unreachable(args: &UnreachableArgs, cli: &Cli) -> Result<()> {
        use crate::analysis::MagellanBridge;
        use crate::analysis::DeadSymbolJson;
        use crate::cfg::reachability::find_unreachable;
        use crate::cfg::load_cfg_from_db;
        use crate::storage::MirageDb;
        use petgraph::visit::EdgeRef;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

        // For --include-uncalled, also open Magellan database
        let uncalled_functions: Option<Vec<DeadSymbolJson>> = if args.include_uncalled {
            match MagellanBridge::open(&db_path) {
                Ok(bridge) => {
                    match bridge.dead_symbols("main") {
                        Ok(dead) => {
                            let json_symbols: Vec<DeadSymbolJson> = dead.iter().map(|d| d.into()).collect();
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
                    eprintln!("Warning: Could not open Magellan database for --include-uncalled: {}", e);
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
                    output::info("Hint: Run 'mirage index' to create the database");
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
        // Use prepare and execute to handle multiple rows properly
        let mut function_rows: Vec<(String, i64)> = Vec::new();
        let mut stmt = match db.conn().prepare("SELECT name, id FROM graph_entities WHERE kind = 'function'") {
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
            match load_cfg_from_db(db.conn(), function_id) {
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
                                        .map(|edge| {
                                            let source_block = &cfg[edge.source()];
                                            let edge_type = cfg.edge_weight(edge.id()).unwrap();
                                            IncomingEdge {
                                                from_block: source_block.id,
                                                edge_type: format!("{:?}", edge_type),
                                            }
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
                    if uncalled_functions.is_none() || uncalled_functions.as_ref().map(|v| v.is_empty()).unwrap_or(false) {
                        output::info("No unreachable code found");
                    }
                    return Ok(());
                }

                println!("Unreachable Code Blocks:");
                println!("  Total blocks: {}", total_blocks);
                println!("  Functions with unreachable: {}/{}", functions_with_unreachable, total_functions);
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
                                println!("    Block {} has no incoming edges (entry or isolated)", block.block_id);
                            } else {
                                println!("    Block {} incoming edges:", block.block_id);
                                for edge in &block.incoming_edges {
                                    println!("      from block {} ({})", edge.from_block, edge.edge_type);
                                }
                            }
                        }
                        println!();
                    }
                }
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                // For multi-function mode, flatten blocks across all functions
                let all_blocks: Vec<UnreachableBlock> = all_results.iter().flat_map(|r| r.blocks.clone()).collect();

                let response = UnreachableResponse {
                    function: "all".to_string(),
                    total_functions,
                    functions_with_unreachable,
                    unreachable_count: total_blocks,
                    blocks: all_blocks,
                    uncalled_functions: uncalled_functions,
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
        use crate::cfg::{PathLimits, enumerate_paths, load_cfg_from_db};
        use crate::storage::MirageDb;
        use rusqlite::OptionalExtension;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

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
                    output::info("Hint: Run 'mirage index' to create the database");
                    std::process::exit(output::EXIT_DATABASE);
                }
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
        // Load CFG from database for this function
        let cfg = match load_cfg_from_db(db.conn(), function_id) {
            Ok(cfg) => cfg,
            Err(_e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::new(
                        "CgfLoadError",
                        &format!("Failed to load CFG for function_id {}", function_id),
                        output::E_CFG_ERROR,
                    );
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!("Failed to load CFG for function_id {}", function_id));
                    output::info("The function data may be corrupted. Try re-running 'mirage index'");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

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

    pub fn blast_zone(args: &BlastZoneArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::{find_reachable_from_block, load_cfg_from_db, resolve_function_name};
        use crate::storage::{compute_path_impact_from_db, get_function_name, MirageDb};
        use rusqlite::OptionalExtension;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

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
                    output::info("Hint: Run 'mirage index' to create the database");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Determine query type: path-based or block-based
        if let Some(ref path_id) = args.path_id {
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
            let (function_id, path_kind): (i64, String) = match db.conn().query_row(
                "SELECT function_id, path_kind FROM cfg_paths WHERE path_id = ?1",
                rusqlite::params![path_id_trimmed],
                |row| Ok((row.get(0)?, row.get(1)?))
            ).optional() {
                Ok(Some(data)) => data,
                Ok(None) => {
                    let msg = format!("Path '{}' not found in cache", path_id_trimmed);
                    if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                        let error = output::JsonError::new("PathNotFound", &msg, output::E_PATH_NOT_FOUND);
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
                        let error = output::JsonError::new("DatabaseError", &msg, output::E_DATABASE_NOT_FOUND);
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
                let msg = format!("Path '{}' is an error path (use --include-errors to analyze)", path_id_trimmed);
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::new("ErrorPathExcluded", &msg, output::E_INVALID_INPUT);
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
            let cfg = match load_cfg_from_db(db.conn(), function_id) {
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
                        output::info("The function may be corrupted. Try re-running 'mirage index'");
                        std::process::exit(output::EXIT_DATABASE);
                    }
                }
            };

            // Get function name for display
            let function_name = get_function_name(db.conn(), function_id)
                .unwrap_or_else(|| format!("<function_{}>", function_id));

            // Compute path impact
            let max_depth = if args.max_depth == 100 { None } else { Some(args.max_depth) };
            let impact = match compute_path_impact_from_db(db.conn(), path_id_trimmed, &cfg, max_depth) {
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
                    println!("Impact Scope:");
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
            let function_ref = args.function.as_ref().expect("--function is required for block-based analysis");

            // Resolve function name/ID to function_id
            let function_id = match resolve_function_name(db.conn(), function_ref) {
                Ok(id) => id,
                Err(_e) => {
                    if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                        let error = output::JsonError::function_not_found(function_ref);
                        let wrapper = output::JsonResponse::new(error);
                        println!("{}", wrapper.to_json());
                        std::process::exit(output::EXIT_DATABASE);
                    } else {
                        output::error(&format!("Function '{}' not found in database", function_ref));
                        output::info("Hint: Run 'mirage index' to index your code");
                        std::process::exit(output::EXIT_DATABASE);
                    }
                }
            };

            // Get function name for display
            let function_name = get_function_name(db.conn(), function_id)
                .unwrap_or_else(|| format!("<function_{}>", function_id));

            // Load CFG from database
            let cfg = match load_cfg_from_db(db.conn(), function_id) {
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
                        output::error(&format!("Failed to load CFG for function '{}'", function_ref));
                        output::info("The function may be corrupted. Try re-running 'mirage index'");
                        std::process::exit(output::EXIT_DATABASE);
                    }
                }
            };

            // Determine block ID (default to entry block 0)
            let block_id = args.block_id.unwrap_or(0);

            // Validate block_id exists in CFG
            let block_exists = cfg.node_indices().any(|n| cfg[n].id == block_id);
            if !block_exists {
                let valid_blocks: Vec<usize> = cfg.node_indices()
                    .map(|n| cfg[n].id)
                    .collect();
                let msg = format!("Block {} not found in function '{}'. Valid blocks: {:?}", block_id, function_ref, valid_blocks);
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::new("BlockNotFound", &msg, output::E_BLOCK_NOT_FOUND);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_VALIDATION);
                } else {
                    output::error(&msg);
                    std::process::exit(output::EXIT_VALIDATION);
                }
            }

            // Compute block impact
            let max_depth = if args.max_depth == 100 { None } else { Some(args.max_depth) };
            let impact = find_reachable_from_block(&cfg, block_id, max_depth);

            // Output
            match cli.output {
                OutputFormat::Human => {
                    println!("Block Impact Analysis (Blast Zone)");
                    println!();
                    println!("Function: {}", function_name);
                    println!("Source block: {}", impact.source_block_id);
                    println!();
                    println!("Impact Scope:");
                    println!("  Reachable blocks: {}", impact.reachable_count);
                    if impact.reachable_count > 0 {
                        println!("  Affected blocks: {:?}", impact.reachable_blocks);
                    } else {
                        println!("  Affected blocks: (none - block has no downstream impact)");
                    }
                    println!("  Max depth reached: {}", impact.max_depth_reached);
                    println!("  Contains cycles: {}", if impact.has_cycles { "yes (loop detected)" } else { "no" });
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

    pub fn patterns(args: &PatternsArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::{detect_if_else_patterns, detect_match_patterns};
        use crate::cfg::{resolve_function_name, load_cfg_from_db};
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

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
                    output::info("Hint: Run 'mirage index' to create the database");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Resolve function name/ID to function_id
        let function_id = match resolve_function_name(db.conn(), &args.function) {
            Ok(id) => id,
            Err(_e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::function_not_found(&args.function);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!("Function '{}' not found in database", args.function));
                    output::info("Hint: Run 'mirage index' to index your code");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Load CFG from database
        let cfg = match load_cfg_from_db(db.conn(), function_id) {
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
                    output::error(&format!("Failed to load CFG for function '{}'", args.function));
                    output::info("The function may be corrupted. Try re-running 'mirage index'");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Detect patterns based on filter flags
        let show_if_else = !args.r#match;  // Show if/else unless --match only
        let show_match = !args.if_else;    // Show match unless --if-else only

        let if_else_patterns = if show_if_else {
            detect_if_else_patterns(&cfg)
        } else {
            vec![]
        };

        let match_patterns = if show_match {
            detect_match_patterns(&cfg)
        } else {
            vec![]
        };

        // Convert to response format
        let if_else_infos: Vec<IfElseInfo> = if_else_patterns.iter().map(|p| {
            IfElseInfo {
                condition_block: cfg[p.condition].id,
                true_branch: cfg[p.true_branch].id,
                false_branch: cfg[p.false_branch].id,
                merge_point: p.merge_point.map(|n| cfg[n].id),
                has_else: p.has_else(),
            }
        }).collect();

        let match_infos: Vec<MatchInfo> = match_patterns.iter().map(|p| {
            MatchInfo {
                switch_block: cfg[p.switch_node].id,
                branch_count: p.branch_count(),
                targets: p.targets.iter().map(|n| cfg[*n].id).collect(),
                otherwise: cfg[p.otherwise].id,
            }
        }).collect();

        // Output based on format
        match cli.output {
            OutputFormat::Human => {
                println!("Function: {}", args.function);
                println!();

                if show_if_else {
                    println!("If/Else Patterns: {}", if_else_patterns.len());
                    if if_else_patterns.is_empty() {
                        output::info("No if/else patterns detected");
                    } else {
                        for (i, info) in if_else_infos.iter().enumerate() {
                            println!("  Pattern {}:", i + 1);
                            println!("    Condition: Block {}", info.condition_block);
                            println!("    True branch: Block {}", info.true_branch);
                            println!("    False branch: Block {}", info.false_branch);
                            if let Some(merge) = info.merge_point {
                                println!("    Merge point: Block {}", merge);
                                println!("    Has else: {}", info.has_else);
                            } else {
                                println!("    Merge point: None (no else)");
                            }
                            println!();
                        }
                    }
                    println!();
                }

                if show_match {
                    println!("Match Patterns: {}", match_patterns.len());
                    if match_patterns.is_empty() {
                        output::info("No match patterns detected");
                    } else {
                        for (i, info) in match_infos.iter().enumerate() {
                            println!("  Pattern {}:", i + 1);
                            println!("    Switch: Block {}", info.switch_block);
                            println!("    Branch count: {}", info.branch_count);
                            println!("    Targets: {:?}", info.targets);
                            println!("    Otherwise: Block {}", info.otherwise);
                            println!();
                        }
                    }
                }
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let response = PatternsResponse {
                    function: args.function.clone(),
                    if_else_count: if_else_patterns.len(),
                    match_count: match_patterns.len(),
                    if_else_patterns: if_else_infos,
                    match_patterns: match_infos,
                };
                let wrapper = output::JsonResponse::new(response);
                match cli.output {
                    OutputFormat::Json => println!("{}", wrapper.to_json()),
                    OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                    _ => unreachable!(),
                }
            }
        }

        Ok(())
    }

    pub fn frontiers(args: &FrontiersArgs, cli: &Cli) -> Result<()> {
        use crate::cfg::{compute_dominance_frontiers, DominatorTree};
        use crate::cfg::{resolve_function_name, load_cfg_from_db};
        use crate::storage::MirageDb;

        // Resolve database path
        let db_path = super::resolve_db_path(cli.db.clone())?;

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
                    output::info("Hint: Run 'mirage index' to create the database");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Resolve function name/ID to function_id
        let function_id = match resolve_function_name(db.conn(), &args.function) {
            Ok(id) => id,
            Err(_e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::function_not_found(&args.function);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!("Function '{}' not found in database", args.function));
                    output::info("Hint: Run 'mirage index' to index your code");
                    std::process::exit(output::EXIT_DATABASE);
                }
            }
        };

        // Load CFG from database
        let cfg = match load_cfg_from_db(db.conn(), function_id) {
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
                    output::error(&format!("Failed to load CFG for function '{}'", args.function));
                    output::info("The function may be corrupted. Try re-running 'mirage index'");
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
            let iterated_blocks: Vec<usize> = iterated_frontier.iter()
                .map(|&n| cfg[n].id)
                .collect();

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
            let target_node = cfg.node_indices()
                .find(|&n| cfg[n].id == node_id);

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
            let frontier_blocks: Vec<usize> = frontier.iter()
                .map(|&n| cfg[n].id)
                .collect();

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
            let nodes_with_frontiers: Vec<NodeFrontier> = frontiers.nodes_with_frontiers()
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
                    println!("Nodes with non-empty dominance frontiers: {}", nodes_with_frontiers.len());
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
        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION)?;

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

        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

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

        // blocks is now Vec<PathBlock> with block_id and terminator
        assert_eq!(summary.blocks.len(), 3, "should have 3 blocks");
        assert_eq!(summary.blocks[0].block_id, 0, "first block_id should be 0");
        assert_eq!(summary.blocks[1].block_id, 1, "second block_id should be 1");
        assert_eq!(summary.blocks[2].block_id, 2, "third block_id should be 2");
        assert_eq!(summary.blocks[0].terminator, "Unknown", "terminator should be Unknown placeholder");

        // Optional fields should be None until populated in future plans
        assert!(summary.summary.is_none(), "summary should be None");
        assert!(summary.source_range.is_none(), "source_range should be None");
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

    /// Test PathSummary::from_with_cfg with source locations
    #[test]
    fn test_path_summary_from_with_cfg() {
        use crate::cfg::{BasicBlock, BlockKind, EdgeType, Path, PathKind, SourceLocation, Terminator};
        use petgraph::graph::DiGraph;
        use std::path::PathBuf;

        // Create a test CFG with source locations
        let mut g = DiGraph::new();

        let loc0 = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 0,
            byte_end: 10,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 10,
        };

        let loc1 = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 11,
            byte_end: 20,
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 10,
        };

        let loc2 = SourceLocation {
            file_path: PathBuf::from("test.rs"),
            byte_start: 21,
            byte_end: 30,
            start_line: 3,
            start_column: 1,
            end_line: 3,
            end_column: 10,
        };

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec!["let x = 1".to_string()],
            terminator: Terminator::Goto { target: 1 },
            source_location: Some(loc0),
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec!["if x > 0".to_string()],
            terminator: Terminator::SwitchInt {
                targets: vec![2],
                otherwise: 2,
            },
            source_location: Some(loc1),
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec!["return true".to_string()],
            terminator: Terminator::Return,
            source_location: Some(loc2),
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);

        // Create a path and use from_with_cfg
        let path = Path::new(vec![0, 1, 2], PathKind::Normal);
        let summary = PathSummary::from_with_cfg(path, &g);

        // Check terminator is populated
        assert_eq!(summary.blocks[0].terminator, "Goto { target: 1 }");
        assert!(summary.blocks[1].terminator.contains("SwitchInt"));
        assert_eq!(summary.blocks[2].terminator, "Return");

        // Check source_range is populated
        assert!(summary.source_range.is_some(), "source_range should be Some");
        let sr = summary.source_range.as_ref().unwrap();
        assert_eq!(sr.file_path, "test.rs");
        assert_eq!(sr.start_line, 1);
        assert_eq!(sr.end_line, 3);
    }

    /// Test PathSummary::from_with_cfg with no source locations (graceful None)
    #[test]
    fn test_path_summary_from_with_cfg_no_source_locations() {
        use crate::cfg::{Path, PathKind};

        // Use the test CFG which has no source locations
        let cfg = cmds::create_test_cfg();
        let path = Path::new(vec![0, 1, 2], PathKind::Normal);
        let summary = PathSummary::from_with_cfg(path, &cfg);

        // Terminator should still be populated
        assert!(summary.blocks[0].terminator.contains("Goto"));
        assert!(summary.blocks[1].terminator.contains("SwitchInt"));
        assert_eq!(summary.blocks[2].terminator, "Return");

        // source_range should be None when no source locations exist
        assert!(summary.source_range.is_none(), "source_range should be None when CFG has no locations");
    }

    // ------------------------------------------------------------------------
    // Path Caching Tests
    // ------------------------------------------------------------------------

    /// Test that first call enumerates paths (cache miss)
    #[test]
    fn test_paths_cache_miss_first_call() {
        use crate::cfg::get_or_enumerate_paths;
        use crate::storage::create_schema;
        use rusqlite::Connection;

        // Create an in-memory database with Mirage schema
        let mut conn = Connection::open_in_memory().unwrap();

        // Create Magellan schema first (required for Mirage schema)
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
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        // Create Mirage schema
        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        // Get test CFG and limits
        let cfg = cmds::create_test_cfg();
        let limits = PathLimits::default();
        let test_function_id: i64 = 1;  // First auto-increment ID;
        // Insert a test function entity (required for foreign key constraint)
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();

        // Enable foreign key enforcement
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        let test_function_hash: &str = "test_cfg";

        // First call should enumerate (no cache)
        let paths1 = get_or_enumerate_paths(
            &cfg,
            test_function_id,
            test_function_hash,
            &limits,
            &mut conn,
        ).unwrap();

        // Verify we got paths
        assert!(!paths1.is_empty(), "First call should enumerate and return paths");
        assert_eq!(paths1.len(), 2, "Test CFG should have 2 paths");

        // Verify paths were stored in database
        let path_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(path_count, 2, "Paths should be stored in database after first call");

        // Verify function_hash was stored
        let stored_hash: Option<String> = conn.query_row(
            "SELECT function_hash FROM cfg_blocks WHERE function_id = ? LIMIT 1",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(stored_hash.as_deref(), Some(test_function_hash), "Function hash should be stored");
    }

    /// Test that second call returns cached paths (cache hit)
    #[test]
    fn test_paths_cache_hit_second_call() {
        use crate::cfg::get_or_enumerate_paths;
        use crate::storage::create_schema;
        use rusqlite::Connection;

        // Create an in-memory database with Mirage schema
        let mut conn = Connection::open_in_memory().unwrap();

        // Create Magellan schema first
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
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        // Create Mirage schema
        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();
        // Insert a test function entity (required for foreign key constraint)
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();

        // Enable foreign key enforcement
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        // Get test CFG and limits
        let cfg = cmds::create_test_cfg();
        let limits = PathLimits::default();
        let test_function_id: i64 = 1;  // First auto-increment ID;
        let test_function_hash: &str = "test_cfg";

        // First call - cache miss, enumerates and stores
        let paths1 = get_or_enumerate_paths(
            &cfg,
            test_function_id,
            test_function_hash,
            &limits,
            &mut conn,
        ).unwrap();
        // Verify hash was stored after first call
        let stored_hash: Option<String> = conn.query_row(
            "SELECT function_hash FROM cfg_blocks WHERE function_id = ? LIMIT 1",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(stored_hash.as_deref(), Some(test_function_hash), "Hash should be stored after first call");

        // Verify paths were stored
        let path_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(path_count, 2, "Should have 2 paths stored after first call");

        // Second call - cache hit, should return same paths
        let paths2 = get_or_enumerate_paths(
            &cfg,
            test_function_id,
            test_function_hash,
            &limits,
            &mut conn,
        ).unwrap();

        // Should return same number of paths
        assert_eq!(paths2.len(), paths1.len(), "Cache hit should return same number of paths");

        // Paths should have identical path_ids (cache hit returns same data)
        let mut path_ids1: Vec<_> = paths1.iter().map(|p| &p.path_id).collect();
        let mut path_ids2: Vec<_> = paths2.iter().map(|p| &p.path_id).collect();
        path_ids1.sort();
        path_ids2.sort();

        assert_eq!(path_ids1, path_ids2, "Cache hit should return paths with same IDs");

        // Verify path entries match
        for (p1, p2) in paths1.iter().zip(paths2.iter()) {
            assert_eq!(p1.path_id, p2.path_id, "Path IDs should match on cache hit");
            assert_eq!(p1.kind, p2.kind, "Path kinds should match on cache hit");
            assert_eq!(p1.blocks, p2.blocks, "Path blocks should match on cache hit");
        }
    }

    /// Test that function hash change invalidates cache
    #[test]
    fn test_paths_cache_invalidation_on_hash_change() {
        use crate::cfg::get_or_enumerate_paths;
        use crate::storage::create_schema;
        use rusqlite::Connection;

        // Create an in-memory database with Mirage schema
        let mut conn = Connection::open_in_memory().unwrap();

        // Create Magellan schema first
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
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        // Create Mirage schema
        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();
        // Insert a test function entity (required for foreign key constraint)
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();

        // Enable foreign key enforcement
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        // Get test CFG and limits
        let cfg = cmds::create_test_cfg();
        let limits = PathLimits::default();
        let test_function_id: i64 = 1;  // First auto-increment ID;
        let test_function_hash_v1: &str = "test_cfg_v1";
        let test_function_hash_v2: &str = "test_cfg_v2";

        // First call with hash v1 - cache miss, enumerates and stores
        let paths1 = get_or_enumerate_paths(
            &cfg,
            test_function_id,
            test_function_hash_v1,
            &limits,
            &mut conn,
        ).unwrap();

        // Verify paths were stored
        let stored_hash_v1: Option<String> = conn.query_row(
            "SELECT function_hash FROM cfg_blocks WHERE function_id = ? LIMIT 1",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(stored_hash_v1.as_deref(), Some(test_function_hash_v1), "Hash v1 should be stored");

        // Second call with different hash - cache invalidation, should re-enumerate
        let paths2 = get_or_enumerate_paths(
            &cfg,
            test_function_id,
            test_function_hash_v2,
            &limits,
            &mut conn,
        ).unwrap();

        // Should still return paths (re-enumerated with new hash)
        assert!(!paths2.is_empty(), "Should re-enumerate after hash change");
        assert_eq!(paths2.len(), paths1.len(), "Re-enumeration should produce same paths");

        // Verify hash was updated in database
        let stored_hash_v2: Option<String> = conn.query_row(
            "SELECT function_hash FROM cfg_blocks WHERE function_id = ? LIMIT 1",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(stored_hash_v2.as_deref(), Some(test_function_hash_v2), "Hash v2 should replace v1");

        // Verify old paths were invalidated
        let path_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(path_count, 2, "Should have 2 paths after invalidation (old replaced with new)");
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
                    incoming_edges: vec![],
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
            uncalled_functions: None,
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
            incoming_edges: vec![],
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

    /// Test that --show-branches includes incoming edge details
    #[test]
    fn test_unreachable_show_branches_with_edges() {
        use crate::cfg::reachability::find_unreachable;
        use petgraph::visit::EdgeRef;

        // Create a CFG with an unreachable block that HAS incoming edges
        // This simulates a block that's only reachable from an unreachable source
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

        // b3 and b4 are both unreachable, but b4 has an incoming edge from b3
        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Normal,
            statements: vec!["unreachable branch".to_string()],
            terminator: Terminator::Goto { target: 4 },
            source_location: None,
        });

        let b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Exit,
            statements: vec!["unreachable code".to_string()],
            terminator: Terminator::Unreachable,
            source_location: None,
        });

        // Only connect entry to b1, making b3 and b4 unreachable
        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        // b3 -> b4 edge exists, but both blocks are unreachable
        g.add_edge(b3, b4, EdgeType::Fallthrough);

        // Build UnreachableBlock structs with show_branches=true
        let unreachable_indices = find_unreachable(&g);
        let blocks: Vec<UnreachableBlock> = unreachable_indices
            .iter()
            .map(|&idx| {
                let block = &g[idx];
                let kind_str = format!("{:?}", block.kind);
                let terminator_str = format!("{:?}", block.terminator);

                // Collect incoming edges
                let incoming_edges: Vec<IncomingEdge> = g
                    .edge_references()
                    .filter(|edge| edge.target() == idx)
                    .map(|edge| {
                        let source_block = &g[edge.source()];
                        let edge_type = g.edge_weight(edge.id()).unwrap();
                        IncomingEdge {
                            from_block: source_block.id,
                            edge_type: format!("{:?}", edge_type),
                        }
                    })
                    .collect();

                UnreachableBlock {
                    block_id: block.id,
                    kind: kind_str,
                    statements: block.statements.clone(),
                    terminator: terminator_str,
                    incoming_edges,
                }
            })
            .collect();

        // Should find 2 unreachable blocks (3 and 4)
        assert_eq!(blocks.len(), 2);

        // Block 3 should have no incoming edges (isolated unreachable code)
        let block3 = blocks.iter().find(|b| b.block_id == 3).unwrap();
        assert_eq!(block3.incoming_edges.len(), 0);

        // Block 4 should have 1 incoming edge from block 3
        let block4 = blocks.iter().find(|b| b.block_id == 4).unwrap();
        assert_eq!(block4.incoming_edges.len(), 1);
        assert_eq!(block4.incoming_edges[0].from_block, 3);
        assert_eq!(block4.incoming_edges[0].edge_type, "Fallthrough");
    }

    /// Test that --show-branches JSON output includes incoming_edges field
    #[test]
    fn test_unreachable_show_branches_json_output() {
        use crate::cfg::reachability::find_unreachable;
        use crate::output::JsonResponse;
        use petgraph::visit::EdgeRef;

        // Create the same CFG as above
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
            kind: BlockKind::Normal,
            statements: vec!["unreachable branch".to_string()],
            terminator: Terminator::Goto { target: 4 },
            source_location: None,
        });

        let b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Exit,
            statements: vec!["unreachable code".to_string()],
            terminator: Terminator::Unreachable,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b3, b4, EdgeType::Fallthrough);

        // Build UnreachableBlock structs with incoming edges
        let unreachable_indices = find_unreachable(&g);
        let blocks: Vec<UnreachableBlock> = unreachable_indices
            .iter()
            .map(|&idx| {
                let block = &g[idx];
                UnreachableBlock {
                    block_id: block.id,
                    kind: format!("{:?}", block.kind),
                    statements: block.statements.clone(),
                    terminator: format!("{:?}", block.terminator),
                    incoming_edges: g
                        .edge_references()
                        .filter(|edge| edge.target() == idx)
                        .map(|edge| {
                            let source_block = &g[edge.source()];
                            let edge_type = g.edge_weight(edge.id()).unwrap();
                            IncomingEdge {
                                from_block: source_block.id,
                                edge_type: format!("{:?}", edge_type),
                            }
                        })
                        .collect(),
                }
            })
            .collect();

        let response = UnreachableResponse {
            function: "test".to_string(),
            total_functions: 1,
            functions_with_unreachable: 1,
            unreachable_count: 2,
            blocks,
        };

        let wrapper = JsonResponse::new(response);
        let json = wrapper.to_json();

        // Verify JSON contains incoming_edges field
        assert!(json.contains("\"incoming_edges\""));
        // Verify block 4 has an incoming edge from block 3
        assert!(json.contains("\"from_block\":3"));
        assert!(json.contains("\"edge_type\":\"Fallthrough\""));
    }

    /// Test that IncomingEdge struct serializes correctly
    #[test]
    fn test_incoming_edge_serialization() {
        let edge = IncomingEdge {
            from_block: 5,
            edge_type: "TrueBranch".to_string(),
        };

        let serialized = serde_json::to_string(&edge).unwrap();
        assert!(serialized.contains("\"from_block\":5"));
        assert!(serialized.contains("\"edge_type\":\"TrueBranch\""));
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
        crate::storage::create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION)?;

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

// ============================================================================
// Output Format Consistency Tests (06-07)
// ============================================================================

#[cfg(test)]
mod output_format_tests {
    use super::*;
    use crate::output::JsonResponse;

    /// Test that all response structs serialize correctly to JSON
    #[test]
    fn test_all_response_types_serialize() {
        // PathsResponse
        let paths_resp = PathsResponse {
            function: "test_func".to_string(),
            total_paths: 2,
            error_paths: 0,
            paths: vec![],
        };
        let paths_json = serde_json::to_string(&paths_resp);
        assert!(paths_json.is_ok(), "PathsResponse should serialize");

        // DominanceResponse
        let dom_resp = DominanceResponse {
            function: "test_func".to_string(),
            kind: "dominators".to_string(),
            root: Some(0),
            dominance_tree: vec![],
            must_pass_through: None,
        };
        let dom_json = serde_json::to_string(&dom_resp);
        assert!(dom_json.is_ok(), "DominanceResponse should serialize");

        // UnreachableResponse
        let unreach_resp = UnreachableResponse {
            function: "test_func".to_string(),
            total_functions: 1,
            functions_with_unreachable: 0,
            unreachable_count: 0,
            blocks: vec![],
        };
        let unreach_json = serde_json::to_string(&unreach_resp);
        assert!(unreach_json.is_ok(), "UnreachableResponse should serialize");

        // VerifyResult
        let verify_res = VerifyResult {
            path_id: "test_path".to_string(),
            valid: true,
            found_in_cache: true,
            function_id: Some(1),
            reason: "Test".to_string(),
            current_paths: 2,
        };
        let verify_json = serde_json::to_string(&verify_res);
        assert!(verify_json.is_ok(), "VerifyResult should serialize");
    }

    /// Test that JsonResponse wrapper works for all response types
    #[test]
    fn test_json_response_wrapper_for_all_commands() {
        // PathsResponse wrapped
        let paths_resp = PathsResponse {
            function: "test_func".to_string(),
            total_paths: 2,
            error_paths: 0,
            paths: vec![],
        };
        let paths_wrapper = JsonResponse::new(paths_resp);
        assert_eq!(paths_wrapper.schema_version, "1.0.0");
        assert_eq!(paths_wrapper.tool, "mirage");
        assert!(!paths_wrapper.execution_id.is_empty());

        // DominanceResponse wrapped
        let dom_resp = DominanceResponse {
            function: "test_func".to_string(),
            kind: "dominators".to_string(),
            root: Some(0),
            dominance_tree: vec![],
            must_pass_through: None,
        };
        let dom_wrapper = JsonResponse::new(dom_resp);
        assert_eq!(dom_wrapper.schema_version, "1.0.0");
        assert_eq!(dom_wrapper.tool, "mirage");

        // UnreachableResponse wrapped
        let unreach_resp = UnreachableResponse {
            function: "test_func".to_string(),
            total_functions: 1,
            functions_with_unreachable: 0,
            unreachable_count: 0,
            blocks: vec![],
        };
        let unreach_wrapper = JsonResponse::new(unreach_resp);
        assert_eq!(unreach_wrapper.schema_version, "1.0.0");
        assert_eq!(unreach_wrapper.tool, "mirage");

        // VerifyResult wrapped
        let verify_res = VerifyResult {
            path_id: "test_path".to_string(),
            valid: true,
            found_in_cache: true,
            function_id: Some(1),
            reason: "Test".to_string(),
            current_paths: 2,
        };
        let verify_wrapper = JsonResponse::new(verify_res);
        assert_eq!(verify_wrapper.schema_version, "1.0.0");
        assert_eq!(verify_wrapper.tool, "mirage");
    }

    /// Test that to_json() produces compact JSON
    #[test]
    fn test_json_response_compact_format() {
        let data = vec!["item1", "item2"];
        let wrapper = JsonResponse::new(data);
        let compact = wrapper.to_json();

        // Compact JSON should not have unnecessary whitespace
        assert!(!compact.contains("\n"), "Compact JSON should not have newlines");
        assert!(compact.contains("\"item1\""), "Compact JSON should contain data");
    }

    /// Test that to_pretty_json() produces formatted JSON
    #[test]
    fn test_json_response_pretty_format() {
        let data = vec!["item1", "item2"];
        let wrapper = JsonResponse::new(data);
        let pretty = wrapper.to_pretty_json();

        // Pretty JSON should have newlines for formatting
        assert!(pretty.contains("\n"), "Pretty JSON should have newlines");
        assert!(pretty.contains("  "), "Pretty JSON should have indentation");

        // Both formats should produce valid JSON with same data
        let compact = wrapper.to_json();
        let compact_val: serde_json::Value = serde_json::from_str(&compact).unwrap();
        let pretty_val: serde_json::Value = serde_json::from_str(&pretty).unwrap();
        assert_eq!(compact_val, pretty_val, "Both formats should produce same data");
    }

    /// Test that JsonResponse contains required fields
    #[test]
    fn test_json_response_required_fields() {
        let data = "test_data";
        let wrapper = JsonResponse::new(data);

        // Check all required fields exist and have correct values
        assert_eq!(wrapper.schema_version, "1.0.0");
        assert_eq!(wrapper.tool, "mirage");
        assert!(!wrapper.execution_id.is_empty());
        assert!(!wrapper.timestamp.is_empty());

        // Verify execution_id format (should be timestamp-processid)
        assert!(wrapper.execution_id.contains("-"), "execution_id should contain hyphen");

        // Verify timestamp is valid RFC3339 format
        let parsed_time = chrono::DateTime::parse_from_rfc3339(&wrapper.timestamp);
        assert!(parsed_time.is_ok(), "timestamp should be valid RFC3339");
    }

    /// Test that format selection logic works correctly
    #[test]
    fn test_output_format_enum_matches() {
        // Test that all three formats are distinct
        assert_ne!(OutputFormat::Human, OutputFormat::Json);
        assert_ne!(OutputFormat::Human, OutputFormat::Pretty);
        assert_ne!(OutputFormat::Json, OutputFormat::Pretty);

        // Test equality
        assert_eq!(OutputFormat::Human, OutputFormat::Human);
        assert_eq!(OutputFormat::Json, OutputFormat::Json);
        assert_eq!(OutputFormat::Pretty, OutputFormat::Pretty);
    }

    /// Test that human format doesn't contain JSON artifacts
    #[test]
    fn test_human_output_no_json_artifacts() {
        // Human format should print readable text, not JSON
        // This test verifies the pattern: Human output uses println!, not JsonResponse

        let function_name = "test_function";
        let path_count = 5;

        // Simulate human format output
        let mut output = String::new();
        output.push_str(&format!("Function: {}\n", function_name));
        output.push_str(&format!("Total paths: {}\n", path_count));

        // Human output should not contain JSON artifacts
        assert!(!output.contains("{"), "Human output should not contain JSON objects");
        assert!(!output.contains("}"), "Human output should not contain JSON objects");
        assert!(!output.contains("\""), "Human output should not contain JSON quotes");
        assert!(!output.contains("schema_version"), "Human output should not contain JSON metadata");
    }

    /// Test that JSON output contains all expected metadata
    #[test]
    fn test_json_output_has_metadata() {
        let data = "test_data";
        let wrapper = JsonResponse::new(data);
        let json = wrapper.to_json();

        // JSON should contain all metadata fields
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"execution_id\""));
        assert!(json.contains("\"tool\""));
        assert!(json.contains("\"timestamp\""));
        assert!(json.contains("\"data\""));
    }

    /// Test error response format
    #[test]
    fn test_error_response_format() {
        use crate::output::JsonError;

        let error = JsonError::new("category", "message", "CODE");
        assert_eq!(error.error, "category");
        assert_eq!(error.message, "message");
        assert_eq!(error.code, "CODE");
        assert!(error.remediation.is_none());

        let error_with_remediation = error.with_remediation("Try X instead");
        assert_eq!(error_with_remediation.remediation, Some("Try X instead".to_string()));

        // Error response should serialize
        let json = serde_json::to_string(&error_with_remediation);
        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("\"error\""));
        assert!(json_str.contains("\"message\""));
        assert!(json_str.contains("\"code\""));
        assert!(json_str.contains("\"remediation\""));
    }

    /// Test that all CLI struct variants can be created with different output formats
    #[test]
    fn test_cli_with_different_output_formats() {
        let formats = vec![
            OutputFormat::Human,
            OutputFormat::Json,
            OutputFormat::Pretty,
        ];

        for format in formats {
            let cli = Cli {
                db: Some("./test.db".to_string()),
                output: format,
                command: Commands::Status(StatusArgs {}),
            };

            assert_eq!(cli.output, format);
            assert_eq!(cli.db, Some("./test.db".to_string()));
        }
    }

    /// Test CfgFormat enum values
    #[test]
    fn test_cfg_format_enum() {
        let formats = vec![CfgFormat::Human, CfgFormat::Dot, CfgFormat::Json];

        for format in &formats {
            match format {
                CfgFormat::Human => assert!(true),
                CfgFormat::Dot => assert!(true),
                CfgFormat::Json => assert!(true),
            }
        }

        // Test distinctness
        assert_ne!(CfgFormat::Human, CfgFormat::Dot);
        assert_ne!(CfgFormat::Human, CfgFormat::Json);
        assert_ne!(CfgFormat::Dot, CfgFormat::Json);
    }

    /// Test that response field naming follows snake_case convention
    #[test]
    fn test_response_snake_case_naming() {
        // All JSON field names should use snake_case
        let paths_resp = PathsResponse {
            function: "test".to_string(),
            total_paths: 1,
            error_paths: 0,
            paths: vec![],
        };
        let json = serde_json::to_string(&paths_resp).unwrap();

        // Check for snake_case fields
        assert!(json.contains("\"function\""));
        assert!(json.contains("\"total_paths\""));
        assert!(json.contains("\"error_paths\""));

        // Should not have camelCase
        assert!(!json.contains("\"totalPaths\""));
        assert!(!json.contains("\"errorPaths\""));
    }

    /// Test loops command detects natural loops
    #[test]
    fn test_loops_detects_loops() {
        use crate::cfg::{detect_natural_loops, BasicBlock, BlockKind, EdgeType, Terminator};
        use petgraph::graph::DiGraph;

        // Create a simple loop: 0 -> 1 -> 2 -> 1
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 3 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec!["loop body".to_string()],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b3, EdgeType::FalseBranch);
        g.add_edge(b2, b1, EdgeType::LoopBack);

        let loops = detect_natural_loops(&g);

        // Should detect one loop
        assert_eq!(loops.len(), 1, "Should detect exactly one loop");
        assert_eq!(loops[0].header.index(), 1, "Loop header should be block 1");
    }

    /// Test loops command with empty CFG
    #[test]
    fn test_loops_empty_cfg() {
        use crate::cfg::detect_natural_loops;
        use petgraph::graph::DiGraph;
        let empty_cfg: crate::cfg::Cfg = DiGraph::new();
        let loops = detect_natural_loops(&empty_cfg);

        assert!(loops.is_empty(), "Empty CFG should have no loops");
    }

    /// Test loops response serialization
    #[test]
    fn test_loops_response_serialization() {
        use crate::output::JsonResponse;

        let response = LoopsResponse {
            function: "test_func".to_string(),
            loop_count: 2,
            loops: vec![
                LoopInfo {
                    header: 1,
                    back_edge_from: 2,
                    body_size: 2,
                    nesting_level: 0,
                    body_blocks: vec![1, 2],
                },
                LoopInfo {
                    header: 3,
                    back_edge_from: 4,
                    body_size: 3,
                    nesting_level: 1,
                    body_blocks: vec![1, 2, 3],
                },
            ],
        };

        // Should serialize without errors
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"function\""));
        assert!(json.contains("\"loop_count\""));
        assert!(json.contains("\"loops\""));

        // Test with JsonResponse wrapper
        let wrapper = JsonResponse::new(response);
        let wrapped_json = wrapper.to_json();
        assert!(wrapped_json.contains("\"schema_version\""));
        assert!(wrapped_json.contains("\"execution_id\""));
    }

    /// Test LoopsArgs struct fields
    #[test]
    fn test_loops_args_fields() {
        let args = LoopsArgs {
            function: "my_function".to_string(),
            verbose: true,
        };

        assert_eq!(args.function, "my_function");
        assert!(args.verbose);
    }

    /// Test LoopInfo struct fields
    #[test]
    fn test_loop_info_fields() {
        let loop_info = LoopInfo {
            header: 5,
            back_edge_from: 7,
            body_size: 3,
            nesting_level: 2,
            body_blocks: vec![5, 6, 7],
        };

        assert_eq!(loop_info.header, 5);
        assert_eq!(loop_info.back_edge_from, 7);
        assert_eq!(loop_info.body_size, 3);
        assert_eq!(loop_info.nesting_level, 2);
        assert_eq!(loop_info.body_blocks, vec![5, 6, 7]);
    }

    /// Test loops command with json output format
    #[test]
    fn test_loops_json_output_format() {
        use crate::output::JsonResponse;

        let response = LoopsResponse {
            function: "json_test".to_string(),
            loop_count: 1,
            loops: vec![LoopInfo {
                header: 1,
                back_edge_from: 2,
                body_size: 2,
                nesting_level: 0,
                body_blocks: vec![1, 2],
            }],
        };

        let wrapper = JsonResponse::new(response);
        let json = wrapper.to_json();

        // Verify JSON structure
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"execution_id\""));
        assert!(json.contains("\"tool\""));
        assert!(json.contains("\"timestamp\""));
        assert!(json.contains("\"data\""));
    }

    /// Test loops command with verbose flag
    #[test]
    fn test_loops_verbose_flag() {
        let args_verbose = LoopsArgs {
            function: "test".to_string(),
            verbose: true,
        };

        let args_not_verbose = LoopsArgs {
            function: "test".to_string(),
            verbose: false,
        };

        assert!(args_verbose.verbose);
        assert!(!args_not_verbose.verbose);
    }

    /// Test loops nesting level calculation
    #[test]
    fn test_loops_nesting_levels() {
        let loop_outer = LoopInfo {
            header: 1,
            back_edge_from: 3,
            body_size: 3,
            nesting_level: 0, // Outermost
            body_blocks: vec![1, 2, 3],
        };

        let loop_inner = LoopInfo {
            header: 2,
            back_edge_from: 4,
            body_size: 2,
            nesting_level: 1, // Nested inside outer
            body_blocks: vec![2, 4],
        };

        assert_eq!(loop_outer.nesting_level, 0);
        assert_eq!(loop_inner.nesting_level, 1);
    }

    /// Test loops response with no loops
    #[test]
    fn test_loops_response_empty() {
        use crate::output::JsonResponse;

        let response = LoopsResponse {
            function: "no_loops_func".to_string(),
            loop_count: 0,
            loops: vec![],
        };

        let wrapper = JsonResponse::new(response);
        let json = wrapper.to_json();

        // Should handle empty loops gracefully
        assert!(json.contains("\"loop_count\":0"));
        assert!(json.contains("\"loops\":[]"));
    }

    /// Test patterns command with if/else detection
    #[test]
    fn test_patterns_if_else_detection() {
        use crate::cfg::{detect_if_else_patterns, detect_match_patterns};

        let cfg = cmds::create_test_cfg();

        // Detect patterns
        let if_else_patterns = detect_if_else_patterns(&cfg);
        let match_patterns = detect_match_patterns(&cfg);

        // Test CFG has a simple if/else (block 1 -> blocks 2 and 3)
        // This is a diamond pattern, so it should be detected
        assert!(!if_else_patterns.is_empty(), "Should detect if/else pattern");

        // Check pattern structure
        let pattern = &if_else_patterns[0];
        assert_eq!(cfg[pattern.condition].id, 1);
        assert_eq!(cfg[pattern.true_branch].id, 2);
        assert_eq!(cfg[pattern.false_branch].id, 3);

        // Our test CFG doesn't have a match statement
        assert!(match_patterns.is_empty(), "Should not detect match patterns in simple if/else");
    }

    /// Test patterns command with --if-else filter
    #[test]
    fn test_patterns_if_else_filter() {
        // Test argument parsing - command structure is correct
        let args = PatternsArgs {
            function: "test_func".to_string(),
            if_else: true,
            r#match: false,
        };

        // Verify args are parsed correctly
        assert!(args.if_else);
        assert!(!args.r#match);
        assert_eq!(args.function, "test_func");
    }

    /// Test patterns command with --match filter
    #[test]
    fn test_patterns_match_filter() {
        // Test argument parsing - command structure is correct
        let args = PatternsArgs {
            function: "test_func".to_string(),
            if_else: false,
            r#match: true,
        };

        // Verify args are parsed correctly
        assert!(!args.if_else);
        assert!(args.r#match);
        assert_eq!(args.function, "test_func");
    }

    /// Test patterns command with JSON output
    #[test]
    fn test_patterns_json_output() {
        // Test argument parsing - command structure is correct
        let args = PatternsArgs {
            function: "test_func".to_string(),
            if_else: false,
            r#match: false,
        };

        let cli = Cli {
            db: None,
            output: OutputFormat::Json,
            command: Commands::Patterns(args.clone()),
        };

        // Verify CLI structure
        assert!(matches!(cli.output, OutputFormat::Json));
    }

    /// Test patterns response struct serialization
    #[test]
    fn test_patterns_response_serialization() {
        let response = PatternsResponse {
            function: "test_func".to_string(),
            if_else_count: 1,
            match_count: 0,
            if_else_patterns: vec![IfElseInfo {
                condition_block: 1,
                true_branch: 2,
                false_branch: 3,
                merge_point: Some(4),
                has_else: true,
            }],
            match_patterns: vec![],
        };

        // Should serialize to JSON
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"function\""));
        assert!(json.contains("\"if_else_count\""));
        assert!(json.contains("\"match_count\""));

        // Check snake_case naming
        assert!(json.contains("\"if_else_patterns\""));
        assert!(json.contains("\"condition_block\""));
        assert!(json.contains("\"merge_point\""));
    }
}

// ============================================================================
// frontiers() Command Tests
// ============================================================================

#[cfg(test)]
mod frontiers_tests {
    use super::*;
    use crate::cfg::{compute_dominance_frontiers, DominatorTree};
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
                id INTEGER PRIMARY KEY,
                type TEXT NOT NULL,
                name TEXT,
                source_file TEXT
            )",
            [],
        )?;

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, strftime('%s', 'now'))",
            [REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION],
        )?;

        Ok(file)
    }

    /// Test frontiers response struct serialization
    #[test]
    fn test_frontiers_response_serialization() {
        use crate::output::JsonResponse;

        let response = FrontiersResponse {
            function: "test_func".to_string(),
            nodes_with_frontiers: 2,
            frontiers: vec![
                NodeFrontier {
                    node: 1,
                    frontier_set: vec![3],
                },
                NodeFrontier {
                    node: 2,
                    frontier_set: vec![3],
                },
            ],
        };

        let wrapper = JsonResponse::new(response);
        let json = wrapper.to_json();

        // Verify JSON structure
        assert!(json.contains("\"function\":\"test_func\""));
        assert!(json.contains("\"nodes_with_frontiers\":2"));
        assert!(json.contains("\"frontiers\":["));
    }

    /// Test iterated frontier response struct serialization
    #[test]
    fn test_iterated_frontier_response_serialization() {
        use crate::output::JsonResponse;

        let response = IteratedFrontierResponse {
            function: "test_func".to_string(),
            iterated_frontier: vec![3, 4],
        };

        let wrapper = JsonResponse::new(response);
        let json = wrapper.to_json();

        // Verify JSON structure
        assert!(json.contains("\"function\":\"test_func\""));
        assert!(json.contains("\"iterated_frontier\":[3,4]"));
    }

    /// Test basic frontier computation (diamond CFG)
    #[test]
    fn test_frontiers_basic() {
        use crate::cfg::{BasicBlock, BlockKind, Terminator, EdgeType};
        use petgraph::graph::DiGraph;

        // Create diamond CFG: 0 -> 1,2 -> 3
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![1], otherwise: 2 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec!["branch 1".to_string()],
            terminator: Terminator::Goto { target: 3 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec!["branch 2".to_string()],
            terminator: Terminator::Goto { target: 3 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::TrueBranch);
        g.add_edge(b0, b2, EdgeType::FalseBranch);
        g.add_edge(b1, b3, EdgeType::Fallthrough);
        g.add_edge(b2, b3, EdgeType::Fallthrough);

        // Compute dominance frontiers
        let dom_tree = DominatorTree::new(&g).expect("CFG has entry");
        let frontiers = compute_dominance_frontiers(&g, dom_tree);

        // In diamond CFG:
        // DF[1] = {3} (1 dominates itself, pred of 3, doesn't strictly dominate 3)
        // DF[2] = {3} (2 dominates itself, pred of 3, doesn't strictly dominate 3)
        let df1 = frontiers.frontier(b1);
        assert!(df1.contains(&b3));
        assert_eq!(df1.len(), 1);

        let df2 = frontiers.frontier(b2);
        assert!(df2.contains(&b3));
        assert_eq!(df2.len(), 1);

        // Entry (0) has empty frontier (strictly dominates all nodes)
        let df0 = frontiers.frontier(b0);
        assert!(df0.is_empty());
    }

    /// Test --iterated flag functionality
    #[test]
    fn test_frontiers_iterated_flag() {
        let args = FrontiersArgs {
            function: "test_func".to_string(),
            iterated: true,
            node: None,
        };

        assert!(args.iterated);
        assert!(args.node.is_none());
    }

    /// Test --node flag functionality
    #[test]
    fn test_frontiers_node_flag() {
        let args = FrontiersArgs {
            function: "test_func".to_string(),
            iterated: false,
            node: Some(5),
        };

        assert!(!args.iterated);
        assert_eq!(args.node, Some(5));
    }

    /// Test frontiers with linear CFG (empty frontiers)
    #[test]
    fn test_frontiers_linear_cfg() {
        use crate::cfg::{BasicBlock, BlockKind, Terminator, EdgeType};
        use petgraph::graph::DiGraph;

        // Linear CFG: 0 -> 1 -> 2 -> 3
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 3 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::Fallthrough);
        g.add_edge(b2, b3, EdgeType::Fallthrough);

        // Compute dominance frontiers
        let dom_tree = DominatorTree::new(&g).expect("CFG has entry");
        let frontiers = compute_dominance_frontiers(&g, dom_tree);

        // Linear CFG has no dominance frontiers (no join points)
        let nodes_with_frontiers: Vec<_> = frontiers.nodes_with_frontiers().collect();
        assert!(nodes_with_frontiers.is_empty());
    }

    /// Test frontiers with loop CFG (self-frontier)
    #[test]
    fn test_frontiers_loop_cfg() {
        use crate::cfg::{BasicBlock, BlockKind, Terminator, EdgeType};
        use petgraph::graph::DiGraph;

        // Loop CFG: 0 -> 1 <-> 2 (back edge), 1 -> 3 (exit)
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 3 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec!["loop body".to_string()],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b3, EdgeType::FalseBranch);
        g.add_edge(b2, b1, EdgeType::LoopBack);

        // Compute dominance frontiers
        let dom_tree = DominatorTree::new(&g).expect("CFG has entry");
        let frontiers = compute_dominance_frontiers(&g, dom_tree);

        // Loop header (1) should have self-frontier due to back edge
        let df1 = frontiers.frontier(b1);
        assert!(df1.contains(&b1), "Loop header should have self-frontier");
    }

    /// Test frontiers command with json output format
    #[test]
    fn test_frontiers_json_output_format() {
        use crate::output::JsonResponse;

        let response = FrontiersResponse {
            function: "json_test".to_string(),
            nodes_with_frontiers: 2,
            frontiers: vec![
                NodeFrontier {
                    node: 1,
                    frontier_set: vec![3],
                },
                NodeFrontier {
                    node: 2,
                    frontier_set: vec![3],
                },
            ],
        };

        let wrapper = JsonResponse::new(response);
        let json = wrapper.to_json();

        // Verify JSON structure with metadata
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"execution_id\""));
        assert!(json.contains("\"tool\""));
        assert!(json.contains("\"timestamp\""));
        assert!(json.contains("\"data\""));
    }

    /// Test frontiers response with empty frontiers
    #[test]
    fn test_frontiers_response_empty() {
        use crate::output::JsonResponse;

        let response = FrontiersResponse {
            function: "linear_func".to_string(),
            nodes_with_frontiers: 0,
            frontiers: vec![],
        };

        let wrapper = JsonResponse::new(response);
        let json = wrapper.to_json();

        // Should handle empty frontiers gracefully
        assert!(json.contains("\"nodes_with_frontiers\":0"));
        assert!(json.contains("\"frontiers\":[]"));
    }
}
