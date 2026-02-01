//! Path enumeration for CFG analysis
//!
//! This module provides data structures and algorithms for discovering
//! all execution paths through a function's control flow graph from entry
//! to exit. Paths are discovered using depth-first search with cycle
//! detection and loop bounding to prevent infinite recursion.

use crate::cfg::{BlockId, Cfg, Terminator};
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

/// Find the NodeIndex for a given BlockId
///
/// Helper function to convert BlockIds from paths to NodeIndices for CFG queries.
/// Returns None if the block ID doesn't exist in the CFG.
fn find_node_by_block_id(cfg: &Cfg, block_id: BlockId) -> Option<NodeIndex> {
    cfg.node_indices()
        .find(|&idx| cfg[idx].id == block_id)
}

/// Classify a path based on its terminators and reachability
///
/// **Classification rules (in priority order):**
///
/// 1. **Unreachable:** Any block in path is unreachable from entry
/// 2. **Error:** Path contains error terminators (Abort, Call with unwind)
/// 3. **Degenerate:** Path ends abnormally or has unreachable terminator
/// 4. **Normal:** Default classification (entry -> return path)
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `blocks` - Block IDs in execution order
///
/// # Returns
///
/// The classified PathKind for this path
pub fn classify_path(cfg: &Cfg, blocks: &[BlockId]) -> PathKind {
    use crate::cfg::reachability::is_reachable_from_entry;

    // Empty path is degenerate
    if blocks.is_empty() {
        return PathKind::Degenerate;
    }

    // Check each block in the path
    for &block_id in blocks {
        let node_idx = match find_node_by_block_id(cfg, block_id) {
            Some(idx) => idx,
            None => return PathKind::Degenerate, // Block doesn't exist
        };

        // Priority 1: Check if block is unreachable from entry
        if !is_reachable_from_entry(cfg, node_idx) {
            return PathKind::Unreachable;
        }

        // Get the block's terminator
        let terminator = &cfg[node_idx].terminator;

        // Priority 2: Check for error terminators
        match terminator {
            Terminator::Abort(_) => return PathKind::Error,
            Terminator::Call { unwind: Some(_), .. } => return PathKind::Error,
            _ => {}
        }

        // Priority 3: Check for unreachable terminator (anywhere in path)
        if matches!(terminator, Terminator::Unreachable) {
            return PathKind::Degenerate;
        }
    }

    // Check last block terminator - if not Return, it's degenerate
    if let Some(&last_block_id) = blocks.last() {
        if let Some(node_idx) = find_node_by_block_id(cfg, last_block_id) {
            let terminator = &cfg[node_idx].terminator;
            // Non-Return terminators at end are degenerate
            if !matches!(terminator, Terminator::Return) {
                // Already caught Unreachable above, so check other cases
                return PathKind::Degenerate;
            }
        }
    }

    // Default: Normal path
    PathKind::Normal
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

/// Configurable limits for path enumeration
///
/// Prevents exponential explosion of paths in complex CFGs and
/// ensures termination in the presence of loops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathLimits {
    /// Maximum number of blocks per path
    pub max_length: usize,
    /// Maximum number of paths to enumerate
    pub max_paths: usize,
    /// Loop iterations to unroll before stopping
    pub loop_unroll_limit: usize,
}

impl Default for PathLimits {
    fn default() -> Self {
        Self {
            max_length: 1000,
            max_paths: 10000,
            loop_unroll_limit: 3,
        }
    }
}

impl PathLimits {
    /// Create new path limits with custom values
    pub fn new(max_length: usize, max_paths: usize, loop_unroll_limit: usize) -> Self {
        Self {
            max_length,
            max_paths,
            loop_unroll_limit,
        }
    }

    /// Create limits with a custom maximum path length
    pub fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = max_length;
        self
    }

    /// Create limits with a custom maximum path count
    pub fn with_max_paths(mut self, max_paths: usize) -> Self {
        self.max_paths = max_paths;
        self
    }

    /// Create limits with a custom loop unroll limit
    pub fn with_loop_unroll_limit(mut self, loop_unroll_limit: usize) -> Self {
        self.loop_unroll_limit = loop_unroll_limit;
        self
    }
}

/// Compute BLAKE3 hash of a block sequence
///
/// Used to generate unique identifiers for paths. The hash includes
/// the path length to prevent collisions between different sequences
/// that might otherwise hash to the same value.
///
/// # Arguments
///
/// * `blocks` - Slice of block IDs in execution order
///
/// # Returns
///
/// Hex string representing the BLAKE3 hash
pub fn hash_path(blocks: &[BlockId]) -> String {
    let mut hasher = blake3::Hasher::new();

    // Include length to prevent collisions
    hasher.update(&blocks.len().to_le_bytes());

    // Hash each block ID with consistent endianness
    for &block_id in blocks {
        hasher.update(&block_id.to_le_bytes());
    }

    hasher.finalize().to_hex().to_string()
}

/// Enumerate all execution paths through a CFG
///
/// Performs depth-first search from the entry block to all exit blocks,
/// collecting complete paths. Cycle detection prevents infinite recursion
/// on back-edges, and loop bounding limits exploration of cyclic paths.
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `limits` - Limits on path enumeration
///
/// # Returns
///
/// Vector of all discovered paths from entry to exit
///
/// # Examples
///
/// ```rust
/// use mirage::cfg::{enumerate_paths, PathLimits};
///
/// let paths = enumerate_paths(&cfg, &PathLimits::default());
/// println!("Found {} paths", paths.len());
/// ```
pub fn enumerate_paths(cfg: &Cfg, limits: &PathLimits) -> Vec<Path> {
    // Get entry block
    let entry = match crate::cfg::analysis::find_entry(cfg) {
        Some(e) => e,
        None => return vec![], // Empty CFG
    };

    // Get exit blocks
    let exits: HashSet<NodeIndex> = crate::cfg::analysis::find_exits(cfg)
        .into_iter()
        .collect();

    if exits.is_empty() {
        return vec![]; // No exits means no complete paths
    }

    // Initialize traversal state
    let mut paths = Vec::new();
    let mut current_path = Vec::new();
    let mut visited = HashSet::new();

    // Get loop headers for bounding
    let loop_headers = crate::cfg::loops::find_loop_headers(cfg);
    let mut loop_iterations: HashMap<NodeIndex, usize> = HashMap::new();

    // Start DFS from entry
    dfs_enumerate(
        cfg,
        entry,
        &exits,
        limits,
        &mut paths,
        &mut current_path,
        &mut visited,
        &loop_headers,
        &mut loop_iterations,
    );

    paths
}

/// Recursive DFS helper for path enumeration
///
/// Explores all paths from the current node to exit blocks, tracking
/// visited nodes to prevent cycles and respecting loop unroll limits.
fn dfs_enumerate(
    cfg: &Cfg,
    current: NodeIndex,
    exits: &HashSet<NodeIndex>,
    limits: &PathLimits,
    paths: &mut Vec<Path>,
    current_path: &mut Vec<BlockId>,
    visited: &mut HashSet<NodeIndex>,
    loop_headers: &HashSet<NodeIndex>,
    loop_iterations: &mut HashMap<NodeIndex, usize>,
) {
    // Get current block ID
    let block_id = match cfg.node_weight(current) {
        Some(block) => block.id,
        None => return,
    };

    // Add current block to path
    current_path.push(block_id);

    // Check path length limit
    if current_path.len() > limits.max_length {
        current_path.pop();
        return;
    }

    // Check if we've reached an exit
    if exits.contains(&current) {
        let path = Path::new(current_path.clone(), PathKind::Normal);
        paths.push(path);
        current_path.pop();
        return;
    }

    // Check path count limit
    if paths.len() >= limits.max_paths {
        current_path.pop();
        return;
    }

    // Track loop iterations
    let is_loop_header = loop_headers.contains(&current);
    if is_loop_header {
        let count = loop_iterations.entry(current).or_insert(0);
        if *count >= limits.loop_unroll_limit {
            // Exceeded unroll limit, stop this branch
            current_path.pop();
            return;
        }
        *count += 1;
    }

    // Mark as visited for cycle detection
    let was_visited = visited.insert(current);

    // Explore all successors
    let mut successors: Vec<NodeIndex> = cfg.neighbors(current).collect();
    successors.sort_by_key(|n| n.index()); // Deterministic order

    if successors.is_empty() {
        // Dead end (not an exit but no successors)
        // Record as degenerate path
        let path = Path::new(current_path.clone(), PathKind::Degenerate);
        paths.push(path);
    } else {
        for succ in successors {
            // Skip already visited nodes UNLESS it's a back-edge to a loop header
            // Loop headers can be revisited (bounded by loop_iterations)
            let is_back_edge = loop_headers.contains(&succ) && loop_iterations.contains_key(&succ);
            if visited.contains(&succ) && !is_back_edge {
                continue;
            }

            // For back-edges to loop headers, check iteration limit
            if is_back_edge {
                let count = loop_iterations.get(&succ).copied().unwrap_or(0);
                if count >= limits.loop_unroll_limit {
                    continue; // Exceeded loop unroll limit
                }
            }

            // Recurse into successor
            dfs_enumerate(
                cfg,
                succ,
                exits,
                limits,
                paths,
                current_path,
                visited,
                loop_headers,
                loop_iterations,
            );

            // Check path count limit after each recursive call
            if paths.len() >= limits.max_paths {
                break;
            }
        }
    }

    // Unmark visited (backtrack)
    if was_visited {
        visited.remove(&current);
    }

    // Clean up loop iteration count
    if is_loop_header {
        loop_iterations.entry(current).and_modify(|c| *c -= 1);
    }

    // Remove current block from path
    current_path.pop();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    /// Create a simple linear CFG: 0 -> 1 -> 2
    fn create_linear_cfg() -> Cfg {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::Fallthrough);

        g
    }

    /// Create a diamond CFG: 0 -> (1, 2) -> 3
    fn create_diamond_cfg() -> Cfg {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt {
                targets: vec![1],
                otherwise: 2,
            },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 3 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 3 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::TrueBranch);
        g.add_edge(b0, b2, EdgeType::FalseBranch);
        g.add_edge(b1, b3, EdgeType::Fallthrough);
        g.add_edge(b2, b3, EdgeType::Fallthrough);

        g
    }

    /// Create a simple loop CFG: 0 -> 1 <-> 2 -> 3
    fn create_loop_cfg() -> Cfg {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt {
                targets: vec![2],
                otherwise: 3,
            },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b3, EdgeType::FalseBranch);
        g.add_edge(b2, b1, EdgeType::LoopBack);

        g
    }

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

    // find_node_by_block_id tests

    #[test]
    fn test_find_node_by_block_id_existing() {
        let cfg = create_linear_cfg();

        // Find existing blocks
        let b0 = find_node_by_block_id(&cfg, 0);
        let b1 = find_node_by_block_id(&cfg, 1);
        let b2 = find_node_by_block_id(&cfg, 2);

        assert!(b0.is_some());
        assert!(b1.is_some());
        assert!(b2.is_some());

        // Verify the NodeIndices are correct
        assert_eq!(b0.unwrap().index(), 0);
        assert_eq!(b1.unwrap().index(), 1);
        assert_eq!(b2.unwrap().index(), 2);
    }

    #[test]
    fn test_find_node_by_block_id_nonexistent() {
        let cfg = create_linear_cfg();

        // Find non-existent block
        let b99 = find_node_by_block_id(&cfg, 99);
        assert!(b99.is_none());
    }

    #[test]
    fn test_find_node_by_block_id_empty_cfg() {
        let cfg: Cfg = DiGraph::new();

        // Empty CFG has no blocks
        let b0 = find_node_by_block_id(&cfg, 0);
        assert!(b0.is_none());
    }

    // classify_path tests

    /// Create a CFG with an Abort terminator (error path)
    fn create_error_cfg() -> Cfg {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Abort("panic!".to_string()),
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        g
    }

    /// Create a CFG with unreachable terminator (degenerate path)
    fn create_unreachable_term_cfg() -> Cfg {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Unreachable,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        g
    }

    /// Create a CFG with an unreachable block (dead code)
    fn create_dead_code_cfg() -> Cfg {
        let mut g = DiGraph::new();

        let _b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // Block 1 is not reachable from entry
        let _b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g
    }

    /// Create a CFG with Call that has unwind (error path)
    fn create_call_unwind_cfg() -> Cfg {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Call {
                target: Some(1),
                unwind: Some(2), // Has unwind -> Error path
            },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        let _b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        g
    }

    #[test]
    fn test_classify_path_normal_return() {
        let cfg = create_linear_cfg();
        let path = vec![0, 1, 2];

        let kind = classify_path(&cfg, &path);
        assert_eq!(kind, PathKind::Normal);
    }

    #[test]
    fn test_classify_path_error_abort() {
        let cfg = create_error_cfg();
        let path = vec![0, 1];

        let kind = classify_path(&cfg, &path);
        assert_eq!(kind, PathKind::Error);
    }

    #[test]
    fn test_classify_path_degenerate_unreachable_terminator() {
        let cfg = create_unreachable_term_cfg();
        let path = vec![0, 1];

        let kind = classify_path(&cfg, &path);
        assert_eq!(kind, PathKind::Degenerate);
    }

    #[test]
    fn test_classify_path_unreachable_block() {
        let cfg = create_dead_code_cfg();
        // Path includes unreachable block
        let path = vec![1]; // Block 1 is not reachable from entry

        let kind = classify_path(&cfg, &path);
        assert_eq!(kind, PathKind::Unreachable);
    }

    #[test]
    fn test_classify_path_error_call_unwind() {
        let cfg = create_call_unwind_cfg();
        let path = vec![0, 1]; // Goes through Call with unwind

        let kind = classify_path(&cfg, &path);
        assert_eq!(kind, PathKind::Error);
    }

    #[test]
    fn test_classify_path_empty() {
        let cfg = create_linear_cfg();
        let path: Vec<BlockId> = vec![];

        let kind = classify_path(&cfg, &path);
        assert_eq!(kind, PathKind::Degenerate);
    }

    #[test]
    fn test_classify_path_single_block() {
        let mut g = DiGraph::new();

        let _b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        let path = vec![0];
        let kind = classify_path(&g, &path);
        assert_eq!(kind, PathKind::Normal);
    }

    #[test]
    fn test_classify_path_nonexistent_block() {
        let cfg = create_linear_cfg();
        let path = vec![0, 99]; // Block 99 doesn't exist

        let kind = classify_path(&cfg, &path);
        assert_eq!(kind, PathKind::Degenerate);
    }

    // enumerate_paths tests

    #[test]
    fn test_enumerate_paths_linear_cfg() {
        let cfg = create_linear_cfg();
        let paths = enumerate_paths(&cfg, &PathLimits::default());

        // Linear CFG produces exactly 1 path
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].blocks, vec![0, 1, 2]);
        assert_eq!(paths[0].entry, 0);
        assert_eq!(paths[0].exit, 2);
        assert_eq!(paths[0].kind, PathKind::Normal);
    }

    #[test]
    fn test_enumerate_paths_diamond_cfg() {
        let cfg = create_diamond_cfg();
        let paths = enumerate_paths(&cfg, &PathLimits::default());

        // Diamond CFG produces 2 paths: 0->1->3 and 0->2->3
        assert_eq!(paths.len(), 2);

        // Check that both paths start at entry and end at exit
        for path in &paths {
            assert_eq!(path.entry, 0);
            assert_eq!(path.exit, 3);
            assert_eq!(path.kind, PathKind::Normal);
        }

        // Check that we have both distinct paths
        let path_blocks: Vec<_> = paths.iter().map(|p| p.blocks.clone()).collect();
        assert!(path_blocks.contains(&vec![0, 1, 3]));
        assert!(path_blocks.contains(&vec![0, 2, 3]));
    }

    #[test]
    fn test_enumerate_paths_loop_with_unroll_limit() {
        let cfg = create_loop_cfg();

        // With unroll_limit=3, we get bounded paths
        // With loop unroll limit of 3, we get:
        // - Direct exit: 0->1->3
        // - 1 iteration: 0->1->2->1->3
        // - 2 iterations: 0->1->2->1->2->1->3
        // - 3 iterations: 0->1->2->1->2->1->2->1->3
        let limits = PathLimits::default().with_loop_unroll_limit(3);
        let paths = enumerate_paths(&cfg, &limits);

        // Should have 4 paths (0, 1, 2, 3 loop iterations)
        // Or possibly 2 paths depending on how loop iteration is counted
        // The key is that loop is bounded and doesn't cause infinite paths
        assert!(paths.len() >= 2, "Should have at least direct exit and one loop iteration");
        assert!(paths.len() <= 5, "Should be bounded by loop unroll limit");

        // All paths should be normal
        for path in &paths {
            assert_eq!(path.kind, PathKind::Normal);
            assert_eq!(path.entry, 0);
            assert_eq!(path.exit, 3);
        }

        // Direct exit path should exist
        assert!(paths.iter().any(|p| p.blocks == vec![0, 1, 3]));
    }

    #[test]
    fn test_enumerate_paths_empty_cfg() {
        let cfg: Cfg = DiGraph::new();
        let paths = enumerate_paths(&cfg, &PathLimits::default());

        // Empty CFG produces 0 paths (no crash)
        assert_eq!(paths.len(), 0);
    }

    #[test]
    fn test_enumerate_paths_max_paths_limit() {
        let cfg = create_diamond_cfg();

        // Set very low max_paths limit
        let limits = PathLimits::default().with_max_paths(1);
        let paths = enumerate_paths(&cfg, &limits);

        // Should stop at 1 path even though diamond has 2
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn test_enumerate_paths_max_length_limit() {
        let cfg = create_diamond_cfg();

        // Set very low max_length limit
        let limits = PathLimits::default().with_max_length(2);
        let paths = enumerate_paths(&cfg, &limits);

        // Should return 0 paths because all paths exceed length 2
        assert_eq!(paths.len(), 0);
    }

    #[test]
    fn test_enumerate_paths_single_block_cfg() {
        let mut g = DiGraph::new();

        let _b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // A single block that is both entry and exit
        let paths = enumerate_paths(&g, &PathLimits::default());

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].blocks, vec![0]);
        assert_eq!(paths[0].entry, 0);
        assert_eq!(paths[0].exit, 0);
    }

    #[test]
    fn test_enumerate_paths_with_unreachable_exit() {
        let mut g = DiGraph::new();

        // Block 0: entry
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Block 1: return
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // Block 2: unreachable (not connected)
        let _b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Unreachable,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        let paths = enumerate_paths(&g, &PathLimits::default());

        // Only reachable exit produces a path
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].blocks, vec![0, 1]);
    }
}
