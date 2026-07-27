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
//!
//! Two additional risk factors are available but *opt-in* (ported from
//! code-review-graph (MIT, Tirth Kanani 2026) `compute_risk_score`,
//! changes.py:312):
//! - coverage-inverse (`--coverage`): the fraction of CFG blocks with zero
//!   coverage hits adds up to [`COVERAGE_WEIGHT`] — uncovered code is
//!   riskier;
//! - git churn (`--churn`): the number of commits touching the function's
//!   file in a trailing window saturates at [`CHURN_SATURATION`] commits for
//!   up to [`CHURN_WEIGHT`].
//!
//! Both are OFF by default so the default `risk` score stays bit-identical
//! to `suggest`'s shared composite score on every database (the P2
//! risk/suggest agreement invariant) — including coverage-instrumented ones.

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
    /// Fraction of CFG blocks with zero coverage hits, in `[0,1]`.
    /// `None` when coverage analysis was not requested (`--coverage` off) or
    /// when no coverage rows exist for this function — the coverage-inverse
    /// term is then skipped rather than penalising the score. Factor mined
    /// from code-review-graph (MIT, Tirth Kanani 2026) `compute_risk_score`,
    /// changes.py:312.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncovered_ratio: Option<f64>,
    /// Number of commits touching this function's file in the churn window.
    /// `None` unless churn analysis was requested (`--churn`) and git could
    /// answer. Factor mined from code-review-graph's opt-in churn term
    /// (`_CHURN_WEIGHT`, `_CHURN_SATURATION`), rescaled to mirage's
    /// unbounded score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub churn_commits: Option<u32>,
}

/// Coverage-inverse weight on mirage's additive scale (comparable to the
/// existing `error_weight` max of 5.0). Uncovered code is riskier — mirrors
/// code-review-graph's 0.30-untested-to-0.05-tested inverse term.
const COVERAGE_WEIGHT: f64 = 5.0;

/// Churn weight and saturation on mirage's additive scale. Mirrors
/// code-review-graph's opt-in churn term (`_CHURN_WEIGHT=0.15`,
/// `_CHURN_SATURATION=10.0`), rescaled to mirage's unbounded score.
const CHURN_WEIGHT: f64 = 4.0;
const CHURN_SATURATION: f64 = 10.0;

/// Compute risk with the default factor set (CFG-derived factors only).
///
/// Equivalent to [`compute_risk_with_factors`] with coverage analysis off and
/// no churn input; the score is bit-identical to `suggest`'s shared
/// composite score for the same function.
pub fn compute_risk(
    db: &MirageDb,
    function_id: i64,
    function_name: &str,
    file_path: Option<&str>,
) -> Result<RiskReport> {
    compute_risk_with_factors(db, function_id, function_name, file_path, false, None)
}

/// Compute risk, optionally folding in the opt-in coverage-inverse and
/// git-churn factors.
///
/// - `include_coverage`: when true, query the `cfg_block_coverage` table and
///   fold the uncovered-block ratio into the score (gracefully skipped when
///   the function has no coverage rows). When false, the coverage-inverse
///   term is skipped entirely.
/// - `churn_commits`: number of commits touching the function's file in the
///   analysis window (see [`resolve_churn`]); `None` skips the churn factor.
///
/// With `include_coverage == false` and `churn_commits == None` the score is
/// bit-identical to `suggest`'s shared composite score — this is what keeps
/// the default CLI output in lockstep with `mirage suggest`.
pub fn compute_risk_with_factors(
    db: &MirageDb,
    function_id: i64,
    function_name: &str,
    file_path: Option<&str>,
    include_coverage: bool,
    churn_commits: Option<u32>,
) -> Result<RiskReport> {
    let conn = db.conn()?;

    let cfg = load_cfg_from_db_with_conn(conn, function_id)?;

    let uncovered_ratio = if include_coverage {
        query_uncovered_ratio(db, function_id)
    } else {
        None
    };

    Ok(compute_risk_from_cfg_with_factors(
        &cfg,
        function_name,
        file_path,
        uncovered_ratio,
        churn_commits,
    ))
}

/// Compute a risk report from an already-loaded CFG, default factors only.
///
/// Always terminates in bounded time: enumeration stops at
/// `PathLimits::max_paths` and unwinds immediately once saturated.
pub fn compute_risk_from_cfg(
    cfg: &Cfg,
    function_name: &str,
    file_path: Option<&str>,
) -> RiskReport {
    compute_risk_from_cfg_with_factors(cfg, function_name, file_path, None, None)
}

/// Compute a risk report from an already-loaded CFG plus the opt-in factors.
///
/// `uncovered_ratio` and `churn_commits` behave as in
/// [`compute_risk_with_factors`]; pass `None` for either to skip that term.
pub fn compute_risk_from_cfg_with_factors(
    cfg: &Cfg,
    function_name: &str,
    file_path: Option<&str>,
    uncovered_ratio: Option<f64>,
    churn_commits: Option<u32>,
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

    let base_score = composite_score(
        cyclomatic_complexity,
        path_count,
        error_path_ratio,
        block_count,
        max_nesting_depth,
    );

    // Opt-in coverage-inverse term (mined from code-review-graph): uncovered
    // code is riskier. Skipped entirely when coverage analysis was not
    // requested or no coverage data exists for the function.
    let coverage_weight = uncovered_ratio
        .map(|u| u.clamp(0.0, 1.0) * COVERAGE_WEIGHT)
        .unwrap_or(0.0);

    // Opt-in git-churn term (mined from code-review-graph): frequently
    // changed files are riskier, saturating at CHURN_SATURATION commits.
    let churn_weight = churn_commits
        .map(|c| (c as f64 / CHURN_SATURATION).min(1.0) * CHURN_WEIGHT)
        .unwrap_or(0.0);

    // Both opt-in terms are exactly 0.0 when off, so the default score is
    // bit-identical to `suggest`'s shared composite score (P2 agreement).
    let risk_score = base_score + coverage_weight + churn_weight;

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
            uncovered_ratio,
            churn_commits,
        },
    }
}

/// Fraction of a function's CFG blocks with zero coverage hits, in `[0,1]`.
///
/// Returns `None` when the function has no coverage rows at all — meaning the
/// repo is not instrumented for this function — so the coverage-inverse risk
/// term is skipped rather than penalising every function equally. A block is
/// counted covered when its `hit_count > 0`.
fn query_uncovered_ratio(db: &MirageDb, function_id: i64) -> Option<f64> {
    let conn = db.conn().ok()?;
    // LEFT JOIN so blocks with no coverage row count as uncovered, but require
    // at least one coverage row to exist for the function before scoring.
    let sql = "SELECT COUNT(*) AS total, \
                      SUM(CASE WHEN COALESCE(bc.hit_count, 0) > 0 THEN 1 ELSE 0 END) AS covered, \
                      SUM(CASE WHEN bc.block_id IS NOT NULL THEN 1 ELSE 0 END) AS have_rows \
               FROM cfg_blocks bb \
               LEFT JOIN cfg_block_coverage bc ON bb.id = bc.block_id \
               WHERE bb.function_id = ?1";
    let mut stmt = conn.prepare(sql).ok()?;
    let (total, covered, have_rows): (i64, i64, i64) = stmt
        .query_row([function_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            ))
        })
        .ok()?;
    if total == 0 || have_rows == 0 {
        // No blocks, or no coverage instrumentation for this function.
        return None;
    }
    let uncovered = (total - covered).max(0) as f64;
    Some((uncovered / total as f64).clamp(0.0, 1.0))
}

/// Count commits touching `file_path` over a trailing window of `window_days`.
///
/// Ports code-review-graph's `compute_file_churn` (MIT, Tirth Kanani 2026):
/// runs `git log --since=<N>.days.ago --no-renames` scoped to the single file
/// and counts the resulting commits. Renames are deliberately not followed —
/// churn belongs to the path as it existed in each commit. Returns `None` on
/// any git failure (not a repo, git absent, non-zero exit) so the churn factor
/// is skipped rather than fabricated. `repo_dir` is the directory git runs in.
fn count_file_churn(repo_dir: &std::path::Path, file_path: &str, window_days: u32) -> Option<u32> {
    if window_days == 0 {
        return None;
    }
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("log")
        .arg(format!("--since={window_days}.days.ago"))
        .arg("--no-renames")
        .arg("--format=%H")
        .arg("--")
        .arg(file_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let count = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    Some(count as u32)
}

/// Resolve the churn commit count for a function's file, if churn analysis is
/// requested. Returns `None` when `file_path` is absent or git cannot answer.
pub fn resolve_churn(file_path: Option<&str>, window_days: u32) -> Option<u32> {
    let file_path = file_path?;
    let path = std::path::Path::new(file_path);
    // Run git from the file's parent directory when the path is absolute so the
    // command works regardless of the process cwd; otherwise use cwd (".").
    let repo_dir = if path.is_absolute() {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    } else {
        std::path::PathBuf::from(".")
    };
    count_file_churn(&repo_dir, file_path, window_days)
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
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

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

    /// entry --Fallthrough--> exit(Return): the smallest non-empty CFG.
    fn two_block_cfg() -> Cfg {
        let mut g = DiGraph::new();
        let entry = g.add_node(block(0, BlockKind::Entry, Terminator::Goto { target: 1 }));
        let exit = g.add_node(block(1, BlockKind::Exit, Terminator::Return));
        g.add_edge(entry, exit, EdgeType::Fallthrough);
        g
    }

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
        let empty = DiGraph::<BasicBlock, EdgeType>::new();
        assert_eq!(compute_cyclomatic_complexity(&empty), 0);
    }

    #[test]
    fn test_default_factors_bit_identical_to_shared_composite() {
        // The P2 agreement invariant at the library level: with both opt-in
        // factors off, the risk score must equal severity::composite_score
        // exactly (bit-for-bit), which is what `suggest` reports.
        let cfg = two_block_cfg();
        let report = compute_risk_from_cfg(&cfg, "f", None);
        let shared = composite_score(
            report.factors.cyclomatic_complexity,
            report.factors.path_count,
            report.factors.error_path_ratio,
            report.factors.block_count,
            report.factors.max_nesting_depth,
        );
        assert_eq!(report.risk_score.to_bits(), shared.to_bits());
        assert!(report.factors.uncovered_ratio.is_none());
        assert!(report.factors.churn_commits.is_none());
    }

    #[test]
    fn test_coverage_inverse_increases_risk() {
        // Fully uncovered adds COVERAGE_WEIGHT; fully covered adds nothing.
        let cfg = two_block_cfg();
        let base = compute_risk_from_cfg(&cfg, "f", None);
        let covered = compute_risk_from_cfg_with_factors(&cfg, "f", None, Some(0.0), None);
        let uncovered = compute_risk_from_cfg_with_factors(&cfg, "f", None, Some(1.0), None);
        assert_eq!(
            base.risk_score.to_bits(),
            covered.risk_score.to_bits(),
            "0% uncovered must match no-coverage-data bit-for-bit"
        );
        assert!(
            uncovered.risk_score > covered.risk_score,
            "uncovered code must score higher"
        );
        assert!(
            (uncovered.risk_score - covered.risk_score - COVERAGE_WEIGHT).abs() < 1e-9,
            "full-uncovered delta must equal COVERAGE_WEIGHT"
        );
        assert_eq!(uncovered.factors.uncovered_ratio, Some(1.0));
    }

    #[test]
    fn test_churn_saturates() {
        let cfg = two_block_cfg();
        let none = compute_risk_from_cfg(&cfg, "f", None);
        let some = compute_risk_from_cfg_with_factors(&cfg, "f", None, None, Some(5));
        let saturated = compute_risk_from_cfg_with_factors(&cfg, "f", None, None, Some(100));
        assert!(some.risk_score > none.risk_score, "churn must add risk");
        assert!(
            saturated.risk_score > some.risk_score,
            "more churn must add more risk up to cap"
        );
        assert!(
            (saturated.risk_score - none.risk_score - CHURN_WEIGHT).abs() < 1e-9,
            "saturated churn delta must equal CHURN_WEIGHT"
        );
        // Beyond saturation there is no further increase.
        let over = compute_risk_from_cfg_with_factors(&cfg, "f", None, None, Some(10));
        assert!(
            (over.risk_score - saturated.risk_score).abs() < 1e-9,
            "churn must saturate at CHURN_SATURATION commits"
        );
        assert_eq!(some.factors.churn_commits, Some(5));
    }

    #[test]
    fn test_resolve_churn_counts_commits_in_real_repo() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path();
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(args)
                .env("GIT_AUTHOR_NAME", "mirage-test")
                .env("GIT_AUTHOR_EMAIL", "mirage-test@example.com")
                .env("GIT_COMMITTER_NAME", "mirage-test")
                .env("GIT_COMMITTER_EMAIL", "mirage-test@example.com")
                .status()
                .expect("failed to spawn git");
            assert!(status.success(), "git {args:?} failed");
        };
        git(&["init", "-q"]);
        let file = repo.join("churned.rs");
        for i in 0..3 {
            std::fs::write(&file, format!("// rev {i}\n")).unwrap();
            git(&["add", "churned.rs"]);
            git(&["commit", "-q", "-m", &format!("rev {i}")]);
        }

        let abs = file.to_str().unwrap();
        assert_eq!(resolve_churn(Some(abs), 90), Some(3));
        // A zero-day window disables churn resolution.
        assert_eq!(resolve_churn(Some(abs), 0), None);
        // No file path at all -> no churn factor.
        assert_eq!(resolve_churn(None, 90), None);
        // A path outside any git repo yields None, never a fabricated count.
        assert_eq!(resolve_churn(Some("/nonexistent/dir/file.rs"), 90), None);
    }
}
