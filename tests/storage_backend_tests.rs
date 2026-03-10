//! Layer 1: Unit Tests for MirageDb Backend Compatibility
//!
//! These tests verify that MirageDb works with all three backends:
//! - SQLite (default)
//! - GeometricDB (.geo files)
//! - Native V3

use mirage_analyzer::storage::{Backend, BackendFormat, MirageDb};
use std::path::Path;
use tempfile::TempDir;

#[cfg(feature = "backend-sqlite")]
#[test]
fn test_detect_sqlite_backend() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create a simple SQLite database
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE test (id INTEGER)", []).unwrap();
    }

    let format = BackendFormat::detect(&db_path).unwrap();
    assert!(matches!(format, BackendFormat::SQLite));
}

#[cfg(feature = "backend-geometric")]
#[test]
fn test_detect_geometric_backend() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.geo");

    // Create a geometric database using Magellan's API
    {
        let _backend = magellan::graph::geometric_backend::GeometricBackend::create(&db_path)
            .expect("Failed to create geometric backend");
    }

    let format = BackendFormat::detect(&db_path).unwrap();
    assert!(matches!(format, BackendFormat::Geometric));
}

#[cfg(feature = "backend-sqlite")]
#[test]
fn test_open_sqlite_database() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create minimal database with required tables
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        // Create minimal graph_entities table for Magellan compatibility
        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY,
                fqn TEXT NOT NULL,
                name TEXT NOT NULL,
                kind TEXT NOT NULL
            )",
            [],
        ).unwrap();
        // Create minimal cfg_blocks table for Mirage
        conn.execute(
            "CREATE TABLE cfg_blocks (
                id INTEGER PRIMARY KEY,
                function_id INTEGER NOT NULL,
                kind TEXT NOT NULL,
                terminator TEXT NOT NULL
            )",
            [],
        ).unwrap();
    }

    let db = MirageDb::open(&db_path);
    assert!(db.is_ok(), "Should open SQLite database");

    let db = db.unwrap();
    assert!(db.is_sqlite(), "Should detect SQLite backend");
}

#[cfg(feature = "backend-geometric")]
#[test]
fn test_open_geometric_database() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.geo");

    // Create geometric database with some data
    {
        let backend = magellan::graph::geometric_backend::GeometricBackend::create(&db_path)
            .expect("Failed to create geometric backend");

        // Insert a test symbol
        let _ = backend.insert_symbol_internal(&magellan::graph::geometric_backend::SymbolData {
            fqn: "test::function".to_string(),
            name: "function".to_string(),
            kind: magellan::ingest::SymbolKind::Function,
            language: magellan::ingest::Language::Rust,
            file_path: "test.rs".to_string(),
            byte_start: 0,
            byte_end: 10,
            start_line: 1,
            start_col: 0,
            end_line: 2,
            end_col: 0,
        });
    }

    let db = MirageDb::open(&db_path);
    assert!(db.is_ok(), "Should open Geometric database: {:?}", db.err());
}

#[cfg(feature = "backend-sqlite")]
#[test]
fn test_get_cfg_blocks_from_sqlite() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create database with CFG data
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();

        // Create minimal schema
        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY,
                fqn TEXT NOT NULL,
                name TEXT NOT NULL,
                kind TEXT NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE TABLE cfg_blocks (
                id INTEGER PRIMARY KEY,
                function_id INTEGER NOT NULL,
                kind TEXT NOT NULL,
                terminator TEXT NOT NULL
            )",
            [],
        ).unwrap();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (fqn, name, kind) VALUES (?, ?, ?)",
            ["test::function", "function", "fn"],
        )
        .unwrap();

        // Insert CFG block
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator) VALUES (1, 'Entry', 'Return')",
            [],
        )
        .unwrap();
    }

    let db = MirageDb::open(&db_path).unwrap();
    let blocks = db.storage().get_cfg_blocks(1);

    assert!(blocks.is_ok(), "Should get CFG blocks");
    let blocks = blocks.unwrap();
    assert_eq!(blocks.len(), 1, "Should have 1 CFG block");
    assert_eq!(blocks[0].kind, "Entry");
}

#[cfg(feature = "backend-geometric")]
#[test]
fn test_get_cfg_blocks_from_geometric() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.geo");

    // Create geometric database with CFG data
    {
        let backend = magellan::graph::geometric_backend::GeometricBackend::create(&db_path)
            .expect("Failed to create geometric backend");

        let symbol_id = backend
            .insert_symbol_internal(&magellan::graph::geometric_backend::SymbolData {
                fqn: "test::function".to_string(),
                name: "function".to_string(),
                kind: magellan::ingest::SymbolKind::Function,
                language: magellan::ingest::Language::Rust,
                file_path: "test.rs".to_string(),
                byte_start: 0,
                byte_end: 10,
                start_line: 1,
                start_col: 0,
                end_line: 2,
                end_col: 0,
            })
            .unwrap();

        // Insert CFG block
        let _block = backend.insert_cfg_block(magellan::graph::geometric_backend::CfgBlock {
            id: 0,
            function_id: symbol_id as i64,
            block_kind: "Entry".to_string(),
            terminator: "Return".to_string(),
            byte_start: 0,
            byte_end: 10,
            start_line: 1,
            start_col: 0,
            end_line: 2,
            end_col: 0,
            dominator_depth: 0,
            loop_nesting: 0,
            branch_count: 0,
        });
    }

    let db = MirageDb::open(&db_path).unwrap();
    let blocks = db.storage().get_cfg_blocks(1); // Using symbol_id = 1

    assert!(
        blocks.is_ok(),
        "Should get CFG blocks from Geometric backend"
    );
    let blocks = blocks.unwrap();
    assert_eq!(blocks.len(), 1, "Should have 1 CFG block");
}
