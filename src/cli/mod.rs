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

    pub fn paths(_args: PathsArgs) -> Result<()> {
        // TODO: Implement M3 (Path Enumeration)
        output::error("Path enumeration not yet implemented (Milestone 3)");
        std::process::exit(1);
    }

    pub fn cfg(_args: CfgArgs) -> Result<()> {
        // TODO: Implement M2 (CFG Construction)
        output::error("CFG visualization not yet implemented (Milestone 2)");
        std::process::exit(1);
    }

    pub fn dominators(_args: DominatorsArgs) -> Result<()> {
        // TODO: Implement M4 (Dominance Analysis)
        output::error("Dominance analysis not yet implemented (Milestone 4)");
        std::process::exit(1);
    }

    pub fn unreachable(_args: UnreachableArgs) -> Result<()> {
        // TODO: Implement M5 (Advanced Analysis)
        output::error("Unreachable code detection not yet implemented (Milestone 5)");
        std::process::exit(1);
    }

    pub fn verify(_args: VerifyArgs) -> Result<()> {
        // TODO: Implement path verification
        output::error("Path verification not yet implemented");
        std::process::exit(1);
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
