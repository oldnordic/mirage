use super::enumeration::{enumerate_paths, enumerate_paths_with_context};
use super::limits::{EnumerationContext, PathLimits};
use super::*;
use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
use petgraph::graph::DiGraph;
use std::time::Duration;

#[test]
fn test_path_limits_max_length_long_path() {
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
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
    });
    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 4 },
        source_location: None,
    });
    let b4 = g.add_node(BasicBlock {
        id: 4,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });
    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);
    g.add_edge(b3, b4, EdgeType::Fallthrough);
    let limits = PathLimits::default().with_max_length(3);
    let paths = enumerate_paths(&g, &limits);
    assert_eq!(
        paths.len(),
        0,
        "Path exceeds max_length, should return 0 paths"
    );
}

#[test]
fn test_path_limits_max_paths_exact() {
    let cfg = test_fixtures::create_diamond_cfg();
    let limits = PathLimits::default().with_max_paths(1);
    let paths = enumerate_paths(&cfg, &limits);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].entry, 0);
    assert_eq!(paths[0].exit, 3);
}

#[test]
fn test_path_limits_loop_unroll_exact() {
    let cfg = test_fixtures::create_loop_cfg();
    let limits = PathLimits::default().with_loop_unroll_limit(1);
    let paths = enumerate_paths(&cfg, &limits);
    assert_eq!(paths.len(), 1);
    assert!(paths.iter().any(|p| p.blocks == vec![0, 1, 3]));
}

#[test]
fn test_path_limits_loop_unroll_limit_2() {
    let cfg = test_fixtures::create_loop_cfg();
    let limits = PathLimits::default().with_loop_unroll_limit(2);
    let paths = enumerate_paths(&cfg, &limits);
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().any(|p| p.blocks == vec![0, 1, 3]));
    assert!(paths.iter().any(|p| p.blocks == vec![0, 1, 2, 1, 3]));
}

#[test]
fn test_self_loop_terminates() {
    let cfg = test_fixtures::create_self_loop_cfg();
    let limits = PathLimits::default();
    let paths = enumerate_paths(&cfg, &limits);
    assert!(paths.len() <= limits.loop_unroll_limit + 1);
}

#[test]
fn test_self_loop_with_low_limit() {
    let cfg = test_fixtures::create_self_loop_cfg();
    let limits = PathLimits::default().with_loop_unroll_limit(1);
    let paths = enumerate_paths(&cfg, &limits);
    assert!(paths.len() <= 2);
}

#[test]
fn test_nested_loop_bounding() {
    let cfg = test_fixtures::create_nested_loop_cfg();
    let limits = PathLimits::default().with_loop_unroll_limit(2);
    let paths = enumerate_paths(&cfg, &limits);
    assert!(paths.len() <= 9);
    assert!(!paths.is_empty());
}

#[test]
fn test_nested_loop_bounding_three_levels() {
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
            otherwise: 6,
        },
        source_location: None,
    });
    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![3],
            otherwise: 1,
        },
        source_location: None,
    });
    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![4],
            otherwise: 2,
        },
        source_location: None,
    });
    let b4 = g.add_node(BasicBlock {
        id: 4,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
    });
    let b6 = g.add_node(BasicBlock {
        id: 6,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });
    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);
    g.add_edge(b1, b6, EdgeType::FalseBranch);
    g.add_edge(b2, b3, EdgeType::TrueBranch);
    g.add_edge(b2, b1, EdgeType::LoopBack);
    g.add_edge(b3, b4, EdgeType::TrueBranch);
    g.add_edge(b3, b2, EdgeType::LoopBack);
    g.add_edge(b4, b3, EdgeType::LoopBack);
    let limits = PathLimits::default().with_loop_unroll_limit(2);
    let paths = enumerate_paths(&g, &limits);
    assert!(paths.len() <= 27);
}

#[test]
fn test_nested_loop_independent_counters() {
    let cfg = test_fixtures::create_nested_loop_cfg();
    let loop_headers = crate::cfg::loops::find_loop_headers(&cfg);
    assert_eq!(loop_headers.len(), 2);
    let limits = PathLimits::default().with_loop_unroll_limit(2);
    let paths = enumerate_paths(&cfg, &limits);
    assert!(!paths.is_empty());
    assert!(paths.len() <= 9);
}

#[test]
fn test_path_limits_quick_analysis() {
    let limits = PathLimits::quick_analysis();
    assert_eq!(limits.max_length, 100);
    assert_eq!(limits.max_paths, 1000);
    assert_eq!(limits.loop_unroll_limit, 2);
}

#[test]
fn test_path_limits_thorough() {
    let limits = PathLimits::thorough();
    assert_eq!(limits.max_length, 10000);
    assert_eq!(limits.max_paths, 100000);
    assert_eq!(limits.loop_unroll_limit, 5);
}

#[test]
fn test_path_limits_builder_chaining() {
    let limits = PathLimits::quick_analysis()
        .with_max_length(200)
        .with_max_paths(5000)
        .with_loop_unroll_limit(3);
    assert_eq!(limits.max_length, 200);
    assert_eq!(limits.max_paths, 5000);
    assert_eq!(limits.loop_unroll_limit, 3);
}

#[test]
fn test_path_limits_presets_differ_from_default() {
    let default = PathLimits::default();
    let quick = PathLimits::quick_analysis();
    let thorough = PathLimits::thorough();
    assert!(quick.max_length < default.max_length);
    assert!(quick.max_paths < default.max_paths);
    assert!(quick.loop_unroll_limit < default.loop_unroll_limit);
    assert!(thorough.max_length > default.max_length);
    assert!(thorough.max_paths > default.max_paths);
    assert!(thorough.loop_unroll_limit > default.loop_unroll_limit);
}

#[test]
fn test_path_limits_quick_vs_thorough_on_loop() {
    let cfg = test_fixtures::create_loop_cfg();
    let quick_paths = enumerate_paths(&cfg, &PathLimits::quick_analysis());
    let thorough_paths = enumerate_paths(&cfg, &PathLimits::thorough());
    assert!(thorough_paths.len() >= quick_paths.len());
}

#[test]
#[ignore = "benchmark test - run with cargo test -- --ignored"]
fn test_perf_linear_cfg_10_blocks() {
    use std::time::Instant;
    let cfg = test_fixtures::create_large_linear_cfg(10);
    let limits = PathLimits::default();
    let start = Instant::now();
    let paths = enumerate_paths(&cfg, &limits);
    let duration = start.elapsed();
    assert_eq!(paths.len(), 1);
    assert!(duration < Duration::from_millis(10));
    println!("Linear 10 blocks: {:?}", duration);
}

#[test]
#[ignore = "benchmark test - run with cargo test -- --ignored"]
fn test_perf_linear_cfg_100_blocks() {
    use std::time::Instant;
    let cfg = test_fixtures::create_large_linear_cfg(100);
    let limits = PathLimits::default();
    let start = Instant::now();
    let paths = enumerate_paths(&cfg, &limits);
    let duration = start.elapsed();
    assert_eq!(paths.len(), 1);
    assert!(duration < Duration::from_millis(100));
    println!("Linear 100 blocks: {:?}", duration);
}

#[test]
#[ignore = "benchmark test - run with cargo test -- --ignored"]
fn test_perf_diamond_cfg_10_branches() {
    use std::time::Instant;
    let cfg = test_fixtures::create_large_diamond_cfg();
    let limits = PathLimits::default();
    let start = Instant::now();
    let paths = enumerate_paths(&cfg, &limits);
    let duration = start.elapsed();
    assert!(!paths.is_empty());
    assert!(duration < Duration::from_millis(50));
    println!(
        "Diamond 10 branches: {} paths in {:?}",
        paths.len(),
        duration
    );
}

#[test]
#[ignore = "benchmark test - run with cargo test -- --ignored"]
fn test_perf_single_loop_unroll_3() {
    use std::time::Instant;
    let cfg = test_fixtures::create_loop_cfg();
    let limits = PathLimits::default().with_loop_unroll_limit(3);
    let start = Instant::now();
    let paths = enumerate_paths(&cfg, &limits);
    let duration = start.elapsed();
    assert!(!paths.is_empty());
    assert!(duration < Duration::from_millis(100));
    println!(
        "Single loop (unroll=3): {} paths in {:?}",
        paths.len(),
        duration
    );
}

#[test]
#[ignore = "benchmark test - run with cargo test -- --ignored"]
fn test_perf_nested_loops() {
    use std::time::Instant;
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
            otherwise: 5,
        },
        source_location: None,
    });
    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![3],
            otherwise: 1,
        },
        source_location: None,
    });
    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
    });
    let b5 = g.add_node(BasicBlock {
        id: 5,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });
    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);
    g.add_edge(b1, b5, EdgeType::FalseBranch);
    g.add_edge(b2, b3, EdgeType::TrueBranch);
    g.add_edge(b2, b1, EdgeType::LoopBack);
    g.add_edge(b3, b2, EdgeType::LoopBack);
    let limits = PathLimits::default().with_loop_unroll_limit(2);
    let start = Instant::now();
    let paths = enumerate_paths(&g, &limits);
    let duration = start.elapsed();
    assert!(!paths.is_empty());
    assert!(duration < Duration::from_millis(500));
    println!(
        "Nested 2 loops (unroll=2): {} paths in {:?}",
        paths.len(),
        duration
    );
}

#[test]
#[ignore = "benchmark test - run with cargo test -- --ignored"]
fn test_perf_enumeration_context_reuse() {
    use std::time::Instant;
    let cfg = test_fixtures::create_diamond_cfg();
    let ctx = EnumerationContext::new(&cfg);
    let limits = PathLimits::default();
    let start = Instant::now();
    for _ in 0..100 {
        let _ = enumerate_paths_with_context(&cfg, &limits, &ctx);
    }
    let duration = start.elapsed();
    println!("100 calls with context: {:?}", duration);
    assert!(duration < Duration::from_millis(100));
}
