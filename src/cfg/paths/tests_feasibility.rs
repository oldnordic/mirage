use super::feasibility::{
    classify_path, classify_path_precomputed, is_feasible_path, is_feasible_path_precomputed,
};
use super::PathKind;
use super::*;
use crate::cfg::reachability::find_reachable;
use std::collections::HashSet;

#[test]
fn test_classify_path_normal_return() {
    let cfg = test_fixtures::create_linear_cfg();
    let path = vec![0, 1, 2];
    let kind = classify_path(&cfg, &path);
    assert_eq!(kind, PathKind::Normal);
}

#[test]
fn test_classify_path_error_abort() {
    let cfg = test_fixtures::create_error_cfg();
    let path = vec![0, 1];
    let kind = classify_path(&cfg, &path);
    assert_eq!(kind, PathKind::Error);
}

#[test]
fn test_classify_path_degenerate_unreachable_terminator() {
    let cfg = test_fixtures::create_unreachable_term_cfg();
    let path = vec![0, 1];
    let kind = classify_path(&cfg, &path);
    assert_eq!(kind, PathKind::Degenerate);
}

#[test]
fn test_classify_path_unreachable_block() {
    let cfg = test_fixtures::create_dead_code_cfg();
    let path = vec![1];
    let kind = classify_path(&cfg, &path);
    assert_eq!(kind, PathKind::Unreachable);
}

#[test]
fn test_classify_path_error_call_unwind() {
    let cfg = test_fixtures::create_call_unwind_cfg();
    let path = vec![0, 1];
    let kind = classify_path(&cfg, &path);
    assert_eq!(kind, PathKind::Error);
}

#[test]
fn test_classify_path_empty() {
    let cfg = test_fixtures::create_linear_cfg();
    let path: Vec<crate::cfg::BlockId> = vec![];
    let kind = classify_path(&cfg, &path);
    assert_eq!(kind, PathKind::Degenerate);
}

#[test]
fn test_classify_path_single_block() {
    use crate::cfg::{BasicBlock, BlockKind, Terminator};
    use petgraph::graph::DiGraph;

    let mut g = DiGraph::new();
    let _b0 = g.add_node(BasicBlock {
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
    let path = vec![0];
    let kind = classify_path(&g, &path);
    assert_eq!(kind, PathKind::Normal);
}

#[test]
fn test_classify_path_nonexistent_block() {
    let cfg = test_fixtures::create_linear_cfg();
    let path = vec![0, 99];
    let kind = classify_path(&cfg, &path);
    assert_eq!(kind, PathKind::Degenerate);
}

#[test]
fn test_classify_path_precomputed_matches_classify_path() {
    let cfg = test_fixtures::create_diamond_cfg();
    let reachable_nodes = find_reachable(&cfg);
    let reachable_blocks: HashSet<crate::cfg::BlockId> =
        reachable_nodes.iter().map(|&idx| cfg[idx].id).collect();

    let test_paths = vec![vec![0, 1, 3], vec![0, 2, 3], vec![0, 1], vec![0]];

    for path in test_paths {
        let kind1 = classify_path(&cfg, &path);
        let kind2 = classify_path_precomputed(&cfg, &path, &reachable_blocks);
        assert_eq!(
            kind1, kind2,
            "classify_path_precomputed should match classify_path for path {:?}",
            path
        );
    }
}

#[test]
fn test_classify_path_precomputed_unreachable() {
    let cfg = test_fixtures::create_dead_code_cfg();
    let reachable_nodes = find_reachable(&cfg);
    let reachable_blocks: HashSet<crate::cfg::BlockId> =
        reachable_nodes.iter().map(|&idx| cfg[idx].id).collect();
    let path = vec![1];
    let kind = classify_path_precomputed(&cfg, &path, &reachable_blocks);
    assert_eq!(kind, PathKind::Unreachable);
}

#[test]
fn test_classify_path_precomputed_performance() {
    use std::time::Instant;

    let cfg = test_fixtures::create_diamond_cfg();
    let reachable_nodes = find_reachable(&cfg);
    let reachable_blocks: HashSet<crate::cfg::BlockId> =
        reachable_nodes.iter().map(|&idx| cfg[idx].id).collect();

    let test_paths: Vec<Vec<crate::cfg::BlockId>> = (0..1000).map(|_| vec![0, 1, 3]).collect();

    let start = Instant::now();
    for path in &test_paths {
        let _ = classify_path_precomputed(&cfg, path, &reachable_blocks);
    }
    let precomputed_duration = start.elapsed();

    assert!(
        precomputed_duration.as_millis() < 10,
        "classify_path_precomputed should classify 1000 paths in <10ms, took {}ms",
        precomputed_duration.as_millis()
    );
}

#[test]
fn test_classify_path_precomputed_all_kinds() {
    let cfg_normal = test_fixtures::create_linear_cfg();
    let reachable = find_reachable(&cfg_normal)
        .iter()
        .map(|&idx| cfg_normal[idx].id)
        .collect();
    assert_eq!(
        classify_path_precomputed(&cfg_normal, &[0, 1, 2], &reachable),
        PathKind::Normal
    );

    let cfg_error = test_fixtures::create_error_cfg();
    let reachable = find_reachable(&cfg_error)
        .iter()
        .map(|&idx| cfg_error[idx].id)
        .collect();
    assert_eq!(
        classify_path_precomputed(&cfg_error, &[0, 1], &reachable),
        PathKind::Error
    );

    let cfg_degen = test_fixtures::create_unreachable_term_cfg();
    let reachable = find_reachable(&cfg_degen)
        .iter()
        .map(|&idx| cfg_degen[idx].id)
        .collect();
    assert_eq!(
        classify_path_precomputed(&cfg_degen, &[0, 1], &reachable),
        PathKind::Degenerate
    );
}

#[test]
fn test_is_feasible_path_empty_path() {
    let cfg = test_fixtures::create_linear_cfg();
    let empty_path: Vec<crate::cfg::BlockId> = vec![];
    assert!(
        !is_feasible_path(&cfg, &empty_path),
        "Empty path should be infeasible"
    );
}

#[test]
fn test_is_feasible_path_non_entry_first_block() {
    let cfg = test_fixtures::create_diamond_cfg();
    let path = vec![1, 3];
    assert!(
        !is_feasible_path(&cfg, &path),
        "Path starting from non-entry block should be infeasible"
    );
}

#[test]
fn test_is_feasible_path_dead_end_goto() {
    let cfg = test_fixtures::create_linear_cfg();
    let path = vec![0, 1];
    assert!(
        !is_feasible_path(&cfg, &path),
        "Path ending in Goto should be infeasible (dead end)"
    );
}

#[test]
fn test_is_feasible_path_valid_return() {
    let cfg = test_fixtures::create_linear_cfg();
    let path = vec![0, 1, 2];
    assert!(
        is_feasible_path(&cfg, &path),
        "Complete path ending in Return should be feasible"
    );
}

#[test]
fn test_is_feasible_path_abort_is_feasible() {
    let cfg = test_fixtures::create_error_cfg();
    let path = vec![0, 1];
    assert!(
        is_feasible_path(&cfg, &path),
        "Path ending in Abort should be feasible (error path but reachable)"
    );
}

#[test]
fn test_is_feasible_path_unreachable_terminator() {
    let cfg = test_fixtures::create_unreachable_term_cfg();
    let path = vec![0, 1];
    assert!(
        !is_feasible_path(&cfg, &path),
        "Path ending in Unreachable should be infeasible"
    );
}

#[test]
fn test_is_feasible_path_nonexistent_block() {
    let cfg = test_fixtures::create_linear_cfg();
    let path = vec![0, 99];
    assert!(
        !is_feasible_path(&cfg, &path),
        "Path with nonexistent block should be infeasible"
    );
}

#[test]
fn test_is_feasible_path_switch_int_dead_end() {
    let cfg = test_fixtures::create_diamond_cfg();
    let path = vec![0];
    assert!(
        !is_feasible_path(&cfg, &path),
        "Path ending in SwitchInt should be infeasible (dead end)"
    );
}

#[test]
fn test_is_feasible_path_complete_diamond() {
    let cfg = test_fixtures::create_diamond_cfg();
    let path1 = vec![0, 1, 3];
    let path2 = vec![0, 2, 3];
    assert!(
        is_feasible_path(&cfg, &path1),
        "Complete diamond path 0->1->3 should be feasible"
    );
    assert!(
        is_feasible_path(&cfg, &path2),
        "Complete diamond path 0->2->3 should be feasible"
    );
}

#[test]
fn test_is_feasible_path_call_no_unwind() {
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    let mut g = DiGraph::new();
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Call {
            target: Some(1),
            unwind: None,
        },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });
    let _b1 = g.add_node(BasicBlock {
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
    g.add_edge(
        b0,
        petgraph::graph::NodeIndex::new(1),
        EdgeType::Fallthrough,
    );
    let path = vec![0];
    assert!(
        is_feasible_path(&g, &path),
        "Path ending in Call with no unwind should be feasible"
    );
}

#[test]
fn test_is_feasible_path_call_with_unwind_and_target() {
    let cfg = test_fixtures::create_call_unwind_cfg();
    let path = vec![0];
    assert!(
        is_feasible_path(&cfg, &path),
        "Path ending in Call with both unwind and target should be feasible"
    );
}

#[test]
fn test_is_feasible_path_call_always_unwinds() {
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    let mut g = DiGraph::new();
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Call {
            target: None,
            unwind: Some(1),
        },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });
    let _b1 = g.add_node(BasicBlock {
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
    g.add_edge(
        b0,
        petgraph::graph::NodeIndex::new(1),
        EdgeType::Fallthrough,
    );
    let path = vec![0];
    assert!(
        !is_feasible_path(&g, &path),
        "Path ending in Call with only unwind (no target) should be infeasible"
    );
}

#[test]
fn test_is_feasible_path_precomputed_matches_basic() {
    let cfg = test_fixtures::create_diamond_cfg();
    let reachable_nodes = find_reachable(&cfg);
    let reachable_blocks: HashSet<crate::cfg::BlockId> =
        reachable_nodes.iter().map(|&idx| cfg[idx].id).collect();

    let test_paths = vec![vec![0, 1, 3], vec![0, 2, 3], vec![0, 1], vec![]];

    for path in test_paths {
        let basic = is_feasible_path(&cfg, &path);
        let precomputed = is_feasible_path_precomputed(&cfg, &path, &reachable_blocks);
        assert_eq!(basic, precomputed, "Mismatch for {:?}", path);
    }
}

#[test]
fn test_is_feasible_path_precomputed_unreachable_block() {
    let cfg = test_fixtures::create_dead_code_cfg();
    let reachable_nodes = find_reachable(&cfg);
    let reachable_blocks: HashSet<crate::cfg::BlockId> =
        reachable_nodes.iter().map(|&idx| cfg[idx].id).collect();
    let path = vec![1];
    assert!(
        !is_feasible_path_precomputed(&cfg, &path, &reachable_blocks),
        "Path with unreachable block should be infeasible"
    );
}

#[test]
fn test_is_feasible_path_precomputed_performance() {
    use std::time::Instant;

    let cfg = test_fixtures::create_diamond_cfg();
    let reachable_nodes = find_reachable(&cfg);
    let reachable_blocks: HashSet<crate::cfg::BlockId> =
        reachable_nodes.iter().map(|&idx| cfg[idx].id).collect();

    let test_paths: Vec<Vec<crate::cfg::BlockId>> = (0..1000).map(|_| vec![0, 1, 3]).collect();

    let start = Instant::now();
    for path in &test_paths {
        let _ = is_feasible_path_precomputed(&cfg, path, &reachable_blocks);
    }
    let precomputed_duration = start.elapsed();

    assert!(
        precomputed_duration.as_millis() < 5,
        "is_feasible_path_precomputed should check 1000 paths in <5ms, took {}ms",
        precomputed_duration.as_millis()
    );
}

#[test]
fn test_is_feasible_path_precomputed_all_criteria() {
    let cfg_normal = test_fixtures::create_linear_cfg();
    let reachable_normal: HashSet<crate::cfg::BlockId> = find_reachable(&cfg_normal)
        .iter()
        .map(|&idx| cfg_normal[idx].id)
        .collect();
    assert!(is_feasible_path_precomputed(
        &cfg_normal,
        &[0, 1, 2],
        &reachable_normal
    ));

    let cfg_error = test_fixtures::create_error_cfg();
    let reachable_error: HashSet<crate::cfg::BlockId> = find_reachable(&cfg_error)
        .iter()
        .map(|&idx| cfg_error[idx].id)
        .collect();
    assert!(is_feasible_path_precomputed(
        &cfg_error,
        &[0, 1],
        &reachable_error
    ));

    let cfg_degen = test_fixtures::create_unreachable_term_cfg();
    let reachable_degen: HashSet<crate::cfg::BlockId> = find_reachable(&cfg_degen)
        .iter()
        .map(|&idx| cfg_degen[idx].id)
        .collect();
    assert!(!is_feasible_path_precomputed(
        &cfg_degen,
        &[0, 1],
        &reachable_degen
    ));
}

#[test]
fn test_classify_with_feasibility_dead_end() {
    let cfg = test_fixtures::create_linear_cfg();
    let reachable: HashSet<crate::cfg::BlockId> = find_reachable(&cfg)
        .iter()
        .map(|&idx| cfg[idx].id)
        .collect();
    let path = vec![0, 1];
    let kind = classify_path_precomputed(&cfg, &path, &reachable);
    assert_eq!(kind, PathKind::Degenerate);
}

#[test]
fn test_classify_with_feasibility_valid_exit() {
    let cfg = test_fixtures::create_linear_cfg();
    let reachable: HashSet<crate::cfg::BlockId> = find_reachable(&cfg)
        .iter()
        .map(|&idx| cfg[idx].id)
        .collect();
    let path = vec![0, 1, 2];
    let kind = classify_path_precomputed(&cfg, &path, &reachable);
    assert_eq!(kind, PathKind::Normal);
}

#[test]
fn test_classify_with_feasibility_error_path() {
    let cfg = test_fixtures::create_error_cfg();
    let reachable: HashSet<crate::cfg::BlockId> = find_reachable(&cfg)
        .iter()
        .map(|&idx| cfg[idx].id)
        .collect();
    let path = vec![0, 1];
    let kind = classify_path_precomputed(&cfg, &path, &reachable);
    assert_eq!(kind, PathKind::Error);
}

#[test]
fn test_classify_with_feasibility_switch_int_dead_end() {
    let cfg = test_fixtures::create_diamond_cfg();
    let reachable: HashSet<crate::cfg::BlockId> = find_reachable(&cfg)
        .iter()
        .map(|&idx| cfg[idx].id)
        .collect();
    let path = vec![0];
    let kind = classify_path_precomputed(&cfg, &path, &reachable);
    assert_eq!(kind, PathKind::Degenerate);
}

#[test]
fn test_classify_with_feasibility_priority_order() {
    let cfg = test_fixtures::create_dead_code_cfg();
    let reachable: HashSet<crate::cfg::BlockId> = find_reachable(&cfg)
        .iter()
        .map(|&idx| cfg[idx].id)
        .collect();
    let path = vec![1];
    let kind = classify_path_precomputed(&cfg, &path, &reachable);
    assert_eq!(kind, PathKind::Unreachable);
}

#[test]
fn test_classify_with_feasibility_complete_paths() {
    let cfg = test_fixtures::create_diamond_cfg();
    let reachable: HashSet<crate::cfg::BlockId> = find_reachable(&cfg)
        .iter()
        .map(|&idx| cfg[idx].id)
        .collect();
    assert_eq!(
        classify_path_precomputed(&cfg, &[0, 1, 3], &reachable),
        PathKind::Normal
    );
    assert_eq!(
        classify_path_precomputed(&cfg, &[0, 2, 3], &reachable),
        PathKind::Normal
    );
}

#[test]
fn test_classify_with_feasibility_call_terminator() {
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    let mut g = DiGraph::new();
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Call {
            target: Some(1),
            unwind: None,
        },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });
    let _b1 = g.add_node(BasicBlock {
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
    g.add_edge(
        b0,
        petgraph::graph::NodeIndex::new(1),
        EdgeType::Fallthrough,
    );
    let reachable: HashSet<crate::cfg::BlockId> =
        find_reachable(&g).iter().map(|&idx| g[idx].id).collect();
    let path = vec![0];
    let kind = classify_path_precomputed(&g, &path, &reachable);
    assert_eq!(kind, PathKind::Normal);
}

#[test]
fn test_feasibility_limitation_conflicting_conditions() {
    let cfg = test_fixtures::create_conflicting_conditions_cfg();
    let reachable: HashSet<crate::cfg::BlockId> = find_reachable(&cfg)
        .iter()
        .map(|&idx| cfg[idx].id)
        .collect();
    let path = vec![0, 1, 3];
    assert!(
        is_feasible_path_precomputed(&cfg, &path, &reachable),
        "Static analysis marks conflicting path as feasible (limitation)"
    );
    assert_eq!(
        classify_path_precomputed(&cfg, &path, &reachable),
        PathKind::Normal,
        "Conflicting path is classified as Normal (static limitation)"
    );
}

#[test]
fn test_feasibility_documentation_accuracy() {
    let cfg = test_fixtures::create_linear_cfg();
    let reachable: HashSet<crate::cfg::BlockId> = find_reachable(&cfg)
        .iter()
        .map(|&idx| cfg[idx].id)
        .collect();
    assert!(!is_feasible_path_precomputed(&cfg, &[1, 2], &reachable));
    assert!(!is_feasible_path_precomputed(&cfg, &[0, 1], &reachable));
    assert!(is_feasible_path_precomputed(&cfg, &[0, 1, 2], &reachable));
}
