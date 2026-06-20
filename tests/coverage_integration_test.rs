//! Coverage integration tests for Mirage
//!
//! Verify that coverage data from Magellan's cfg_block_coverage table
//! is correctly exposed through the JSON export and CLI commands.

use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

use mirage::cfg::{export_json, BasicBlock, BlockKind, Cfg, EdgeType, Terminator};
use petgraph::graph::DiGraph;

/// Create a test SQLite database with CFG and coverage data
fn create_test_db_with_coverage() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

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
        "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
         VALUES (1, 11, 3, 0)",
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
            cfg_hash TEXT,
            statements TEXT,
            cfg_condition TEXT
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE cfg_block_coverage (
            block_id INTEGER PRIMARY KEY,
            hit_count INTEGER NOT NULL
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE cfg_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            function_id INTEGER NOT NULL,
            source_idx INTEGER NOT NULL,
            target_idx INTEGER NOT NULL,
            edge_type TEXT NOT NULL
        )",
        [],
    )
    .unwrap();

    // Insert test function
    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data)
         VALUES ('Symbol', 'test_function', 'src/test.rs', '{\"kind\":\"Function\",\"symbol_id\":\"test_function_symbol\"}')",
        [],
    )
    .unwrap();

    // Insert blocks
    conn.execute(
        "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                 start_line, start_col, end_line, end_col)
         VALUES (1, 'entry', 'fallthrough', 0, 10, 1, 0, 1, 10)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                 start_line, start_col, end_line, end_col)
         VALUES (1, 'normal', 'conditional', 10, 50, 2, 4, 5, 8)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                 start_line, start_col, end_line, end_col)
         VALUES (1, 'return', 'return', 50, 60, 5, 0, 5, 10)",
        [],
    )
    .unwrap();

    // Insert coverage data: block 1 = 5 hits, block 2 = 10 hits, block 3 = 0 hits
    conn.execute(
        "INSERT INTO cfg_block_coverage (block_id, hit_count) VALUES (1, 5)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cfg_block_coverage (block_id, hit_count) VALUES (2, 10)",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO cfg_edges (function_id, source_idx, target_idx, edge_type)
         VALUES (1, 0, 1, 'fallthrough')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cfg_edges (function_id, source_idx, target_idx, edge_type)
         VALUES (1, 1, 2, 'conditional_true')",
        [],
    )
    .unwrap();

    (dir, db_path)
}

/// Build a simple test CFG with db_id populated
fn create_test_cfg_with_db_ids() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: Some(1),
        kind: BlockKind::Entry,
        statements: vec!["entry".to_string()],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: Some(2),
        kind: BlockKind::Normal,
        statements: vec!["branch".to_string()],
        terminator: Terminator::SwitchInt {
            targets: vec![2],
            otherwise: 2,
        },
        source_location: None,
        coord_x: 1,
        coord_y: 0,
        coord_z: 1,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: Some(3),
        kind: BlockKind::Exit,
        statements: vec!["return".to_string()],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 2,
        coord_y: 0,
        coord_z: 2,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);

    g
}

#[test]
fn test_export_json_includes_coverage() {
    let cfg = create_test_cfg_with_db_ids();
    let mut coverage = HashMap::new();
    coverage.insert(1i64, 5i64);
    coverage.insert(2i64, 10i64);

    let export = export_json(&cfg, "test_function", Some(&coverage));

    // Block 0 (db_id=1) should have coverage with 5 hits
    let block0 = export.blocks.iter().find(|b| b.id == 0).unwrap();
    assert!(
        block0.coverage.is_some(),
        "Block 0 should have coverage data"
    );
    assert_eq!(block0.coverage.as_ref().unwrap().hit_count, 5);

    // Block 1 (db_id=2) should have coverage with 10 hits
    let block1 = export.blocks.iter().find(|b| b.id == 1).unwrap();
    assert!(
        block1.coverage.is_some(),
        "Block 1 should have coverage data"
    );
    assert_eq!(block1.coverage.as_ref().unwrap().hit_count, 10);

    // Block 2 (db_id=3) should have no coverage (not in map)
    let block2 = export.blocks.iter().find(|b| b.id == 2).unwrap();
    assert!(
        block2.coverage.is_none(),
        "Block 2 should not have coverage data"
    );
}

#[test]
fn test_export_json_without_coverage() {
    let cfg = create_test_cfg_with_db_ids();

    let export = export_json(&cfg, "test_function", None);

    for block in &export.blocks {
        assert!(
            block.coverage.is_none(),
            "Block {} should not have coverage when None is passed",
            block.id
        );
    }
}

#[test]
fn test_export_json_coverage_db_id_none() {
    let mut cfg = DiGraph::new();
    let _b0 = cfg.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let mut coverage = HashMap::new();
    coverage.insert(1i64, 5i64);

    let export = export_json(&cfg, "test_function", Some(&coverage));

    let block0 = export.blocks.iter().find(|b| b.id == 0).unwrap();
    assert!(
        block0.coverage.is_none(),
        "Block without db_id should not have coverage"
    );
}

#[test]
fn test_coverage_query_from_database() {
    let (_dir, db_path) = create_test_db_with_coverage();
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    // Query coverage for function 1
    let mut stmt = conn
        .prepare(
            "SELECT bc.block_id, bc.hit_count
             FROM cfg_block_coverage bc
             JOIN cfg_blocks bb ON bc.block_id = bb.id
             WHERE bb.function_id = ?1",
        )
        .unwrap();

    let rows: Vec<(i64, i64)> = stmt
        .query_map([1i64], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(rows.len(), 2, "Should have 2 coverage rows");
    assert!(rows.contains(&(1, 5)), "Block 1 should have 5 hits");
    assert!(rows.contains(&(2, 10)), "Block 2 should have 10 hits");
}

#[test]
fn test_cfg_blocks_table_matches_current_magellan_schema() {
    let (_dir, db_path) = create_test_db_with_coverage();
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    let mut stmt = conn.prepare("PRAGMA table_info(cfg_blocks)").unwrap();
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert!(columns.contains(&"cfg_hash".to_string()));
    assert!(columns.contains(&"statements".to_string()));
    assert!(columns.contains(&"cfg_condition".to_string()));
    assert!(!columns.contains(&"coord_x".to_string()));
    assert!(!columns.contains(&"coord_y".to_string()));
    assert!(!columns.contains(&"coord_z".to_string()));
}

#[test]
fn test_coverage_command_outputs_rows_with_kind_and_hits() {
    let (_dir, db_path) = create_test_db_with_coverage();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mirage"))
        .arg("--db")
        .arg(&db_path)
        .arg("--output")
        .arg("json")
        .arg("coverage")
        .arg("--function")
        .arg("test_function")
        .output()
        .expect("failed to run mirage coverage command");

    assert!(
        output.status.success(),
        "coverage command failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("coverage command should emit JSON");
    let coverage = json["data"]["coverage"]
        .as_array()
        .expect("coverage should be an array");

    assert_eq!(
        coverage.len(),
        3,
        "coverage command should return all CFG blocks"
    );
    assert_eq!(coverage[0]["kind"], "entry");
    assert_eq!(coverage[0]["hit_count"], 5);
    assert_eq!(coverage[1]["kind"], "normal");
    assert_eq!(coverage[1]["hit_count"], 10);
    assert_eq!(coverage[2]["kind"], "return");
    assert_eq!(coverage[2]["hit_count"], 0);
}
