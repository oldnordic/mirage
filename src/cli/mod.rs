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
