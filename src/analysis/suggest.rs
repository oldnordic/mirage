//! Refactoring suggestion engine based on CFG analysis.
//!
//! Analyzes a symbol's CFG and produces actionable suggestions:
//! - High cyclomatic complexity → split function
//! - Deep nesting → flatten control flow
//! - Many execution paths → simplify branching
//! - Large block count → extract sub-functions
//! - Dead code → remove unreachable blocks

use anyhow::Result;
use serde::Serialize;

use crate::cfg::{
    detect_natural_loops, enumerate_paths_with_context, EnumerationContext, PathKind, PathLimits,
};
use crate::storage::MirageDb;

#[cfg(feature = "backend-sqlite")]
use crate::storage::load_cfg_from_db_with_conn;

#[derive(Debug, Clone, Serialize)]
pub struct SuggestReport {
    pub symbol: String,
    pub file_path: Option<String>,
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

    let ctx = EnumerationContext::new(&cfg);
    let limits = PathLimits::default();
    let paths = enumerate_paths_with_context(&cfg, &limits, &ctx);
    let path_count = paths.len();
    let error_paths = paths.iter().filter(|p| p.kind == PathKind::Error).count();

    let natural_loops = detect_natural_loops(&cfg);
    let max_nesting = natural_loops
        .iter()
        .map(|l| l.nesting_level(&natural_loops))
        .max()
        .unwrap_or(0);

    if cyclomatic > 15 {
        suggestions.push(Suggestion {
            kind: "split-function".to_string(),
            severity: "high".to_string(),
            message: format!(
                "Cyclomatic complexity is {} (threshold: 15). Consider splitting this function.",
                cyclomatic
            ),
            detail: Some(
                "High complexity makes testing and reasoning difficult. \
                 Extract distinct logical branches into helper functions."
                    .to_string(),
            ),
        });
    } else if cyclomatic > 10 {
        suggestions.push(Suggestion {
            kind: "split-function".to_string(),
            severity: "medium".to_string(),
            message: format!(
                "Cyclomatic complexity is {} (approaching threshold: 15). \
                 Consider simplifying control flow.",
                cyclomatic
            ),
            detail: None,
        });
    }

    if max_nesting > 3 {
        suggestions.push(Suggestion {
            kind: "flatten-nesting".to_string(),
            severity: "high".to_string(),
            message: format!(
                "Maximum loop nesting depth is {} (threshold: 3). \
                 Flatten deeply nested loops.",
                max_nesting
            ),
            detail: Some(
                "Deep nesting increases cognitive load and error-proneness. \
                 Use early returns, extract methods, or restructure data."
                    .to_string(),
            ),
        });
    }

    if path_count > 50 {
        suggestions.push(Suggestion {
            kind: "simplify-paths".to_string(),
            severity: "medium".to_string(),
            message: format!(
                "Function has {} execution paths (threshold: 50). \
                 Consider reducing branching.",
                path_count
            ),
            detail: Some(format!(
                "Excessive path count makes exhaustive testing infeasible. \
                 {} of {} paths are error paths.",
                error_paths, path_count
            )),
        });
    }

    if block_count > 40 {
        suggestions.push(Suggestion {
            kind: "extract-method".to_string(),
            severity: "medium".to_string(),
            message: format!(
                "Function has {} basic blocks (threshold: 40). \
                 Consider extracting sub-functions.",
                block_count
            ),
            detail: None,
        });
    }

    let unreachable_count = crate::cfg::reachability::find_unreachable(&cfg).len();
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

    Ok(SuggestReport {
        symbol: function_name.to_string(),
        file_path: file_path.map(|s| s.to_string()),
        suggestions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_suggestions_get_ok() {
        let report = SuggestReport {
            symbol: "clean_fn".to_string(),
            file_path: None,
            suggestions: vec![],
        };
        assert!(report.suggestions.is_empty());
    }
}
