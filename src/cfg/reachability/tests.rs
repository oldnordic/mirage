use super::*;
use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
use petgraph::graph::DiGraph;

fn create_test_cfg_with_unreachable() -> Cfg {
    let mut g = DiGraph::new();

    // Block 0: entry, goes to 1
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    // Block 1: normal, goes to 2
    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    // Block 2: exit
    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    // Block 3: unreachable (no edges to it)
    let _b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["unreachable code".to_string()],
        terminator: Terminator::Unreachable,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::Fallthrough);

    g
}

#[test]
fn test_find_unreachable() {
    let cfg = create_test_cfg_with_unreachable();
    let unreachable = find_unreachable(&cfg);

    assert_eq!(unreachable.len(), 1);
    let block_id = cfg.node_weight(unreachable[0]).unwrap().id;
    assert_eq!(block_id, 3);
}

#[test]
fn test_find_reachable() {
    let cfg = create_test_cfg_with_unreachable();
    let reachable = find_reachable(&cfg);

    assert_eq!(reachable.len(), 3);
    let ids: Vec<_> = reachable
        .iter()
        .map(|&idx| cfg.node_weight(idx).unwrap().id)
        .collect();
    assert!(ids.contains(&0));
    assert!(ids.contains(&1));
    assert!(ids.contains(&2));
    assert!(!ids.contains(&3));
}

#[test]
fn test_is_reachable_from_entry() {
    let cfg = create_test_cfg_with_unreachable();

    let b0 = NodeIndex::new(0);
    let b3 = NodeIndex::new(3);

    assert!(is_reachable_from_entry(&cfg, b0));
    assert!(!is_reachable_from_entry(&cfg, b3));
}

#[test]
fn test_empty_cfg() {
    let cfg: Cfg = DiGraph::new();
    assert!(find_unreachable(&cfg).is_empty());
    assert!(find_reachable(&cfg).is_empty());
}

#[test]
fn test_fully_reachable_cfg() {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);

    assert!(find_unreachable(&g).is_empty());
}

#[test]
fn test_can_reach_simple() {
    let mut g = DiGraph::new();

    // Create: 0 -> 1 -> 2
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::Fallthrough);

    // All nodes can reach themselves
    assert!(can_reach(&g, b0, b0));
    assert!(can_reach(&g, b1, b1));
    assert!(can_reach(&g, b2, b2));

    // Forward reachability
    assert!(can_reach(&g, b0, b1));
    assert!(can_reach(&g, b0, b2));
    assert!(can_reach(&g, b1, b2));

    // No backward reachability
    assert!(!can_reach(&g, b1, b0));
    assert!(!can_reach(&g, b2, b0));
    assert!(!can_reach(&g, b2, b1));
}

#[test]
fn test_can_reach_diamond() {
    let mut g = DiGraph::new();

    // Diamond: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![1],
            otherwise: 2,
        },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::TrueBranch);
    g.add_edge(b0, b2, EdgeType::FalseBranch);
    g.add_edge(b1, b3, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);

    // All nodes reachable from entry
    assert!(can_reach(&g, b0, b1));
    assert!(can_reach(&g, b0, b2));
    assert!(can_reach(&g, b0, b3));

    // Branches can't reach each other
    assert!(!can_reach(&g, b1, b2));
    assert!(!can_reach(&g, b2, b1));
}

#[test]
fn test_can_reach_cached() {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);

    let mut space = DfsSpace::new(&g);

    // First query
    assert!(can_reach_cached(&g, b0, b1, &mut space));

    // Second query (space is reset internally by has_path_connecting)
    assert!(!can_reach_cached(&g, b1, b0, &mut space));
}

#[test]
fn test_reachability_cache() {
    let mut g = DiGraph::new();

    // Create a linear chain: 0 -> 1 -> 2 -> 3
    let nodes = (0..4)
        .map(|i| {
            g.add_node(BasicBlock {
                id: i,
                db_id: None,
                kind: if i == 0 {
                    BlockKind::Entry
                } else if i == 3 {
                    BlockKind::Exit
                } else {
                    BlockKind::Normal
                },
                statements: vec![],
                terminator: if i < 3 {
                    Terminator::Goto { target: i + 1 }
                } else {
                    Terminator::Return
                },
                source_location: None,
                coord_x: 0,
                coord_y: 0,
                coord_z: 0,
            })
        })
        .collect::<Vec<_>>();

    for i in 0..3 {
        g.add_edge(nodes[i], nodes[i + 1], EdgeType::Fallthrough);
    }

    let mut cache = ReachabilityCache::new(&g);

    // Multiple queries using same cache
    assert!(cache.can_reach(&g, nodes[0], nodes[3]));
    assert!(cache.can_reach(&g, nodes[1], nodes[3]));
    assert!(!cache.can_reach(&g, nodes[3], nodes[0]));
}

#[test]
fn test_find_reachable_from_block_linear() {
    let mut g = DiGraph::new();

    // Create: 0 -> 1 -> 2 -> 3
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);

    // From block 0, can reach 1, 2, 3
    let impact = find_reachable_from_block(&g, 0, None);
    assert_eq!(impact.source_block_id, 0);
    assert_eq!(impact.reachable_count, 3);
    assert!(impact.reachable_blocks.contains(&1));
    assert!(impact.reachable_blocks.contains(&2));
    assert!(impact.reachable_blocks.contains(&3));
    assert!(!impact.has_cycles);
}

#[test]
fn test_find_reachable_from_block_diamond() {
    let mut g = DiGraph::new();

    // Diamond: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![1],
            otherwise: 2,
        },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::TrueBranch);
    g.add_edge(b0, b2, EdgeType::FalseBranch);
    g.add_edge(b1, b3, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);

    // From block 0, can reach 1, 2, 3
    let impact = find_reachable_from_block(&g, 0, None);
    assert_eq!(impact.source_block_id, 0);
    assert_eq!(impact.reachable_count, 3);
    assert!(impact.reachable_blocks.contains(&1));
    assert!(impact.reachable_blocks.contains(&2));
    assert!(impact.reachable_blocks.contains(&3));
}

#[test]
fn test_find_reachable_from_block_max_depth() {
    let mut g = DiGraph::new();

    // Create: 0 -> 1 -> 2 -> 3
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);

    // With max_depth=1, from block 0 can only reach block 1
    let impact = find_reachable_from_block(&g, 0, Some(1));
    assert_eq!(impact.source_block_id, 0);
    assert_eq!(impact.reachable_count, 1);
    assert!(impact.reachable_blocks.contains(&1));
    assert!(!impact.reachable_blocks.contains(&2));
    assert!(!impact.reachable_blocks.contains(&3));
    assert_eq!(impact.max_depth_reached, 1);
}

#[test]
fn test_find_reachable_from_block_not_found() {
    let g = DiGraph::new();

    // Block 99 doesn't exist
    let impact = find_reachable_from_block(&g, 99, None);
    assert_eq!(impact.source_block_id, 99);
    assert_eq!(impact.reachable_count, 0);
    assert!(impact.reachable_blocks.is_empty());
}

#[test]
fn test_find_reachable_from_block_with_loop() {
    let mut g = DiGraph::new();

    // Create a loop: 0 -> 1 -> 2 -> 1 (back edge)
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![1],
            otherwise: 3,
        },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::Fallthrough);
    g.add_edge(b2, b1, EdgeType::LoopBack); // Back edge
    g.add_edge(b2, b3, EdgeType::LoopExit);

    // From block 1, should detect cycle
    let impact = find_reachable_from_block(&g, 1, Some(10));
    assert_eq!(impact.source_block_id, 1);
    assert!(impact.has_cycles);
}

#[test]
fn test_compute_path_impact() {
    let mut g = DiGraph::new();

    // Diamond: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![1],
            otherwise: 2,
        },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::TrueBranch);
    g.add_edge(b0, b2, EdgeType::FalseBranch);
    g.add_edge(b1, b3, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);

    // Path: 0 -> 1 -> 3
    // From 0: reaches 1, 2, 3
    // From 1: reaches 3
    // From 3: reaches nothing (exit)
    // Combined impact: {1, 2, 3} U {3} U {} = {1, 2, 3}
    // Minus path blocks {0, 1, 3} = {2}
    let impact = compute_path_impact(&g, &[0, 1, 3], None);
    assert_eq!(impact.path_length, 3);
    // Block 2 is the only block not in the path but reachable from it
    assert!(impact.unique_blocks_affected.contains(&2));
}
