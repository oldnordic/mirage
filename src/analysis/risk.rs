//! Risk scoring for functions based on CFG analysis.
//!
//! Computes a composite risk score from:
//! - Cyclomatic complexity (decision nodes in CFG)
//! - Path count (execution paths through function)
//! - Nesting depth (max loop nesting)
//! - Error path ratio (error paths / total paths)
//! - Block count (function size indicator)
//!
//! Path enumeration is *bounded*: it stops at `PathLimits::max_paths` and the
//! report then carries `path_count_truncated: true` plus an estimate of the
//! true path count, instead of hanging on huge dispatch CFGs.
//!
//! The headline severity (`risk_level`) comes from
//! [`crate::analysis::severity`], the same mapping `suggest` uses, so the two
//! subcommands cannot disagree on severity for the same function.

use anyhow::Result;
use serde::Serialize;

use crate::analysis::severity::{classify_score, composite_score};
use crate::cfg::{
    detect_natural_loops, enumerate_paths_with_context_outcome, estimate_path_count, Cfg,
    EnumerationContext, PathKind, PathLimits,
};
use crate::storage::MirageDb;

#[cfg(feature = "backend-sqlite")]
use crate::storage::load_cfg_from_db_with_conn;

#[derive(Debug, Clone, Serialize)]
pub struct RiskReport {
    pub function: String,
    pub file_path: Option<String>,
    pub risk_score: f64,
    pub risk_level: String,
    pub factors: RiskFactors,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskFactors {
    pub cyclomatic_complexity: usize,
    pub path_count: usize,
    /// True when enumeration stopped early (path cap or work budget);
    /// `path_count` is then a lower bound, not the true path count.
    pub path_count_truncated: bool,
    /// True when truncation was caused by the DFS work budget
    /// (`PathLimits::max_visits`) rather than the path cap — the CFG is too
    /// dense/loopy for exact enumeration within the budget.
    pub path_count_budget_exhausted: bool,
    /// Estimated true path count (branch/loop upper bound), present only when
    /// `path_count_truncated` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_count_estimated: Option<usize>,
    pub error_path_count: usize,
    pub error_path_ratio: f64,
    pub block_count: usize,
    pub max_nesting_depth: usize,
    pub loop_count: usize,
}

pub fn compute_risk(
    db: &MirageDb,
    function_id: i64,
    function_name: &str,
    file_path: Option<&str>,
) -> Result<RiskReport> {
    let conn = db.conn()?;

    let cfg = load_cfg_from_db_with_conn(conn, function_id)?;

    Ok(compute_risk_from_cfg(&cfg, function_name, file_path))
}

/// Compute a risk report from an already-loaded CFG.
///
/// Always terminates in bounded time: enumeration stops at
/// `PathLimits::max_paths` and unwinds immediately once saturated.
pub fn compute_risk_from_cfg(
    cfg: &Cfg,
    function_name: &str,
    file_path: Option<&str>,
) -> RiskReport {
    let ctx = EnumerationContext::new(cfg);
    let limits = PathLimits::default();
    let outcome = enumerate_paths_with_context_outcome(cfg, &limits, &ctx);

    let path_count = outcome.paths.len();
    let path_count_truncated = outcome.truncated;
    let path_count_budget_exhausted = outcome.budget_exhausted;
    // estimate_path_count saturates at usize::MAX for astronomically large
    // counts; don't present the sentinel as a number.
    let path_count_estimated = if path_count_truncated {
        let est = estimate_path_count(cfg, limits.loop_unroll_limit);
        if est == usize::MAX {
            None
        } else {
            Some(est)
        }
    } else {
        None
    };
    let error_path_count = outcome
        .paths
        .iter()
        .filter(|p| p.kind == PathKind::Error)
        .count();
    let error_path_ratio = if path_count > 0 {
        error_path_count as f64 / path_count as f64
    } else {
        0.0
    };

    let cyclomatic_complexity = compute_cyclomatic_complexity(cfg);
    let natural_loops = detect_natural_loops(cfg);
    let loop_count = natural_loops.len();
    let max_nesting_depth = natural_loops
        .iter()
        .map(|l| l.nesting_level(&natural_loops))
        .max()
        .unwrap_or(0);
    let block_count = cfg.node_count();

    let risk_score = composite_score(
        cyclomatic_complexity,
        path_count,
        error_path_ratio,
        block_count,
        max_nesting_depth,
    );

    let risk_level = classify_score(risk_score).to_string();

    RiskReport {
        function: function_name.to_string(),
        file_path: file_path.map(|s| s.to_string()),
        risk_score,
        risk_level,
        factors: RiskFactors {
            cyclomatic_complexity,
            path_count,
            path_count_truncated,
            path_count_budget_exhausted,
            path_count_estimated,
            error_path_count,
            error_path_ratio,
            block_count,
            max_nesting_depth,
            loop_count,
        },
    }
}

fn compute_cyclomatic_complexity(cfg: &Cfg) -> usize {
    if cfg.node_count() == 0 {
        return 0;
    }
    let edges = cfg.edge_count();
    let nodes = cfg.node_count();
    if edges >= nodes {
        edges - nodes + 2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_risk() {
        assert_eq!(classify_score(0.0), "low");
        assert_eq!(classify_score(5.0), "low");
        assert_eq!(classify_score(8.0), "medium");
        assert_eq!(classify_score(14.0), "medium");
        assert_eq!(classify_score(15.0), "high");
        assert_eq!(classify_score(24.0), "high");
        assert_eq!(classify_score(25.0), "critical");
        assert_eq!(classify_score(100.0), "critical");
    }

    #[test]
    fn test_score_monotonic() {
        let s1 = composite_score(5, 10, 0.1, 20, 1);
        let s2 = composite_score(20, 50, 0.5, 100, 3);
        assert!(s2 > s1, "higher inputs should produce higher score");
    }

    #[test]
    fn test_cyclomatic_empty() {
        use crate::cfg::edge::EdgeType;
        let empty = petgraph::graph::DiGraph::<crate::cfg::BasicBlock, EdgeType>::new();
        assert_eq!(compute_cyclomatic_complexity(&empty), 0);
    }
}
