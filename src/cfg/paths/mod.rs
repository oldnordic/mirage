//! Path enumeration for CFG analysis
//!
//! This module provides data structures and algorithms for discovering
//! all execution paths through a function's control flow graph from entry
//! to exit. Paths are discovered using depth-first search with cycle
//! detection and loop bounding to prevent infinite recursion.
//!
//! ## Feasibility Checking
//!
//! This module provides **STATIC** feasibility checking only. It does NOT
//! perform symbolic execution or data flow analysis.
//!
//! ### What we check:
//! - Entry block is actually Entry kind
//! - Exit block has valid terminator (Return, Abort, Call with target)
//! - All blocks are reachable from entry
//! - No dead ends (Goto/SwitchInt as last terminator)
//!
//! ### What we DON'T check (requires symbolic execution):
//! - Conflicting branch conditions (e.g., `if x > 5 && x < 3`)
//! - Data-dependent constraints (array bounds, divide by zero)
//! - Runtime panic conditions
//!
//! ### Tradeoff:
//! Static checking is fast (O(n)) and sound (never falsely claims feasible).
//! Symbolic execution is precise but slow (>100x) and complex.
//!
//! For most code intelligence queries, static checking is sufficient.
//! Future work may add symbolic execution for specific paths.
//!
//! ## Path Classification
//!
//! Paths are categorized based on their structure and content:
//! - **Normal:** Standard entry → return path
//! - **Error:** Contains panic, abort, or error propagation
//! - **Degenerate:** Dead end, infinite loop, or infeasible path
//! - **Unreachable:** Statically unreachable code path

pub mod cached;
pub mod enumeration;
pub mod feasibility;
pub mod incremental;
pub mod limits;
pub mod metadata;

pub use cached::{
    check_path_explosion, enumerate_paths_cached, enumerate_paths_cached_with_context,
    estimate_path_count,
};
pub use enumeration::{
    enumerate_paths, enumerate_paths_iterative, enumerate_paths_outcome,
    enumerate_paths_with_context, enumerate_paths_with_context_outcome, PathEnumeration,
};
pub use feasibility::{
    classify_path, classify_path_precomputed, is_feasible_path, is_feasible_path_precomputed,
};
pub use incremental::{enumerate_paths_incremental, IncrementalPathsResult};
pub use limits::{hash_path, EnumerationContext, PathLimits};
pub use metadata::{
    enumerate_paths_with_metadata, get_or_enumerate_paths, EnumerationStats, LimitsHit,
    PathEnumerationResult,
};

use crate::cfg::{BlockId, Cfg};
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

/// Execution path through a CFG
///
/// Represents a sequence of basic blocks from an entry block to an exit block.
/// Each path has a unique identifier derived from a BLAKE3 hash of the block
/// sequence for deduplication and comparison.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Path {
    /// Unique identifier (BLAKE3 hash of block sequence)
    pub path_id: String,
    /// Ordered block IDs in execution order
    pub blocks: Vec<BlockId>,
    /// Classification of this path
    pub kind: PathKind,
    /// First block (entry)
    pub entry: BlockId,
    /// Last block (exit)
    pub exit: BlockId,
}

impl Path {
    /// Create a new path from a block sequence
    pub fn new(blocks: Vec<BlockId>, kind: PathKind) -> Self {
        let entry = *blocks.first().unwrap_or(&0);
        let exit = *blocks.last().unwrap_or(&0);
        let path_id = hash_path(&blocks);

        Self {
            path_id,
            blocks,
            kind,
            entry,
            exit,
        }
    }

    /// Create a path with a pre-existing path_id (for loading from cache)
    ///
    /// # Arguments
    ///
    /// * `path_id` - The stored path identifier
    /// * `blocks` - Ordered block IDs in execution order
    /// * `kind` - Path classification
    ///
    /// # Note
    ///
    /// This bypasses the normal path_id computation. Use only when loading
    /// previously stored paths where the path_id was already computed.
    pub fn with_id(path_id: String, blocks: Vec<BlockId>, kind: PathKind) -> Self {
        let entry = *blocks.first().unwrap_or(&0);
        let exit = *blocks.last().unwrap_or(&0);

        Self {
            path_id,
            blocks,
            kind,
            entry,
            exit,
        }
    }

    /// Get the length of this path (number of blocks)
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Check if this path is empty
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Get an iterator over the blocks in this path
    pub fn iter(&self) -> impl Iterator<Item = &BlockId> {
        self.blocks.iter()
    }

    /// Check if this path contains a specific block
    pub fn contains(&self, block_id: BlockId) -> bool {
        self.blocks.contains(&block_id)
    }
}

impl std::fmt::Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path_id)
    }
}

/// Classification of execution paths
///
/// Paths are categorized based on their structure and content.
/// Classification is used for analysis and reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PathKind {
    /// Standard entry -> return path
    Normal,
    /// Contains panic, abort, or error propagation
    Error,
    /// Dead end or infinite loop (no valid exit)
    Degenerate,
    /// Statically unreachable code path
    Unreachable,
}

impl PathKind {
    /// Check if this path represents a normal execution
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    /// Check if this path represents an error condition
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error)
    }

    /// Check if this path is degenerate (abnormal structure)
    pub fn is_degenerate(&self) -> bool {
        matches!(self, Self::Degenerate)
    }

    /// Check if this path is unreachable
    pub fn is_unreachable(&self) -> bool {
        matches!(self, Self::Unreachable)
    }
}

/// Find the NodeIndex for a given BlockId
///
/// Helper function to convert BlockIds from paths to NodeIndices for CFG queries.
/// Returns None if the block ID doesn't exist in the CFG.
fn find_node_by_block_id(cfg: &Cfg, block_id: BlockId) -> Option<NodeIndex> {
    cfg.node_indices().find(|&idx| cfg[idx].id == block_id)
}

#[cfg(test)]
mod test_fixtures;
#[cfg(test)]
mod tests_enumeration;
#[cfg(test)]
mod tests_enumeration_advanced;
#[cfg(test)]
mod tests_feasibility;

#[cfg(test)]
mod tests {
    use super::*;
    use petgraph::graph::DiGraph;

    #[test]
    fn test_hash_path_deterministic() {
        let blocks = vec![0, 1, 2];
        let hash1 = hash_path(&blocks);
        let hash2 = hash_path(&blocks);

        assert_eq!(hash1, hash2, "Same input should produce same hash");
    }

    #[test]
    fn test_hash_path_different_sequences() {
        let blocks1 = vec![0, 1, 2];
        let blocks2 = vec![0, 2, 1];

        assert_ne!(hash_path(&blocks1), hash_path(&blocks2));
    }

    #[test]
    fn test_hash_path_length_collision_protection() {
        let blocks1 = vec![1, 2, 3];
        let blocks2 = vec![1, 2, 3, 0];

        assert_ne!(hash_path(&blocks1), hash_path(&blocks2));
    }

    #[test]
    fn test_path_new() {
        let blocks = vec![0, 1, 2];
        let path = Path::new(blocks.clone(), PathKind::Normal);

        assert_eq!(path.blocks, blocks);
        assert_eq!(path.entry, 0);
        assert_eq!(path.exit, 2);
        assert_eq!(path.kind, PathKind::Normal);
        assert!(!path.path_id.is_empty());
    }

    #[test]
    fn test_path_len() {
        let blocks = vec![0, 1, 2];
        let path = Path::new(blocks, PathKind::Normal);

        assert_eq!(path.len(), 3);
        assert!(!path.is_empty());
    }

    #[test]
    fn test_path_contains() {
        let blocks = vec![0, 1, 2];
        let path = Path::new(blocks, PathKind::Normal);

        assert!(path.contains(0));
        assert!(path.contains(1));
        assert!(path.contains(2));
        assert!(!path.contains(3));
    }

    #[test]
    fn test_path_limits_default() {
        let limits = PathLimits::default();

        assert_eq!(limits.max_length, 1000);
        assert_eq!(limits.max_paths, 10000);
        assert_eq!(limits.loop_unroll_limit, 3);
    }

    #[test]
    fn test_path_limits_custom() {
        let limits = PathLimits::new(100, 500, 5);

        assert_eq!(limits.max_length, 100);
        assert_eq!(limits.max_paths, 500);
        assert_eq!(limits.loop_unroll_limit, 5);
    }

    #[test]
    fn test_path_limits_builder() {
        let limits = PathLimits::default()
            .with_max_length(200)
            .with_max_paths(1000)
            .with_loop_unroll_limit(10);

        assert_eq!(limits.max_length, 200);
        assert_eq!(limits.max_paths, 1000);
        assert_eq!(limits.loop_unroll_limit, 10);
    }

    #[test]
    fn test_path_kind_is_normal() {
        assert!(PathKind::Normal.is_normal());
        assert!(!PathKind::Error.is_normal());
        assert!(!PathKind::Degenerate.is_normal());
        assert!(!PathKind::Unreachable.is_normal());
    }

    #[test]
    fn test_path_kind_is_error() {
        assert!(PathKind::Error.is_error());
        assert!(!PathKind::Normal.is_error());
    }

    #[test]
    fn test_path_kind_is_degenerate() {
        assert!(PathKind::Degenerate.is_degenerate());
        assert!(!PathKind::Normal.is_degenerate());
    }

    #[test]
    fn test_path_kind_is_unreachable() {
        assert!(PathKind::Unreachable.is_unreachable());
        assert!(!PathKind::Normal.is_unreachable());
    }

    fn create_linear_cfg() -> Cfg {
        test_fixtures::create_linear_cfg()
    }

    #[test]
    fn test_find_node_by_block_id_existing() {
        let cfg = create_linear_cfg();

        let b0 = find_node_by_block_id(&cfg, 0);
        let b1 = find_node_by_block_id(&cfg, 1);
        let b2 = find_node_by_block_id(&cfg, 2);

        assert!(b0.is_some());
        assert!(b1.is_some());
        assert!(b2.is_some());

        assert_eq!(b0.unwrap().index(), 0);
        assert_eq!(b1.unwrap().index(), 1);
        assert_eq!(b2.unwrap().index(), 2);
    }

    #[test]
    fn test_find_node_by_block_id_nonexistent() {
        let cfg = create_linear_cfg();

        let b99 = find_node_by_block_id(&cfg, 99);
        assert!(b99.is_none());
    }

    #[test]
    fn test_find_node_by_block_id_empty_cfg() {
        let cfg: Cfg = DiGraph::new();

        let b0 = find_node_by_block_id(&cfg, 0);
        assert!(b0.is_none());
    }
}
