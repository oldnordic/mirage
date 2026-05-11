//! Backend parity tests for mirage
//!
//! Verify that SQLite backend produces correct results.
//! Tests follow the TDD pattern: RED (failing test), GREEN (implementation passes),
//! REFACTOR (cleanup while maintaining passing tests).

use std::path::PathBuf;
use tempfile::TempDir;

// Import storage items
use mirage::storage::{Backend, CfgBlockData, StorageTrait};

/// Create a test SQLite database with CFG data
///
/// This helper creates a minimal Magellan v7 database with:
/// - magellan_meta table (schema version 7)
fn create_test_database_sqlite() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");

    let conn = rusqlite::Connection::open(&db_path).unwrap();

    // Create magellan_meta table
    conn.execute(
        "CREATE TABLE magellan_meta (key TEXT PRIMARY KEY, value TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO magellan_meta (key, value) VALUES ('schema_version', '7')",
        [],
    )
    .unwrap();

    // Create graph_entities table
    conn.execute(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            file_path TEXT,
            data TEXT NOT NULL DEFAULT '{}'
        )",
        [],
    )
    .unwrap();

    // Insert a test function entity
    conn.execute(
        "INSERT INTO graph_entities (id, kind, name, file_path, data)
         VALUES (1, 'Symbol', 'test_function', 'src/test.rs', '{\"kind\": \"Function\"}')",
        [],
    )
    .unwrap();

    // Create cfg_blocks table
    conn.execute(
        "CREATE TABLE cfg_blocks (
            id INTEGER PRIMARY KEY,
            function_id INTEGER NOT NULL,
            kind TEXT NOT NULL,
            terminator TEXT,
            byte_start INTEGER,
            byte_end INTEGER,
            start_line INTEGER,
            start_col INTEGER,
            end_line INTEGER,
            end_col INTEGER,
            coord_x INTEGER DEFAULT 0,
            coord_y INTEGER DEFAULT 0,
            coord_z INTEGER DEFAULT 0,
            cfg_condition TEXT
        )",
        [],
    )
    .unwrap();

    // Insert test CFG blocks
    conn.execute(
        "INSERT INTO cfg_blocks
         (id, function_id, kind, terminator, byte_start, byte_end, start_line, start_col, end_line, end_col, coord_x, coord_y, coord_z)
         VALUES (1, 1, 'entry', 'fallthrough', 0, 10, 1, 0, 1, 10, 0, 0, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cfg_blocks
         (id, function_id, kind, terminator, byte_start, byte_end, start_line, start_col, end_line, end_col, coord_x, coord_y, coord_z)
         VALUES (2, 1, 'normal', 'conditional', 10, 50, 2, 0, 3, 20, 1, 0, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cfg_blocks
         (id, function_id, kind, terminator, byte_start, byte_end, start_line, start_col, end_line, end_col, coord_x, coord_y, coord_z)
         VALUES (3, 1, 'return', 'return', 50, 60, 5, 0, 5, 10, 2, 0, 2)",
        [],
    )
    .unwrap();

    (dir, db_path)
}

// ============================================================================
// Task 1: Test CFG block retrieval
// ============================================================================

#[test]
fn test_cfg_blocks_parity_sqlite() {
    let (_dir, db_path) = create_test_database_sqlite();

    // Open SQLite backend using Backend enum
    let backend = Backend::detect_and_open(&db_path).unwrap();

    // Test function ID 1
    let blocks = backend.get_cfg_blocks(1).unwrap();

    assert_eq!(blocks.len(), 3, "Should have 3 CFG blocks");

    // Verify first block (entry)
    assert_eq!(blocks[0].kind, "entry", "First block should be entry");
    assert_eq!(blocks[0].terminator, "fallthrough");
    assert_eq!(blocks[0].byte_start, 0);
    assert_eq!(blocks[0].byte_end, 10);
    assert_eq!(blocks[0].start_line, 1);
    assert_eq!(blocks[0].start_col, 0);
    assert_eq!(blocks[0].end_line, 1);
    assert_eq!(blocks[0].end_col, 10);

    // Verify second block (conditional)
    assert_eq!(blocks[1].kind, "normal", "Second block should be normal");
    assert_eq!(blocks[1].terminator, "conditional");
    assert_eq!(blocks[1].byte_start, 10);
    assert_eq!(blocks[1].byte_end, 50);

    // Verify third block (return)
    assert_eq!(blocks[2].kind, "return", "Third block should be return");
    assert_eq!(blocks[2].terminator, "return");
    assert_eq!(blocks[2].byte_start, 50);
    assert_eq!(blocks[2].byte_end, 60);
}

// ============================================================================
// Task 1: Test entity query
// ============================================================================

#[test]
fn test_entity_parity_sqlite() {
    let (_dir, db_path) = create_test_database_sqlite();

    let backend = Backend::detect_and_open(&db_path).unwrap();

    // Test entity ID 1 (should exist)
    let entity = backend.get_entity(1);
    assert!(entity.is_some(), "Entity 1 should exist");

    let entity = entity.unwrap();
    assert_eq!(entity.id, 1);
    assert_eq!(entity.kind, "Symbol");
    assert_eq!(entity.name, "test_function");
    assert_eq!(entity.file_path, Some("src/test.rs".to_string()));

    // Test non-existent entity
    let entity = backend.get_entity(999);
    assert!(entity.is_none(), "Entity 999 should not exist");
}

// ============================================================================
// Task 1: Test empty result handling
// ============================================================================

#[test]
fn test_empty_result_sqlite() {
    let (_dir, db_path) = create_test_database_sqlite();

    let backend = Backend::detect_and_open(&db_path).unwrap();

    // Query non-existent function should return empty Vec, not error
    let blocks = backend.get_cfg_blocks(999).unwrap();
    assert_eq!(
        blocks.len(),
        0,
        "Non-existent function should return empty Vec"
    );
}

// ============================================================================
// Task 1: Test CfgBlockData field parity
// ============================================================================

#[test]
fn test_cfg_block_data_fields() {
    // Verify CfgBlockData has all expected fields
    let block = CfgBlockData {
        id: 1,
        kind: "entry".to_string(),
        terminator: "fallthrough".to_string(),
        byte_start: 0,
        byte_end: 10,
        start_line: 1,
        start_col: 0,
        end_line: 1,
        end_col: 10,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
        cfg_condition: None,
    };

    assert_eq!(block.kind, "entry");
    assert_eq!(block.terminator, "fallthrough");
    assert_eq!(block.byte_start, 0);
    assert_eq!(block.byte_end, 10);
    assert_eq!(block.start_line, 1);
    assert_eq!(block.start_col, 0);
    assert_eq!(block.end_line, 1);
    assert_eq!(block.end_col, 10);
}

// ============================================================================
// Task 1: Test StorageTrait implementation
// ============================================================================

#[test]
fn test_storage_trait_impl_sqlite() {
    // This test verifies that Backend implements StorageTrait
    // at compile time. If it compiles, the trait is implemented.
    fn assert_storage_trait<T: StorageTrait>(_t: &T) {}

    let (_dir, db_path) = create_test_database_sqlite();
    let backend = Backend::detect_and_open(&db_path).unwrap();

    // Verify StorageTrait is implemented
    assert_storage_trait(&backend);
}
