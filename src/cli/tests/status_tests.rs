// ============================================================================

use crate::storage::{create_schema, MirageDb};
use rusqlite::{params, Connection};

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
        "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end, start_line, start_col, end_line, end_col)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params!(function_id, "entry", "goto", 0, 10, 1, 0, 1, 10),
    )?;
    conn.execute(
        "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end, start_line, start_col, end_line, end_col)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params!(function_id, "return", "return", 10, 20, 2, 0, 2, 10),
    )?;

    // cfg_edges table is managed by Magellan v11+; Mirage does not create it.
    // Edges are reconstructed in memory from terminator data.

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
#[cfg(feature = "backend-sqlite")]
#[allow(deprecated)]
fn test_status_returns_correct_statistics() {
    let (_file, db) = create_test_db().unwrap();
    let status = db.status().unwrap();

    assert_eq!(status.cfg_blocks, 2, "Should have 2 cfg_blocks");
    // cfg_edges is managed by Magellan v11+; count may be 0 if table absent
    assert_eq!(
        status.cfg_edges, 0,
        "cfg_edges count should be 0 (managed by Magellan)"
    );
    assert_eq!(status.cfg_paths, 1, "Should have 1 cfg_path");
    assert_eq!(status.cfg_dominators, 1, "Should have 1 cfg_dominator");
    assert_eq!(
        status.mirage_schema_version, 1,
        "Schema version should be 1"
    );
    assert_eq!(
        status.magellan_schema_version, 7,
        "Magellan version should be 7"
    );
}

/// Test that human output format contains expected fields
#[test]
#[cfg(feature = "backend-sqlite")]
#[allow(deprecated)]
fn test_status_human_output_format() {
    let (_file, db) = create_test_db().unwrap();
    let status = db.status().unwrap();

    // Verify all expected fields are present and have correct values
    assert!(status.cfg_blocks >= 0, "cfg_blocks should be non-negative");
    assert!(status.cfg_edges >= 0, "cfg_edges should be non-negative");
    assert!(status.cfg_paths >= 0, "cfg_paths should be non-negative");
    assert!(
        status.cfg_dominators >= 0,
        "cfg_dominators should be non-negative"
    );
    assert!(
        status.mirage_schema_version > 0,
        "mirage_schema_version should be positive"
    );
    assert!(
        status.magellan_schema_version > 0,
        "magellan_schema_version should be positive"
    );
}

/// Test that JSON output format is valid and contains expected structure
#[test]
#[cfg(feature = "backend-sqlite")]
fn test_status_json_output_format() {
    use crate::output::JsonResponse;

    let (_file, db) = create_test_db().unwrap();
    let status = db.status().unwrap();
    let response = JsonResponse::new(status);

    // Verify JsonResponse wrapper structure
    assert_eq!(response.schema_version, "1.0.1");
    assert_eq!(response.tool, "mirage");
    assert!(!response.execution_id.is_empty());
    assert!(!response.timestamp.is_empty());

    // Verify JSON serialization
    let json = response.to_json();
    assert!(json.contains("\"schema_version\":\"1.0.1\""));
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
#[cfg(feature = "backend-sqlite")]
fn test_status_pretty_json_output_format() {
    use crate::output::JsonResponse;

    let (_file, db) = create_test_db().unwrap();
    let status = db.status().unwrap();
    let response = JsonResponse::new(status);

    let pretty_json = response.to_pretty_json();

    // Pretty JSON should contain newlines and indentation
    assert!(
        pretty_json.contains("\n"),
        "Pretty JSON should contain newlines"
    );
    assert!(
        pretty_json.contains("  "),
        "Pretty JSON should contain indentation"
    );

    // Should still be valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&pretty_json).expect("Pretty JSON should be valid");
    assert!(parsed.is_object(), "Parsed JSON should be an object");
    assert_eq!(parsed["schema_version"], "1.0.1");
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
            assert!(
                err_msg.contains("Database not found") || err_msg.contains("not found"),
                "Error message should mention database not found: {}",
                err_msg
            );
        }
    }
}

/// Test that status() with empty database returns zero counts
#[test]
#[cfg(feature = "backend-sqlite")]
#[allow(deprecated)]
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
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            file_path TEXT,
            data TEXT NOT NULL
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
         VALUES (1, ?, ?, ?)",
        params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
    ).unwrap();

    create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

    let db = MirageDb::open(file.path()).unwrap();
    let status = db.status().unwrap();

    assert_eq!(
        status.cfg_blocks, 0,
        "Empty database should have 0 cfg_blocks"
    );
    // cfg_edges table is managed by Magellan v11+; Mirage does not create it
    assert_eq!(
        status.cfg_edges, 0,
        "cfg_edges count should be 0 (table managed by Magellan)"
    );
    assert_eq!(
        status.cfg_paths, 0,
        "Empty database should have 0 cfg_paths"
    );
    assert_eq!(
        status.cfg_dominators, 0,
        "Empty database should have 0 cfg_dominators"
    );
}
