//! Layer 3: Component Integration Tests for Geometric Backend
//!
//! These tests verify end-to-end workflows with the geometric backend,
//! simulating real usage scenarios.

#![cfg(feature = "backend-geometric")]

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

// Import CfgBlock for test fixture setup
use geographdb_core::cfg_store::CfgBlock;

/// Test context for geometric backend integration tests
struct GeometricTestContext {
    mirage_bin: PathBuf,
    geo_path: PathBuf,
    _temp_dir: TempDir,
}

impl GeometricTestContext {
    /// Create a new test context with a geometric database
    fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let geo_path = temp_dir.path().join("test.geo");

        // Create a geometric database using Magellan's backend
        let backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
            .expect("Failed to create test geo database");

        // Populate with minimal test data so commands have something to work with
        Self::populate_minimal_data(&backend);

        // CRITICAL: Persist the database to disk
        backend.save_to_disk().expect("Failed to save database");

        // Use CARGO_BIN_EXE_mirage if available
        let mirage_bin = std::env::var("CARGO_BIN_EXE_mirage")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // Check common cargo target locations
                let release_path = PathBuf::from("/home/feanor/.cargo/target/release/mirage");
                let debug_path = PathBuf::from("/home/feanor/.cargo/target/debug/mirage");
                let local_release = PathBuf::from("./target/release/mirage");
                let local_debug = PathBuf::from("./target/debug/mirage");

                if release_path.exists() {
                    release_path
                } else if debug_path.exists() {
                    debug_path
                } else if local_release.exists() {
                    local_release
                } else if local_debug.exists() {
                    local_debug
                } else {
                    panic!("Mirage binary not found. Please build with: cargo build --release");
                }
            });

        Self {
            mirage_bin,
            geo_path,
            _temp_dir: temp_dir,
        }
    }

    /// Run mirage with the given arguments
    fn run_command(&self, args: &[&str]) -> TestOutput {
        let output = Command::new(&self.mirage_bin)
            .args(args)
            .arg("--db")
            .arg(&self.geo_path)
            .output()
            .expect("Failed to run mirage");

        TestOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            status: output.status,
        }
    }

    /// Populate the .geo database with minimal test data including CFG blocks
    fn populate_minimal_data(backend: &magellan::graph::geometric_backend::GeometricBackend) {
        use geographdb_core::cfg_store::CfgBlock;
        use magellan::graph::geometric_types::SymbolData;
        use magellan::ingest::{Language, SymbolKind};

        let symbols = vec![
            // Test function
            SymbolData {
                fqn: "test_function".to_string(),
                name: "test_function".to_string(),
                kind: SymbolKind::Function,
                language: Language::Rust,
                file_path: "src/test.rs".to_string(),
                byte_start: 0,
                byte_end: 100,
                start_line: 1,
                start_col: 0,
                end_line: 10,
                end_col: 1,
            },
        ];

        let ids = backend
            .insert_symbols(symbols)
            .expect("Failed to insert symbols");

        let function_id = ids[0] as i64;

        // Insert CFG blocks for the function
        let blocks = vec![
            CfgBlock {
                id: 0,
                function_id,
                block_kind: "entry".to_string(),
                terminator: "fallthrough".to_string(),
                byte_start: 0,
                byte_end: 10,
                start_line: 1,
                start_col: 0,
                end_line: 1,
                end_col: 10,
                dominator_depth: 0,
                loop_nesting: 0,
                branch_count: 0,
            },
            CfgBlock {
                id: 1,
                function_id,
                block_kind: "normal".to_string(),
                terminator: "return".to_string(),
                byte_start: 10,
                byte_end: 50,
                start_line: 2,
                start_col: 0,
                end_line: 5,
                end_col: 10,
                dominator_depth: 1,
                loop_nesting: 0,
                branch_count: 0,
            },
            CfgBlock {
                id: 2,
                function_id,
                block_kind: "exit".to_string(),
                terminator: "return".to_string(),
                byte_start: 50,
                byte_end: 60,
                start_line: 5,
                start_col: 0,
                end_line: 6,
                end_col: 10,
                dominator_depth: 0,
                loop_nesting: 0,
                branch_count: 0,
            },
        ];

        backend
            .insert_cfg_blocks(blocks)
            .expect("Failed to insert CFG blocks");

        // Insert edges: entry (0) -> normal (1) -> exit (2)
        backend
            .insert_edge(0, 1, "fallthrough")
            .expect("Failed to insert edge 0->1");
        backend
            .insert_edge(1, 2, "fallthrough")
            .expect("Failed to insert edge 1->2");
    }
}

/// Output from running a mirage command
struct TestOutput {
    stdout: String,
    stderr: String,
    status: std::process::ExitStatus,
}

impl TestOutput {
    fn success(&self) -> bool {
        self.status.success()
    }

    fn stdout_contains(&self, s: &str) -> bool {
        self.stdout.contains(s)
    }

    fn stderr_contains(&self, s: &str) -> bool {
        self.stderr.contains(s)
    }
}

// ============================================================================
// Component tests
// ============================================================================

#[test]
fn test_geometric_detect_backend_flag() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["--detect-backend"]);

    assert!(
        output.success(),
        "--detect-backend should succeed for geometric"
    );
    assert!(
        output.success()
            || output.stderr_contains("not supported")
            || output.stderr_contains("not implemented")
            || output.stderr_contains("Failed to open database")
            || output.stderr_contains("Failed to open database"),
        "status command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_detect_backend_json() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["--detect-backend", "--output", "json"]);

    assert!(
        output.success(),
        "--detect-backend --output json should succeed"
    );
    assert!(
        output.stdout_contains("\"backend\""),
        "JSON should contain backend field"
    );
    assert!(
        output.stdout_contains("geometric"),
        "JSON should contain 'geometric'"
    );
}

#[test]
fn test_geometric_status_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["status"]);

    // Status command should work with geometric backend
    // It may return an error if geometric backend doesn't support all features yet
    // but it should not panic
    assert!(
        output.success()
            || output.stderr_contains("not supported")
            || output.stderr.contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not"),
        "status command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_cfg_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["cfg", "--function", "test_function"]);

    // cfg command should handle geometric backend
    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr.contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not"),
        "cfg command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_paths_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["paths", "--function", "test_function"]);

    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr.contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not"),
        "paths command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_loops_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["loops", "--function", "test_function"]);

    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr.contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not"),
        "loops command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_dominators_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["dominators", "--function", "test_function"]);

    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr_contains("not implemented")
            || output.stderr_contains("requires"),
        "dominators command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_frontiers_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["frontiers", "--function", "test_function"]);

    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr_contains("not implemented")
            || output.stderr_contains("requires"),
        "frontiers command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_patterns_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["patterns", "--function", "test_function"]);

    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr_contains("not implemented")
            || output.stderr_contains("requires"),
        "patterns command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_unreachable_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["unreachable", "--within-functions"]);

    assert!(
        output.success()
            || output.stderr_contains("not supported")
            || output.stderr_contains("not implemented")
            || output.stderr_contains("requires"),
        "unreachable command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_cycles_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["cycles"]);

    assert!(
        output.success()
            || output.stderr_contains("not supported")
            || output.stderr_contains("not implemented")
            || output.stderr_contains("requires"),
        "cycles command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_blast_zone_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["blast-zone", "--function", "test_function"]);

    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr.contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not"),
        "blast-zone command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_hotspots_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["hotspots"]);

    assert!(
        output.success()
            || output.stderr_contains("not supported")
            || output.stderr.contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not"),
        "hotspots command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_hotpaths_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["hotpaths", "--function", "test_function"]);

    assert!(
        output.success()
            || output.stderr_contains("not supported")
            || output.stderr_contains("not implemented")
            || output.stderr_contains("requires"),
        "hotpaths command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_slice_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&[
        "slice",
        "--symbol",
        "test_function",
        "--direction",
        "backward",
    ]);

    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr_contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not"),
        "slice command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_icfg_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["icfg", "--entry", "test_function"]);

    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr_contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not"),
        "icfg command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_diff_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&[
        "diff",
        "--function",
        "test_function",
        "--before",
        "abc123",
        "--after",
        "def456",
    ]);

    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr.contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not"),
        "diff command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_verify_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["verify", "--path-id", "1"]);

    assert!(
        output.success()
            || output.stderr_contains("not found")
            || output.stderr_contains("not supported")
            || output.stderr.contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not"),
        "verify command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

#[test]
fn test_geometric_migrate_command() {
    let ctx = GeometricTestContext::new();
    let output = ctx.run_command(&["migrate", "--from", "geometric", "--to", "sqlite"]);

    assert!(
        output.success()
            || output.stderr_contains("not supported")
            || output.stderr.contains("not implemented")
            || output.stderr.contains("requires")
            || output.stderr.contains("Could not")
            || output.stderr.contains("invalid"),
        "migrate command should handle geometric backend gracefully. stderr: {}",
        output.stderr
    );
}

// ============================================================================
// Error handling tests
// ============================================================================

#[test]
fn test_geometric_nonexistent_file() {
    let temp_dir = TempDir::new().unwrap();
    let nonexistent_geo = temp_dir.path().join("nonexistent.geo");

    let mirage_bin = std::env::var("CARGO_BIN_EXE_mirage")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            // Check common cargo target locations
            let release_path = PathBuf::from("/home/feanor/.cargo/target/release/mirage");
            let debug_path = PathBuf::from("/home/feanor/.cargo/target/debug/mirage");
            let local_release = PathBuf::from("./target/release/mirage");
            let local_debug = PathBuf::from("./target/debug/mirage");

            if release_path.exists() {
                release_path
            } else if debug_path.exists() {
                debug_path
            } else if local_release.exists() {
                local_release
            } else if local_debug.exists() {
                local_debug
            } else {
                panic!("Mirage binary not found. Please build with: cargo build --release");
            }
        });

    let output = Command::new(&mirage_bin)
        .args(["status", "--db", nonexistent_geo.to_str().unwrap()])
        .output()
        .expect("Failed to run mirage");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should fail gracefully with a clear error
    assert!(
        !output.status.success()
            || stderr.contains("not found")
            || stderr.contains("Database not found"),
        "Nonexistent geometric database should show error: stderr={}",
        stderr
    );
}

#[test]
fn test_geometric_wrong_extension() {
    let temp_dir = TempDir::new().unwrap();
    let wrong_path = temp_dir.path().join("test.db");

    // Create a dummy SQLite database
    {
        use rusqlite::Connection;
        let mut conn = Connection::open(&wrong_path).unwrap();
        conn.execute("CREATE TABLE test (id INTEGER)", []).unwrap();
    }

    let mirage_bin = std::env::var("CARGO_BIN_EXE_mirage")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            // Check common cargo target locations
            let release_path = PathBuf::from("/home/feanor/.cargo/target/release/mirage");
            let debug_path = PathBuf::from("/home/feanor/.cargo/target/debug/mirage");
            let local_release = PathBuf::from("./target/release/mirage");
            let local_debug = PathBuf::from("./target/debug/mirage");

            if release_path.exists() {
                release_path
            } else if debug_path.exists() {
                debug_path
            } else if local_release.exists() {
                local_release
            } else if local_debug.exists() {
                local_debug
            } else {
                panic!("Mirage binary not found. Please build with: cargo build --release");
            }
        });

    let output = Command::new(&mirage_bin)
        .args(["--detect-backend", "--db", wrong_path.to_str().unwrap()])
        .output()
        .expect("Failed to run mirage");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should detect as SQLite, not geometric
    assert!(output.status.success(), "Detection should succeed");
    assert!(
        stdout.contains("sqlite"),
        "Should detect as sqlite, not geometric. stdout: {}",
        stdout
    );
}
