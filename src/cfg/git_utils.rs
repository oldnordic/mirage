//! Git utilities for incremental analysis
//!
//! This module provides functions to detect changed files in a git repository,
//! enabling incremental analysis of only modified functions.

use anyhow::Result;
use git2::Repository;
use std::path::Path;
use std::collections::HashSet;

/// Get list of changed Rust files since a git revision
///
/// # Arguments
///
/// * `repo_path` - Path to git repository
/// * `since_revision` - Git revision (e.g., "HEAD~1", "main")
///
/// # Returns
///
/// * `Ok(HashSet<String>)` - Set of changed file paths (relative to repo root)
///
/// # Examples
///
/// ```no_run
/// use mirage::cfg::git_utils::get_changed_rust_files;
/// use std::path::Path;
///
/// let changed = get_changed_rust_files(Path::new("."), "HEAD~1").unwrap();
/// for file in changed {
///     println!("Changed: {}", file);
/// }
/// ```
pub fn get_changed_rust_files(
    repo_path: &Path,
    since_revision: &str,
) -> Result<HashSet<String>> {
    let repo = Repository::open(repo_path)
        .map_err(|e| anyhow::anyhow!("Failed to open git repository: {}", e))?;

    // Get commit object for revision
    let rev = repo.revparse_single(since_revision)
        .map_err(|e| anyhow::anyhow!("Failed to parse revision '{}': {}", since_revision, e))?;

    // Get parent commit (for HEAD~1 pattern)
    let parent = rev.as_commit()
        .and_then(|c| c.parent(0).ok())
        .ok_or_else(|| anyhow::anyhow!("Commit has no parent"))?;

    // Get tree for parent
    let parent_tree = parent.tree()?;

    // Get HEAD tree
    let head_tree = repo.head()
        .map_err(|e| anyhow::anyhow!("Failed to get HEAD: {}", e))?
        .peel_to_tree()?;

    // Create diff between parent and HEAD
    let diff = repo.diff_tree_to_tree(
        Some(&parent_tree),
        Some(&head_tree),
        None,
    )?;

    // Collect changed Rust files
    let mut changed_files = HashSet::new();

    diff.foreach(
        &mut |delta, _progress| {
            if let Some(new_file) = delta.new_file().path() {
                if new_file.extension().map_or(false, |e| e == "rs") {
                    if let Some(path_str) = new_file.to_str() {
                        changed_files.insert(path_str.to_string());
                    }
                }
            }
            true
        },
        None,
        None,
        None,
    )?;

    Ok(changed_files)
}

/// Get list of function entity IDs for functions in changed files
///
/// This queries the database for all functions defined in the
/// changed Rust files, enabling selective re-analysis.
///
/// # Arguments
///
/// * `backend` - MirageDb backend (provides access to GraphBackend)
/// * `repo_path` - Path to git repository
/// * `since_revision` - Git revision
///
/// # Returns
///
/// * `Ok(Vec<i64>)` - Vector of function entity IDs
///
/// # Note
///
/// For large codebases, this queries all entities and filters by file path.
/// Future versions may add a file index table for O(1) lookups.
pub fn get_changed_functions(
    backend: &crate::storage::MirageDb,
    repo_path: &Path,
    since_revision: &str,
) -> Result<Vec<i64>> {
    use sqlitegraph::SnapshotId;

    let changed_files = get_changed_rust_files(repo_path, since_revision)?;

    // Use backend.backend() to access GraphBackend
    let graph_backend = backend.backend();
    let snapshot = SnapshotId::current();

    let mut function_ids = Vec::new();

    // Get all entities - entity_ids() takes no arguments
    let all_entities = graph_backend.entity_ids()?;

    for entity_id in all_entities {
        // get_node takes snapshot_id first, then entity_id
        // Returns Result<GraphEntity> (not Option)
        if let Ok(entity) = graph_backend.get_node(snapshot, entity_id) {
            // GraphEntity has kind: String and data: Map<String, Value>
            // Check if this is a function
            let is_function = entity.kind == "Symbol"
                && entity.data.get("kind")
                    .and_then(|v| v.as_str())
                    .map_or(false, |k| k == "Function");

            if is_function {
                // Check if function is in a changed file
                // file info is in entity.data.get("file")
                if let Some(entity_file_value) = entity.data.get("file") {
                    if let Some(entity_file_str) = entity_file_value.as_str() {
                        for changed_file in &changed_files {
                            // Match if file path contains the changed file or vice versa
                            // This handles both relative and absolute path variations
                            if entity_file_str.contains(changed_file) || changed_file.contains(entity_file_str) {
                                function_ids.push(entity_id);
                                break; // Found a match, no need to check other changed files
                            }
                        }
                    }
                }
            }
        }
    }

    // Deduplicate function_ids
    function_ids.sort();
    function_ids.dedup();

    Ok(function_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_changed_rust_files() {
        // This test requires a git repository
        // Skip in CI environments without git
        let repo_path = Path::new(".");
        if let Ok(changed) = get_changed_rust_files(repo_path, "HEAD~1") {
            // If successful, we should get a HashSet (may be empty)
            println!("Changed files: {:?}", changed);
        }
    }
}
