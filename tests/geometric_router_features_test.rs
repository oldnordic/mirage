//! Integration tests for GeometricRouter analysis features
//!
//! Tests verify that the geometric backend router correctly implements:
//! - Path enumeration
//! - Loop detection
//! - Dominator analysis
//! - Dominance frontiers
//! - Blast zone analysis
//! - Program slicing
//! - Hotspot detection
//! - ICFG construction

#![cfg(feature = "backend-geometric")]

use mirage::router::{BackendRouter, GeometricRouter, SliceDirection};
use tempfile::TempDir;

/// Create a test .geo database with a function that has:
/// - Entry block -> Block A -> Exit block (linear path)
/// - Entry block -> Block B (conditional) -> Exit block (branch)
fn create_test_geo_with_cfg() -> (TempDir, std::path::PathBuf, u64) {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test.geo");

    // Create geometric database
    let backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");

    // Insert a test function symbol
    use magellan::ingest::{Language, SymbolKind};
    let symbols = vec![magellan::graph::geometric_backend::InsertSymbol {
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
    }];

    let ids = backend
        .insert_symbols(symbols)
        .expect("Failed to insert symbols");
    let function_id = ids[0];

    // Insert CFG blocks for the function
    // Block 0: Entry -> Block 1 (conditional)
    // Block 1: Conditional -> Block 2 (true) or Block 3 (false)
    // Block 2: True branch -> Block 4
    // Block 3: False branch -> Block 4
    // Block 4: Exit
    use geographdb_core::SerializableCfgBlock;

    let blocks = vec![
        SerializableCfgBlock {
            id: 0,
            function_id: function_id as i64,
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
            out_edges: vec![],
        },
        SerializableCfgBlock {
            id: 1,
            function_id: function_id as i64,
            block_kind: "conditional".to_string(),
            terminator: "conditional".to_string(),
            byte_start: 10,
            byte_end: 30,
            start_line: 2,
            start_col: 0,
            end_line: 2,
            end_col: 20,
            dominator_depth: 1,
            loop_nesting: 0,
            branch_count: 1,
            out_edges: vec![],
        },
        SerializableCfgBlock {
            id: 2,
            function_id: function_id as i64,
            block_kind: "normal".to_string(),
            terminator: "fallthrough".to_string(),
            byte_start: 30,
            byte_end: 50,
            start_line: 3,
            start_col: 0,
            end_line: 4,
            end_col: 10,
            dominator_depth: 2,
            loop_nesting: 0,
            branch_count: 0,
            out_edges: vec![],
        },
        SerializableCfgBlock {
            id: 3,
            function_id: function_id as i64,
            block_kind: "normal".to_string(),
            terminator: "fallthrough".to_string(),
            byte_start: 50,
            byte_end: 70,
            start_line: 5,
            start_col: 0,
            end_line: 6,
            end_col: 10,
            dominator_depth: 2,
            loop_nesting: 0,
            branch_count: 0,
            out_edges: vec![],
        },
        SerializableCfgBlock {
            id: 4,
            function_id: function_id as i64,
            block_kind: "exit".to_string(),
            terminator: "return".to_string(),
            byte_start: 70,
            byte_end: 80,
            start_line: 7,
            start_col: 0,
            end_line: 7,
            end_col: 10,
            dominator_depth: 1,
            loop_nesting: 0,
            branch_count: 0,
            out_edges: vec![],
        },
    ];

    for block in blocks {
        backend
            .insert_cfg_block(block)
            .expect("Failed to insert CFG block");
    }

    // Insert CFG edges: 0->1, 1->2, 1->3, 2->4, 3->4
    backend
        .insert_cfg_edge(0, 1, 0)
        .expect("Failed to insert edge");
    backend
        .insert_cfg_edge(1, 2, 0)
        .expect("Failed to insert edge");
    backend
        .insert_cfg_edge(1, 3, 0)
        .expect("Failed to insert edge");
    backend
        .insert_cfg_edge(2, 4, 0)
        .expect("Failed to insert edge");
    backend
        .insert_cfg_edge(3, 4, 0)
        .expect("Failed to insert edge");

    // Save the database
    backend.save_to_disk().expect("Failed to save database");

    (temp_dir, geo_path, function_id)
}

/// Create a test .geo database with a loop:
/// Entry -> Loop Header -> Body -> Back to Header -> Exit
fn create_test_geo_with_loop() -> (TempDir, std::path::PathBuf, u64) {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test_loop.geo");

    let backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");

    use magellan::ingest::{Language, SymbolKind};
    let symbols = vec![magellan::graph::geometric_backend::InsertSymbol {
        fqn: "loop_function".to_string(),
        name: "loop_function".to_string(),
        kind: SymbolKind::Function,
        language: Language::Rust,
        file_path: "src/loop.rs".to_string(),
        byte_start: 0,
        byte_end: 100,
        start_line: 1,
        start_col: 0,
        end_line: 10,
        end_col: 1,
    }];

    let ids = backend
        .insert_symbols(symbols)
        .expect("Failed to insert symbols");
    let function_id = ids[0];

    use geographdb_core::SerializableCfgBlock;
    let blocks = vec![
        SerializableCfgBlock {
            id: 0,
            function_id: function_id as i64,
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
            out_edges: vec![],
        },
        SerializableCfgBlock {
            id: 1,
            function_id: function_id as i64,
            block_kind: "loop_header".to_string(),
            terminator: "conditional".to_string(),
            byte_start: 10,
            byte_end: 30,
            start_line: 2,
            start_col: 0,
            end_line: 2,
            end_col: 20,
            dominator_depth: 1,
            loop_nesting: 1,
            branch_count: 1,
            out_edges: vec![],
        },
        SerializableCfgBlock {
            id: 2,
            function_id: function_id as i64,
            block_kind: "loop_body".to_string(),
            terminator: "fallthrough".to_string(),
            byte_start: 30,
            byte_end: 50,
            start_line: 3,
            start_col: 0,
            end_line: 4,
            end_col: 10,
            dominator_depth: 2,
            loop_nesting: 1,
            branch_count: 0,
            out_edges: vec![],
        },
        SerializableCfgBlock {
            id: 3,
            function_id: function_id as i64,
            block_kind: "exit".to_string(),
            terminator: "return".to_string(),
            byte_start: 50,
            byte_end: 60,
            start_line: 5,
            start_col: 0,
            end_line: 5,
            end_col: 10,
            dominator_depth: 1,
            loop_nesting: 0,
            branch_count: 0,
            out_edges: vec![],
        },
    ];

    for block in blocks {
        backend
            .insert_cfg_block(block)
            .expect("Failed to insert CFG block");
    }

    // Insert CFG edges with back edge: 0->1, 1->2 (loop), 1->3 (exit), 2->1 (back edge)
    backend
        .insert_cfg_edge(0, 1, 0)
        .expect("Failed to insert edge");
    backend
        .insert_cfg_edge(1, 2, 0)
        .expect("Failed to insert edge");
    backend
        .insert_cfg_edge(1, 3, 0)
        .expect("Failed to insert edge");
    backend
        .insert_cfg_edge(2, 1, 0)
        .expect("Failed to insert edge");

    backend.save_to_disk().expect("Failed to save database");

    (temp_dir, geo_path, function_id)
}

#[test]
fn test_geometric_router_open() {
    let (_temp, geo_path, _function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path);
    assert!(router.is_ok(), "Should open geometric database");
}

#[test]
fn test_geometric_router_status() {
    let (_temp, geo_path, _function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let status = router.status();
    assert!(status.is_ok(), "Should get status");

    let status = status.unwrap();
    assert!(status.cfg_blocks > 0, "Should have CFG blocks");
}

#[test]
fn test_geometric_router_resolve_function() {
    let (_temp, geo_path, function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    // Resolve by ID
    let resolved = router.resolve_function(&function_id.to_string());
    assert!(resolved.is_ok(), "Should resolve function by ID");
    assert_eq!(resolved.unwrap(), function_id as i64);

    // Resolve by name
    let resolved = router.resolve_function("test_function");
    assert!(resolved.is_ok(), "Should resolve function by name");
    assert_eq!(resolved.unwrap(), function_id as i64);
}

#[test]
fn test_geometric_router_load_cfg() {
    let (_temp, geo_path, function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let cfg = router.load_cfg(function_id as i64);
    assert!(cfg.is_ok(), "Should load CFG");

    let cfg = cfg.unwrap();
    assert_eq!(cfg.node_count(), 5, "Should have 5 blocks");
}

#[test]
fn test_geometric_router_get_cfg_blocks() {
    let (_temp, geo_path, function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let blocks = router.get_cfg_blocks(function_id as i64);
    assert!(blocks.is_ok(), "Should get CFG blocks");

    let blocks = blocks.unwrap();
    assert_eq!(blocks.len(), 5, "Should have 5 blocks");
}

#[test]
fn test_geometric_router_enumerate_paths() {
    let (_temp, geo_path, function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let paths = router.enumerate_paths(function_id as i64, 100);
    assert!(paths.is_ok(), "Should enumerate paths");

    let paths = paths.unwrap();
    // Should have at least 2 paths: entry->1->2->4 and entry->1->3->4
    assert!(!paths.is_empty(), "Should have paths");
}

#[test]
fn test_geometric_router_get_dominators() {
    let (_temp, geo_path, function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let dominators = router.get_dominators(function_id as i64);
    assert!(dominators.is_ok(), "Should compute dominators");

    let dominators = dominators.unwrap();
    assert_eq!(dominators.function_id, function_id as i64);
    // Entry block (block 0) should dominate everything
    assert!(
        dominators.dominators.contains_key(&0),
        "Should have entry block"
    );
}

#[test]
fn test_geometric_router_get_loops() {
    let (_temp, geo_path, function_id) = create_test_geo_with_loop();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let loops = router.get_loops(function_id as i64);
    assert!(loops.is_ok(), "Should detect loops");

    let loops = loops.unwrap();
    // Should detect 1 loop (the back edge from block 2 to block 1)
    assert!(!loops.is_empty(), "Should detect at least one loop");
}

#[test]
fn test_geometric_router_get_frontiers() {
    let (_temp, geo_path, function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let frontiers = router.get_frontiers(function_id as i64);
    assert!(frontiers.is_ok(), "Should compute dominance frontiers");

    let frontiers = frontiers.unwrap();
    assert_eq!(frontiers.function_id, function_id as i64);
}

#[test]
fn test_geometric_router_find_cycles() {
    let (_temp, geo_path, _function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let cycles = router.find_cycles();
    assert!(cycles.is_ok(), "Should find cycles");

    let cycles = cycles.unwrap();
    // Empty database has no call graph cycles
    assert!(
        cycles.is_empty(),
        "Should have no call graph cycles in simple test"
    );
}

#[test]
fn test_geometric_router_get_blast_zone() {
    let (_temp, geo_path, function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let blast_zone = router.get_blast_zone(function_id as i64, None);
    assert!(blast_zone.is_ok(), "Should compute blast zone");

    let blast_zone = blast_zone.unwrap();
    assert_eq!(blast_zone.center_function, function_id as i64);
}

#[test]
fn test_geometric_router_slice_forward() {
    let (_temp, geo_path, _function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let slice = router.slice("test_function", SliceDirection::Forward);
    assert!(slice.is_ok(), "Should compute forward slice");
}

#[test]
fn test_geometric_router_slice_backward() {
    let (_temp, geo_path, _function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let slice = router.slice("test_function", SliceDirection::Backward);
    assert!(slice.is_ok(), "Should compute backward slice");
}

#[test]
fn test_geometric_router_get_hotspots() {
    let (_temp, geo_path, _function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let hotspots = router.get_hotspots();
    assert!(hotspots.is_ok(), "Should compute hotspots");

    let hotspots = hotspots.unwrap();
    assert!(!hotspots.is_empty(), "Should have at least one hotspot");
}

#[test]
fn test_geometric_router_get_icfg() {
    let (_temp, geo_path, function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let icfg = router.get_icfg(function_id as i64);
    assert!(icfg.is_ok(), "Should build ICFG");

    let icfg = icfg.unwrap();
    assert_eq!(icfg.entry_function, function_id as i64);
}

#[test]
fn test_geometric_router_get_call_graph() {
    let (_temp, geo_path, _function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let call_graph = router.get_call_graph();
    assert!(call_graph.is_ok(), "Should get call graph");

    let call_graph = call_graph.unwrap();
    assert!(
        !call_graph.nodes.is_empty(),
        "Should have at least one node"
    );
}

#[test]
fn test_geometric_router_get_patterns() {
    let (_temp, geo_path, function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    let patterns = router.get_patterns(function_id as i64);
    assert!(patterns.is_ok(), "Should detect patterns");

    let patterns = patterns.unwrap();
    // Should detect if/else pattern in our test CFG
    assert!(!patterns.is_empty(), "Should detect branching patterns");
}

// ============================================================================
// Function Resolution Tests (deduplication fix)
// ============================================================================

#[test]
fn geometric_mirage_resolve_function_by_unique_simple_name() {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test_resolve.geo");

    // Create geometric database with unique symbol
    let backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");

    use magellan::ingest::{Language, SymbolKind};
    let symbols = vec![magellan::graph::geometric_backend::InsertSymbol {
        fqn: "crate::module::unique_function".to_string(),
        name: "unique_function".to_string(),
        kind: SymbolKind::Function,
        language: Language::Rust,
        file_path: "src/lib.rs".to_string(),
        byte_start: 0,
        byte_end: 100,
        start_line: 10,
        start_col: 4,
        end_line: 20,
        end_col: 5,
    }];

    let ids = backend
        .insert_symbols(symbols)
        .expect("Failed to insert symbols");
    let function_id = ids[0];
    backend.save_to_disk().unwrap();

    // Test resolution by unique simple name
    let router = GeometricRouter::open(&geo_path).unwrap();
    let resolved = router.resolve_function("unique_function");
    assert!(
        resolved.is_ok(),
        "Should resolve unique function by simple name: {:?}",
        resolved.err()
    );
    assert_eq!(resolved.unwrap(), function_id as i64);
}

#[test]
fn geometric_mirage_resolve_function_by_fqn() {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test_resolve_fqn.geo");

    let backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");

    use magellan::ingest::{Language, SymbolKind};
    let symbols = vec![magellan::graph::geometric_backend::InsertSymbol {
        fqn: "crate::my_module::my_function".to_string(),
        name: "my_function".to_string(),
        kind: SymbolKind::Function,
        language: Language::Rust,
        file_path: "src/my_module.rs".to_string(),
        byte_start: 0,
        byte_end: 100,
        start_line: 1,
        start_col: 0,
        end_line: 10,
        end_col: 1,
    }];

    let ids = backend
        .insert_symbols(symbols)
        .expect("Failed to insert symbols");
    let function_id = ids[0];
    backend.save_to_disk().unwrap();

    let router = GeometricRouter::open(&geo_path).unwrap();
    let resolved = router.resolve_function("crate::my_module::my_function");
    assert!(resolved.is_ok(), "Should resolve function by FQN");
    assert_eq!(resolved.unwrap(), function_id as i64);
}

#[test]
fn geometric_mirage_resolve_function_by_id() {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test_resolve_id.geo");

    let backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");

    use magellan::ingest::{Language, SymbolKind};
    let symbols = vec![magellan::graph::geometric_backend::InsertSymbol {
        fqn: "test_by_id".to_string(),
        name: "test_by_id".to_string(),
        kind: SymbolKind::Function,
        language: Language::Rust,
        file_path: "src/test.rs".to_string(),
        byte_start: 0,
        byte_end: 100,
        start_line: 1,
        start_col: 0,
        end_line: 10,
        end_col: 1,
    }];

    let ids = backend
        .insert_symbols(symbols)
        .expect("Failed to insert symbols");
    let function_id = ids[0];
    backend.save_to_disk().unwrap();

    let router = GeometricRouter::open(&geo_path).unwrap();
    let resolved = router.resolve_function(&function_id.to_string());
    assert!(resolved.is_ok(), "Should resolve function by numeric ID");
    assert_eq!(resolved.unwrap(), function_id as i64);
}

#[test]
fn geometric_mirage_resolve_function_reports_ambiguity() {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test_resolve_ambiguous.geo");

    let backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");

    use magellan::ingest::{Language, SymbolKind};
    // Insert two DIFFERENT functions with the same name in different files
    let symbols = vec![
        magellan::graph::geometric_backend::InsertSymbol {
            fqn: "crate::a::common".to_string(),
            name: "common".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            file_path: "src/a.rs".to_string(),
            byte_start: 0,
            byte_end: 100,
            start_line: 1, // Different location
            start_col: 0,
            end_line: 10,
            end_col: 1,
        },
        magellan::graph::geometric_backend::InsertSymbol {
            fqn: "crate::b::common".to_string(),
            name: "common".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            file_path: "src/b.rs".to_string(),
            byte_start: 0,
            byte_end: 100,
            start_line: 5, // Different location
            start_col: 0,
            end_line: 15,
            end_col: 1,
        },
    ];

    backend
        .insert_symbols(symbols)
        .expect("Failed to insert symbols");
    backend.save_to_disk().unwrap();

    let router = GeometricRouter::open(&geo_path).unwrap();
    let resolved = router.resolve_function("common");
    assert!(
        resolved.is_err(),
        "Should report ambiguity for multiple different functions"
    );
    let err_msg = resolved.unwrap_err().to_string();
    assert!(
        err_msg.contains("Ambiguous") || err_msg.contains("candidates"),
        "Error should mention ambiguity: {}",
        err_msg
    );
}

#[test]
fn geometric_mirage_resolve_function_deduplicates_duplicates() {
    // This test verifies that duplicate symbols (same name, file, location)
    // are deduplicated before ambiguity checking
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test_resolve_dedup.geo");

    let backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");

    use magellan::ingest::{Language, SymbolKind};
    let file_path = "src/test.rs".to_string();
    let name = "duplicate_func".to_string();
    let line = 10u64;
    let col = 4u64;

    // Insert the same symbol multiple times (simulating duplicate indexing)
    for _ in 0..5 {
        let symbol = magellan::graph::geometric_backend::InsertSymbol {
            fqn: "crate::test::duplicate_func".to_string(),
            name: name.clone(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            file_path: file_path.clone(),
            byte_start: 0,
            byte_end: 100,
            start_line: line,
            start_col: col,
            end_line: 20,
            end_col: 5,
        };
        backend
            .insert_symbols(vec![symbol])
            .expect("Failed to insert symbol");
    }

    backend.save_to_disk().unwrap();

    let router = GeometricRouter::open(&geo_path).unwrap();
    // Should resolve successfully because duplicates are deduplicated
    let resolved = router.resolve_function("duplicate_func");
    assert!(
        resolved.is_ok(),
        "Should resolve function even with duplicates in DB: {:?}",
        resolved.err()
    );
}

#[test]
fn geometric_mirage_cfg_command_works_with_unique_simple_name() {
    let (_temp, geo_path, function_id) = create_test_geo_with_cfg();
    let router = GeometricRouter::open(&geo_path).unwrap();

    // First verify we can resolve the function
    let resolved = router.resolve_function("test_function");
    assert!(resolved.is_ok(), "Should resolve test_function by name");
    assert_eq!(resolved.unwrap(), function_id as i64);

    // Then verify we can load its CFG
    let cfg = router.load_cfg(function_id as i64);
    assert!(cfg.is_ok(), "Should load CFG after resolving by name");
}
