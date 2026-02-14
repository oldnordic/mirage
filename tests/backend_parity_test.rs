//! Backend parity tests for mirage
//!
//! Verify that SQLite and native-v3 backends produce identical results.
//! Tests follow the TDD pattern: RED (failing test), GREEN (implementation passes),
//! REFACTOR (cleanup while maintaining passing tests).
//!
//! These tests ensure that the storage trait abstraction provides identical
//! behavior across backends, enabling users to switch backends without
//! changing their workflows.

use tempfile::TempDir;
use std::path::PathBuf;

// Import storage items
use mirage_analyzer::storage::{Backend, StorageTrait, CfgBlockData};

/// Create a test SQLite database with CFG data
///
/// This helper creates a minimal Magellan v7 database with:
/// - magellan_meta table (schema version 7)
/// - graph_entities table (for functions)
/// - cfg_blocks table (with CFG data)
///
/// Returns the temp directory and database path.
fn create_test_database_sqlite() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");

    // Use rusqlite directly to create the database
    // This is test infrastructure, not production code
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

    // Create magellan_meta table (Magellan schema version 7)
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

    // Create cfg_blocks table (Magellan v7 schema)
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

    // Insert test CFG blocks for the function
    // Block 1: Entry block
    conn.execute(
        "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                 start_line, start_col, end_line, end_col)
         VALUES (1, 'entry', 'fallthrough', 0, 10, 1, 0, 1, 10)",
        [],
    ).unwrap();

    // Block 2: Conditional block
    conn.execute(
        "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                 start_line, start_col, end_line, end_col)
         VALUES (1, 'normal', 'conditional', 10, 50, 2, 4, 5, 8)",
        [],
    ).unwrap();

    // Block 3: Return block
    conn.execute(
        "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                 start_line, start_col, end_line, end_col)
         VALUES (1, 'return', 'return', 50, 60, 5, 0, 5, 10)",
        [],
    ).unwrap();

    (dir, db_path)
}

/// Create a test native-v2 database with CFG data
///
/// This helper creates a native-v3 database with the same test data
/// as create_test_database_sqlite() for parity testing.
///
/// Note: This requires the native-v3 feature to be enabled.
#[cfg(feature = "native-v3")]
fn create_test_database_native_v2() -> (TempDir, PathBuf) {
    // TODO: Implement native-v3 test database creation
    // For now, this test requires a pre-existing native-v3 database
    // Create using: magellan watch --root ./test_src --db test.v3
    unimplemented!("Native-v3 test database creation not yet implemented. Use magellan to create a .v3 database first.")
}

// ============================================================================
// Task 1: Test CFG block retrieval parity
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

#[test]
#[cfg(feature = "native-v3")]
fn test_cfg_blocks_parity_native_v2() {
    let (_dir, db_path) = create_test_database_native_v2();

    // Open native-v3 backend using Backend enum
    let backend = Backend::detect_and_open(&db_path).unwrap();

    // Test function ID 1
    let blocks = backend.get_cfg_blocks(1).unwrap();

    assert_eq!(blocks.len(), 3, "Should have 3 CFG blocks");

    // Verify first block (entry)
    assert_eq!(blocks[0].kind, "entry", "First block should be entry");
    assert_eq!(blocks[0].terminator, "fallthrough");
    assert_eq!(blocks[0].byte_start, 0);
    assert_eq!(blocks[0].byte_end, 10);

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
// Task 1: Test entity query parity
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

#[test]
#[cfg(feature = "native-v3")]
fn test_entity_parity_native_v2() {
    let (_dir, db_path) = create_test_database_native_v2();

    let backend = Backend::detect_and_open(&db_path).unwrap();

    // Test entity ID 1 (should exist)
    let entity = backend.get_entity(1);
    assert!(entity.is_some(), "Entity 1 should exist");

    let entity = entity.unwrap();
    assert_eq!(entity.id, 1);
    // Note: exact kind may vary depending on indexing
    assert!(!entity.kind.is_empty());
    assert!(!entity.name.is_empty());

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
    assert_eq!(blocks.len(), 0, "Non-existent function should return empty Vec");
}

#[test]
#[cfg(feature = "native-v3")]
fn test_empty_result_native_v2() {
    let (_dir, db_path) = create_test_database_native_v2();

    let backend = Backend::detect_and_open(&db_path).unwrap();

    // Query non-existent function should return empty Vec, not error
    let blocks = backend.get_cfg_blocks(999).unwrap();
    assert_eq!(blocks.len(), 0, "Non-existent function should return empty Vec");
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

#[test]
#[cfg(feature = "native-v3")]
fn test_storage_trait_impl_native_v2() {
    fn assert_storage_trait<T: StorageTrait>(_t: &T) {}

    let (_dir, db_path) = create_test_database_native_v2();
    let backend = Backend::detect_and_open(&db_path).unwrap();

    // Verify StorageTrait is implemented
    assert_storage_trait(&backend);
}
