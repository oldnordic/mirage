// ============================================================================

use crate::cfg::{DominatorTree, PostDominatorTree};
use crate::cli::responses::*;
use crate::cli::*;
use tempfile::NamedTempFile;

/// Create a minimal test database
fn create_minimal_db() -> anyhow::Result<NamedTempFile> {
    use crate::storage::{REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION};
    let file = NamedTempFile::new()?;
    let conn = rusqlite::Connection::open(file.path())?;

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
    conn.execute(
        "CREATE TABLE mirage_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            mirage_schema_version INTEGER NOT NULL,
            magellan_schema_version INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE cfg_blocks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            function_id INTEGER NOT NULL,
            kind TEXT NOT NULL,
            byte_start INTEGER NOT NULL,
            byte_end INTEGER NOT NULL,
            terminator TEXT NOT NULL,
            function_hash TEXT NOT NULL
        )",
        [],
    )?;

    // cfg_edges table is managed by Magellan v11+; Mirage does not create it.

    conn.execute(
        "CREATE TABLE cfg_paths (
            path_id TEXT PRIMARY KEY,
            function_id INTEGER NOT NULL,
            path_kind TEXT NOT NULL,
            entry_block INTEGER NOT NULL,
            exit_block INTEGER NOT NULL,
            length INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE cfg_dominators (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            block_id INTEGER NOT NULL,
            dominator_id INTEGER NOT NULL,
            is_strict INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "INSERT INTO mirage_meta (id, mirage_schema_version, magellan_schema_version, created_at)
         VALUES (1, 1, 4, 0)",
        [],
    )?;

    // Add a test function
    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
        rusqlite::params!("function", "test_func", "test.rs", "{}"),
    )?;

    Ok(file)
}

/// Test that DominatorTree can be computed from test CFG
#[test]
fn test_dominator_tree_computation() {
    let cfg = cmds::create_test_cfg();
    let dom_tree = DominatorTree::new(&cfg);

    assert!(
        dom_tree.is_some(),
        "DominatorTree should be computed successfully"
    );

    let dom_tree = dom_tree.unwrap();
    // Entry block (0) should be the root
    assert_eq!(cfg[dom_tree.root()].id, 0, "Root should be entry block");
}

/// Test that PostDominatorTree can be computed from test CFG
#[test]
fn test_post_dominator_tree_computation() {
    let cfg = cmds::create_test_cfg();
    let post_dom_tree = PostDominatorTree::new(&cfg);

    assert!(
        post_dom_tree.is_some(),
        "PostDominatorTree should be computed successfully"
    );

    let post_dom_tree = post_dom_tree.unwrap();
    // Root of post-dominator tree should be an exit block
    let root_id = cfg[post_dom_tree.root()].id;
    assert!(root_id == 2 || root_id == 3, "Root should be an exit block");
}

/// Test immediate dominator relationships in test CFG
#[test]
fn test_immediate_dominator_relationships() {
    let cfg = cmds::create_test_cfg();
    let dom_tree = DominatorTree::new(&cfg).unwrap();

    // Find nodes by block ID
    let node_0 = cfg.node_indices().find(|&n| cfg[n].id == 0).unwrap();
    let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();
    let node_2 = cfg.node_indices().find(|&n| cfg[n].id == 2).unwrap();
    let node_3 = cfg.node_indices().find(|&n| cfg[n].id == 3).unwrap();

    // Entry (0) has no immediate dominator
    assert_eq!(
        dom_tree.immediate_dominator(node_0),
        None,
        "Entry should have no dominator"
    );

    // Node 1 is dominated by entry (0)
    assert_eq!(
        dom_tree.immediate_dominator(node_1),
        Some(node_0),
        "Node 1 should be dominated by entry"
    );

    // Node 2 is dominated by node 1 (through true branch)
    assert_eq!(
        dom_tree.immediate_dominator(node_2),
        Some(node_1),
        "Node 2 should be dominated by node 1"
    );

    // Node 3 is dominated by node 1 (through false branch)
    assert_eq!(
        dom_tree.immediate_dominator(node_3),
        Some(node_1),
        "Node 3 should be dominated by node 1"
    );
}

/// Test dominates() method
#[test]
fn test_dominates_method() {
    let cfg = cmds::create_test_cfg();
    let dom_tree = DominatorTree::new(&cfg).unwrap();

    let node_0 = cfg.node_indices().find(|&n| cfg[n].id == 0).unwrap();
    let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();
    let node_2 = cfg.node_indices().find(|&n| cfg[n].id == 2).unwrap();

    // Entry dominates all nodes
    assert!(dom_tree.dominates(node_0, node_0), "Node dominates itself");
    assert!(dom_tree.dominates(node_0, node_1), "Entry dominates node 1");
    assert!(dom_tree.dominates(node_0, node_2), "Entry dominates node 2");

    // Non-entry doesn't dominate entry
    assert!(
        !dom_tree.dominates(node_1, node_0),
        "Node 1 does not dominate entry"
    );
}

/// Test children() method returns dominated nodes
#[test]
fn test_dominator_tree_children() {
    let cfg = cmds::create_test_cfg();
    let dom_tree = DominatorTree::new(&cfg).unwrap();

    let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();

    // Node 1 should have 2 children (blocks 2 and 3)
    let children = dom_tree.children(node_1);
    assert_eq!(children.len(), 2, "Node 1 should have 2 children");

    let child_ids: Vec<_> = children.iter().map(|&n| cfg[n].id).collect();
    assert!(child_ids.contains(&2), "Children should include block 2");
    assert!(child_ids.contains(&3), "Children should include block 3");
}

/// Test DominatorsArgs struct has expected fields
#[test]
fn test_dominators_args_fields() {
    let args = DominatorsArgs {
        function: "test_func".to_string(),
        file: None,
        must_pass_through: Some("1".to_string()),
        post: false,
        inter_procedural: false,
    };

    assert_eq!(args.function, "test_func");
    assert_eq!(args.must_pass_through, Some("1".to_string()));
    assert!(!args.post);
    assert!(!args.inter_procedural);
}

/// Test DominatorsArgs with --post flag
#[test]
fn test_dominators_args_with_post_flag() {
    let args = DominatorsArgs {
        function: "my_function".to_string(),
        file: None,
        must_pass_through: None,
        post: true,
        inter_procedural: false,
    };

    assert_eq!(args.function, "my_function");
    assert!(args.post, "post flag should be true");
    assert!(
        args.must_pass_through.is_none(),
        "must_pass_through should be None"
    );
    assert!(!args.inter_procedural);
}

/// Test DominanceResponse struct serializes correctly
#[test]
fn test_dominance_response_serialization() {
    let response = DominanceResponse {
        function: "test".to_string(),
        kind: "dominators".to_string(),
        root: Some(0),
        dominance_tree: vec![DominatorEntry {
            block: 0,
            immediate_dominator: None,
            dominated: vec![1],
        }],
        must_pass_through: None,
    };

    let json = serde_json::to_string(&response);
    assert!(json.is_ok(), "DominanceResponse should serialize to JSON");

    let json_str = json.unwrap();
    assert!(json_str.contains("\"function\":\"test\""));
    assert!(json_str.contains("\"kind\":\"dominators\""));
    assert!(json_str.contains("\"root\":0"));
}

/// Test MustPassThroughResult struct
#[test]
fn test_must_pass_through_result() {
    let result = MustPassThroughResult {
        block: 1,
        must_pass: vec![1, 2, 3],
    };

    assert_eq!(result.block, 1);
    assert_eq!(result.must_pass.len(), 3);
    assert_eq!(result.must_pass, vec![1, 2, 3]);

    // Verify it serializes correctly
    let json = serde_json::to_string(&result);
    assert!(json.is_ok());
    let json_str = json.unwrap();
    assert!(json_str.contains("\"block\":1"));
    assert!(json_str.contains("\"must_pass\":[1,2,3]"));
}

/// Test DominatorEntry struct
#[test]
fn test_dominator_entry() {
    let entry = DominatorEntry {
        block: 5,
        immediate_dominator: Some(2),
        dominated: vec![6, 7],
    };

    assert_eq!(entry.block, 5);
    assert_eq!(entry.immediate_dominator, Some(2));
    assert_eq!(entry.dominated, vec![6, 7]);
}

/// Test post-dominates() method
#[test]
fn test_post_dominates_method() {
    let cfg = cmds::create_test_cfg();
    let post_dom_tree = PostDominatorTree::new(&cfg).unwrap();

    let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();
    let node_2 = cfg.node_indices().find(|&n| cfg[n].id == 2).unwrap();

    // Exit post-dominates nodes that can reach it
    assert!(
        post_dom_tree.post_dominates(node_2, node_2),
        "Node post-dominates itself"
    );
    assert!(
        post_dom_tree.post_dominates(node_2, node_1),
        "Exit post-dominates node 1"
    );
}

/// Test immediate post-dominator relationships
#[test]
fn test_immediate_post_dominator_relationships() {
    let cfg = cmds::create_test_cfg();
    let post_dom_tree = PostDominatorTree::new(&cfg).unwrap();

    let node_0 = cfg.node_indices().find(|&n| cfg[n].id == 0).unwrap();
    let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();

    // Node 1 should be immediately post-dominated by an exit (2 or 3)
    let ipdom_1 = post_dom_tree.immediate_post_dominator(node_1);
    assert!(
        ipdom_1.is_some(),
        "Node 1 should have an immediate post-dominator"
    );

    // Node 0 should be immediately post-dominated by node 1
    let ipdom_0 = post_dom_tree.immediate_post_dominator(node_0);
    assert_eq!(
        ipdom_0,
        Some(node_1),
        "Node 0 should be immediately post-dominated by node 1"
    );
}

/// Test that empty CFG returns None for DominatorTree
#[test]
fn test_empty_cfg_dominator_tree() {
    use petgraph::graph::DiGraph;
    let empty_cfg: crate::cfg::Cfg = DiGraph::new();
    let dom_tree = DominatorTree::new(&empty_cfg);

    assert!(
        dom_tree.is_none(),
        "Empty CFG should produce None for DominatorTree"
    );
}

/// Test that empty CFG returns None for PostDominatorTree
#[test]
fn test_empty_cfg_post_dominator_tree() {
    use petgraph::graph::DiGraph;
    let empty_cfg: crate::cfg::Cfg = DiGraph::new();
    let post_dom_tree = PostDominatorTree::new(&empty_cfg);

    assert!(
        post_dom_tree.is_none(),
        "Empty CFG should produce None for PostDominatorTree"
    );
}

/// Test JsonResponse wrapper for DominanceResponse
#[test]
fn test_dominance_response_json_wrapper() {
    use crate::output::JsonResponse;

    let response = DominanceResponse {
        function: "wrapped_test".to_string(),
        kind: "dominators".to_string(),
        root: Some(0),
        dominance_tree: vec![],
        must_pass_through: None,
    };

    let wrapper = JsonResponse::new(response);

    assert_eq!(wrapper.schema_version, "1.0.1");
    assert_eq!(wrapper.tool, "mirage");
    assert!(!wrapper.execution_id.is_empty());
    assert!(!wrapper.timestamp.is_empty());

    // Verify JSON contains expected fields
    let json = wrapper.to_json();
    assert!(json.contains("\"schema_version\":\"1.0.1\""));
    assert!(json.contains("\"tool\":\"mirage\""));
    assert!(json.contains("wrapped_test"));
}

/// Test must-pass-through query with valid block
#[test]
fn test_must_pass_through_valid_block() {
    let cfg = cmds::create_test_cfg();
    let dom_tree = DominatorTree::new(&cfg).unwrap();

    let node_1 = cfg.node_indices().find(|&n| cfg[n].id == 1).unwrap();

    // All nodes dominated by node 1 should include 1, 2, and 3
    let must_pass: Vec<usize> = cfg
        .node_indices()
        .filter(|&n| dom_tree.dominates(node_1, n))
        .map(|n| cfg[n].id)
        .collect();

    assert_eq!(must_pass.len(), 3, "Block 1 should dominate 3 blocks");
    assert!(must_pass.contains(&1), "Must include block 1 itself");
    assert!(must_pass.contains(&2), "Must include block 2");
    assert!(must_pass.contains(&3), "Must include block 3");
}

/// Test that non-existent block ID is handled gracefully
#[test]
fn test_nonexistent_block_id() {
    let cfg = cmds::create_test_cfg();

    // Block ID 99 doesn't exist in test CFG
    let found = cfg.node_indices().find(|&n| cfg[n].id == 99);
    assert!(found.is_none(), "Non-existent block should not be found");
}

/// Test JSON output for dominators command structure
#[test]
fn test_dominators_json_structure() {
    use crate::output::JsonResponse;

    let response = DominanceResponse {
        function: "json_test".to_string(),
        kind: "post-dominators".to_string(),
        root: Some(3),
        dominance_tree: vec![DominatorEntry {
            block: 3,
            immediate_dominator: None,
            dominated: vec![0, 2],
        }],
        must_pass_through: Some(MustPassThroughResult {
            block: 0,
            must_pass: vec![0, 1],
        }),
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    assert!(json.contains("\"kind\":\"post-dominators\""));
    assert!(json.contains("\"root\":3"));
    assert!(json.contains("\"must_pass_through\""));
    assert!(json.contains("\"block\":0"));
}
