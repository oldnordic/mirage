use crate::cfg::Cfg;
use rusqlite;

use super::{enumerate_paths, enumerate_paths_iterative, Path, PathKind, PathLimits};

/// Path enumeration result with metadata
///
/// Enhanced result that includes statistics about the enumeration
/// to help AI agents understand the analysis quality.
#[derive(Debug, Clone)]
pub struct PathEnumerationResult {
    /// The enumerated paths
    pub paths: Vec<Path>,
    /// Whether the enumeration hit any limits
    pub limits_hit: LimitsHit,
    /// Statistics about the enumeration
    pub stats: EnumerationStats,
}

/// Which limits were hit during enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LimitsHit {
    /// No limits hit - complete enumeration
    None,
    /// Hit max path length limit
    MaxLength,
    /// Hit max path count limit
    MaxPaths,
    /// Hit loop unroll limit
    LoopUnroll,
    /// Multiple limits hit
    Multiple,
}

/// Statistics about path enumeration
#[derive(Debug, Clone)]
pub struct EnumerationStats {
    /// Total paths found
    pub total_paths: usize,
    /// Normal paths (entry → return)
    pub normal_paths: usize,
    /// Error paths (contains panic/abort)
    pub error_paths: usize,
    /// Degenerate paths (dead ends, infinite loops)
    pub degenerate_paths: usize,
    /// Unreachable paths
    pub unreachable_paths: usize,
    /// Average path length
    pub avg_path_length: f64,
    /// Maximum path length found
    pub max_path_length: usize,
    /// Number of loops detected
    pub loop_count: usize,
}

/// Enumerate paths with detailed metadata
///
/// Returns both the paths and information about enumeration quality.
/// This helps AI agents understand if results are complete or truncated.
pub fn enumerate_paths_with_metadata(cfg: &Cfg, limits: &PathLimits) -> PathEnumerationResult {
    let paths = enumerate_paths_iterative(cfg, limits);

    let mut stats = EnumerationStats {
        total_paths: paths.len(),
        normal_paths: 0,
        error_paths: 0,
        degenerate_paths: 0,
        unreachable_paths: 0,
        avg_path_length: 0.0,
        max_path_length: 0,
        loop_count: 0,
    };

    if paths.is_empty() {
        return PathEnumerationResult {
            paths,
            limits_hit: LimitsHit::None,
            stats,
        };
    }

    let mut total_length = 0;

    for path in &paths {
        match path.kind {
            PathKind::Normal => stats.normal_paths += 1,
            PathKind::Error => stats.error_paths += 1,
            PathKind::Degenerate => stats.degenerate_paths += 1,
            PathKind::Unreachable => stats.unreachable_paths += 1,
        }

        let len = path.len();
        total_length += len;
        if len > stats.max_path_length {
            stats.max_path_length = len;
        }
    }

    stats.avg_path_length = total_length as f64 / paths.len() as f64;

    stats.loop_count = crate::cfg::loops::find_loop_headers(cfg).len();

    let limits_hit = if paths.len() >= limits.max_paths {
        LimitsHit::MaxPaths
    } else if stats.max_path_length >= limits.max_length {
        LimitsHit::MaxLength
    } else {
        LimitsHit::None
    };

    PathEnumerationResult {
        paths,
        limits_hit,
        stats,
    }
}

/// Get paths from cache or enumerate them
///
/// This bridge function connects the caching layer to path enumeration.
/// It checks if cached paths exist for the given function and hash,
/// and only enumerates if the cache is stale or empty.
pub fn get_or_enumerate_paths(
    cfg: &Cfg,
    function_id: i64,
    function_hash: &str,
    limits: &PathLimits,
    db_conn: &mut rusqlite::Connection,
) -> Result<Vec<Path>, String> {
    use crate::storage::paths::{get_cached_paths, invalidate_function_paths, store_paths};

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
            if !paths.is_empty() {
                return Ok(paths);
            }
        }
    }

    let paths = enumerate_paths(cfg, limits);

    let _ = invalidate_function_paths(db_conn, function_id);

    store_paths(db_conn, function_id, &paths)
        .map_err(|e| format!("Failed to store enumerated paths: {}", e))?;

    Ok(paths)
}
