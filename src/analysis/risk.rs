//! Risk scoring for functions based on CFG analysis.
//!
//! Computes a composite risk score from:
//! - Cyclomatic complexity (decision nodes in CFG)
//! - Path count (execution paths through function)
//! - Nesting depth (max loop nesting)
//! - Error path ratio (error paths / total paths)
//! - Block count (function size indicator)

use anyhow::Result;
use serde::Serialize;

use crate::cfg::{
    detect_natural_loops, enumerate_paths_with_context, Cfg, EnumerationContext, PathKind,
    PathLimits,
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

    let ctx = EnumerationContext::new(&cfg);
    let limits = PathLimits::default();
    let paths = enumerate_paths_with_context(&cfg, &limits, &ctx);

    let path_count = paths.len();
    let error_path_count = paths.iter().filter(|p| p.kind == PathKind::Error).count();
    let error_path_ratio = if path_count > 0 {
        error_path_count as f64 / path_count as f64
    } else {
        0.0
    };

    let cyclomatic_complexity = compute_cyclomatic_complexity(&cfg);
    let natural_loops = detect_natural_loops(&cfg);
    let loop_count = natural_loops.len();
    let max_nesting_depth = natural_loops
        .iter()
        .map(|l| l.nesting_level(&natural_loops))
        .max()
        .unwrap_or(0);
    let block_count = cfg.node_count();

    let risk_score = score(
        cyclomatic_complexity,
        path_count,
        error_path_ratio,
        block_count,
        max_nesting_depth,
    );

    let risk_level = classify_risk(risk_score);

    Ok(RiskReport {
        function: function_name.to_string(),
        file_path: file_path.map(|s| s.to_string()),
        risk_score,
        risk_level,
        factors: RiskFactors {
            cyclomatic_complexity,
            path_count,
            error_path_count,
            error_path_ratio,
            block_count,
            max_nesting_depth,
            loop_count,
        },
    })
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

fn score(
    complexity: usize,
    path_count: usize,
    error_ratio: f64,
    block_count: usize,
    nesting: usize,
) -> f64 {
    let complexity_weight = (complexity as f64).ln_1p() * 3.0;
    let path_weight = (path_count as f64).ln_1p() * 2.0;
    let error_weight = error_ratio * 5.0;
    let size_weight = (block_count as f64).ln_1p() * 1.0;
    let nesting_weight = (nesting as f64) * 4.0;

    complexity_weight + path_weight + error_weight + size_weight + nesting_weight
}

fn classify_risk(score: f64) -> String {
    if score >= 25.0 {
        "critical".to_string()
    } else if score >= 15.0 {
        "high".to_string()
    } else if score >= 8.0 {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_risk() {
        assert_eq!(classify_risk(0.0), "low");
        assert_eq!(classify_risk(5.0), "low");
        assert_eq!(classify_risk(8.0), "medium");
        assert_eq!(classify_risk(14.0), "medium");
        assert_eq!(classify_risk(15.0), "high");
        assert_eq!(classify_risk(24.0), "high");
        assert_eq!(classify_risk(25.0), "critical");
        assert_eq!(classify_risk(100.0), "critical");
    }

    #[test]
    fn test_score_monotonic() {
        let s1 = score(5, 10, 0.1, 20, 1);
        let s2 = score(20, 50, 0.5, 100, 3);
        assert!(s2 > s1, "higher inputs should produce higher score");
    }

    #[test]
    fn test_cyclomatic_empty() {
        use crate::cfg::edge::EdgeType;
        let empty = petgraph::graph::DiGraph::<crate::cfg::BasicBlock, EdgeType>::new();
        assert_eq!(compute_cyclomatic_complexity(&empty), 0);
    }
}
