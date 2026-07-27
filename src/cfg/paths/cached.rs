use crate::cfg::Cfg;
use rusqlite;

use super::{enumerate_paths_with_context, EnumerationContext, Path, PathLimits};

/// Enumerate paths with integrated caching and pre-computed context
///
/// This is the highest-level path enumeration function that combines:
/// 1. Hash-based cache invalidation (via update_function_paths_if_changed)
/// 2. Pre-computed enumeration context (avoid redundant analysis)
/// 3. Optimized batch storage (via store_paths_batch)
pub fn enumerate_paths_cached(
    cfg: &Cfg,
    function_id: i64,
    function_hash: &str,
    limits: &PathLimits,
    db_conn: &mut rusqlite::Connection,
) -> Result<Vec<Path>, String> {
    use crate::storage::paths::{get_cached_paths, update_function_paths_if_changed};

    let current_hash: Option<String> = db_conn
        .query_row(
            "SELECT cfg_hash FROM cfg_blocks WHERE function_id = ?1 LIMIT 1",
            rusqlite::params![function_id],
            |row| row.get(0),
        )
        .unwrap_or(None);

    if let Some(ref hash) = current_hash {
        if hash == function_hash {
            let paths = get_cached_paths(db_conn, function_id)
                .map_err(|e| format!("Failed to retrieve cached paths: {}", e))?;
            return Ok(paths);
        }
    }

    let ctx = EnumerationContext::new(cfg);
    let paths = enumerate_paths_with_context(cfg, limits, &ctx);

    update_function_paths_if_changed(db_conn, function_id, function_hash, &paths)
        .map_err(|e| format!("Failed to store enumerated paths: {}", e))?;

    Ok(paths)
}

/// Enumerate paths with external context and integrated caching
///
/// Similar to `enumerate_paths_cached` but allows you to provide a pre-computed
/// EnumerationContext. Use this when performing multiple operations on the same CFG.
pub fn enumerate_paths_cached_with_context(
    cfg: &Cfg,
    function_id: i64,
    function_hash: &str,
    limits: &PathLimits,
    ctx: &EnumerationContext,
    db_conn: &mut rusqlite::Connection,
) -> Result<Vec<Path>, String> {
    use crate::storage::paths::{get_cached_paths, update_function_paths_if_changed};

    let current_hash: Option<String> = db_conn
        .query_row(
            "SELECT cfg_hash FROM cfg_blocks WHERE function_id = ?1 LIMIT 1",
            rusqlite::params![function_id],
            |row| row.get(0),
        )
        .unwrap_or(None);

    if let Some(ref hash) = current_hash {
        if hash == function_hash {
            return get_cached_paths(db_conn, function_id)
                .map_err(|e| format!("Failed to retrieve cached paths: {}", e));
        }
    }

    let paths = enumerate_paths_with_context(cfg, limits, ctx);

    update_function_paths_if_changed(db_conn, function_id, function_hash, &paths)
        .map_err(|e| format!("Failed to store enumerated paths: {}", e))?;

    Ok(paths)
}

/// Estimate the number of paths in a CFG before enumeration
///
/// This provides an upper bound on path count using cyclomatic complexity
/// and loop structure analysis. Use this to warn users about potential
/// path explosion before running expensive enumeration.
pub fn estimate_path_count(cfg: &Cfg, loop_unroll_limit: usize) -> usize {
    let loop_headers = crate::cfg::loops::find_loop_headers(cfg);
    let loop_count = loop_headers.len();

    let mut branch_count = 0;
    for node in cfg.node_indices() {
        if loop_headers.contains(&node) {
            continue;
        }
        let out_degree = cfg
            .neighbors_directed(node, petgraph::Direction::Outgoing)
            .count();
        if out_degree > 1 {
            branch_count += out_degree - 1;
        }
    }

    if loop_count == 0 && branch_count == 0 {
        return 1;
    }

    let unroll_factor = (loop_unroll_limit + 1) as u128;

    // u128 intermediate so moderately-large estimates (e.g. 2^40) stay
    // meaningful; only truly astronomical counts saturate to usize::MAX,
    // which callers must treat as "exceeds representable range".
    let branch_factor: u128 = if branch_count < 128 {
        1u128 << branch_count
    } else {
        u128::MAX
    };

    let loop_factor: u128 = unroll_factor.saturating_pow(loop_count as u32);

    usize::try_from(branch_factor.saturating_mul(loop_factor)).unwrap_or(usize::MAX)
}

/// Check if path enumeration may exceed limits
///
/// This is a convenience wrapper around `estimate_path_count` that
/// compares the estimate against a limit and returns a warning if
/// the estimate suggests explosion.
pub fn check_path_explosion(cfg: &Cfg, limits: &PathLimits) -> Option<usize> {
    let estimate = estimate_path_count(cfg, limits.loop_unroll_limit);
    if estimate > limits.max_paths {
        Some(estimate)
    } else {
        None
    }
}
