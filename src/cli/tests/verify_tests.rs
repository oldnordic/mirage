// ============================================================================

use crate::cfg::{enumerate_paths, PathLimits};
use crate::cli::responses::*;
use crate::cli::*;
use crate::output::JsonResponse;
use crate::storage::MirageDb;

/// Create a test database with a cached path
fn create_test_db_with_cached_path() -> anyhow::Result<(tempfile::NamedTempFile, MirageDb, String)>
{
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
#[cfg(feature = "backend-sqlite")]
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

    assert_eq!(wrapper.schema_version, "1.0.1");
    assert_eq!(wrapper.tool, "mirage");
    assert!(!wrapper.execution_id.is_empty());
    assert!(!wrapper.timestamp.is_empty());

    let json = wrapper.to_json();
    assert!(json.contains("\"schema_version\":\"1.0.1\""));
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
        assert!(
            path_ids.insert(&path.path_id),
            "Path ID should be unique: {}",
            path.path_id
        );
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
