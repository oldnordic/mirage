//! Shared severity classification for `risk` and `suggest`.
//!
//! Before this module existed, `risk` classified a composite score with its
//! own bands (`analysis/risk.rs`) while `suggest` assigned per-issue
//! severities from independent hard-coded thresholds (`analysis/suggest.rs`),
//! so the two subcommands could disagree on the severity of the *same*
//! function (e.g. `compute_mul_mat`: risk=CRITICAL vs suggest=MEDIUM).
//!
//! Both subcommands now derive their headline severity from the same
//! composite score and the same band boundaries defined here, and `suggest`'s
//! per-issue thresholds are the shared constants below — identical inputs can
//! no longer produce contradictory severities.

/// Composite score at or above which severity is `critical`.
pub const SCORE_CRITICAL: f64 = 25.0;
/// Composite score at or above which severity is `high`.
pub const SCORE_HIGH: f64 = 15.0;
/// Composite score at or above which severity is `medium`.
pub const SCORE_MEDIUM: f64 = 8.0;

/// Cyclomatic complexity above which `suggest` emits a HIGH split-function.
pub const CC_SPLIT_HIGH: usize = 15;
/// Cyclomatic complexity above which `suggest` emits a MEDIUM split-function.
pub const CC_SPLIT_MEDIUM: usize = 10;
/// Loop nesting depth above which `suggest` emits a HIGH flatten-nesting.
pub const NESTING_HIGH: usize = 3;
/// Enumerated path count above which `suggest` emits a MEDIUM simplify-paths.
pub const PATH_COUNT_NOTABLE: usize = 50;
/// Block count above which `suggest` emits a MEDIUM extract-method.
pub const BLOCK_COUNT_NOTABLE: usize = 40;

/// Map a composite risk score to a severity band.
///
/// This is the single source of truth for score → severity; `risk`'s
/// `risk_level` and `suggest`'s `overall_severity` both come from here.
pub fn classify_score(score: f64) -> &'static str {
    if score >= SCORE_CRITICAL {
        "critical"
    } else if score >= SCORE_HIGH {
        "high"
    } else if score >= SCORE_MEDIUM {
        "medium"
    } else {
        "low"
    }
}

/// Composite risk score from CFG factors.
///
/// Weights: cyclomatic complexity ×3 (log), path count ×2 (log),
/// error-path ratio ×5 (linear), block count ×1 (log), nesting depth ×4
/// (linear). Path count is the *enumerated* count and may be capped by
/// `PathLimits::max_paths`; callers should surface the truncation flag
/// alongside the score when the cap was hit.
pub fn composite_score(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_score_bands() {
        assert_eq!(classify_score(0.0), "low");
        assert_eq!(classify_score(7.9), "low");
        assert_eq!(classify_score(8.0), "medium");
        assert_eq!(classify_score(14.9), "medium");
        assert_eq!(classify_score(15.0), "high");
        assert_eq!(classify_score(24.9), "high");
        assert_eq!(classify_score(25.0), "critical");
        assert_eq!(classify_score(100.0), "critical");
    }

    #[test]
    fn test_composite_score_monotonic() {
        let s1 = composite_score(5, 10, 0.1, 20, 1);
        let s2 = composite_score(20, 50, 0.5, 100, 3);
        assert!(s2 > s1, "higher inputs should produce higher score");
    }
}
