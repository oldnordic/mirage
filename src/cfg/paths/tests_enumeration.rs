use super::cached::{
    check_path_explosion, enumerate_paths_cached, enumerate_paths_cached_with_context,
    estimate_path_count,
};
use super::enumeration::{
    enumerate_paths, enumerate_paths_iterative, enumerate_paths_with_context,
};
use super::limits::{EnumerationContext, PathLimits};
use super::metadata::{enumerate_paths_with_metadata, get_or_enumerate_paths, LimitsHit};
use super::PathKind;
use super::*;
use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};
use petgraph::graph::DiGraph;

#[test]
fn test_enumerate_paths_linear_cfg() {
    let cfg = test_fixtures::create_linear_cfg();
    let paths = enumerate_paths(&cfg, &PathLimits::default());
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].blocks, vec![0, 1, 2]);
    assert_eq!(paths[0].entry, 0);
    assert_eq!(paths[0].exit, 2);
    assert_eq!(paths[0].kind, PathKind::Normal);
}

#[test]
fn test_enumerate_paths_diamond_cfg() {
    let cfg = test_fixtures::create_diamond_cfg();
    let paths = enumerate_paths(&cfg, &PathLimits::default());
    assert_eq!(paths.len(), 2);
    for path in &paths {
        assert_eq!(path.entry, 0);
        assert_eq!(path.exit, 3);
        assert_eq!(path.kind, PathKind::Normal);
    }
    let path_blocks: Vec<_> = paths.iter().map(|p| p.blocks.clone()).collect();
    assert!(path_blocks.contains(&vec![0, 1, 3]));
    assert!(path_blocks.contains(&vec![0, 2, 3]));
}

#[test]
fn test_enumerate_paths_loop_with_unroll_limit() {
    let cfg = test_fixtures::create_loop_cfg();
    let limits = PathLimits::default().with_loop_unroll_limit(3);
    let paths = enumerate_paths(&cfg, &limits);
    assert!(paths.len() >= 2);
    assert!(paths.len() <= 5);
    for path in &paths {
        assert_eq!(path.kind, PathKind::Normal);
        assert_eq!(path.entry, 0);
        assert_eq!(path.exit, 3);
    }
    assert!(paths.iter().any(|p| p.blocks == vec![0, 1, 3]));
}

#[test]
fn test_enumerate_paths_empty_cfg() {
    let cfg: Cfg = DiGraph::new();
    let paths = enumerate_paths(&cfg, &PathLimits::default());
    assert_eq!(paths.len(), 0);
}

#[test]
fn test_enumerate_paths_max_paths_limit() {
    let cfg = test_fixtures::create_diamond_cfg();
    let limits = PathLimits::default().with_max_paths(1);
    let paths = enumerate_paths(&cfg, &limits);
    assert_eq!(paths.len(), 1);
}

#[test]
fn test_enumerate_paths_max_length_limit() {
    let cfg = test_fixtures::create_diamond_cfg();
    let limits = PathLimits::default().with_max_length(2);
    let paths = enumerate_paths(&cfg, &limits);
    assert_eq!(paths.len(), 0);
}

#[test]
fn test_enumerate_paths_single_block_cfg() {
    let mut g = DiGraph::new();
    let _b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });
    let paths = enumerate_paths(&g, &PathLimits::default());
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].blocks, vec![0]);
}

#[test]
fn test_enumerate_paths_with_unreachable_exit() {
    let mut g = DiGraph::new();
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });
    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });
    let _b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Unreachable,
        source_location: None,
    });
    g.add_edge(b0, b1, EdgeType::Fallthrough);
    let paths = enumerate_paths(&g, &PathLimits::default());
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].blocks, vec![0, 1]);
}

#[test]
fn test_enumerate_paths_classification_diamond() {
    let cfg = test_fixtures::create_diamond_cfg();
    let paths = enumerate_paths(&cfg, &PathLimits::default());
    assert_eq!(paths.len(), 2);
    for path in &paths {
        assert_eq!(path.kind, PathKind::Normal);
    }
}

#[test]
fn test_enumerate_paths_classification_with_error() {
    let cfg = test_fixtures::create_error_cfg();
    let paths = enumerate_paths(&cfg, &PathLimits::default());
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].kind, PathKind::Error);
}

#[test]
fn test_enumerate_paths_classification_with_unreachable() {
    let cfg = test_fixtures::create_dead_code_cfg();
    let paths = enumerate_paths(&cfg, &PathLimits::default());
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].blocks, vec![0]);
    assert_eq!(paths[0].kind, PathKind::Normal);
}

#[test]
fn test_enumerate_paths_classification_mixed() {
    let mut g = DiGraph::new();
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
    });
    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });
    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Abort("panic!".to_string()),
        source_location: None,
    });
    g.add_edge(b0, b1, EdgeType::TrueBranch);
    g.add_edge(b0, b2, EdgeType::FalseBranch);
    let paths = enumerate_paths(&g, &PathLimits::default());
    assert_eq!(paths.len(), 2);
    assert_eq!(
        paths.iter().filter(|p| p.kind == PathKind::Normal).count(),
        1
    );
    assert_eq!(
        paths.iter().filter(|p| p.kind == PathKind::Error).count(),
        1
    );
}

#[test]
fn test_enumerate_paths_try_operator_self_loop() {
    let mut g = DiGraph::new();
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });
    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![2],
            otherwise: 2,
        },
        source_location: None,
    });
    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
    });
    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });
    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b1, EdgeType::Return);
    g.add_edge(b1, b2, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);
    let paths = enumerate_paths(&g, &PathLimits::default());
    assert!(!paths.is_empty());
    assert!(paths.iter().any(|p| p.blocks == vec![0, 1, 2, 3]));
}

#[test]
fn test_enumerate_paths_implicit_return_dead_end() {
    let mut g = DiGraph::new();
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });
    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
    });
    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 0 },
        source_location: None,
    });
    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::Fallthrough);
    let paths = enumerate_paths(&g, &PathLimits::default());
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].blocks, vec![0, 1, 2]);
}

#[test]
fn test_enumeration_context_new() {
    let cfg = test_fixtures::create_linear_cfg();
    let ctx = EnumerationContext::new(&cfg);
    assert_eq!(ctx.reachable_count(), 3);
    assert_eq!(ctx.loop_count(), 0);
    assert_eq!(ctx.exit_count(), 1);
}

#[test]
fn test_enumeration_context_with_loop() {
    let cfg = test_fixtures::create_loop_cfg();
    let ctx = EnumerationContext::new(&cfg);
    assert_eq!(ctx.loop_count(), 1);
    assert!(ctx.reachable_count() > 0);
    assert!(ctx.exit_count() > 0);
}

#[test]
fn test_enumeration_context_diamond_cfg() {
    let cfg = test_fixtures::create_diamond_cfg();
    let ctx = EnumerationContext::new(&cfg);
    assert_eq!(ctx.loop_count(), 0);
    assert_eq!(ctx.reachable_count(), 4);
    assert_eq!(ctx.exit_count(), 1);
}

#[test]
fn test_enumeration_context_is_reachable() {
    let cfg = test_fixtures::create_dead_code_cfg();
    let ctx = EnumerationContext::new(&cfg);
    assert!(ctx.is_reachable(0));
    assert!(!ctx.is_reachable(1));
}

#[test]
fn test_enumeration_context_is_loop_header() {
    use petgraph::graph::NodeIndex;
    let cfg = test_fixtures::create_loop_cfg();
    let ctx = EnumerationContext::new(&cfg);
    assert!(ctx.is_loop_header(NodeIndex::new(1)));
    assert!(!ctx.is_loop_header(NodeIndex::new(0)));
}

#[test]
fn test_enumeration_context_is_exit() {
    use petgraph::graph::NodeIndex;
    let cfg = test_fixtures::create_diamond_cfg();
    let ctx = EnumerationContext::new(&cfg);
    assert!(ctx.is_exit(NodeIndex::new(3)));
    assert!(!ctx.is_exit(NodeIndex::new(0)));
}

#[test]
fn test_enumerate_paths_with_context_matches_basic() {
    let cfg = test_fixtures::create_diamond_cfg();
    let limits = PathLimits::default();
    let ctx = EnumerationContext::new(&cfg);
    let paths_basic = enumerate_paths(&cfg, &limits);
    let paths_context = enumerate_paths_with_context(&cfg, &limits, &ctx);
    assert_eq!(paths_basic.len(), paths_context.len());
    let mut sorted_basic: Vec<_> = paths_basic.iter().collect();
    let mut sorted_context: Vec<_> = paths_context.iter().collect();
    sorted_basic.sort_by_key(|p| p.blocks.clone());
    sorted_context.sort_by_key(|p| p.blocks.clone());
    for (basic, context) in sorted_basic.iter().zip(sorted_context.iter()) {
        assert_eq!(basic.blocks, context.blocks);
        assert_eq!(basic.kind, context.kind);
    }
}

#[test]
fn test_enumerate_paths_with_context_linear_cfg() {
    let cfg = test_fixtures::create_linear_cfg();
    let limits = PathLimits::default();
    let ctx = EnumerationContext::new(&cfg);
    let paths = enumerate_paths_with_context(&cfg, &limits, &ctx);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].blocks, vec![0, 1, 2]);
}

#[test]
fn test_enumerate_paths_with_context_with_loop() {
    let cfg = test_fixtures::create_loop_cfg();
    let limits = PathLimits::default();
    let ctx = EnumerationContext::new(&cfg);
    let paths = enumerate_paths_with_context(&cfg, &limits, &ctx);
    assert!(!paths.is_empty());
    assert!(paths.len() <= 4);
}

#[test]
fn test_enumerate_paths_with_context_performance() {
    use std::time::Instant;
    let cfg = test_fixtures::create_diamond_cfg();
    let limits = PathLimits::default();
    let start = Instant::now();
    let _paths1 = enumerate_paths(&cfg, &limits);
    let basic_time = start.elapsed();
    let ctx_start = Instant::now();
    let ctx = EnumerationContext::new(&cfg);
    let ctx_creation_time = ctx_start.elapsed();
    let start = Instant::now();
    let _paths2 = enumerate_paths_with_context(&cfg, &limits, &ctx);
    let context_time = start.elapsed();
    println!(
        "Basic: {:?}, Context creation: {:?}, Context enum: {:?}",
        basic_time, ctx_creation_time, context_time
    );
    let start = Instant::now();
    let _paths3 = enumerate_paths_with_context(&cfg, &limits, &ctx);
    let context_time2 = start.elapsed();
    println!("Second context call: {:?}", context_time2);
    assert!(context_time2.as_millis() < 100);
}

#[test]
fn test_enumerate_paths_with_context_multiple_calls() {
    let cfg = test_fixtures::create_diamond_cfg();
    let ctx = EnumerationContext::new(&cfg);
    let limits1 = PathLimits::default().with_max_paths(10);
    let limits2 = PathLimits::default().with_max_paths(100);
    let paths1 = enumerate_paths_with_context(&cfg, &limits1, &ctx);
    let paths2 = enumerate_paths_with_context(&cfg, &limits2, &ctx);
    assert_eq!(paths1.len(), paths2.len());
}

#[test]
fn test_enumerate_paths_iterative_diamond_cfg() {
    let cfg = test_fixtures::create_simple_diamond_cfg();
    let limits = PathLimits::default();
    let paths = enumerate_paths_iterative(&cfg, &limits);
    assert_eq!(paths.len(), 2);
    assert_ne!(paths[0].blocks, paths[1].blocks);
    assert_eq!(paths[0].entry, 0);
    assert_eq!(paths[0].exit, 3);
    assert_eq!(paths[1].entry, 0);
    assert_eq!(paths[1].exit, 3);
}

#[test]
fn test_enumerate_paths_iterative_produces_same_results_as_recursive() {
    let cfg = test_fixtures::create_simple_diamond_cfg();
    let limits = PathLimits::default();
    let recursive_paths = enumerate_paths(&cfg, &limits);
    let iterative_paths = enumerate_paths_iterative(&cfg, &limits);
    assert_eq!(recursive_paths.len(), iterative_paths.len());
    let recursive_set: std::collections::HashSet<Vec<_>> =
        recursive_paths.iter().map(|p| p.blocks.clone()).collect();
    let iterative_set: std::collections::HashSet<Vec<_>> =
        iterative_paths.iter().map(|p| p.blocks.clone()).collect();
    assert_eq!(recursive_set, iterative_set);
}

#[test]
fn test_enumerate_paths_with_metadata_diamond() {
    let cfg = test_fixtures::create_simple_diamond_cfg();
    let result = enumerate_paths_with_metadata(&cfg, &PathLimits::default());
    assert_eq!(result.stats.total_paths, 2);
    assert_eq!(result.stats.normal_paths, 2);
    assert_eq!(result.stats.error_paths, 0);
    assert_eq!(result.stats.avg_path_length, 3.0);
    assert_eq!(result.stats.max_path_length, 3);
    assert_eq!(result.limits_hit, LimitsHit::None);
}

#[test]
fn test_enumerate_paths_with_metadata_limits_hit() {
    let cfg = test_fixtures::create_simple_diamond_cfg();
    let limits = PathLimits {
        max_paths: 1,
        ..Default::default()
    };
    let result = enumerate_paths_with_metadata(&cfg, &limits);
    assert_eq!(result.limits_hit, LimitsHit::MaxPaths);
    assert_eq!(result.stats.total_paths, 1);
}

#[test]
fn test_enumerate_paths_iterative_with_loop() {
    let mut g = DiGraph::new();
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
    });
    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 0 },
        source_location: None,
    });
    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });
    g.add_edge(b0, b1, EdgeType::TrueBranch);
    g.add_edge(b0, b2, EdgeType::FalseBranch);
    g.add_edge(b1, b0, EdgeType::LoopBack);
    let limits = PathLimits {
        loop_unroll_limit: 2,
        ..Default::default()
    };
    let paths = enumerate_paths_iterative(&g, &limits);
    assert!(paths.len() >= 2);
}

#[test]
fn test_enumerate_paths_iterative_empty_cfg() {
    let cfg = DiGraph::new();
    let paths = enumerate_paths_iterative(&cfg, &PathLimits::default());
    assert_eq!(paths.len(), 0);
}

#[test]
fn test_get_or_enumerate_paths_cache_miss_enumerates() {
    let mut conn = test_fixtures::setup_test_db();
    let cfg = test_fixtures::create_linear_cfg();
    let function_hash = "test_hash_123";
    let limits = PathLimits::default();
    let paths = get_or_enumerate_paths(&cfg, 1, function_hash, &limits, &mut conn).unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].blocks, vec![0, 1, 2]);
}

#[test]
fn test_get_or_enumerate_paths_cache_hit_retrieves() {
    let mut conn = test_fixtures::setup_test_db();
    let cfg = test_fixtures::create_linear_cfg();
    let function_hash = "test_hash_456";
    let limits = PathLimits::default();
    let paths1 = get_or_enumerate_paths(&cfg, 1, function_hash, &limits, &mut conn).unwrap();
    assert_eq!(paths1.len(), 1);
    let paths2 = get_or_enumerate_paths(&cfg, 1, function_hash, &limits, &mut conn).unwrap();
    assert_eq!(paths2.len(), 1);
    assert_eq!(paths2[0].blocks, vec![0, 1, 2]);
}

#[test]
fn test_get_or_enumerate_paths_hash_change_invalidates() {
    let mut conn = test_fixtures::setup_test_db();
    let cfg = test_fixtures::create_linear_cfg();
    let hash1 = "test_hash_v1";
    let hash2 = "test_hash_v3";
    let limits = PathLimits::default();
    let paths1 = get_or_enumerate_paths(&cfg, 1, hash1, &limits, &mut conn).unwrap();
    assert_eq!(paths1.len(), 1);
    let paths2 = get_or_enumerate_paths(&cfg, 1, hash2, &limits, &mut conn).unwrap();
    assert_eq!(paths2.len(), 1);
    assert_eq!(paths1[0].blocks, paths2[0].blocks);
}

#[test]
fn test_enumerate_paths_cached_cache_miss_enumerates() {
    let mut conn = test_fixtures::setup_test_db();
    let cfg = test_fixtures::create_linear_cfg();
    let function_hash = "test_hash_123";
    let limits = PathLimits::default();
    let paths = enumerate_paths_cached(&cfg, 1, function_hash, &limits, &mut conn).unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].blocks, vec![0, 1, 2]);
}

#[test]
fn test_enumerate_paths_cached_cache_hit_retrieves() {
    let mut conn = test_fixtures::setup_test_db();
    let cfg = test_fixtures::create_linear_cfg();
    let function_hash = "test_hash_456";
    let limits = PathLimits::default();
    let paths1 = enumerate_paths_cached(&cfg, 1, function_hash, &limits, &mut conn).unwrap();
    assert_eq!(paths1.len(), 1);
    let paths2 = enumerate_paths_cached(&cfg, 1, function_hash, &limits, &mut conn).unwrap();
    assert_eq!(paths2.len(), 1);
    assert_eq!(paths2[0].blocks, vec![0, 1, 2]);
}

#[test]
fn test_enumerate_paths_cached_hash_change_invalidates() {
    let mut conn = test_fixtures::setup_test_db();
    let cfg = test_fixtures::create_linear_cfg();
    let hash1 = "test_hash_v1";
    let hash2 = "test_hash_v3";
    let limits = PathLimits::default();
    let paths1 = enumerate_paths_cached(&cfg, 1, hash1, &limits, &mut conn).unwrap();
    assert_eq!(paths1.len(), 1);
    let paths2 = enumerate_paths_cached(&cfg, 1, hash2, &limits, &mut conn).unwrap();
    assert_eq!(paths2.len(), 1);
    assert_eq!(paths1[0].blocks, paths2[0].blocks);
}

#[test]
fn test_enumerate_paths_cached_with_context() {
    let mut conn = test_fixtures::setup_test_db();
    let cfg = test_fixtures::create_diamond_cfg();
    let function_hash = "test_hash_ctx";
    let limits = PathLimits::default();
    let ctx = EnumerationContext::new(&cfg);
    let paths1 =
        enumerate_paths_cached_with_context(&cfg, 1, function_hash, &limits, &ctx, &mut conn)
            .unwrap();
    assert_eq!(paths1.len(), 2);
    let paths2 =
        enumerate_paths_cached_with_context(&cfg, 1, function_hash, &limits, &ctx, &mut conn)
            .unwrap();
    assert_eq!(paths2.len(), 2);
}

#[test]
fn test_estimate_path_count_linear_cfg() {
    let cfg = test_fixtures::create_linear_cfg();
    let estimate = estimate_path_count(&cfg, 3);
    assert_eq!(estimate, 1);
}

#[test]
fn test_estimate_path_count_diamond_cfg() {
    let cfg = test_fixtures::create_diamond_cfg();
    let estimate = estimate_path_count(&cfg, 3);
    assert_eq!(estimate, 2);
}

#[test]
fn test_estimate_path_count_single_loop() {
    let cfg = test_fixtures::create_loop_cfg();
    let estimate = estimate_path_count(&cfg, 3);
    assert_eq!(estimate, 4);
}

#[test]
fn test_estimate_path_count_increases_with_loop_limit() {
    let cfg = test_fixtures::create_loop_cfg();
    let estimate3 = estimate_path_count(&cfg, 3);
    let estimate5 = estimate_path_count(&cfg, 5);
    assert!(estimate5 >= estimate3);
}

#[test]
fn test_check_path_explosion_safe() {
    let cfg = test_fixtures::create_linear_cfg();
    let limits = PathLimits::default();
    let result = check_path_explosion(&cfg, &limits);
    assert!(result.is_none());
}

#[test]
fn test_check_path_explosion_exceeds_limit() {
    let cfg = test_fixtures::create_diamond_cfg();
    let limits = PathLimits {
        max_length: 100,
        max_paths: 1,
        loop_unroll_limit: 3,
        ..Default::default()
    };
    let result = check_path_explosion(&cfg, &limits);
    assert!(result.is_some());
    if let Some(estimate) = result {
        assert!(estimate > 1);
    }
}

#[test]
fn test_estimate_path_count_no_overflow() {
    let cfg = test_fixtures::create_loop_cfg();
    let estimate = estimate_path_count(&cfg, 1000);
    assert!(estimate > 0);
    assert!(estimate < usize::MAX);
}
