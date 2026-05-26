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
#[command(
    long_about = "Mirage is a path-aware code intelligence engine that operates on graphs, not text.

It materializes behavior explicitly: paths, proofs, counterexamples.

NOT:
  - A search tool (llmgrep already does this)
  - An embedding tool
  - Static analysis / linting

IS:
  - Path enumeration and verification
  - Graph-based reasoning about code behavior
  - Truth engine that materializes facts for LLM consumption

The Golden Rule: An agent may only speak if it can reference a graph artifact."
)]
pub struct Cli {
    /// Path to the Magellan/Mirage database
    #[arg(global = true, long, env = "MIRAGE_DB")]
    pub db: Option<String>,

    /// Output format
    #[arg(global = true, long, value_enum, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,

    /// Detect and report backend format (sqlite or geometric)
    #[arg(long, global = true, default_value = "false")]
    pub detect_backend: bool,

    /// Record command telemetry (opt-in, local only)
    #[arg(long, global = true, default_value = "false")]
    pub record: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
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

    /// Show cycles in code (call graph SCCs and function loops)
    Cycles(CyclesArgs),

    /// Perform program slicing (backward/forward impact analysis)
    Slice(SliceArgs),

    /// Show high-risk functions (hotspots)
    Hotspots(HotspotsArgs),

    /// Show most-traversed execution paths (hot paths)
    Hotpaths(HotpathsArgs),

    /// Show CFG differences between two snapshots
    Diff(DiffArgs),

    /// Show inter-procedural CFG (combined function CFGs with call/return edges)
    Icfg(IcfgArgs),

    /// Show per-block coverage for a function
    Coverage(CoverageArgs),

    /// Migrate database between storage backends
    Migrate(MigrateArgs),

    /// List source documents from graph memory
    Docs(DocsArgs),

    /// Compute risk score for a function
    Risk(RiskArgs),

    /// Suggest refactoring actions for a symbol
    Suggest(SuggestArgs),

    /// Show code statistics from the database
    Stats(StatsArgs),
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

    /// File path to disambiguate functions with same name (optional)
    #[arg(long)]
    pub file: Option<String>,

    /// Show only error paths
    #[arg(long)]
    pub show_errors: bool,

    /// Maximum path length (for pruning)
    #[arg(long)]
    pub max_length: Option<usize>,

    /// Show block details for each path
    #[arg(long)]
    pub with_blocks: bool,

    /// Incremental mode: analyze only changed functions since git revision
    #[arg(long)]
    pub incremental: bool,

    /// Git revision for incremental analysis (e.g., "HEAD~1")
    #[arg(long)]
    pub since: Option<String>,

    /// Sort paths by coverage hit count (highest first)
    #[arg(long)]
    pub by_coverage: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct CfgArgs {
    /// Function symbol ID or name
    #[arg(long)]
    pub function: String,

    /// File path to disambiguate functions with same name (optional)
    #[arg(long)]
    pub file: Option<String>,

    /// Output format
    #[arg(long, value_enum)]
    pub format: Option<CfgFormat>,
}

#[derive(Parser, Debug, Clone)]
pub struct CoverageArgs {
    /// Function symbol ID or name
    #[arg(long)]
    pub function: String,

    /// File path to disambiguate functions with same name (optional)
    #[arg(long)]
    pub file: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct DominatorsArgs {
    /// Function symbol ID or name
    #[arg(long)]
    pub function: String,

    /// File path to disambiguate functions with same name (optional)
    #[arg(long)]
    pub file: Option<String>,

    /// Show blocks that must pass through this block
    #[arg(long)]
    pub must_pass_through: Option<String>,

    /// Show post-dominators instead of dominators
    #[arg(long)]
    pub post: bool,

    /// Use inter-procedural (call graph) dominance instead of intra-procedural (CFG)
    #[arg(long)]
    pub inter_procedural: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct LoopsArgs {
    /// Function to analyze for loops
    #[arg(long)]
    pub function: String,

    /// File path to disambiguate functions with same name (optional)
    #[arg(long)]
    pub file: Option<String>,

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

    /// File path to disambiguate functions with same name (optional)
    #[arg(long)]
    pub file: Option<String>,

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

    /// File path to disambiguate functions with same name (optional)
    #[arg(long)]
    pub file: Option<String>,

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

    /// File path to disambiguate functions with same name (optional)
    #[arg(long)]
    pub file: Option<String>,

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

    /// Use call graph for inter-procedural impact analysis
    #[arg(long)]
    pub use_call_graph: bool,
}

/// Cycle type filter for the cycles command
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleTypeArg {
    /// Show all cycles (default)
    All,
    /// Show only inter-function cycles (mutual recursion, size > 1)
    InterFunction,
    /// Show only self-loops (single recursive function)
    SelfLoop,
}

#[derive(Parser, Debug, Clone)]
pub struct CyclesArgs {
    /// Show call graph cycles (mutual recursion between functions)
    #[arg(long)]
    pub call_graph: bool,

    /// Show function loops (within individual functions)
    #[arg(long)]
    pub function_loops: bool,

    /// Show both types of cycles (default)
    #[arg(long)]
    pub both: bool,

    /// Filter cycle type: all, inter-function, or self-loop
    #[arg(long, value_enum, default_value = "all")]
    pub cycle_type: CycleTypeArg,

    /// Verbose output (show cycle members/loop bodies)
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct SliceArgs {
    /// Symbol ID or FQN to slice
    #[arg(long)]
    pub symbol: String,

    /// Slice direction: backward (what affects) or forward (what affects)
    #[arg(long, value_enum)]
    pub direction: SliceDirectionArg,

    /// Show detailed symbol information
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct HotspotsArgs {
    /// Entry point symbol (default: main)
    #[arg(long, default_value = "main")]
    pub entry: String,

    /// Maximum number of hotspots to return
    #[arg(long, default_value = "20")]
    pub top: usize,

    /// Minimum path count threshold
    #[arg(long)]
    pub min_paths: Option<usize>,

    /// Show detailed metrics for each hotspot
    #[arg(long)]
    pub verbose: bool,

    /// Use inter-procedural analysis (requires Magellan DB)
    /// Enabled by default. Use --intra-procedural to force intra-procedural analysis.
    #[arg(long, default_value = "true")]
    pub inter_procedural: bool,

    /// Use intra-procedural analysis only (faster, but may show 0 functions if cfg_blocks not populated)
    #[arg(long, conflicts_with = "inter_procedural")]
    pub intra_procedural: bool,
}

/// Hot path detection arguments
#[derive(Parser, Debug, Clone)]
pub struct HotpathsArgs {
    /// Function symbol ID or name
    #[arg(long)]
    pub function: String,

    /// Number of hot paths to return (default: 10)
    #[arg(long, default_value = "10")]
    pub top: usize,

    /// Show rationale for hotness scores
    #[arg(long)]
    pub rationale: bool,

    /// Minimum hotness threshold (0.0 to 1.0)
    #[arg(long)]
    pub min_score: Option<f64>,
}

/// Migrate database between storage backends
#[derive(Parser, Debug, Clone)]
pub struct MigrateArgs {
    /// Source backend format
    #[arg(long, value_enum)]
    pub from: BackendFormat,

    /// Target backend format
    #[arg(long, value_enum)]
    pub to: BackendFormat,

    /// Database path to migrate
    #[arg(short, long)]
    pub db: String,

    /// Create backup before migration
    #[arg(long)]
    pub backup: bool,

    /// Dry run: detect format only without migrating
    #[arg(long)]
    pub dry_run: bool,
}

/// Source documents listing arguments
#[derive(Parser, Debug, Clone)]
pub struct DocsArgs {
    /// Filter by source kind (wiki, code, message, etc.)
    #[arg(long)]
    pub kind: Option<String>,

    /// Filter by tag
    #[arg(long)]
    pub tag: Option<String>,

    /// Maximum number of results
    #[arg(long, default_value = "50")]
    pub limit: usize,
}

/// Risk analysis arguments
#[derive(Parser, Debug, Clone)]
pub struct RiskArgs {
    /// Function symbol ID or name
    #[arg(long)]
    pub function: String,

    /// File path to disambiguate functions with same name (optional)
    #[arg(long)]
    pub file: Option<String>,
}

/// Suggest refactoring arguments
#[derive(Parser, Debug, Clone)]
pub struct SuggestArgs {
    /// Symbol ID or name to analyze
    #[arg(long)]
    pub symbol: String,

    /// File path to disambiguate (optional)
    #[arg(long)]
    pub file: Option<String>,
}

/// Code statistics arguments
#[derive(Parser, Debug, Clone, Copy)]
pub struct StatsArgs {}

/// Inter-procedural CFG arguments
#[derive(Parser, Debug, Clone)]
pub struct IcfgArgs {
    /// Entry function symbol ID or name
    #[arg(long)]
    pub entry: String,

    /// Maximum depth for call graph traversal (default: 3)
    #[arg(long, default_value = "3")]
    pub depth: usize,

    /// Include return edges (default: true)
    #[arg(long, default_value = "true")]
    pub return_edges: bool,

    /// Output format
    #[arg(long, value_enum)]
    pub format: Option<IcfgFormat>,
}

/// ICFG output format
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcfgFormat {
    /// DOT graph format (for graphviz)
    Dot,
    /// JSON format
    Json,
    /// Human-readable summary
    Human,
}

/// Diff command arguments
#[derive(Parser, Debug, Clone)]
pub struct DiffArgs {
    /// Function symbol ID or name to compare
    #[arg(long)]
    pub function: String,

    /// Before snapshot ID (transaction ID or "current")
    #[arg(long)]
    pub before: String,

    /// After snapshot ID (transaction ID or "current")
    #[arg(long)]
    pub after: String,

    /// Show edge differences
    #[arg(long)]
    pub show_edges: bool,

    /// Show detailed block changes
    #[arg(long)]
    pub verbose: bool,
}

/// Backend format for migration
#[derive(clap::ValueEnum, Clone, Debug, Copy, PartialEq, Eq)]
pub enum BackendFormat {
    /// SQLite database (traditional backend)
    Sqlite,
    /// Geometric database (.geo files, Magellan 3.0+)
    Geometric,
}

impl std::fmt::Display for BackendFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite => write!(f, "sqlite"),
            Self::Geometric => write!(f, "geometric"),
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceDirectionArg {
    /// Backward: what affects this symbol
    Backward,
    /// Forward: what this symbol affects
    Forward,
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
/// Priority: CLI arg > MIRAGE_DB env var > auto-discover in common locations
/// Auto-discovery searches: .magellan/*.db, .forge/*.db, *.db in current directory
pub fn resolve_db_path(cli_db: Option<String>) -> anyhow::Result<String> {
    if let Some(path) = cli_db {
        return Ok(path);
    }

    // Try environment variable
    if let Ok(path) = std::env::var("MIRAGE_DB") {
        return Ok(path);
    }

    // Auto-discover database in common locations
    if let Some(path) = auto_discover_db() {
        eprintln!("Info: Auto-discovered database at {}", path);
        return Ok(path);
    }

    Err(anyhow::anyhow!(
        "No database specified. Use --db, set MIRAGE_DB env var, \
         or run from a directory with a .db file"
    ))
}

/// Auto-discover database file in common locations
///
/// Searches in priority order:
/// 1. .magellan/*.db files (Magellan's conventional location)
/// 2. .forge/*.db files
/// 3. *.db in current directory
/// 4. mirage.db or magellan.db in current directory
fn auto_discover_db() -> Option<String> {
    use std::path::Path;

    // Search directories in priority order
    let search_dirs = [".magellan", ".forge", "."];

    for dir in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut db_files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let path = e.path();
                    path.extension().map(|ext| ext == "db").unwrap_or(false)
                })
                .map(|e| e.path())
                .collect();

            // Sort for deterministic results
            db_files.sort();

            // Return first match, preferring current Magellan/Mirage database names.
            if let Some(preferred) = db_files.iter().find(|p| {
                let name = p
                    .file_stem()
                    .map(|s| s.to_string_lossy())
                    .unwrap_or_default();
                name == "magellan" || name == "mirage"
            }) {
                return Some(preferred.to_string_lossy().to_string());
            }

            // Otherwise return first .db file
            if let Some(first) = db_files.first() {
                return Some(first.to_string_lossy().to_string());
            }
        }
    }

    // Check for specific filenames in current directory
    let candidates = [
        ".magellan/mirage.db",
        ".magellan/magellan.db",
        "mirage.db",
        "magellan.db",
        "graph.db",
    ];
    for name in &candidates {
        if Path::new(name).exists() {
            return Some(name.to_string());
        }
    }

    None
}

/// Detect the git repository path from the database path
///
/// Starts from the db path and searches upward for .git directory.
/// Falls back to current directory if not found.
fn detect_repo_path(db_path: &str) -> std::path::PathBuf {
    use std::path::Path;

    let db_path = Path::new(db_path);

    // Start from db path and search up for .git directory
    let mut path = if db_path.is_absolute() {
        db_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(db_path))
            .unwrap_or_else(|_| db_path.to_path_buf())
    };

    // Search up the directory tree
    while path.pop() {
        let git_dir = path.join(".git");
        if git_dir.exists() {
            return path;
        }
    }

    // Fallback to current directory
    Path::new(".").to_path_buf()
}

pub mod cmds;
pub mod responses;
pub mod tests;
