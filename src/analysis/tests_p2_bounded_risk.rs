//! P2 regression tests: bounded path enumeration (no unbounded hang) and
//! risk/suggest severity alignment.
//!
//! - `test_risk_bomb_cfg_bounded_with_truncation`: a synthetic 2^N-path "bomb"
//!   CFG must complete risk analysis under a time budget and report
//!   `path_count_truncated: true` (previously `risk` hung >570s on such CFGs).
//! - `test_risk_suggest_severity_agree_on_shared_fixture`: `risk`'s
//!   `risk_level` and `suggest`'s `overall_severity` must agree on identical
//!   input (previously CRITICAL vs MEDIUM on the same function).

use crate::analysis::risk::compute_risk_from_cfg;
use crate::analysis::suggest::compute_suggestions_from_cfg;
use crate::cfg::paths::PathLimits;
use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};
use petgraph::graph::DiGraph;
use std::time::{Duration, Instant};

fn block(id: usize, kind: BlockKind, terminator: Terminator) -> BasicBlock {
    BasicBlock {
        id,
        db_id: None,
        kind,
        statements: vec![],
        terminator,
        source_location: None,
    }
}

/// Build a chain of `diamonds` branch diamonds: 2^diamonds execution paths.
///
/// Layout per diamond i: split s_i --True/False--> a_i, b_i --Fallthrough-->
/// merge m_i --Fallthrough--> s_{i+1}. Final merge falls through to an exit
/// block with a Return terminator.
fn bomb_cfg(diamonds: usize) -> Cfg {
    let mut g = DiGraph::new();
    let mut next_id: usize = 0;
    let mut fresh = || {
        let id = next_id;
        next_id += 1;
        id
    };

    let entry_id = fresh();
    let exit_id = fresh();
    // Reserve IDs for splits/arms/merges.
    let splits: Vec<usize> = (0..diamonds).map(|_| fresh()).collect();
    let arms_a: Vec<usize> = (0..diamonds).map(|_| fresh()).collect();
    let arms_b: Vec<usize> = (0..diamonds).map(|_| fresh()).collect();
    let merges: Vec<usize> = (0..diamonds).map(|_| fresh()).collect();

    let entry = g.add_node(block(
        entry_id,
        BlockKind::Entry,
        Terminator::Goto { target: splits[0] },
    ));
    let exit = g.add_node(block(exit_id, BlockKind::Exit, Terminator::Return));

    let mut split_idxs = Vec::new();
    let mut a_idxs = Vec::new();
    let mut b_idxs = Vec::new();
    let mut merge_idxs = Vec::new();

    for i in 0..diamonds {
        let s = g.add_node(block(
            splits[i],
            BlockKind::Normal,
            Terminator::SwitchInt {
                targets: vec![arms_a[i]],
                otherwise: arms_b[i],
            },
        ));
        let a = g.add_node(block(
            arms_a[i],
            BlockKind::Normal,
            Terminator::Goto { target: merges[i] },
        ));
        let b = g.add_node(block(
            arms_b[i],
            BlockKind::Normal,
            Terminator::Goto { target: merges[i] },
        ));
        let merge_target = if i + 1 < diamonds {
            splits[i + 1]
        } else {
            exit_id
        };
        let m = g.add_node(block(
            merges[i],
            BlockKind::Normal,
            Terminator::Goto {
                target: merge_target,
            },
        ));
        split_idxs.push(s);
        a_idxs.push(a);
        b_idxs.push(b);
        merge_idxs.push(m);
    }

    g.add_edge(entry, split_idxs[0], EdgeType::Fallthrough);
    for i in 0..diamonds {
        g.add_edge(split_idxs[i], a_idxs[i], EdgeType::TrueBranch);
        g.add_edge(split_idxs[i], b_idxs[i], EdgeType::FalseBranch);
        g.add_edge(a_idxs[i], merge_idxs[i], EdgeType::Fallthrough);
        g.add_edge(b_idxs[i], merge_idxs[i], EdgeType::Fallthrough);
        let next = if i + 1 < diamonds {
            split_idxs[i + 1]
        } else {
            exit
        };
        g.add_edge(merge_idxs[i], next, EdgeType::Fallthrough);
    }

    g
}

#[test]
fn test_bomb_cfg_path_count_small() {
    // Sanity: 3 diamonds → exactly 8 paths, no truncation.
    let cfg = bomb_cfg(3);
    let report = compute_risk_from_cfg(&cfg, "bomb3", None);
    assert_eq!(report.factors.path_count, 8);
    assert!(!report.factors.path_count_truncated);
    assert!(report.factors.path_count_estimated.is_none());
}

#[test]
fn test_risk_bomb_cfg_bounded_with_truncation() {
    // 40 diamonds → 2^40 ≈ 1.1e12 paths. Before the bounded-enumeration fix
    // this shape of CFG hung `mirage risk` for >570s. It must now complete
    // well under a generous budget and honestly report truncation.
    let cfg = bomb_cfg(40);

    let start = Instant::now();
    let report = compute_risk_from_cfg(&cfg, "bomb40", None);
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(60),
        "risk on bomb CFG took {:?}; bounded enumeration regressed",
        elapsed
    );
    assert!(
        report.factors.path_count_truncated,
        "2^40-path CFG must report path_count_truncated"
    );
    assert_eq!(
        report.factors.path_count,
        PathLimits::default().max_paths,
        "truncated report must pin path_count at the cap"
    );
    assert_eq!(
        report.factors.path_count_estimated,
        Some(1_099_511_627_776), // 2^40, exact for a pure 40-diamond chain
        "truncated report must carry an honest estimate above the cap"
    );
}

#[test]
fn test_suggest_bomb_cfg_bounded_with_truncation() {
    let cfg = bomb_cfg(40);

    let start = Instant::now();
    let report = compute_suggestions_from_cfg(&cfg, "bomb40", None);
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(60),
        "suggest on bomb CFG took {:?}; bounded enumeration regressed",
        elapsed
    );
    assert!(report.path_count_truncated);
    assert!(report.path_count_estimated.is_some());
}

#[test]
fn test_work_budget_bounds_enumeration() {
    // A tiny max_visits budget must stop enumeration even when the path cap
    // is nowhere near reached, and report budget (not cap) truncation.
    use crate::cfg::enumerate_paths_outcome;

    let cfg = bomb_cfg(40);
    let limits = PathLimits::default().with_max_visits(1_000);

    let start = Instant::now();
    let outcome = enumerate_paths_outcome(&cfg, &limits);
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(60),
        "budgeted enumeration took {:?}",
        elapsed
    );
    assert!(outcome.truncated, "budget exhaustion must mark truncated");
    assert!(
        outcome.budget_exhausted,
        "truncation must be attributed to the work budget, not the path cap"
    );
    assert!(
        outcome.paths.len() < limits.max_paths,
        "budget stopped enumeration before the path cap"
    );
}

#[test]
fn test_risk_suggest_severity_agree_on_shared_fixture() {
    // Moderately complex fixture: 6 diamonds (64 paths, CC = 7).
    // Pre-alignment this class of function could score risk=high/critical
    // while suggest emitted only medium severities. The headline severities
    // must now be identical because both derive from the same composite score.
    let cfg = bomb_cfg(6);

    let risk = compute_risk_from_cfg(&cfg, "fixture", None);
    let suggest = compute_suggestions_from_cfg(&cfg, "fixture", None);

    assert_eq!(
        risk.risk_level, suggest.overall_severity,
        "risk and suggest must agree on headline severity for identical input"
    );
    assert_eq!(
        risk.risk_score, suggest.risk_score,
        "risk and suggest must compute the identical composite score"
    );
    assert!(!risk.factors.path_count_truncated);
    assert!(!suggest.path_count_truncated);
}

#[test]
fn test_risk_suggest_severity_agree_when_truncated() {
    // Agreement must also hold on the truncated (saturated) path.
    let cfg = bomb_cfg(40);

    let risk = compute_risk_from_cfg(&cfg, "fixture", None);
    let suggest = compute_suggestions_from_cfg(&cfg, "fixture", None);

    assert_eq!(risk.risk_level, suggest.overall_severity);
    assert_eq!(risk.risk_score, suggest.risk_score);
    assert!(risk.factors.path_count_truncated);
    assert!(suggest.path_count_truncated);
    assert_eq!(
        risk.factors.path_count_estimated, suggest.path_count_estimated,
        "both tools must report the same truncation estimate"
    );
}
