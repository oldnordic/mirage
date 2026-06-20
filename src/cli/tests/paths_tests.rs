// ============================================================================

use crate::cfg::{enumerate_paths, PathKind, PathLimits};
use crate::cli::responses::*;
use crate::cli::*;

/// Test that paths() command enumerates paths from a test CFG
#[test]
fn test_paths_enumeration_basic() {
    let cfg = cmds::create_test_cfg();
    let limits = PathLimits::default();
    let paths = enumerate_paths(&cfg, &limits);

    // Test CFG has 2 paths (entry -> true -> return, entry -> false -> return)
    assert!(!paths.is_empty(), "Should find at least one path");
    assert_eq!(paths.len(), 2, "Test CFG should have exactly 2 paths");

    // Both paths should be Normal kind (no errors in test CFG)
    let normal_count = paths.iter().filter(|p| p.kind == PathKind::Normal).count();
    assert_eq!(normal_count, 2, "Both paths should be Normal");
}

/// Test that show_errors flag filters to error paths only
#[test]
fn test_paths_show_errors_filter() {
    let cfg = cmds::create_test_cfg();
    let limits = PathLimits::default();
    let mut paths = enumerate_paths(&cfg, &limits);

    // Filter to error paths
    paths.retain(|p| p.kind == PathKind::Error);

    // Test CFG has no error paths
    assert_eq!(paths.len(), 0, "Test CFG should have no error paths");

    // Verify filter worked by checking all remaining paths would be errors
    for path in &paths {
        assert_eq!(
            path.kind,
            PathKind::Error,
            "Filtered paths should all be Error kind"
        );
    }
}

/// Test that max_length limit is applied to path enumeration
#[test]
fn test_paths_max_length_limit() {
    let cfg = cmds::create_test_cfg();

    // Set a very low max_length limit
    let limits = PathLimits::default().with_max_length(1);
    let paths = enumerate_paths(&cfg, &limits);

    // All paths should have length <= 1
    for path in &paths {
        assert!(path.len() <= 1, "Path length should be <= max_length limit");
    }

    // With max_length=1, we should get fewer paths than unrestricted
    let unlimited_paths = enumerate_paths(&cfg, &PathLimits::default());
    assert!(
        paths.len() <= unlimited_paths.len(),
        "Limited enumeration should produce <= paths than unlimited"
    );
}

/// Test that PathsArgs.function is extracted correctly
#[test]
fn test_paths_args_function_extraction() {
    let args = PathsArgs {
        function: "test_function".to_string(),
        semantic_query: None,
        file: None,
        inter_procedural: false,
        show_errors: false,
        max_length: None,
        with_blocks: false,
        incremental: false,
        since: None,
        by_coverage: false,
    };

    assert_eq!(args.function, "test_function");
    assert!(!args.show_errors);
    assert!(args.max_length.is_none());
    assert!(!args.with_blocks);
}

/// Test that PathsArgs with flags set correctly reflects state
#[test]
fn test_paths_args_with_flags() {
    let args = PathsArgs {
        function: "my_func".to_string(),
        semantic_query: None,
        file: None,
        inter_procedural: true,
        show_errors: true,
        max_length: Some(10),
        with_blocks: true,
        incremental: false,
        since: None,
        by_coverage: false,
    };

    assert_eq!(args.function, "my_func");
    assert!(
        args.inter_procedural,
        "inter_procedural flag should be true"
    );
    assert!(args.show_errors, "show_errors flag should be true");
    assert_eq!(args.max_length, Some(10), "max_length should be Some(10)");
    assert!(args.with_blocks, "with_blocks flag should be true");
}

/// Test PathSummary conversion from Path
#[test]
fn test_path_summary_from_path() {
    use crate::cfg::Path;

    let path = Path::new(vec![0, 1, 2], PathKind::Normal);
    let summary = PathSummary::from(path);

    assert!(!summary.path_id.is_empty(), "path_id should not be empty");
    assert_eq!(summary.kind, "Normal", "kind should match PathKind");
    assert_eq!(summary.length, 3, "length should match path length");

    // blocks is now Vec<PathBlock> with block_id and terminator
    assert_eq!(summary.blocks.len(), 3, "should have 3 blocks");
    assert_eq!(summary.blocks[0].block_id, 0, "first block_id should be 0");
    assert_eq!(summary.blocks[1].block_id, 1, "second block_id should be 1");
    assert_eq!(summary.blocks[2].block_id, 2, "third block_id should be 2");
    assert_eq!(
        summary.blocks[0].terminator, "Unknown",
        "terminator should be Unknown placeholder"
    );

    // Optional fields should be None until populated in future plans
    assert!(summary.summary.is_none(), "summary should be None");
    assert!(
        summary.source_range.is_none(),
        "source_range should be None"
    );
}

/// Test PathSummary conversion for different PathKinds
#[test]
fn test_path_summary_different_kinds() {
    use crate::cfg::Path;

    let kinds = vec![
        (PathKind::Normal, "Normal"),
        (PathKind::Error, "Error"),
        (PathKind::Degenerate, "Degenerate"),
        (PathKind::Unreachable, "Unreachable"),
    ];

    for (kind, expected_str) in kinds {
        let path = Path::new(vec![0, 1], kind);
        let summary = PathSummary::from(path);
        assert_eq!(
            summary.kind, expected_str,
            "PathKind::{:?} should serialize to {}",
            kind, expected_str
        );
    }
}

/// Test that multiple paths produce multiple PathSummaries
#[test]
fn test_paths_response_multiple_paths() {
    use crate::cfg::Path;

    let paths = vec![
        Path::new(vec![0, 1], PathKind::Normal),
        Path::new(vec![0, 2], PathKind::Normal),
        Path::new(vec![0, 1, 3], PathKind::Error),
    ];

    let summaries: Vec<PathSummary> = paths.into_iter().map(PathSummary::from).collect();

    assert_eq!(summaries.len(), 3, "Should have 3 summaries");

    // Check that error path is correctly identified
    let error_summaries = summaries.iter().filter(|s| s.kind == "Error").count();
    assert_eq!(error_summaries, 1, "Should have 1 error path");
}

/// Test PathsResponse contains expected metadata
#[test]
fn test_paths_response_metadata() {
    let response = PathsResponse {
        function: "test_func".to_string(),
        total_paths: 5,
        error_paths: 2,
        paths: vec![],
    };

    assert_eq!(response.function, "test_func");
    assert_eq!(response.total_paths, 5);
    assert_eq!(response.error_paths, 2);
    assert!(response.paths.is_empty());
}

/// Test integration: create_test_cfg produces enumerable paths
#[test]
fn test_paths_integration_with_test_cfg() {
    let cfg = cmds::create_test_cfg();
    let limits = PathLimits::default();
    let paths = enumerate_paths(&cfg, &limits);

    // Verify we got the expected number of paths for the diamond CFG
    assert!(!paths.is_empty(), "Test CFG should produce paths");

    // Each path should start at entry (block 0)
    for path in &paths {
        assert_eq!(path.blocks[0], 0, "All paths should start at entry block 0");
        assert_eq!(path.entry, 0, "Path entry should be block 0");
    }

    // Each path should end at an exit block
    for path in &paths {
        assert!(
            path.exit == 2 || path.exit == 3,
            "Path exit should be either block 2 or 3 (the return blocks)"
        );
    }
}

/// Test that with_blocks flag affects output format (integration check)
#[test]
fn test_paths_args_with_blocks_flag() {
    let args_with = PathsArgs {
        function: "test".to_string(),
        semantic_query: None,
        file: None,
        inter_procedural: false,
        show_errors: false,
        max_length: None,
        with_blocks: true,
        incremental: false,
        since: None,
        by_coverage: false,
    };

    let args_without = PathsArgs {
        function: "test".to_string(),
        semantic_query: None,
        file: None,
        inter_procedural: false,
        show_errors: false,
        max_length: None,
        with_blocks: false,
        incremental: false,
        since: None,
        by_coverage: false,
    };

    assert!(args_with.with_blocks, "with_blocks should be true");
    assert!(!args_without.with_blocks, "with_blocks should be false");
}

#[test]
fn test_paths_args_inter_procedural_flag() {
    let args = PathsArgs {
        function: "caller".to_string(),
        semantic_query: None,
        file: None,
        inter_procedural: true,
        show_errors: false,
        max_length: Some(12),
        with_blocks: false,
        incremental: false,
        since: None,
        by_coverage: false,
    };

    assert!(args.inter_procedural);
    assert_eq!(args.max_length, Some(12));
}

/// Test PathSummary::from_with_cfg with source locations
#[test]
fn test_path_summary_from_with_cfg() {
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Path, PathKind, SourceLocation, Terminator};
    use petgraph::graph::DiGraph;
    use std::path::PathBuf;

    // Create a test CFG with source locations
    let mut g = DiGraph::new();

    let loc0 = SourceLocation {
        file_path: PathBuf::from("test.rs"),
        byte_start: 0,
        byte_end: 10,
        start_line: 1,
        start_column: 1,
        end_line: 1,
        end_column: 10,
    };

    let loc1 = SourceLocation {
        file_path: PathBuf::from("test.rs"),
        byte_start: 11,
        byte_end: 20,
        start_line: 2,
        start_column: 1,
        end_line: 2,
        end_column: 10,
    };

    let loc2 = SourceLocation {
        file_path: PathBuf::from("test.rs"),
        byte_start: 21,
        byte_end: 30,
        start_line: 3,
        start_column: 1,
        end_line: 3,
        end_column: 10,
    };

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec!["let x = 1".to_string()],
        terminator: Terminator::Goto { target: 1 },
        source_location: Some(loc0),
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["if x > 0".to_string()],
        terminator: Terminator::SwitchInt {
            targets: vec![2],
            otherwise: 2,
        },
        source_location: Some(loc1),
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["return true".to_string()],
        terminator: Terminator::Return,
        source_location: Some(loc2),
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);

    // Create a path and use from_with_cfg
    let path = Path::new(vec![0, 1, 2], PathKind::Normal);
    let summary = PathSummary::from_with_cfg(path, &g);

    // Check terminator is populated
    assert_eq!(summary.blocks[0].terminator, "Goto { target: 1 }");
    assert!(summary.blocks[1].terminator.contains("SwitchInt"));
    assert_eq!(summary.blocks[2].terminator, "Return");

    // Check source_range is populated
    assert!(
        summary.source_range.is_some(),
        "source_range should be Some"
    );
    let sr = summary.source_range.as_ref().unwrap();
    assert_eq!(sr.file_path, "test.rs");
    assert_eq!(sr.start_line, 1);
    assert_eq!(sr.end_line, 3);
}

/// Test PathSummary::from_with_cfg with no source locations (graceful None)
#[test]
fn test_path_summary_from_with_cfg_no_source_locations() {
    use crate::cfg::{Path, PathKind};

    // Use the test CFG which has no source locations
    let cfg = cmds::create_test_cfg();
    let path = Path::new(vec![0, 1, 2], PathKind::Normal);
    let summary = PathSummary::from_with_cfg(path, &cfg);

    // Terminator should still be populated
    assert!(summary.blocks[0].terminator.contains("Goto"));
    assert!(summary.blocks[1].terminator.contains("SwitchInt"));
    assert_eq!(summary.blocks[2].terminator, "Return");

    // source_range should be None when no source locations exist
    assert!(
        summary.source_range.is_none(),
        "source_range should be None when CFG has no locations"
    );
}

// ------------------------------------------------------------------------
// Path Caching Tests
// ------------------------------------------------------------------------

/// Test that first call enumerates paths (cache miss)
#[test]
fn test_paths_cache_miss_first_call() {
    use crate::cfg::get_or_enumerate_paths;
    use crate::storage::create_schema;
    use rusqlite::Connection;

    // Create an in-memory database with Mirage schema
    let mut conn = Connection::open_in_memory().unwrap();

    // Create Magellan schema first (required for Mirage schema)
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
         VALUES (1, 4, 3, 0)",
        [],
    ).unwrap();

    // Create Mirage schema
    create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

    // Get test CFG and limits
    let cfg = cmds::create_test_cfg();
    let limits = PathLimits::default();
    let test_function_id: i64 = 1; // First auto-increment ID;
                                   // Insert a test function entity (required for foreign key constraint)
    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
        rusqlite::params!("function", "test_func", "test.rs", "{}"),
    )
    .unwrap();

    // Enable foreign key enforcement
    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
    let test_function_hash: &str = "test_cfg";

    // First call should enumerate (no cache)
    let paths1 = get_or_enumerate_paths(
        &cfg,
        test_function_id,
        test_function_hash,
        &limits,
        &mut conn,
    )
    .unwrap();

    // Verify we got paths
    assert!(
        !paths1.is_empty(),
        "First call should enumerate and return paths"
    );
    assert_eq!(paths1.len(), 2, "Test CFG should have 2 paths");

    // Verify paths were stored in database
    let path_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(
        path_count, 2,
        "Paths should be stored in database after first call"
    );

    // Note: function_hash verification removed - not available in Magellan schema
}

/// Test that second call returns cached paths (cache hit)
#[test]
fn test_paths_cache_hit_second_call() {
    use crate::cfg::get_or_enumerate_paths;
    use crate::storage::create_schema;
    use rusqlite::Connection;

    // Create an in-memory database with Mirage schema
    let mut conn = Connection::open_in_memory().unwrap();

    // Create Magellan schema first
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
         VALUES (1, 4, 3, 0)",
        [],
    ).unwrap();

    // Create Mirage schema
    create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();
    // Insert a test function entity (required for foreign key constraint)
    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
        rusqlite::params!("function", "test_func", "test.rs", "{}"),
    )
    .unwrap();

    // Enable foreign key enforcement
    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

    // Get test CFG and limits
    let cfg = cmds::create_test_cfg();
    let limits = PathLimits::default();
    let test_function_id: i64 = 1; // First auto-increment ID;
    let test_function_hash: &str = "test_cfg";

    // First call - cache miss, enumerates and stores
    let paths1 = get_or_enumerate_paths(
        &cfg,
        test_function_id,
        test_function_hash,
        &limits,
        &mut conn,
    )
    .unwrap();

    // Verify paths were stored
    let path_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(path_count, 2, "Should have 2 paths stored after first call");

    // Second call - cache hit, should return same paths
    let paths2 = get_or_enumerate_paths(
        &cfg,
        test_function_id,
        test_function_hash,
        &limits,
        &mut conn,
    )
    .unwrap();

    // Should return same number of paths
    assert_eq!(
        paths2.len(),
        paths1.len(),
        "Cache hit should return same number of paths"
    );

    // Paths should have identical path_ids (cache hit returns same data)
    let mut path_ids1: Vec<_> = paths1.iter().map(|p| &p.path_id).collect();
    let mut path_ids2: Vec<_> = paths2.iter().map(|p| &p.path_id).collect();
    path_ids1.sort();
    path_ids2.sort();

    assert_eq!(
        path_ids1, path_ids2,
        "Cache hit should return paths with same IDs"
    );

    // Verify path entries match
    for (p1, p2) in paths1.iter().zip(paths2.iter()) {
        assert_eq!(p1.path_id, p2.path_id, "Path IDs should match on cache hit");
        assert_eq!(p1.kind, p2.kind, "Path kinds should match on cache hit");
        assert_eq!(
            p1.blocks, p2.blocks,
            "Path blocks should match on cache hit"
        );
    }
}

/// Test that function hash change invalidates cache
#[test]
fn test_paths_cache_invalidation_on_hash_change() {
    use crate::cfg::get_or_enumerate_paths;
    use crate::storage::create_schema;
    use rusqlite::Connection;

    // Create an in-memory database with Mirage schema
    let mut conn = Connection::open_in_memory().unwrap();

    // Create Magellan schema first
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
         VALUES (1, 4, 3, 0)",
        [],
    ).unwrap();

    // Create Mirage schema
    create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();
    // Insert a test function entity (required for foreign key constraint)
    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
        rusqlite::params!("function", "test_func", "test.rs", "{}"),
    )
    .unwrap();

    // Enable foreign key enforcement
    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

    // Get test CFG and limits
    let cfg = cmds::create_test_cfg();
    let limits = PathLimits::default();
    let test_function_id: i64 = 1; // First auto-increment ID;
    let test_function_hash_v1: &str = "test_cfg_v1";
    let test_function_hash_v3: &str = "test_cfg_v3";

    // First call with hash v1 - cache miss, enumerates and stores
    let paths1 = get_or_enumerate_paths(
        &cfg,
        test_function_id,
        test_function_hash_v1,
        &limits,
        &mut conn,
    )
    .unwrap();

    // Verify paths were stored
    let path_count_v1: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(path_count_v1, 2, "Should have 2 paths after first call");

    // Second call with different hash - cache invalidation, should re-enumerate
    // Note: With Magellan schema, hash-based caching is not available
    // Paths are always invalidated and re-stored on each call
    let paths2 = get_or_enumerate_paths(
        &cfg,
        test_function_id,
        test_function_hash_v3,
        &limits,
        &mut conn,
    )
    .unwrap();

    // Should still return paths (re-enumerated)
    assert!(!paths2.is_empty(), "Should re-enumerate");
    assert_eq!(
        paths2.len(),
        paths1.len(),
        "Re-enumeration should produce same paths"
    );

    // Verify paths were updated (old invalidated, new stored)
    let path_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            rusqlite::params![test_function_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(path_count, 2, "Should have 2 paths after re-enumeration");
}
