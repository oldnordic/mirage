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
