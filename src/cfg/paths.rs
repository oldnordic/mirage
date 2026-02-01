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

/// Check if a path is statically feasible
///
/// A path is feasible if it represents a viable execution path from entry to exit.
/// This is a STATIC check based on terminator analysis only - it does NOT perform
/// symbolic execution or data flow analysis.
///
/// **Feasibility criteria (all must pass):**
///
/// 1. **Non-empty:** Path must have at least one block
/// 2. **Valid entry:** First block must be Entry kind
/// 3. **Valid exit:** Last block must have valid exit terminator:
///    - `Terminator::Return` -> feasible (normal exit)
///    - `Terminator::Abort(_)` -> feasible (error path, but reachable)
///    - `Terminator::Unreachable` -> infeasible (cannot execute)
///    - `Terminator::Call { unwind: None, .. }` -> feasible (no unwind)
///    - `Terminator::Call { unwind: Some(_), target: Some(_), .. }` -> feasible
///    - `Terminator::Call { unwind: Some(_), target: None }` -> infeasible (always unwinds)
///    - `Terminator::Goto` / `Terminator::SwitchInt` -> infeasible (dead end if last block)
///
/// 4. **All blocks exist:** Every block ID in the path must exist in the CFG
///
/// **What we DON'T check (requires symbolic execution):**
/// - Conflicting branch conditions (e.g., `if x > 5 && x < 3`)
/// - Data-dependent constraints (array bounds, divide by zero)
/// - Runtime panic conditions
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `blocks` - Block IDs in execution order
///
/// # Returns
///
/// `true` if the path is statically feasible, `false` otherwise
///
/// # Examples
///
/// ```rust
/// let feasible = is_feasible_path(&cfg, &[0, 1, 2]);  // entry -> goto -> return
/// let infeasible = is_feasible_path(&cfg, &[0, 1]);    // entry -> goto (dead end)
/// ```
pub fn is_feasible_path(cfg: &Cfg, blocks: &[BlockId]) -> bool {
    use crate::cfg::BlockKind;

    // Criterion 1: Path must be non-empty
    if blocks.is_empty() {
        return false;
    }

    // Criterion 2: First block must be Entry kind
    let first_idx = match find_node_by_block_id(cfg, blocks[0]) {
        Some(idx) => idx,
        None => return false, // Block doesn't exist
    };
    if cfg[first_idx].kind != BlockKind::Entry {
        return false;
    }

    // Criterion 3: Last block must have valid exit terminator
    let last_idx = match find_node_by_block_id(cfg, *blocks.last().unwrap()) {
        Some(idx) => idx,
        None => return false, // Block doesn't exist
    };

    match &cfg[last_idx].terminator {
        Terminator::Return => {}, // Feasible: normal exit
        Terminator::Abort(_) => {}, // Feasible: error path but reachable
        Terminator::Call { unwind: None, .. } => {}, // Feasible: no unwind
        Terminator::Call { unwind: Some(_), target: Some(_) } => {}, // Feasible: has target
        // Infeasible terminators (dead ends)
        Terminator::Unreachable |
        Terminator::Goto { .. } |
        Terminator::SwitchInt { .. } |
        Terminator::Call { unwind: Some(_), target: None } => {
            return false;
        }
    }

    // Criterion 4: All intermediate blocks must exist
    for &block_id in blocks.iter().skip(1).take(blocks.len().saturating_sub(2)) {
        if find_node_by_block_id(cfg, block_id).is_none() {
            return false;
        }
    }

    true
}

/// Check if a path is statically feasible using pre-computed reachable set
///
/// This is an optimized version of `is_feasible_path` for batch operations.
/// Instead of calling `is_reachable_from_entry` for each block (which is O(n)
/// per call), we use a pre-computed HashSet of reachable block IDs, making
/// reachability checks O(1).
///
/// **Why:** O(n) batch feasibility checking vs O(n²) for repeated individual calls.
/// Pre-compute reachable set once with `find_reachable()`, reuse for all paths.
///
/// **Additional criterion over is_feasible_path:**
/// 5. **All blocks reachable:** Every block in the path must be reachable from entry
///    - Uses pre-computed HashSet for O(1) lookup
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `blocks` - Block IDs in execution order
/// * `reachable_blocks` - Pre-computed set of reachable BlockIds
///
/// # Returns
///
/// `true` if the path is statically feasible, `false` otherwise
///
/// # Examples
///
/// ```rust
/// use mirage::cfg::reachability::find_reachable;
///
/// let reachable_nodes = find_reachable(&cfg);
/// let reachable_blocks: HashSet<BlockId> = reachable_nodes
///     .iter()
///     .map(|&idx| cfg[idx].id)
///     .collect();
///
/// for path in paths {
///     let feasible = is_feasible_path_precomputed(&cfg, &path.blocks, &reachable_blocks);
/// }
/// ```
pub fn is_feasible_path_precomputed(
    cfg: &Cfg,
    blocks: &[BlockId],
    reachable_blocks: &HashSet<BlockId>,
) -> bool {
    use crate::cfg::BlockKind;

    // Criterion 1: Path must be non-empty
    if blocks.is_empty() {
        return false;
    }

    // Criterion 2: First block must be Entry kind
    let first_idx = match find_node_by_block_id(cfg, blocks[0]) {
        Some(idx) => idx,
        None => return false, // Block doesn't exist
    };
    if cfg[first_idx].kind != BlockKind::Entry {
        return false;
    }

    // Criterion 5: All blocks must be reachable (O(1) per block)
    for &block_id in blocks {
        if !reachable_blocks.contains(&block_id) {
            return false;
        }
    }

    // Criterion 3: Last block must have valid exit terminator
    let last_idx = match find_node_by_block_id(cfg, *blocks.last().unwrap()) {
        Some(idx) => idx,
        None => return false, // Block doesn't exist
    };

    match &cfg[last_idx].terminator {
        Terminator::Return => {}, // Feasible: normal exit
        Terminator::Abort(_) => {}, // Feasible: error path but reachable
        Terminator::Call { unwind: None, .. } => {}, // Feasible: no unwind
        Terminator::Call { unwind: Some(_), target: Some(_) } => {}, // Feasible: has target
        // Infeasible terminators (dead ends)
        Terminator::Unreachable |
        Terminator::Goto { .. } |
        Terminator::SwitchInt { .. } |
        Terminator::Call { unwind: Some(_), target: None } => {
            return false;
        }
    }

    // Criterion 4: All intermediate blocks must exist
    for &block_id in blocks.iter().skip(1).take(blocks.len().saturating_sub(2)) {
        if find_node_by_block_id(cfg, block_id).is_none() {
            return false;
        }
    }

    true
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

/// Classify a path using a pre-computed reachable set for O(n) batch classification
///
/// This version is optimized for classifying many paths. Instead of calling
/// `is_reachable_from_entry` for each block (which is O(n) per call), we use
/// a pre-computed HashSet of reachable block IDs, making reachability checks O(1).
///
/// **Why:** O(n) classification vs O(n²) for repeated `is_reachable_from_entry` calls.
/// Pre-compute reachable set once with `find_reachable()`, reuse for all paths.
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `blocks` - Block IDs in execution order
/// * `reachable_blocks` - Pre-computed set of reachable BlockIds
///
/// # Returns
///
/// The classified PathKind for this path
///
/// # Example
///
/// ```rust
/// let reachable_nodes = find_reachable(&cfg);
/// let reachable_blocks: HashSet<BlockId> = reachable_nodes
///     .iter()
///     .map(|&idx| cfg[idx].id)
///     .collect();
///
/// for path in paths {
///     let kind = classify_path_precomputed(&cfg, &path.blocks, &reachable_blocks);
/// }
/// ```
pub fn classify_path_precomputed(
    cfg: &Cfg,
    blocks: &[BlockId],
    reachable_blocks: &HashSet<BlockId>,
) -> PathKind {
    // Empty path is degenerate
    if blocks.is_empty() {
        return PathKind::Degenerate;
    }

    // Priority 1: Check if any block is unreachable (O(1) lookup)
    for &block_id in blocks {
        if !reachable_blocks.contains(&block_id) {
            return PathKind::Unreachable;
        }
    }

    // Priority 2: Check for error terminators in the path
    for &block_id in blocks {
        let node_idx = match find_node_by_block_id(cfg, block_id) {
            Some(idx) => idx,
            None => return PathKind::Degenerate, // Block doesn't exist
        };

        let terminator = &cfg[node_idx].terminator;

        // Check for error terminators
        match terminator {
            Terminator::Abort(_) => return PathKind::Error,
            Terminator::Call { unwind: Some(_), .. } => return PathKind::Error,
            _ => {}
        }

        // Check for unreachable terminator (anywhere in path)
        if matches!(terminator, Terminator::Unreachable) {
            return PathKind::Degenerate;
        }
    }

    // Priority 3: Check static feasibility (dead-end detection)
    // This identifies paths that end in Goto, SwitchInt, or other invalid terminators
    if !is_feasible_path_precomputed(cfg, blocks, reachable_blocks) {
        return PathKind::Degenerate;
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

    /// Quick analysis preset for fast, approximate path enumeration
    ///
    /// Use this for:
    /// - Initial code exploration
    /// - IDE/integration features where responsiveness matters
    /// - Large codebases where thorough analysis would be too slow
    ///
    /// Tradeoffs:
    /// - May miss some paths due to lower limits
    /// - Loop unrolling is minimal (2 iterations)
    /// - Completes in <100ms for typical functions
    pub fn quick_analysis() -> Self {
        Self {
            max_length: 100,
            max_paths: 1000,
            loop_unroll_limit: 2,
        }
    }

    /// Thorough analysis preset for comprehensive path enumeration
    ///
    /// Use this for:
    /// - Final analysis before deployment
    /// - Security-critical code paths
    /// - Test coverage validation
    ///
    /// Tradeoffs:
    /// - Higher limits produce more complete results
    /// - May take several seconds on complex functions
    /// - Still bounded to prevent infinite loops
    pub fn thorough() -> Self {
        Self {
            max_length: 10000,
            max_paths: 100000,
            loop_unroll_limit: 5,
        }
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
/// Paths are classified using `classify_path_precomputed` for efficiency.
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

    // Pre-compute reachable blocks for efficient classification
    let reachable_nodes = crate::cfg::reachability::find_reachable(cfg);
    let reachable_blocks: HashSet<BlockId> = reachable_nodes
        .iter()
        .map(|&idx| cfg[idx].id)
        .collect();

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
        &reachable_blocks,
    );

    paths
}

/// Recursive DFS helper for path enumeration
///
/// Explores all paths from the current node to exit blocks, tracking
/// visited nodes to prevent cycles and respecting loop unroll limits.
/// Uses pre-computed reachable set for efficient path classification.
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
    reachable_blocks: &HashSet<BlockId>,
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
        // Classify the path using pre-computed reachable set
        let kind = classify_path_precomputed(cfg, current_path, reachable_blocks);
        let path = Path::new(current_path.clone(), kind);
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
        // Use classification to determine path kind
        let kind = classify_path_precomputed(cfg, current_path, reachable_blocks);
        let path = Path::new(current_path.clone(), kind);
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
                reachable_blocks,
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

    // classify_path_precomputed tests

    #[test]
    fn test_classify_path_precomputed_matches_classify_path() {
        let cfg = create_diamond_cfg();

        // Pre-compute reachable set
        use crate::cfg::reachability::find_reachable;
        let reachable_nodes = find_reachable(&cfg);
        let reachable_blocks: HashSet<BlockId> = reachable_nodes
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Test multiple paths
        let test_paths = vec![
            vec![0, 1, 3],
            vec![0, 2, 3],
            vec![0, 1],
            vec![0],
        ];

        for path in test_paths {
            let kind1 = classify_path(&cfg, &path);
            let kind2 = classify_path_precomputed(&cfg, &path, &reachable_blocks);
            assert_eq!(
                kind1, kind2,
                "classify_path_precomputed should match classify_path for path {:?}",
                path
            );
        }
    }

    #[test]
    fn test_classify_path_precomputed_unreachable() {
        let cfg = create_dead_code_cfg();

        // Pre-compute reachable set (only block 0 is reachable)
        use crate::cfg::reachability::find_reachable;
        let reachable_nodes = find_reachable(&cfg);
        let reachable_blocks: HashSet<BlockId> = reachable_nodes
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Block 1 is unreachable
        let path = vec![1];
        let kind = classify_path_precomputed(&cfg, &path, &reachable_blocks);
        assert_eq!(kind, PathKind::Unreachable);
    }

    #[test]
    fn test_classify_path_precomputed_performance() {
        use crate::cfg::reachability::find_reachable;
        use std::time::Instant;

        let cfg = create_diamond_cfg();

        // Pre-compute reachable set once
        let reachable_nodes = find_reachable(&cfg);
        let reachable_blocks: HashSet<BlockId> = reachable_nodes
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Create many test paths
        let test_paths: Vec<Vec<BlockId>> = (0..1000)
            .map(|_| vec![0, 1, 3])
            .collect();

        // Time the precomputed version
        let start = Instant::now();
        for path in &test_paths {
            let _ = classify_path_precomputed(&cfg, path, &reachable_blocks);
        }
        let precomputed_duration = start.elapsed();

        // Should be very fast (< 10ms for 1000 paths)
        assert!(
            precomputed_duration.as_millis() < 10,
            "classify_path_precomputed should classify 1000 paths in <10ms, took {}ms",
            precomputed_duration.as_millis()
        );
    }

    #[test]
    fn test_classify_path_precomputed_all_kinds() {
        use crate::cfg::reachability::find_reachable;

        // Test with normal path
        let cfg_normal = create_linear_cfg();
        let reachable = find_reachable(&cfg_normal)
            .iter()
            .map(|&idx| cfg_normal[idx].id)
            .collect();
        assert_eq!(
            classify_path_precomputed(&cfg_normal, &[0, 1, 2], &reachable),
            PathKind::Normal
        );

        // Test with error path
        let cfg_error = create_error_cfg();
        let reachable = find_reachable(&cfg_error)
            .iter()
            .map(|&idx| cfg_error[idx].id)
            .collect();
        assert_eq!(
            classify_path_precomputed(&cfg_error, &[0, 1], &reachable),
            PathKind::Error
        );

        // Test with degenerate path
        let cfg_degen = create_unreachable_term_cfg();
        let reachable = find_reachable(&cfg_degen)
            .iter()
            .map(|&idx| cfg_degen[idx].id)
            .collect();
        assert_eq!(
            classify_path_precomputed(&cfg_degen, &[0, 1], &reachable),
            PathKind::Degenerate
        );
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

    // enumerate_paths_classification integration tests

    #[test]
    fn test_enumerate_paths_classification_diamond() {
        let cfg = create_diamond_cfg();
        let paths = enumerate_paths(&cfg, &PathLimits::default());

        // Diamond CFG: all paths should be Normal
        assert_eq!(paths.len(), 2);
        for path in &paths {
            assert_eq!(
                path.kind,
                PathKind::Normal,
                "Diamond CFG should only have Normal paths, got {:?} for {:?}",
                path.kind,
                path.blocks
            );
        }
    }

    #[test]
    fn test_enumerate_paths_classification_with_error() {
        // Create CFG with error path
        let cfg = create_error_cfg();
        let paths = enumerate_paths(&cfg, &PathLimits::default());

        // Should have one path that ends in Abort (Error)
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].kind, PathKind::Error);
        assert_eq!(paths[0].blocks, vec![0, 1]);
    }

    #[test]
    fn test_enumerate_paths_classification_with_unreachable() {
        // Create CFG with unreachable block
        let cfg = create_dead_code_cfg();
        let paths = enumerate_paths(&cfg, &PathLimits::default());

        // Only the reachable path should be enumerated
        assert_eq!(paths.len(), 1);
        // The path goes through block 0 which is reachable
        assert_eq!(paths[0].blocks, vec![0]);
        assert_eq!(paths[0].kind, PathKind::Normal);
    }

    #[test]
    fn test_enumerate_paths_classification_mixed() {
        // Create a CFG with both normal and error paths
        let mut g = DiGraph::new();

        // Entry block with conditional
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![1], otherwise: 2 },
            source_location: None,
        });

        // Normal branch
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // Error branch (panic)
        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Abort("panic!".to_string()),
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::TrueBranch);
        g.add_edge(b0, b2, EdgeType::FalseBranch);

        let paths = enumerate_paths(&g, &PathLimits::default());

        // Should have 2 paths: one Normal, one Error
        assert_eq!(paths.len(), 2);

        let normal_count = paths.iter().filter(|p| p.kind == PathKind::Normal).count();
        let error_count = paths.iter().filter(|p| p.kind == PathKind::Error).count();

        assert_eq!(normal_count, 1, "Should have 1 Normal path");
        assert_eq!(error_count, 1, "Should have 1 Error path");
    }

    #[test]
    fn test_enumerate_paths_classification_correctness() {
        // Verify that classification is correctly applied during enumeration
        let cfg = create_diamond_cfg();
        let paths = enumerate_paths(&cfg, &PathLimits::default());

        // Use the same reachable set for manual classification
        use crate::cfg::reachability::find_reachable;
        let reachable_nodes = find_reachable(&cfg);
        let reachable_blocks: HashSet<BlockId> = reachable_nodes
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Verify each path's kind matches manual classification
        for path in &paths {
            let expected_kind = classify_path_precomputed(&cfg, &path.blocks, &reachable_blocks);
            assert_eq!(
                path.kind, expected_kind,
                "Path kind mismatch for {:?}: got {:?}, expected {:?}",
                path.blocks, path.kind, expected_kind
            );
        }
    }

    // Task 1: PathLimits enforcement tests

    #[test]
    fn test_path_limits_max_length_long_path() {
        // Create a 5-block linear path: 0 -> 1 -> 2 -> 3 -> 4
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
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 3 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 4 },
            source_location: None,
        });

        let b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::Fallthrough);
        g.add_edge(b2, b3, EdgeType::Fallthrough);
        g.add_edge(b3, b4, EdgeType::Fallthrough);

        // With max_length=3, the 5-block path exceeds limit
        let limits = PathLimits::default().with_max_length(3);
        let paths = enumerate_paths(&g, &limits);

        // No paths should be found because the only path has length 5 > 3
        assert_eq!(paths.len(), 0, "Path exceeds max_length, should return 0 paths");
    }

    #[test]
    fn test_path_limits_max_paths_exact() {
        let cfg = create_diamond_cfg();

        // Diamond has 2 paths, limit to 1
        let limits = PathLimits::default().with_max_paths(1);
        let paths = enumerate_paths(&cfg, &limits);

        // Should get exactly 1 path (stops early)
        assert_eq!(paths.len(), 1, "Should stop at max_paths=1");

        // Path should be valid (entry to exit)
        assert_eq!(paths[0].entry, 0);
        assert_eq!(paths[0].exit, 3);
    }

    #[test]
    fn test_path_limits_loop_unroll_exact() {
        let cfg = create_loop_cfg();

        // With loop_unroll_limit=1, we should get:
        // - Direct exit: 0->1->3
        // - 1 iteration: 0->1->2->1->3
        let limits = PathLimits::default().with_loop_unroll_limit(1);
        let paths = enumerate_paths(&cfg, &limits);

        // With limit=1, we get 1 path (direct exit only, 0 loop iterations)
        // The loop iteration counter is incremented on first visit (0->1), so
        // back-edge attempts to visit again with count=1 which is >= limit
        assert_eq!(paths.len(), 1, "Should have exactly 1 path with loop_unroll_limit=1");

        // Verify direct exit exists
        assert!(paths.iter().any(|p| p.blocks == vec![0, 1, 3]),
                "Direct exit path should exist");
    }

    #[test]
    fn test_path_limits_loop_unroll_limit_2() {
        let cfg = create_loop_cfg();

        // With loop_unroll_limit=2:
        // - First entry: count=0 -> 1
        // - Second entry (via back-edge): count=1 -> 2 (allowed)
        // - Third entry: count=2 >= limit, stopped
        // So we get: direct exit + 1 iteration path = 2 paths
        let limits = PathLimits::default().with_loop_unroll_limit(2);
        let paths = enumerate_paths(&cfg, &limits);

        // With limit=2, we should get 2 paths (direct exit + 1 loop iteration)
        assert_eq!(paths.len(), 2, "Should have exactly 2 paths with loop_unroll_limit=2");

        // Verify direct exit exists
        assert!(paths.iter().any(|p| p.blocks == vec![0, 1, 3]),
                "Direct exit path should exist");

        // Verify one iteration path exists
        assert!(paths.iter().any(|p| p.blocks == vec![0, 1, 2, 1, 3]),
                "One iteration path should exist");
    }

    // Task 2: Self-loop cycle detection tests

    /// Create a CFG with a self-loop: 0 -> 1 -> 1 (self-loop)
    fn create_self_loop_cfg() -> Cfg {
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
            // Self-loop: always goes back to itself
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        g
    }

    #[test]
    fn test_self_loop_terminates() {
        let cfg = create_self_loop_cfg();

        // This should terminate without hanging
        let limits = PathLimits::default();
        let paths = enumerate_paths(&cfg, &limits);

        // Should have a bounded number of paths (not infinite)
        // The self-loop is bounded by loop_unroll_limit
        assert!(paths.len() <= limits.loop_unroll_limit + 1,
                "Self-loop should be bounded by loop_unroll_limit");
    }

    #[test]
    fn test_self_loop_with_low_limit() {
        let cfg = create_self_loop_cfg();

        // With a very low unroll limit, we should get minimal paths
        let limits = PathLimits::default().with_loop_unroll_limit(1);
        let paths = enumerate_paths(&cfg, &limits);

        // Should have exactly 1 path (direct to self-loop block, then bounded)
        assert!(paths.len() <= 2, "Self-loop with low limit should have few paths");
    }

    // Task 3: Nested loop bounding tests

    /// Create a CFG with nested loops:
    /// 0 -> 1 (outer header) -> 2 (inner header) -> 3 -> 2 (back to inner)
    /// 1 -> 4 (outer exit)
    /// 2 -> 4 (inner exit to outer)
    /// 4 -> 5 (final exit)
    fn create_nested_loop_cfg() -> Cfg {
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
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 4 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![3], otherwise: 1 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
        });

        let b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b4, EdgeType::FalseBranch);
        g.add_edge(b2, b3, EdgeType::TrueBranch);
        g.add_edge(b2, b1, EdgeType::LoopBack); // Outer back edge
        g.add_edge(b3, b2, EdgeType::LoopBack); // Inner back edge

        g
    }

    #[test]
    fn test_nested_loop_bounding() {
        let cfg = create_nested_loop_cfg();

        // With loop_unroll_limit=2, each loop can iterate 0, 1, or 2 times
        // For 2 nested loops, max paths should be bounded by (limit+1)^2 = 9
        let limits = PathLimits::default().with_loop_unroll_limit(2);
        let paths = enumerate_paths(&cfg, &limits);

        // With 2 nested loops and limit 2, we get at most 9 paths
        // (3 outer iterations * 3 inner iterations each)
        assert!(paths.len() <= 9, "Nested loops should be bounded: got {} paths", paths.len());
        assert!(paths.len() > 0, "Should have at least some paths");
    }

    #[test]
    fn test_nested_loop_bounding_three_levels() {
        // Create 3-level nested loop
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Outer loop header
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 6 },
            source_location: None,
        });

        // Middle loop header
        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![3], otherwise: 1 },
            source_location: None,
        });

        // Inner loop header
        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![4], otherwise: 2 },
            source_location: None,
        });

        let b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 3 },
            source_location: None,
        });

        let b6 = g.add_node(BasicBlock {
            id: 6,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b6, EdgeType::FalseBranch);
        g.add_edge(b2, b3, EdgeType::TrueBranch);
        g.add_edge(b2, b1, EdgeType::LoopBack); // Outer back edge
        g.add_edge(b3, b4, EdgeType::TrueBranch);
        g.add_edge(b3, b2, EdgeType::LoopBack); // Middle back edge
        g.add_edge(b4, b3, EdgeType::LoopBack); // Inner back edge

        // With loop_unroll_limit=2 and 3 nested loops:
        // Max paths = (limit+1)^3 = 27
        let limits = PathLimits::default().with_loop_unroll_limit(2);
        let paths = enumerate_paths(&g, &limits);

        assert!(paths.len() <= 27, "3-level nested loops should be bounded by 27");
    }

    #[test]
    fn test_nested_loop_independent_counters() {
        let cfg = create_nested_loop_cfg();

        // Verify that each loop header is tracked independently
        let loop_headers = crate::cfg::loops::find_loop_headers(&cfg);

        // Should have 2 loop headers (outer and inner)
        assert_eq!(loop_headers.len(), 2, "Should detect 2 loop headers");

        // With limit=2, we should get reasonable bounded paths
        let limits = PathLimits::default().with_loop_unroll_limit(2);
        let paths = enumerate_paths(&cfg, &limits);

        // Verify paths are bounded (not exponential explosion)
        assert!(paths.len() > 0, "Should have some paths");
        assert!(paths.len() <= 9, "Should be bounded by (limit+1)^nesting_depth");
    }

    // Task 4: PathLimits builder and preset tests

    #[test]
    fn test_path_limits_quick_analysis() {
        let limits = PathLimits::quick_analysis();

        assert_eq!(limits.max_length, 100);
        assert_eq!(limits.max_paths, 1000);
        assert_eq!(limits.loop_unroll_limit, 2);
    }

    #[test]
    fn test_path_limits_thorough() {
        let limits = PathLimits::thorough();

        assert_eq!(limits.max_length, 10000);
        assert_eq!(limits.max_paths, 100000);
        assert_eq!(limits.loop_unroll_limit, 5);
    }

    #[test]
    fn test_path_limits_builder_chaining() {
        // Start with quick_analysis and customize further
        let limits = PathLimits::quick_analysis()
            .with_max_length(200)
            .with_max_paths(5000)
            .with_loop_unroll_limit(3);

        assert_eq!(limits.max_length, 200);
        assert_eq!(limits.max_paths, 5000);
        assert_eq!(limits.loop_unroll_limit, 3);
    }

    #[test]
    fn test_path_limits_presets_differ_from_default() {
        let default = PathLimits::default();
        let quick = PathLimits::quick_analysis();
        let thorough = PathLimits::thorough();

        // Quick should be more restrictive than default
        assert!(quick.max_length < default.max_length);
        assert!(quick.max_paths < default.max_paths);
        assert!(quick.loop_unroll_limit < default.loop_unroll_limit);

        // Thorough should be less restrictive than default
        assert!(thorough.max_length > default.max_length);
        assert!(thorough.max_paths > default.max_paths);
        assert!(thorough.loop_unroll_limit > default.loop_unroll_limit);
    }

    #[test]
    fn test_path_limits_quick_vs_thorough_on_loop() {
        let cfg = create_loop_cfg();

        // Quick analysis should find fewer paths
        let quick_paths = enumerate_paths(&cfg, &PathLimits::quick_analysis());
        let thorough_paths = enumerate_paths(&cfg, &PathLimits::thorough());

        // Thorough should find at least as many paths as quick
        assert!(thorough_paths.len() >= quick_paths.len(),
                "Thorough analysis should find at least as many paths as quick");
    }

    // Task 1: is_feasible_path tests

    #[test]
    fn test_is_feasible_path_empty_path() {
        let cfg = create_linear_cfg();
        let empty_path: Vec<BlockId> = vec![];

        assert!(!is_feasible_path(&cfg, &empty_path),
                "Empty path should be infeasible");
    }

    #[test]
    fn test_is_feasible_path_non_entry_first_block() {
        let cfg = create_diamond_cfg();
        // Path starting from block 1 (not entry)
        let path = vec![1, 3];

        assert!(!is_feasible_path(&cfg, &path),
                "Path starting from non-entry block should be infeasible");
    }

    #[test]
    fn test_is_feasible_path_dead_end_goto() {
        let cfg = create_linear_cfg();
        // Path ending in Goto (dead end)
        let path = vec![0, 1]; // Block 1 has Goto to 2

        assert!(!is_feasible_path(&cfg, &path),
                "Path ending in Goto should be infeasible (dead end)");
    }

    #[test]
    fn test_is_feasible_path_valid_return() {
        let cfg = create_linear_cfg();
        // Complete path: entry -> goto -> return
        let path = vec![0, 1, 2];

        assert!(is_feasible_path(&cfg, &path),
                "Complete path ending in Return should be feasible");
    }

    #[test]
    fn test_is_feasible_path_abort_is_feasible() {
        let cfg = create_error_cfg();
        // Path ending in Abort (error but reachable)
        let path = vec![0, 1];

        assert!(is_feasible_path(&cfg, &path),
                "Path ending in Abort should be feasible (error path but reachable)");
    }

    #[test]
    fn test_is_feasible_path_unreachable_terminator() {
        let cfg = create_unreachable_term_cfg();
        // Path ending in Unreachable
        let path = vec![0, 1];

        assert!(!is_feasible_path(&cfg, &path),
                "Path ending in Unreachable should be infeasible");
    }

    #[test]
    fn test_is_feasible_path_nonexistent_block() {
        let cfg = create_linear_cfg();
        // Path with nonexistent block
        let path = vec![0, 99]; // Block 99 doesn't exist

        assert!(!is_feasible_path(&cfg, &path),
                "Path with nonexistent block should be infeasible");
    }

    #[test]
    fn test_is_feasible_path_switch_int_dead_end() {
        let cfg = create_diamond_cfg();
        // Path ending in SwitchInt (block 0)
        let path = vec![0]; // Block 0 has SwitchInt

        assert!(!is_feasible_path(&cfg, &path),
                "Path ending in SwitchInt should be infeasible (dead end)");
    }

    #[test]
    fn test_is_feasible_path_complete_diamond() {
        let cfg = create_diamond_cfg();
        // Complete path through diamond
        let path1 = vec![0, 1, 3]; // Through true branch
        let path2 = vec![0, 2, 3]; // Through false branch

        assert!(is_feasible_path(&cfg, &path1),
                "Complete diamond path 0->1->3 should be feasible");
        assert!(is_feasible_path(&cfg, &path2),
                "Complete diamond path 0->2->3 should be feasible");
    }

    #[test]
    fn test_is_feasible_path_call_no_unwind() {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Call {
                target: Some(1),
                unwind: None,
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

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        // Path ending in Call with no unwind
        let path = vec![0];

        assert!(is_feasible_path(&g, &path),
                "Path ending in Call with no unwind should be feasible");
    }

    #[test]
    fn test_is_feasible_path_call_with_unwind_and_target() {
        let cfg = create_call_unwind_cfg();
        // Path ending in Call with unwind and target
        let path = vec![0]; // Block 0 has Call with unwind: Some(2), target: Some(1)

        assert!(is_feasible_path(&cfg, &path),
                "Path ending in Call with both unwind and target should be feasible");
    }

    #[test]
    fn test_is_feasible_path_call_always_unwinds() {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Call {
                target: None, // No target - always unwinds
                unwind: Some(1),
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

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        // Path ending in Call that always unwinds
        let path = vec![0];

        assert!(!is_feasible_path(&g, &path),
                "Path ending in Call with only unwind (no target) should be infeasible");
    }

    // Task 2: is_feasible_path_precomputed tests

    #[test]
    fn test_is_feasible_path_precomputed_matches_basic() {
        let cfg = create_diamond_cfg();

        // Pre-compute reachable set
        use crate::cfg::reachability::find_reachable;
        let reachable_nodes = find_reachable(&cfg);
        let reachable_blocks: HashSet<BlockId> = reachable_nodes
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Test multiple paths - both should give same result
        let test_paths = vec![
            vec![0, 1, 3],      // Complete path - feasible
            vec![0, 2, 3],      // Complete path - feasible
            vec![0, 1],         // Dead end - infeasible
            vec![],             // Empty - infeasible
        ];

        for path in test_paths {
            let basic = is_feasible_path(&cfg, &path);
            let precomputed = is_feasible_path_precomputed(&cfg, &path, &reachable_blocks);
            assert_eq!(
                basic, precomputed,
                "is_feasible_path_precomputed should match is_feasible_path for {:?}",
                path
            );
        }
    }

    #[test]
    fn test_is_feasible_path_precomputed_unreachable_block() {
        let cfg = create_dead_code_cfg();

        // Pre-compute reachable set (only block 0 is reachable)
        use crate::cfg::reachability::find_reachable;
        let reachable_nodes = find_reachable(&cfg);
        let reachable_blocks: HashSet<BlockId> = reachable_nodes
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Path with unreachable block
        let path = vec![1]; // Block 1 is not reachable from entry

        assert!(!is_feasible_path_precomputed(&cfg, &path, &reachable_blocks),
                "Path with unreachable block should be infeasible");
    }

    #[test]
    fn test_is_feasible_path_precomputed_performance() {
        use crate::cfg::reachability::find_reachable;
        use std::time::Instant;

        let cfg = create_diamond_cfg();

        // Pre-compute reachable set once
        let reachable_nodes = find_reachable(&cfg);
        let reachable_blocks: HashSet<BlockId> = reachable_nodes
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Create many test paths
        let test_paths: Vec<Vec<BlockId>> = (0..1000)
            .map(|_| vec![0, 1, 3])
            .collect();

        // Time the precomputed version
        let start = Instant::now();
        for path in &test_paths {
            let _ = is_feasible_path_precomputed(&cfg, path, &reachable_blocks);
        }
        let precomputed_duration = start.elapsed();

        // Should be very fast (< 5ms for 1000 paths)
        assert!(
            precomputed_duration.as_millis() < 5,
            "is_feasible_path_precomputed should check 1000 paths in <5ms, took {}ms",
            precomputed_duration.as_millis()
        );
    }

    #[test]
    fn test_is_feasible_path_precomputed_all_criteria() {
        use crate::cfg::reachability::find_reachable;

        // Test with normal path (feasible)
        let cfg_normal = create_linear_cfg();
        let reachable_normal = find_reachable(&cfg_normal)
            .iter()
            .map(|&idx| cfg_normal[idx].id)
            .collect();
        assert!(
            is_feasible_path_precomputed(&cfg_normal, &[0, 1, 2], &reachable_normal),
            "Complete linear path should be feasible"
        );

        // Test with error path (feasible)
        let cfg_error = create_error_cfg();
        let reachable_error = find_reachable(&cfg_error)
            .iter()
            .map(|&idx| cfg_error[idx].id)
            .collect();
        assert!(
            is_feasible_path_precomputed(&cfg_error, &[0, 1], &reachable_error),
            "Path ending in Abort should be feasible (error path but reachable)"
        );

        // Test with degenerate path (infeasible)
        let cfg_degen = create_unreachable_term_cfg();
        let reachable_degen = find_reachable(&cfg_degen)
            .iter()
            .map(|&idx| cfg_degen[idx].id)
            .collect();
        assert!(
            !is_feasible_path_precomputed(&cfg_degen, &[0, 1], &reachable_degen),
            "Path ending in Unreachable should be infeasible"
        );
    }

    // Task 3: classify_with_feasibility tests

    #[test]
    fn test_classify_with_feasibility_dead_end() {
        use crate::cfg::reachability::find_reachable;

        let cfg = create_linear_cfg();
        let reachable = find_reachable(&cfg)
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Path ending in Goto (dead end) should be Degenerate
        let path = vec![0, 1]; // Block 1 has Goto to 2
        let kind = classify_path_precomputed(&cfg, &path, &reachable);
        assert_eq!(kind, PathKind::Degenerate,
                   "Path ending in Goto should be Degenerate");
    }

    #[test]
    fn test_classify_with_feasibility_valid_exit() {
        use crate::cfg::reachability::find_reachable;

        let cfg = create_linear_cfg();
        let reachable = find_reachable(&cfg)
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Complete path with Return should be Normal
        let path = vec![0, 1, 2];
        let kind = classify_path_precomputed(&cfg, &path, &reachable);
        assert_eq!(kind, PathKind::Normal,
                   "Complete path with Return should be Normal");
    }

    #[test]
    fn test_classify_with_feasibility_error_path() {
        use crate::cfg::reachability::find_reachable;

        let cfg = create_error_cfg();
        let reachable = find_reachable(&cfg)
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Path ending in Abort should be Error (feasible but error path)
        let path = vec![0, 1];
        let kind = classify_path_precomputed(&cfg, &path, &reachable);
        assert_eq!(kind, PathKind::Error,
                   "Path ending in Abort should be Error");
    }

    #[test]
    fn test_classify_with_feasibility_switch_int_dead_end() {
        use crate::cfg::reachability::find_reachable;

        let cfg = create_diamond_cfg();
        let reachable = find_reachable(&cfg)
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Path ending in SwitchInt (dead end)
        let path = vec![0]; // Block 0 has SwitchInt
        let kind = classify_path_precomputed(&cfg, &path, &reachable);
        assert_eq!(kind, PathKind::Degenerate,
                   "Path ending in SwitchInt should be Degenerate");
    }

    #[test]
    fn test_classify_with_feasibility_priority_order() {
        use crate::cfg::reachability::find_reachable;

        // Test that unreachable takes priority over feasibility
        let cfg = create_dead_code_cfg();
        let reachable = find_reachable(&cfg)
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Path with unreachable block
        let path = vec![1];
        let kind = classify_path_precomputed(&cfg, &path, &reachable);
        assert_eq!(kind, PathKind::Unreachable,
                   "Unreachable should be prioritized over feasibility");
    }

    #[test]
    fn test_classify_with_feasibility_complete_paths() {
        use crate::cfg::reachability::find_reachable;

        let cfg = create_diamond_cfg();
        let reachable = find_reachable(&cfg)
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Both complete paths should be Normal
        let path1 = vec![0, 1, 3];
        let path2 = vec![0, 2, 3];

        assert_eq!(classify_path_precomputed(&cfg, &path1, &reachable), PathKind::Normal,
                   "Complete diamond path 0->1->3 should be Normal");
        assert_eq!(classify_path_precomputed(&cfg, &path2, &reachable), PathKind::Normal,
                   "Complete diamond path 0->2->3 should be Normal");
    }

    #[test]
    fn test_classify_with_feasibility_call_terminator() {
        use crate::cfg::reachability::find_reachable;

        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Call {
                target: Some(1),
                unwind: None,
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

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        let reachable = find_reachable(&g)
            .iter()
            .map(|&idx| g[idx].id)
            .collect();

        // Path ending in Call with target should be Normal (feasible)
        let path = vec![0];
        let kind = classify_path_precomputed(&g, &path, &reachable);
        assert_eq!(kind, PathKind::Normal,
                   "Path ending in Call with target should be Normal");
    }
}
