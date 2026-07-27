//! Refactoring suggestion engine based on CFG analysis.
//!
//! Analyzes a symbol's CFG and produces actionable suggestions:
//! - High cyclomatic complexity → split function
//! - Deep nesting → flatten control flow
//! - Many execution paths → simplify branching
//! - Large block count → extract sub-functions
//! - Dead code → remove unreachable blocks
//!
//! Severity alignment: the report's `overall_severity` is computed from the
//! same composite score and band mapping as `risk`'s `risk_level`
//! ([`crate::analysis::severity`]), and the per-issue thresholds are the
//! shared constants from that module — so `risk` and `suggest` can no longer
//! contradict each other on the severity of the same function.
//!
//! Path enumeration is bounded; when it saturates at `PathLimits::max_paths`
//! the report carries `path_count_truncated: true` and an estimate of the
//! true path count instead of hanging.

use anyhow::Result;
use serde::Serialize;

use crate::analysis::severity::{
    self, BLOCK_COUNT_NOTABLE, CC_SPLIT_HIGH, CC_SPLIT_MEDIUM, NESTING_HIGH, PATH_COUNT_NOTABLE,
};
use crate::cfg::{
    detect_natural_loops, enumerate_paths_with_context_outcome, estimate_path_count, Cfg,
    EnumerationContext, PathKind, PathLimits,
};
use crate::storage::MirageDb;

#[cfg(feature = "backend-sqlite")]
use crate::storage::load_cfg_from_db_with_conn;

#[derive(Debug, Clone, Serialize)]
pub struct SuggestReport {
    pub symbol: String,
    pub file_path: Option<String>,
    /// Headline severity, from the same composite score + band mapping as
    /// `mirage risk`'s `risk_level`. Guaranteed to agree with `risk` on the
    /// same function.
    pub overall_severity: String,
    /// Composite risk score behind `overall_severity` (same formula as `risk`).
    pub risk_score: f64,
    /// True when path enumeration stopped early (path cap or work budget);
    /// path counts in the suggestions below are then lower bounds.
    pub path_count_truncated: bool,
    /// True when truncation was caused by the DFS work budget rather than the
    /// path cap — the CFG is too dense/loopy for exact enumeration.
    pub path_count_budget_exhausted: bool,
    /// Estimated true path count, present only when `path_count_truncated`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_count_estimated: Option<usize>,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    pub kind: String,
    pub severity: String,
    pub message: String,
    pub detail: Option<String>,
}

pub fn compute_suggestions(
    db: &MirageDb,
    function_id: i64,
    function_name: &str,
    file_path: Option<&str>,
) -> Result<SuggestReport> {
    let conn = db.conn()?;
    let cfg = load_cfg_from_db_with_conn(conn, function_id)?;

    Ok(compute_suggestions_from_cfg(&cfg, function_name, file_path))
}

/// Compute suggestions from an already-loaded CFG.
///
/// Always terminates in bounded time: enumeration stops at
/// `PathLimits::max_paths` and unwinds immediately once saturated.
pub fn compute_suggestions_from_cfg(
    cfg: &Cfg,
    function_name: &str,
    file_path: Option<&str>,
) -> SuggestReport {
    let mut suggestions = Vec::new();

    let block_count = cfg.node_count();
    let edges = cfg.edge_count();
    let cyclomatic = if block_count > 0 && edges >= block_count {
        edges - block_count + 2
    } else if block_count > 0 {
        1
    } else {
        0
    };

    let ctx = EnumerationContext::new(cfg);
    let limits = PathLimits::default();
    let outcome = enumerate_paths_with_context_outcome(cfg, &limits, &ctx);
    let path_count = outcome.paths.len();
    let path_count_truncated = outcome.truncated;
    let path_count_budget_exhausted = outcome.budget_exhausted;
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
    let error_paths = outcome
        .paths
        .iter()
        .filter(|p| p.kind == PathKind::Error)
        .count();
    let error_path_ratio = if path_count > 0 {
        error_paths as f64 / path_count as f64
    } else {
        0.0
    };

    let natural_loops = detect_natural_loops(cfg);
    let max_nesting = natural_loops
        .iter()
        .map(|l| l.nesting_level(&natural_loops))
        .max()
        .unwrap_or(0);

    // Headline severity: identical inputs → identical verdict as `risk`.
    let risk_score = severity::composite_score(
        cyclomatic,
        path_count,
        error_path_ratio,
        block_count,
        max_nesting,
    );
    let overall_severity = severity::classify_score(risk_score).to_string();

    if cyclomatic > CC_SPLIT_HIGH {
        suggestions.push(Suggestion {
            kind: "split-function".to_string(),
            severity: "high".to_string(),
            message: format!(
                "Cyclomatic complexity is {} (threshold: {}). Consider splitting this function.",
                cyclomatic, CC_SPLIT_HIGH
            ),
            detail: Some(
                "High complexity makes testing and reasoning difficult. \
                 Extract distinct logical branches into helper functions."
                    .to_string(),
            ),
        });
    } else if cyclomatic > CC_SPLIT_MEDIUM {
        suggestions.push(Suggestion {
            kind: "split-function".to_string(),
            severity: "medium".to_string(),
            message: format!(
                "Cyclomatic complexity is {} (approaching threshold: {}). \
                 Consider simplifying control flow.",
                cyclomatic, CC_SPLIT_HIGH
            ),
            detail: None,
        });
    }

    if max_nesting > NESTING_HIGH {
        suggestions.push(Suggestion {
            kind: "flatten-nesting".to_string(),
            severity: "high".to_string(),
            message: format!(
                "Maximum loop nesting depth is {} (threshold: {}). \
                 Flatten deeply nested loops.",
                max_nesting, NESTING_HIGH
            ),
            detail: Some(
                "Deep nesting increases cognitive load and error-proneness. \
                 Use early returns, extract methods, or restructure data."
                    .to_string(),
            ),
        });
    }

    if path_count > PATH_COUNT_NOTABLE || path_count_truncated {
        let estimate_str = path_count_estimated
            .map(|e| format!("~{}", e))
            .unwrap_or_else(|| "astronomical (exceeds 2^64)".to_string());
        let detail = if path_count_budget_exhausted {
            format!(
                "Excessive path count makes exhaustive testing infeasible. \
                 {} of the {} enumerated paths are error paths. Enumeration \
                 exhausted its work budget before completing; the CFG is too \
                 dense/loopy for exact enumeration. Estimated true path count: \
                 {}. The score above uses the enumerated sample of {} paths.",
                error_paths, path_count, estimate_str, path_count
            )
        } else if path_count_truncated {
            format!(
                "Excessive path count makes exhaustive testing infeasible. \
                 {} of the {} enumerated paths are error paths. Enumeration was \
                 truncated at the {}-path cap; estimated true path count: {}. \
                 The score above uses the capped count.",
                error_paths, path_count, limits.max_paths, estimate_str
            )
        } else {
            format!(
                "Excessive path count makes exhaustive testing infeasible. \
                 {} of {} paths are error paths.",
                error_paths, path_count
            )
        };
        suggestions.push(Suggestion {
            kind: "simplify-paths".to_string(),
            severity: "medium".to_string(),
            message: if path_count_truncated {
                format!(
                    "Function path count exceeds exact enumeration (sampled {} paths; threshold: {}). \
                     Consider reducing branching.",
                    path_count, PATH_COUNT_NOTABLE
                )
            } else {
                format!(
                    "Function has {} execution paths (threshold: {}). \
                     Consider reducing branching.",
                    path_count, PATH_COUNT_NOTABLE
                )
            },
            detail: Some(detail),
        });
    }

    if block_count > BLOCK_COUNT_NOTABLE {
        suggestions.push(Suggestion {
            kind: "extract-method".to_string(),
            severity: "medium".to_string(),
            message: format!(
                "Function has {} basic blocks (threshold: {}). \
                 Consider extracting sub-functions.",
                block_count, BLOCK_COUNT_NOTABLE
            ),
            detail: None,
        });
    }

    let unreachable_count = crate::cfg::reachability::find_unreachable(cfg).len();
    if unreachable_count > 0 {
        suggestions.push(Suggestion {
            kind: "remove-dead-code".to_string(),
            severity: "low".to_string(),
            message: format!(
                "Function has {} unreachable basic blocks out of {}.",
                unreachable_count, block_count
            ),
            detail: Some(
                "Unreachable code increases maintenance burden without adding value.".to_string(),
            ),
        });
    }

    if suggestions.is_empty() {
        suggestions.push(Suggestion {
            kind: "ok".to_string(),
            severity: "info".to_string(),
            message: "No significant issues detected.".to_string(),
            detail: None,
        });
    }

    SuggestReport {
        symbol: function_name.to_string(),
        file_path: file_path.map(|s| s.to_string()),
        overall_severity,
        risk_score,
        path_count_truncated,
        path_count_budget_exhausted,
        path_count_estimated,
        suggestions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_suggestions_get_ok() {
        let report = SuggestReport {
            symbol: "clean_fn".to_string(),
            file_path: None,
            overall_severity: "low".to_string(),
            risk_score: 0.0,
            path_count_truncated: false,
            path_count_budget_exhausted: false,
            path_count_estimated: None,
            suggestions: vec![],
        };
        assert!(report.suggestions.is_empty());
    }
}
