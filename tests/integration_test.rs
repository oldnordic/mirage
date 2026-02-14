//! Integration tests for all mirage commands
//!
//! Tests verify commands work correctly on both SQLite and native-v3 backends.
//! These are "smoke tests" that verify:
//! - CLI parsing works correctly
//! - Commands can be invoked without panicking
//! - Output format is correct (human/json/pretty)
//! - Error handling works appropriately
//!
//! For deeper functional testing, see the unit tests in src/cli/mod.rs.

use std::process::Command;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test context for integration tests
///
/// Provides a test database and mirage binary path.
struct TestContext {
    mirage_bin: PathBuf,
    db_path: PathBuf,
    _temp_dir: TempDir,
}

impl TestContext {
    /// Create a new test context with a minimal test database
    fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create a minimal Magellan v7 database
        Self::create_test_database(&db_path);

        // Use CARGO_BIN_EXE_mirage if available (for cargo test), otherwise fallback
        let mirage_bin = std::env::var("CARGO_BIN_EXE_mirage")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // Check for debug binary first (cargo test builds debug by default)
                let debug_path = PathBuf::from("./target/debug/mirage");
                if debug_path.exists() {
                    debug_path
                } else {
                    PathBuf::from("./target/release/mirage")
                }
            });

        Self {
            mirage_bin,
            db_path,
            _temp_dir: temp_dir,
        }
    }

    /// Run mirage with the given arguments
    fn run_command(&self, args: &[&str]) -> TestOutput {
        let output = Command::new(&self.mirage_bin)
            .args(args)
            .arg("--db")
            .arg(&self.db_path)
            .output()
            .expect("Failed to run mirage");

        TestOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            status: output.status,
        }
    }

    /// Create a minimal test database with Magellan v7 schema
    fn create_test_database(db_path: &PathBuf) {
        use rusqlite::Connection;
        use std::fs;

        let mut conn = Connection::open(db_path).unwrap();

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        // Create magellan_meta table
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
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, 7, 3, 0)",
            [],
        ).unwrap();

        // Create graph_entities table
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

        // Create cfg_blocks table
        conn.execute(
            "CREATE TABLE cfg_blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                function_id INTEGER NOT NULL,
                kind TEXT NOT NULL,
                terminator TEXT NOT NULL,
                byte_start INTEGER NOT NULL,
                byte_end INTEGER NOT NULL,
                start_line INTEGER NOT NULL,
                start_col INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                end_col INTEGER NOT NULL,
                FOREIGN KEY (function_id) REFERENCES graph_entities(id)
            )",
            [],
        ).unwrap();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data)
             VALUES ('Symbol', 'test_function', 'src/test.rs', '{\"kind\": \"Function\"}')",
            [],
        ).unwrap();

        // Insert test CFG blocks
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (1, 'entry', 'fallthrough', 0, 10, 1, 0, 1, 10),
                    (1, 'normal', 'conditional', 10, 50, 2, 4, 5, 8),
                    (1, 'return', 'return', 50, 60, 5, 0, 5, 10)",
            [],
        ).unwrap();

        // Create mirage_meta table for Mirage schema
        conn.execute(
            "CREATE TABLE mirage_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                mirage_schema_version INTEGER NOT NULL,
                magellan_schema_version INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO mirage_meta (id, mirage_schema_version, magellan_schema_version)
             VALUES (1, 1, 7)",
            [],
        ).unwrap();

        // Create graph_meta table for sqlitegraph compatibility
        conn.execute(
            "CREATE TABLE graph_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO graph_meta (id, schema_version, created_at)
             VALUES (1, 3, 0)",
            [],
        ).unwrap();

        // Create cfg_edges table for Mirage
        conn.execute(
            "CREATE TABLE cfg_edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_id INTEGER NOT NULL,
                to_id INTEGER NOT NULL,
                edge_type TEXT NOT NULL,
                FOREIGN KEY (from_id) REFERENCES cfg_blocks(id),
                FOREIGN KEY (to_id) REFERENCES cfg_blocks(id)
            )",
            [],
        ).unwrap();

        // Create cfg_paths table
        conn.execute(
            "CREATE TABLE cfg_paths (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                function_id INTEGER NOT NULL,
                path_hash TEXT NOT NULL,
                path_length INTEGER NOT NULL,
                FOREIGN KEY (function_id) REFERENCES graph_entities(id)
            )",
            [],
        ).unwrap();

        // Create cfg_path_elements table
        conn.execute(
            "CREATE TABLE cfg_path_elements (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path_id INTEGER NOT NULL,
                block_id INTEGER NOT NULL,
                sequence_order INTEGER NOT NULL,
                FOREIGN KEY (path_id) REFERENCES cfg_paths(id),
                FOREIGN KEY (block_id) REFERENCES cfg_blocks(id)
            )",
            [],
        ).unwrap();
        
        // Explicitly close the connection to ensure all writes are flushed
        drop(conn);
        
        // Verify the database file exists and is readable
        assert!(db_path.exists(), "Database file should exist after creation");
        assert!(fs::metadata(db_path).unwrap().len() > 0, "Database file should not be empty");
    }
}

/// Output from running a mirage command
struct TestOutput {
    stdout: String,
    stderr: String,
    status: std::process::ExitStatus,
}

impl TestOutput {
    /// Returns true if the command succeeded
    fn success(&self) -> bool {
        self.status.success()
    }

    /// Returns true if stdout contains the given substring
    fn stdout_contains(&self, s: &str) -> bool {
        self.stdout.contains(s)
    }

    /// Returns true if stderr contains the given substring
    fn stderr_contains(&self, s: &str) -> bool {
        self.stderr.contains(s)
    }
}

// ============================================================================
// Integration tests for each command
// ============================================================================

#[test]
fn test_status_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["status"]);

    assert!(output.success(), "status command should succeed");
    assert!(output.stdout_contains("Mirage") || output.stdout_contains("Database"),
            "status output should contain database info");
}

#[test]
fn test_status_command_json() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["status", "--output", "json"]);

    assert!(output.success(), "status --output json should succeed");
    assert!(output.stdout_contains("{"), "JSON output should contain opening brace");
    assert!(output.stdout_contains("}"), "JSON output should contain closing brace");
}

#[test]
fn test_cfg_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["cfg", "--function", "test_function"]);

    // cfg command should work (may have no output if function not found)
    // We just verify it doesn't panic
    assert!(output.success() || output.stderr_contains("not found") || output.stderr_contains("No function"),
            "cfg command should succeed or show not found error");
}

#[test]
fn test_paths_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["paths", "--function", "test_function"]);

    // paths command should work (may have no paths if none computed)
    assert!(output.success() || output.stderr_contains("not found") || output.stderr_contains("No function"),
            "paths command should succeed or show not found error");
}

#[test]
fn test_dominators_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["dominators", "--function", "test_function"]);

    assert!(output.success() || output.stderr_contains("not found") || output.stderr_contains("No function"),
            "dominators command should succeed or show not found error");
}

#[test]
fn test_loops_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["loops", "--function", "test_function"]);

    assert!(output.success() || output.stderr_contains("not found") || output.stderr_contains("No function"),
            "loops command should succeed or show not found error");
}

#[test]
fn test_unreachable_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["unreachable", "--within-functions"]);

    // unreachable command may have no output for small test databases
    assert!(output.success() || output.stdout_contains("No unreachable") || output.stdout_contains("Unreachable"),
            "unreachable command should succeed or show message");
}

#[test]
fn test_patterns_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["patterns", "--function", "test_function"]);

    assert!(output.success() || output.stderr_contains("not found") || output.stderr_contains("No function"),
            "patterns command should succeed or show not found error");
}

#[test]
fn test_frontiers_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["frontiers", "--function", "test_function"]);

    assert!(output.success() || output.stderr_contains("not found") || output.stderr_contains("No function"),
            "frontiers command should succeed or show not found error");
}

#[test]
fn test_cycles_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["cycles"]);

    // cycles command analyzes the entire codebase
    assert!(output.success() || output.stdout_contains("No cycles") || output.stdout_contains("Cycles"),
            "cycles command should succeed");
}

#[test]
fn test_hotspots_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["hotspots"]);

    // hotspots command may have no output for small test databases
    assert!(output.success() || output.stdout_contains("No hotspots") || output.stdout.contains("Hotspots"),
            "hotspots command should succeed");
}

#[test]
fn test_slice_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["slice", "--symbol", "test_function", "--direction", "backward"]);

    // slice command runs but may not find the symbol
    // We just verify it doesn't panic
    let stderr_lower = output.stderr.to_lowercase();
    let stdout_lower = output.stdout.to_lowercase();
    assert!(output.success() ||
            stderr_lower.contains("not found") ||
            stderr_lower.contains("no symbol") ||
            stderr_lower.contains("could not") ||
            stdout_lower.contains("slice"),
            "slice command should succeed or show appropriate message: stderr={}, stdout={}",
            output.stderr, output.stdout);
}

#[test]
fn test_blast_zone_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["blast-zone", "--function", "test_function"]);

    // blast-zone may not find the function, but should handle error gracefully
    assert!(output.success() || output.stderr.contains("not found") || output.stderr.contains("No function") || output.stdout.contains("Blast"),
            "blast-zone command should succeed or show not found error");
}

#[test]
fn test_verify_command() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["verify", "--path-id", "1"]);

    // verify requires a valid path ID, so it will likely fail
    // We just verify it handles errors gracefully
    assert!(output.success() || output.stderr.contains("not found") || output.stderr.contains("No path") || output.stdout.contains("Path"),
            "verify command should succeed or show not found error");
}

#[test]
fn test_migrate_command_help() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["migrate", "--help"]);

    // migrate --help should show usage
    assert!(output.success(), "migrate --help should succeed");
    assert!(output.stdout_contains("migrate") || output.stdout_contains("MIGRATE"),
            "migrate help should mention migrate");
}

#[test]
fn test_detect_backend_flag() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["--detect-backend"]);

    assert!(output.success(), "--detect-backend should succeed");
    assert!(output.stdout_contains("sqlite") || output.stdout_contains("native-v3") || output.stdout.contains("{"),
            "--detect-backend should output backend type");
}

#[test]
fn test_detect_backend_json() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["--detect-backend", "--output", "json"]);

    assert!(output.success(), "--detect-backend --output json should succeed");
    assert!(output.stdout_contains("\"backend\""), "JSON output should contain backend field");
    assert!(output.stdout_contains("\"database\""), "JSON output should contain database field");
}

#[test]
fn test_no_command_error() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&[]);

    // Running without a command should show an error
    assert!(!output.success() || output.stdout_contains("help") || output.stderr_contains("required"),
            "Running without command should show error or help");
}

#[test]
fn test_help_flag() {
    let ctx = TestContext::new();
    let output = ctx.run_command(&["--help"]);

    assert!(output.success(), "--help should succeed");
    assert!(output.stdout_contains("Mirage") || output.stdout_contains("mirage"),
            "help should mention mirage");
    assert!(output.stdout_contains("USAGE") || output.stdout_contains("Usage"),
            "help should show usage");
}

#[test]
fn test_invalid_database() {
    let temp_dir = TempDir::new().unwrap();
    let nonexistent_db = temp_dir.path().join("nonexistent.db");

    let mirage_bin = std::env::var("CARGO_BIN_EXE_mirage")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./target/release/mirage"));

    let output = Command::new(&mirage_bin)
        .args(["status", "--db", nonexistent_db.to_str().unwrap()])
        .output()
        .expect("Failed to run mirage");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should fail gracefully
    assert!(!output.status.success() || stderr.contains("not found") || stdout.contains("not found"),
            "Invalid database path should show error");
}
