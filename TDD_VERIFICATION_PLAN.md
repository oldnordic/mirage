# TDD Verification Plan: 4D Coordinate Integration

**Date:** 2026-04-13
**Status:** ✅ BASELINE COMPLETE - All tests passing (236 passed, 0 failed)
**Approach:** Strict TDD - No mocks, no stubs, no placeholders, no TODOs

## Current State Assessment

### ✅ Already Completed
- Database schema includes `coord_x`, `coord_y`, `coord_z` columns
- `BasicBlock` struct has coordinate fields defined
- All 236 existing tests pass
- Project compiles successfully

### 🎯 Verification Goal
Ensure 4D coordinates work end-to-end through the entire pipeline:
1. Database storage → 2. CFG construction → 3. Analysis → 4. Output

---

## Phase 1: Database Integration Tests (Real SQLite)

### Test 1.1: Verify Coordinate Columns Exist
**File:** `tests/coordinate_schema_test.rs`

```rust
#[test]
fn test_coordinate_columns_exist_in_schema() {
    // Given: A fresh SQLite database
    let db = tempfile::NamedTempFile::new().unwrap();
    let backend = SqliteBackend::new(db.path().to_str().unwrap()).unwrap();

    // When: Querying table schema
    let conn = backend.conn().unwrap();
    let columns: Vec<String> = conn
        .prepare("SELECT name FROM pragma_table_info('cfg_blocks') ORDER BY cid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|x| x.ok())
        .collect();

    // Then: Coordinate columns must exist
    assert!(columns.contains(&"coord_x".to_string()), "Missing coord_x column");
    assert!(columns.contains(&"coord_y".to_string()), "Missing coord_y column");
    assert!(columns.contains(&"coord_z".to_string()), "Missing coord_z column");
}
```

### Test 1.2: Coordinate Default Values
**File:** `tests/coordinate_schema_test.rs`

```rust
#[test]
fn test_coordinate_columns_default_to_zero() {
    // Given: A database with a CFG block
    let db = tempfile::NamedTempFile::new().unwrap();
    let backend = SqliteBackend::new(db.path().to_str().unwrap()).unwrap();
    backend.create_schema().unwrap();

    // When: Inserting a block without specifying coordinates
    let conn = backend.conn().unwrap();
    conn.execute(
        "INSERT INTO graph_entities (id, name, kind) VALUES (1, 'test_func', 'function')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO cfg_blocks (function_id, kind, terminator)
         VALUES (1, 'entry', 'return')",
        [],
    ).unwrap();

    // Then: Coordinates should default to 0
    let coords: (i64, i64, i64) = conn
        .query_row(
            "SELECT coord_x, coord_y, coord_z FROM cfg_blocks WHERE id = 1",
            [],
            |row| (row.get(0), row.get(1), row.get(2)),
        )
        .unwrap();

    assert_eq!(coords, (0, 0, 0), "Coordinates should default to zero");
}
```

---

## Phase 2: CFG Construction Tests (Real Data Flow)

### Test 2.1: BasicBlock Coordinate Construction
**File:** `tests/coordinate_cfg_test.rs`

```rust
#[test]
fn test_basicblock_with_coordinates() {
    // Given: A BasicBlock with coordinates
    let block = BasicBlock {
        id: 0,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,  // Dominator depth
        coord_y: 0,  // Loop nesting
        coord_z: 0,  // Branch distance
    };

    // When: Adding to CFG graph
    let mut cfg = Cfg::new();
    let node_idx = cfg.add_node(block);

    // Then: Node should be accessible with coordinates preserved
    let retrieved = cfg.node_weight(node_idx).unwrap();
    assert_eq!(retrieved.coord_x, 0);
    assert_eq!(retrieved.coord_y, 0);
    assert_eq!(retrieved.coord_z, 0);
}
```

### Test 2.2: Coordinate Propagation Through Pipeline
**File:** `tests/coordinate_integration_test.rs`

```rust
#[test]
fn test_coordinates_propagate_from_db_to_cfg() {
    // Given: A database with blocks having non-zero coordinates
    let db = tempfile::NamedTempFile::new().unwrap();
    let backend = SqliteBackend::new(db.path().to_str().unwrap()).unwrap();
    backend.create_schema().unwrap();
    let conn = backend.conn().unwrap();

    // Create function and block with specific coordinates
    conn.execute(
        "INSERT INTO graph_entities (id, name, kind) VALUES (1, 'coord_func', 'function')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO cfg_blocks (function_id, kind, terminator, coord_x, coord_y, coord_z)
         VALUES (1, 'entry', 'goto', 1, 2, 3)",
        [],
    ).unwrap();

    // When: Loading CFG from database
    let cfg = load_cfg_from_db(&backend, "coord_func", None).unwrap();

    // Then: Coordinates should be preserved
    let entry_block = cfg.node_weight(NodeIndex::new(0)).unwrap();
    assert_eq!(entry_block.coord_x, 1, "X coordinate (dominator depth) should match");
    assert_eq!(entry_block.coord_y, 2, "Y coordinate (loop nesting) should match");
    assert_eq!(entry_block.coord_z, 3, "Z coordinate (branch distance) should match");
}
```

---

## Phase 3: Analysis Algorithm Tests (Real Graph Algorithms)

### Test 3.1: Dominator Depth Calculation (coord_x)
**File:** `tests/coordinate_analysis_test.rs`

```rust
#[test]
fn test_dominator_depth_affects_coord_x() {
    // Given: A CFG with nested structure
    let mut cfg = Cfg::new();

    // Entry block (depth 0)
    let b0 = cfg.add_node(BasicBlock {
        id: 0, kind: BlockKind::Entry, statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None, coord_x: 0, coord_y: 0, coord_z: 0,
    });

    // Direct child (depth 1)
    let b1 = cfg.add_node(BasicBlock {
        id: 1, kind: BlockKind::Normal, statements: vec![],
        terminator: Terminator::Return,
        source_location: None, coord_x: 1, coord_y: 0, coord_z: 1,
    });

    cfg.add_edge(b0, b1, EdgeType::Fallthrough);

    // When: Computing dominator tree
    let dom_tree = DominatorTree::new(&cfg, b0);

    // Then: coord_x should reflect dominator depth
    assert_eq!(cfg.node_weight(b0).unwrap().coord_x, 0, "Entry has depth 0");
    assert_eq!(cfg.node_weight(b1).unwrap().coord_x, 1, "Child has depth 1");
}
```

### Test 3.2: Loop Nesting Detection (coord_y)
**File:** `tests/coordinate_analysis_test.rs`

```rust
#[test]
fn test_loop_nesting_affects_coord_y() {
    // Given: A CFG with nested loops
    let mut cfg = Cfg::new();

    // Entry
    let b0 = cfg.add_node(BasicBlock {
        id: 0, kind: BlockKind::Entry, statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None, coord_x: 0, coord_y: 0, coord_z: 0,
    });

    // Loop header (nesting level 1)
    let b1 = cfg.add_node(BasicBlock {
        id: 1, kind: BlockKind::Normal, statements: vec![],
        terminator: Terminator::Conditional,
        source_location: None, coord_x: 1, coord_y: 1, coord_z: 1,
    });

    // Loop body with nested loop (nesting level 2)
    let b2 = cfg.add_node(BasicBlock {
        id: 2, kind: BlockKind::Normal, statements: vec![],
        terminator: Terminator::Conditional,
        source_location: None, coord_x: 2, coord_y: 2, coord_z: 2,
    });

    cfg.add_edge(b0, b1, EdgeType::Fallthrough);
    cfg.add_edge(b1, b2, EdgeType::TrueBranch);
    cfg.add_edge(b2, b1, EdgeType::Fallthrough); // Back edge (nested loop)

    // When: Detecting natural loops
    let loops = detect_natural_loops(&cfg, b0);

    // Then: coord_y should reflect loop nesting depth
    assert_eq!(cfg.node_weight(b0).unwrap().coord_y, 0, "Entry: no loop nesting");
    assert_eq!(cfg.node_weight(b1).unwrap().coord_y, 1, "Outer loop: nesting level 1");
    assert_eq!(cfg.node_weight(b2).unwrap().coord_y, 2, "Inner loop: nesting level 2");
}
```

---

## Phase 4: Output Format Tests (Real CLI)

### Test 4.1: JSON Output Includes Coordinates
**File:** `tests/coordinate_output_test.rs`

```rust
#[test]
fn test_json_output_includes_coordinates() {
    // Given: A CFG with coordinate data
    let mut cfg = Cfg::new();
    let block = BasicBlock {
        id: 0,
        kind: BlockKind::Entry,
        statements: vec!["let x = 42".to_string()],
        terminator: Terminator::Return,
        source_location: Some(SourceLocation {
            file: "test.rs".to_string(),
            byte_start: 0,
            byte_end: 100,
            start_line: 1,
            start_col: 0,
            end_line: 5,
            end_col: 0,
        }),
        coord_x: 5,
        coord_y: 2,
        coord_z: 3,
    };
    cfg.add_node(block);

    // When: Exporting to JSON
    let export = export_json(&cfg, 0, "test_func").unwrap();

    // Then: JSON must include coordinate fields
    let blocks = export.data.get("blocks").unwrap().as_array().unwrap();
    let first_block = &blocks[0];

    assert_eq!(
        first_block.get("coord_x").unwrap().as_i64().unwrap(),
        5,
        "JSON must include coord_x"
    );
    assert_eq!(
        first_block.get("coord_y").unwrap().as_i64().unwrap(),
        2,
        "JSON must include coord_y"
    );
    assert_eq!(
        first_block.get("coord_z").unwrap().as_i64().unwrap(),
        3,
        "JSON must include coord_z"
    );
}
```

### Test 4.2: Human Readable Output Shows Coordinates
**File:** `tests/coordinate_output_test.rs`

```rust
#[test]
fn test_human_output_includes_coordinates() {
    // Given: A CFG with coordinate data
    let mut cfg = Cfg::new();
    let block = BasicBlock {
        id: 0,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 1,
        coord_y: 0,
        coord_z: 1,
    };
    cfg.add_node(block);

    // When: Exporting to human-readable format
    let output = export_human(&cfg, "test_func");

    // Then: Output must mention coordinates
    assert!(
        output.contains("coord_x: 1") || output.contains("X: 1"),
        "Human output must show X coordinate"
    );
    assert!(
        output.contains("coord_y: 0") || output.contains("Y: 0"),
        "Human output must show Y coordinate"
    );
    assert!(
        output.contains("coord_z: 1") || output.contains("Z: 1"),
        "Human output must show Z coordinate"
    );
}
```

---

## Phase 5: End-to-End Integration Tests (Real CLI)

### Test 5.1: CLI CFG Command Shows Coordinates
**File:** `tests/coordinate_cli_test.rs`

```rust
#[test]
fn test_cli_cfg_command_outputs_coordinates() {
    // Given: A database with coordinate data
    let db = tempfile::NamedTempFile::new().unwrap();
    let backend = setup_test_db_with_coords(db.path()).unwrap();

    // When: Running mirage cfg --output json
    let output = Command::new("./target/release/mirage")
        .arg("--db")
        .arg(db.path().to_str().unwrap())
        .arg("cfg")
        .arg("--function")
        .arg("coord_test_func")
        .arg("--output")
        .arg("json")
        .output()
        .unwrap();

    // Then: JSON output must include coordinates
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let blocks = json["data"]["blocks"].as_array().unwrap();

    assert!(!blocks.is_empty(), "Should have at least one block");

    for block in blocks {
        assert!(block.get("coord_x").is_some(), "Each block must have coord_x");
        assert!(block.get("coord_y").is_some(), "Each block must have coord_y");
        assert!(block.get("coord_z").is_some(), "Each block must have coord_z");
    }
}
```

### Test 5.2: Path Summarization Uses Coordinates
**File:** `tests/coordinate_path_test.rs`

```rust
#[test]
fn test_path_summarization_considers_coordinates() {
    // Given: A complex CFG with coordinate data
    let cfg = create_complex_cfg_with_coords();

    // When: Enumerating paths with summarization
    let paths = enumerate_paths_with_metadata(&cfg, NodeIndex::new(0), &PathLimits::default());

    // Then: Path metadata should include coordinate statistics
    assert!(
        paths.stats.max_coord_x > 0,
        "Should track maximum dominator depth"
    );
    assert!(
        paths.stats.max_coord_y > 0,
        "Should track maximum loop nesting"
    );
    assert!(
        paths.stats.max_coord_z > 0,
        "Should track maximum branch distance"
    );
}
```

---

## Phase 6: Performance & Scalability Tests (Real Large Data)

### Test 6.1: Coordinate Calculation Performance
**File:** `tests/coordinate_performance_test.rs`

```rust
#[test]
fn test_coordinate_calculation_performance() {
    // Given: A large CFG (1000 blocks)
    let large_cfg = generate_large_cfg_with_coords(1000);

    // When: Computing coordinates
    let start = std::time::Instant::now();
    compute_dominator_depth(&large_cfg); // coord_x
    compute_loop_nesting(&large_cfg);    // coord_y
    compute_branch_distance(&large_cfg); // coord_z
    let elapsed = start.elapsed();

    // Then: Should complete in reasonable time
    assert!(
        elapsed.as_millis() < 100,
        "Coordinate calculation should be fast (was: {}ms)",
        elapsed.as_millis()
    );
}
```

---

## Success Criteria

### ✅ Pass Criteria
1. All new tests pass (add ~10 new test functions)
2. All existing tests continue to pass (236 tests)
3. No compilation warnings (except the 4 existing deprecation warnings)
4. CLI commands output coordinate data correctly
5. Performance tests complete within time limits

### ❌ Failure Criteria
1. Any test fails
2. Mock or stub detected in test code
3. Placeholder/TODO/FIXME comments
4. Code removal to "make it work fast"
5. Coordinates not present in any output format

---

## Execution Order

1. **Phase 1**: Database schema tests (foundation)
2. **Phase 2**: CFG construction tests (data flow)
3. **Phase 3**: Analysis algorithm tests (logic)
4. **Phase 4**: Output format tests (presentation)
5. **Phase 5**: CLI integration tests (end-to-end)
6. **Phase 6**: Performance tests (scalability)

**Estimated Time**: 2-3 hours (6 phases, ~2-3 tests per phase)

---

## Notes

- **No Shortcuts**: Every phase must complete successfully before moving to next
- **Real Data Only**: Use actual SQLite, real CFG graphs, real CLI commands
- **TDD Strict**: Write failing test first, then implement minimal code to pass
- **No Removal**: Keep all existing functionality, only add coordinate features
- **Full Integration**: Test from database → analysis → CLI output

---

## Next Steps

1. ✅ Run `cargo test` to confirm baseline (236 passing)
2. Create new test files for each phase
3. Implement tests one phase at a time
4. Verify all tests pass after each phase
5. Document any coordinate calculation algorithms discovered

**Status**: Ready to begin TDD verification
