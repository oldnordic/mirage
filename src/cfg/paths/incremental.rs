use super::{enumerate_paths, Path};

/// Result of incremental path enumeration
///
/// Contains the enumerated paths along with statistics about
/// how many functions were analyzed vs skipped.
#[derive(Debug, Clone)]
pub struct IncrementalPathsResult {
    /// Number of functions that were analyzed
    pub analyzed_functions: usize,
    /// Number of functions that were skipped (not changed)
    pub skipped_functions: usize,
    /// All enumerated paths from analyzed functions
    pub paths: Vec<Path>,
}

/// Enumerate paths incrementally - only for functions in changed files
///
/// This function enables incremental analysis by using git diff to detect
/// changed files and only analyzing functions in those files.
///
/// # Arguments
///
/// * `function_id` - Function to analyze (can be "all" for project-wide)
/// * `backend` - MirageDb backend for database access
/// * `repo_path` - Path to git repository
/// * `since_revision` - Git revision (e.g., "HEAD~1")
/// * `max_length` - Optional maximum path length for pruning
///
/// # Returns
///
/// * `Ok(IncrementalPathsResult)` - Results with analyzed/skipped counts and paths
pub fn enumerate_paths_incremental(
    function_id: &str,
    backend: &crate::storage::MirageDb,
    repo_path: &std::path::Path,
    since_revision: &str,
    max_length: Option<usize>,
) -> anyhow::Result<IncrementalPathsResult> {
    use crate::cfg::{load_cfg_from_db, PathLimits};

    if function_id != "all" {
        let fid = function_id
            .parse::<i64>()
            .map_err(|_| anyhow::anyhow!("Invalid function ID: {}", function_id))?;

        let cfg = load_cfg_from_db(backend, fid)?;
        let limits = PathLimits {
            max_length: max_length.unwrap_or(1000),
            ..Default::default()
        };
        let paths = enumerate_paths(&cfg, &limits);

        return Ok(IncrementalPathsResult {
            analyzed_functions: 1,
            skipped_functions: 0,
            paths,
        });
    }

    let changed_function_ids =
        crate::cfg::git_utils::get_changed_functions(backend, repo_path, since_revision)?;

    if changed_function_ids.is_empty() {
        return Ok(IncrementalPathsResult {
            analyzed_functions: 0,
            skipped_functions: 0,
            paths: vec![],
        });
    }

    let mut all_paths = Vec::new();
    let mut analyzed = 0;

    for fid in changed_function_ids {
        match load_cfg_from_db(backend, fid) {
            Ok(cfg) => {
                let limits = PathLimits {
                    max_length: max_length.unwrap_or(1000),
                    ..Default::default()
                };
                let paths = enumerate_paths(&cfg, &limits);
                all_paths.extend(paths);
                analyzed += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to load CFG for function {}: {}", fid, e);
            }
        }
    }

    Ok(IncrementalPathsResult {
        analyzed_functions: analyzed,
        skipped_functions: 0,
        paths: all_paths,
    })
}
