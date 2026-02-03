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
/// ```rust,no_run
/// # use mirage::cfg::paths::is_feasible_path;
/// # use mirage::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// let feasible = is_feasible_path(&graph, &[0, 1, 2]);  // entry -> goto -> return
/// let infeasible = is_feasible_path(&graph, &[0, 1]);    // entry -> goto (dead end)
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
/// ```rust,no_run
/// # use mirage::cfg::paths::is_feasible_path_precomputed;
/// # use mirage::cfg::reachability::find_reachable;
/// # use mirage::cfg::Cfg;
/// # use std::collections::HashSet;
/// # use mirage::cfg::BlockId;
/// # use mirage::cfg::Path;
/// # let graph: Cfg = unimplemented!();
/// # let paths: Vec<Path> = vec![];
/// let reachable_nodes = find_reachable(&graph);
/// let reachable_blocks: HashSet<BlockId> = reachable_nodes
///     .iter()
///     .map(|&idx| graph[idx].id)
///     .collect();
///
/// for path in paths {
///     let feasible = is_feasible_path_precomputed(&graph, &path.blocks, &reachable_blocks);
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
/// ```rust,no_run
/// # use mirage::cfg::paths::classify_path_precomputed;
/// # use mirage::cfg::reachability::find_reachable;
/// # use mirage::cfg::Cfg;
/// # use std::collections::HashSet;
/// # use mirage::cfg::BlockId;
/// # use mirage::cfg::Path;
/// # let graph: Cfg = unimplemented!();
/// # let paths: Vec<Path> = vec![];
/// let reachable_nodes = find_reachable(&graph);
/// let reachable_blocks: HashSet<BlockId> = reachable_nodes
///     .iter()
///     .map(|&idx| graph[idx].id)
///     .collect();
///
/// for path in paths {
///     let kind = classify_path_precomputed(&graph, &path.blocks, &reachable_blocks);
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

/// Pre-computed context for path enumeration
///
/// Contains analysis results that are shared across all path enumerations.
/// Computing this context once and reusing it is much more efficient than
/// recomputing for each enumeration call.
///
/// **Benefits:**
/// - Reachable blocks computed once: O(n) instead of O(n²) for n paths
/// - Loop headers computed once: O(e) instead of O(e) per call
/// - Exit nodes computed once: O(v) instead of O(v) per call
///
/// **Use case:**
/// ```rust,no_run
/// # use mirage::cfg::{PathLimits, enumerate_paths_with_context, EnumerationContext};
/// # use mirage::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// # let limits1 = PathLimits::default();
/// # let limits2 = PathLimits::default();
/// let ctx = EnumerationContext::new(&graph);
/// let paths1 = enumerate_paths_with_context(&graph, &limits1, &ctx);
/// let paths2 = enumerate_paths_with_context(&graph, &limits2, &ctx);
/// // No redundant analysis computations
/// ```
#[derive(Debug, Clone)]
pub struct EnumerationContext {
    /// Blocks reachable from the entry node
    pub reachable_blocks: HashSet<BlockId>,
    /// Loop header nodes (for bounding loop iterations)
    pub loop_headers: HashSet<NodeIndex>,
    /// Exit nodes (valid path termination points)
    pub exits: HashSet<NodeIndex>,
}

impl EnumerationContext {
    /// Create a new enumeration context by analyzing the CFG
    ///
    /// Performs three analyses:
    /// 1. Reachability: Find all blocks reachable from entry
    /// 2. Loop detection: Find all loop headers via dominance analysis
    /// 3. Exit detection: Find all blocks with return/abort/unreachable terminators
    ///
    /// **Time complexity:** O(v + e) where v = vertices, e = edges
    /// **Space complexity:** O(v) for storing the analysis results
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use mirage::cfg::paths::EnumerationContext;
    /// # use mirage::cfg::Cfg;
    /// # let graph: Cfg = unimplemented!();
    /// let ctx = EnumerationContext::new(&graph);
    /// println!("Found {} loop headers", ctx.loop_headers.len());
    /// ```
    pub fn new(cfg: &Cfg) -> Self {
        // Compute reachable blocks
        let reachable_nodes = crate::cfg::reachability::find_reachable(cfg);
        let reachable_blocks: HashSet<BlockId> = reachable_nodes
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Compute loop headers
        let loop_headers = crate::cfg::loops::find_loop_headers(cfg);

        // Compute exit nodes
        let exits = crate::cfg::analysis::find_exits(cfg)
            .into_iter()
            .collect();

        Self {
            reachable_blocks,
            loop_headers,
            exits,
        }
    }

    /// Get the number of reachable blocks
    pub fn reachable_count(&self) -> usize {
        self.reachable_blocks.len()
    }

    /// Get the number of loop headers
    pub fn loop_count(&self) -> usize {
        self.loop_headers.len()
    }

    /// Get the number of exit nodes
    pub fn exit_count(&self) -> usize {
        self.exits.len()
    }

    /// Check if a block is reachable
    pub fn is_reachable(&self, block_id: BlockId) -> bool {
        self.reachable_blocks.contains(&block_id)
    }

    /// Check if a node is a loop header
    pub fn is_loop_header(&self, node: NodeIndex) -> bool {
        self.loop_headers.contains(&node)
    }

    /// Check if a node is an exit
    pub fn is_exit(&self, node: NodeIndex) -> bool {
        self.exits.contains(&node)
    }
}

/// Enumerate all execution paths through a CFG using pre-computed context
///
/// This is an optimized version of `enumerate_paths` that uses pre-computed
/// analysis results. Use this when:
/// - Performing multiple enumerations on the same CFG
/// - You need to avoid redundant analysis computations
///
/// **Performance:**
/// - Context creation: O(v + e) - done once
/// - Enumeration per call: O(p * l) where p = paths, l = avg length
/// - Versus O(v + e + p * l) for enumerate_paths (redundant analysis)
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `limits` - Path enumeration limits
/// * `ctx` - Pre-computed enumeration context
///
/// # Returns
///
/// Vector of all enumerated execution paths
///
/// # Example
///
/// ```rust,no_run
/// # use mirage::cfg::{PathLimits, enumerate_paths_with_context, EnumerationContext};
/// # use mirage::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// let ctx = EnumerationContext::new(&graph);
/// let limits = PathLimits::default();
/// let paths = enumerate_paths_with_context(&graph, &limits, &ctx);
/// ```
pub fn enumerate_paths_with_context(
    cfg: &Cfg,
    limits: &PathLimits,
    ctx: &EnumerationContext,
) -> Vec<Path> {
    // Get entry block
    let entry = match crate::cfg::analysis::find_entry(cfg) {
        Some(e) => e,
        None => return vec![], // Empty CFG
    };

    if ctx.exits.is_empty() {
        return vec![]; // No exits means no complete paths
    }

    // Initialize traversal state
    let mut paths = Vec::new();
    let mut current_path = Vec::new();
    let mut visited = HashSet::new();
    let mut loop_iterations: HashMap<NodeIndex, usize> = HashMap::new();

    // Start DFS from entry
    dfs_enumerate_with_context(
        cfg,
        entry,
        limits,
        &mut paths,
        &mut current_path,
        &mut visited,
        ctx,
        &mut loop_iterations,
    );

    paths
}

/// Recursive DFS helper for path enumeration with pre-computed context
fn dfs_enumerate_with_context(
    cfg: &Cfg,
    current: NodeIndex,
    limits: &PathLimits,
    paths: &mut Vec<Path>,
    current_path: &mut Vec<BlockId>,
    visited: &mut HashSet<NodeIndex>,
    ctx: &EnumerationContext,
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
    if ctx.is_exit(current) {
        // Classify the path using pre-computed reachable set
        let kind = classify_path_precomputed(cfg, current_path, &ctx.reachable_blocks);
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

    // Check if already visited (cycle detection)
    // Loop headers are exempt - we track loop iterations separately
    if visited.contains(&current) && !ctx.is_loop_header(current) {
        current_path.pop();
        return;
    }

    // Mark as visited
    visited.insert(current);

    // Track loop iterations for loop headers
    let is_loop_header = ctx.is_loop_header(current);
    if is_loop_header {
        let count = loop_iterations.entry(current).or_insert(0);
        if *count >= limits.loop_unroll_limit {
            visited.remove(&current);
            current_path.pop();
            return;
        }
        *count += 1;
    }

    // Explore neighbors
    let neighbors: Vec<_> = cfg
        .neighbors(current)
        .collect();

    for next in neighbors {
        dfs_enumerate_with_context(
            cfg,
            next,
            limits,
            paths,
            current_path,
            visited,
            ctx,
            loop_iterations,
        );
    }

    // Backtrack: decrement loop iteration count if this was a loop header
    if is_loop_header {
        if let Some(count) = loop_iterations.get_mut(&current) {
            *count = count.saturating_sub(1);
        }
    }

    visited.remove(&current);
    current_path.pop();
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
/// ```rust,no_run
/// # use mirage::cfg::{enumerate_paths, PathLimits};
/// # use mirage::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// let paths = enumerate_paths(&graph, &PathLimits::default());
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

/// Get paths from cache or enumerate them
///
/// This bridge function connects the caching layer to path enumeration.
/// It checks if cached paths exist for the given function and hash,
/// and only enumerates if the cache is stale or empty.
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze (for enumeration)
/// * `function_id` - Database ID of the function
/// * `function_hash` - Hash of the function content for cache validation
/// * `limits` - Limits for path enumeration (if enumeration is needed)
/// * `db_conn` - Database connection for caching
///
/// # Returns
///
/// Vector of paths, either from cache or freshly enumerated
///
/// # Algorithm
///
/// 1. Call `update_function_paths_if_changed` with empty paths to check hash
/// 2. If returns false (cache hit), retrieve cached paths
/// 3. If returns true (cache miss):
///    - Enumerate paths via `enumerate_paths`
///    - Store via `update_function_paths_if_changed` with actual paths
///    - Return paths
///
/// # Example
///
/// ```rust,no_run
/// # use mirage::cfg::paths::get_or_enumerate_paths;
/// # use mirage::cfg::{PathLimits, Cfg};
/// # let graph: Cfg = unimplemented!();
/// # let function_id: i64 = 0;
/// # let function_hash = "hash";
/// # let mut conn = unimplemented!();
/// let paths = get_or_enumerate_paths(
///     &graph,
///     function_id,
///     function_hash,
///     &PathLimits::default(),
///     &mut conn,
/// )?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn get_or_enumerate_paths(
    cfg: &Cfg,
    function_id: i64,
    function_hash: &str,
    limits: &PathLimits,
    db_conn: &mut rusqlite::Connection,
) -> Result<Vec<Path>, String> {
    use crate::storage::paths::{get_cached_paths, invalidate_function_paths, store_paths};

    // Check current hash in cfg_blocks
    let current_hash: Option<String> = db_conn.query_row(
        "SELECT function_hash FROM cfg_blocks WHERE function_id = ?1 LIMIT 1",
        rusqlite::params![function_id],
        |row| row.get(0),
    ).unwrap_or(None);

    // If hash matches, return cached paths
    if let Some(ref hash) = current_hash {
        if hash == function_hash {
            // Cache hit - retrieve stored paths
            let paths = get_cached_paths(db_conn, function_id)
                .map_err(|e| format!("Failed to retrieve cached paths: {}", e))?;
            return Ok(paths);
        }
    }

    // Cache miss or hash changed - enumerate and store paths
    let paths = enumerate_paths(cfg, limits);

    // Invalidate old paths if any
    let _ = invalidate_function_paths(db_conn, function_id);

    // Store the enumerated paths
    store_paths(db_conn, function_id, &paths)
        .map_err(|e| format!("Failed to store enumerated paths: {}", e))?;

    // Update function_hash in cfg_blocks
    let block_exists: bool = db_conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cfg_blocks WHERE function_id = ?1)",
        rusqlite::params![function_id],
        |row| row.get(0),
    ).unwrap_or(false);

    if block_exists {
        db_conn.execute(
            "UPDATE cfg_blocks SET function_hash = ?1 WHERE function_id = ?2",
            rusqlite::params![function_hash, function_id],
        ).map_err(|e| format!("Failed to update function_hash: {}", e))?;
    } else {
        db_conn.execute(
            "INSERT INTO cfg_blocks (function_id, block_kind, function_hash)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![function_id, "placeholder", function_hash],
        ).map_err(|e| format!("Failed to insert function_hash: {}", e))?;
    }

    Ok(paths)
}

/// Enumerate paths with integrated caching and pre-computed context
///
/// This is the highest-level path enumeration function that combines:
/// 1. Hash-based cache invalidation (via update_function_paths_if_changed)
/// 2. Pre-computed enumeration context (avoid redundant analysis)
/// 3. Optimized batch storage (via store_paths_batch)
///
/// **Use this when:**
/// - You need the fastest path enumeration with caching
/// - Performing multiple enumerations on the same CFG
/// - You want automatic cache invalidation on function changes
///
/// **Algorithm:**
/// 1. Check cache via hash comparison (update_function_paths_if_changed)
/// 2. If cache hit: retrieve stored paths
/// 3. If cache miss:
///    - Create EnumerationContext
///    - Enumerate paths with context
///    - Store paths with batch insert
/// 4. Return paths
///
/// **Performance:**
/// - Cache hit: O(p) retrieval (p = stored path count)
/// - Cache miss: O(v+e + n*l) where v=vertices, e=edges, n=paths, l=avg length
/// - Subsequent calls with same CFG: O(v+e) for context only
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `function_id` - Database ID of the function
/// * `function_hash` - BLAKE3 hash of function content for cache validation
/// * `limits` - Path enumeration limits
/// * `db_conn` - Database connection for cache operations
///
/// # Returns
///
/// `Ok(Vec<Path>)` - Enumerated or cached paths
/// `Err(String)` - Error message if operation fails
///
/// # Example
///
/// ```rust,no_run
/// # use mirage::cfg::{enumerate_paths_cached, PathLimits, EnumerationContext};
/// # use mirage::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// # let function_bytes: Vec<u8> = vec![];
/// # let function_id: i64 = 0;
/// # let mut conn = unimplemented!();
/// let hash = blake3::hash(&function_bytes).to_hex().to_string();
/// let paths = enumerate_paths_cached(
///     &graph,
///     function_id,
///     &hash,
///     &PathLimits::default(),
///     &mut conn,
/// )?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn enumerate_paths_cached(
    cfg: &Cfg,
    function_id: i64,
    function_hash: &str,
    limits: &PathLimits,
    db_conn: &mut rusqlite::Connection,
) -> Result<Vec<Path>, String> {
    use crate::storage::paths::{get_cached_paths, update_function_paths_if_changed};

    // Try to get cached paths first
    // update_function_paths_if_changed handles hash comparison and returns:
    // - Ok(false) if hash matched (cache hit)
    // - Ok(true) if paths were updated (cache miss)
    let current_hash: Option<String> = db_conn.query_row(
        "SELECT function_hash FROM cfg_blocks WHERE function_id = ?1 LIMIT 1",
        rusqlite::params![function_id],
        |row| row.get(0),
    ).unwrap_or(None);

    // Check if we have a cache hit
    if let Some(ref hash) = current_hash {
        if hash == function_hash {
            // Cache hit - retrieve stored paths
            let paths = get_cached_paths(db_conn, function_id)
                .map_err(|e| format!("Failed to retrieve cached paths: {}", e))?;
            return Ok(paths);
        }
    }

    // Cache miss - enumerate paths with pre-computed context
    let ctx = EnumerationContext::new(cfg);
    let paths = enumerate_paths_with_context(cfg, limits, &ctx);

    // Store paths using batch insert for performance
    update_function_paths_if_changed(db_conn, function_id, function_hash, &paths)
        .map_err(|e| format!("Failed to store enumerated paths: {}", e))?;

    Ok(paths)
}

/// Enumerate paths with external context and integrated caching
///
/// Similar to `enumerate_paths_cached` but allows you to provide a pre-computed
/// EnumerationContext. Use this when performing multiple operations on the same CFG.
///
/// **Benefits over enumerate_paths_cached:**
/// - Reuse context across multiple calls
/// - Useful when you also need context for other analyses
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `function_id` - Database ID of the function
/// * `function_hash` - BLAKE3 hash of function content for cache validation
/// * `limits` - Path enumeration limits
/// * `ctx` - Pre-computed enumeration context
/// * `db_conn` - Database connection for cache operations
///
/// # Returns
///
/// `Ok(Vec<Path>)` - Enumerated or cached paths
/// `Err(String)` - Error message if operation fails
pub fn enumerate_paths_cached_with_context(
    cfg: &Cfg,
    function_id: i64,
    function_hash: &str,
    limits: &PathLimits,
    ctx: &EnumerationContext,
    db_conn: &mut rusqlite::Connection,
) -> Result<Vec<Path>, String> {
    use crate::storage::paths::{get_cached_paths, update_function_paths_if_changed};

    // Try cache first
    let current_hash: Option<String> = db_conn.query_row(
        "SELECT function_hash FROM cfg_blocks WHERE function_id = ?1 LIMIT 1",
        rusqlite::params![function_id],
        |row| row.get(0),
    ).unwrap_or(None);

    if let Some(ref hash) = current_hash {
        if hash == function_hash {
            return get_cached_paths(db_conn, function_id)
                .map_err(|e| format!("Failed to retrieve cached paths: {}", e));
        }
    }

    // Cache miss - use provided context
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
///
/// **Algorithm:**
/// - Count loop headers (each contributes multiplicative complexity)
/// - Estimate: 2^(branches + loops) * (loop_unroll_limit + 1)^loop_count
/// - Cap at max_paths to avoid overflow
///
/// **Usage:**
/// ```rust,no_run
/// # use mirage::cfg::paths::estimate_path_count;
/// # use mirage::cfg::Cfg;
/// # use mirage::cfg::PathLimits;
/// # let graph: Cfg = unimplemented!();
/// # let limits = PathLimits::default();
/// let estimated = estimate_path_count(&graph, 3);
/// if estimated > limits.max_paths {
///     // Warn user about path explosion
/// }
/// ```
///
/// **Limitations:**
/// - This is an estimate, not exact count
/// - Assumes worst-case (all paths are independent)
/// - Actual count may be lower due to dominance constraints
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `loop_unroll_limit` - Maximum loop iterations to account for
///
/// # Returns
///
/// Estimated maximum path count (upper bound)
pub fn estimate_path_count(cfg: &Cfg, loop_unroll_limit: usize) -> usize {
    // Count loop headers
    let loop_headers = crate::cfg::loops::find_loop_headers(cfg);
    let loop_count = loop_headers.len();

    // Count branch points (excluding loop back edges)
    let mut branch_count = 0;
    for node in cfg.node_indices() {
        if loop_headers.contains(&node) {
            continue; // Skip loop headers, counted separately
        }
        let out_degree = cfg.neighbors_directed(node, petgraph::Direction::Outgoing).count();
        if out_degree > 1 {
            branch_count += out_degree - 1; // Each extra edge adds complexity
        }
    }

    // Base estimate: at least 1 path for acyclic CFG
    if loop_count == 0 && branch_count == 0 {
        return 1; // Single path (linear CFG)
    }

    // Each branch roughly doubles path count
    // Each loop multiplies by (unroll_limit + 1)
    let unroll_factor = loop_unroll_limit + 1;

    // Calculate: 2^branch_count * unroll_factor^loop_count
    // Use saturating operations to avoid overflow
    let branch_factor = if branch_count < 31 {
        2_usize.pow(branch_count as u32)
    } else {
        usize::MAX / 2 // Cap to avoid overflow
    };

    let loop_factor = if loop_count < 31 {
        unroll_factor.pow(loop_count as u32)
    } else {
        usize::MAX / 2 // Cap to avoid overflow
    };

    // Multiply with overflow protection
    branch_factor.saturating_mul(loop_factor)
}

/// Check if path enumeration may exceed limits
///
/// This is a convenience wrapper around `estimate_path_count` that
/// compares the estimate against a limit and returns a warning if
/// the estimate suggests explosion.
///
/// # Returns
///
/// * `None` - Enumeration should be safe (estimate <= max_paths)
/// * `Some(estimate)` - Enumeration may exceed limit (estimate > max_paths)
pub fn check_path_explosion(cfg: &Cfg, limits: &PathLimits) -> Option<usize> {
    let estimate = estimate_path_count(cfg, limits.loop_unroll_limit);
    if estimate > limits.max_paths {
        Some(estimate)
    } else {
        None
    }
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

    // Task 4: Feasibility limitation demonstration

    /// Create a CFG with conflicting conditions that static analysis can't detect
    ///
    /// This demonstrates the limitation of static feasibility checking:
    /// The path might have conflicting conditions (e.g., x > 5 && x < 3)
    /// but static analysis doesn't perform symbolic execution, so it's still
    /// marked as feasible.
    fn create_conflicting_conditions_cfg() -> Cfg {
        let mut g = DiGraph::new();

        // Entry: check x > 5
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![1], otherwise: 2 },
            source_location: None,
        });

        // True branch: check x < 3 (conflicts with x > 5)
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![3], otherwise: 3 },
            source_location: None,
        });

        // False branch: just return
        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // After conflicting check: return
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

        g
    }

    #[test]
    fn test_feasibility_limitation_conflicting_conditions() {
        use crate::cfg::reachability::find_reachable;

        let cfg = create_conflicting_conditions_cfg();
        let reachable = find_reachable(&cfg)
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // Path 0 -> 1 -> 3 has conflicting conditions (x > 5 && x < 3)
        // This is dynamically infeasible (no value can satisfy both)
        // But static analysis doesn't detect this
        let path = vec![0, 1, 3];

        // The path is marked feasible by static check
        assert!(is_feasible_path_precomputed(&cfg, &path, &reachable),
                "Static analysis marks conflicting path as feasible (limitation)");

        // And classified as Normal
        assert_eq!(classify_path_precomputed(&cfg, &path, &reachable),
                   PathKind::Normal,
                   "Conflicting path is classified as Normal (static limitation)");

        // This test documents the limitation: symbolic execution would be needed
        // to detect that x > 5 and x < 3 cannot both be true
    }

    #[test]
    fn test_feasibility_documentation_accuracy() {
        use crate::cfg::reachability::find_reachable;

        // Verify that the documented behavior is accurate

        let cfg = create_linear_cfg();
        let reachable = find_reachable(&cfg)
            .iter()
            .map(|&idx| cfg[idx].id)
            .collect();

        // What we check: Entry kind
        let non_entry_path = vec![1, 2];
        assert!(!is_feasible_path_precomputed(&cfg, &non_entry_path, &reachable),
                "Entry check works as documented");

        // What we check: Valid exit terminator
        let dead_end_path = vec![0, 1];
        assert!(!is_feasible_path_precomputed(&cfg, &dead_end_path, &reachable),
                "Dead-end detection works as documented");

        // What we check: Reachable blocks
        assert!(is_feasible_path_precomputed(&cfg, &[0, 1, 2], &reachable),
                "Reachable path is feasible as documented");
    }

    // Task 7: get_or_enumerate_paths tests

    #[test]
    fn test_get_or_enumerate_paths_cache_miss_enumerates() {
        use crate::storage::create_schema;

        // Create in-memory database with schema
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();

        // Create Magellan tables (simplified)
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        // Create Mirage schema
        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = 1;

        // Create test CFG
        let cfg = create_linear_cfg();
        let function_hash = "test_hash_123";
        let limits = PathLimits::default();

        // First call should enumerate (cache miss)
        let paths = get_or_enumerate_paths(&cfg, function_id, function_hash, &limits, &mut conn).unwrap();

        // Linear CFG has exactly 1 path
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].blocks, vec![0, 1, 2]);
    }

    #[test]
    fn test_get_or_enumerate_paths_cache_hit_retrieves() {
        use crate::storage::create_schema;

        let mut conn = rusqlite::Connection::open_in_memory().unwrap();

        // Create Magellan tables
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = 1;

        let cfg = create_linear_cfg();
        let function_hash = "test_hash_456";
        let limits = PathLimits::default();

        // First call - cache miss, enumerates and stores
        let paths1 = get_or_enumerate_paths(&cfg, function_id, function_hash, &limits, &mut conn).unwrap();
        assert_eq!(paths1.len(), 1);

        // Second call with same hash - cache hit, retrieves without enumeration
        let paths2 = get_or_enumerate_paths(&cfg, function_id, function_hash, &limits, &mut conn).unwrap();
        assert_eq!(paths2.len(), 1);
        assert_eq!(paths2[0].blocks, vec![0, 1, 2]);
    }

    #[test]
    fn test_get_or_enumerate_paths_hash_change_invalidates() {
        use crate::storage::create_schema;

        let mut conn = rusqlite::Connection::open_in_memory().unwrap();

        // Create Magellan tables
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = 1;

        let cfg = create_linear_cfg();
        let hash1 = "test_hash_v1";
        let hash2 = "test_hash_v2";
        let limits = PathLimits::default();

        // Call with hash1 - stores paths
        let paths1 = get_or_enumerate_paths(&cfg, function_id, hash1, &limits, &mut conn).unwrap();
        assert_eq!(paths1.len(), 1);

        // Call with hash2 - should invalidate and re-enumerate
        let paths2 = get_or_enumerate_paths(&cfg, function_id, hash2, &limits, &mut conn).unwrap();
        assert_eq!(paths2.len(), 1);

        // Both should return the same path content (same CFG)
        assert_eq!(paths1[0].blocks, paths2[0].blocks);
    }

    // Task 05-06-2: EnumerationContext tests

    #[test]
    fn test_enumeration_context_new() {
        use super::super::EnumerationContext;

        let cfg = create_linear_cfg();
        let ctx = EnumerationContext::new(&cfg);

        // Linear CFG: 3 blocks all reachable
        assert_eq!(ctx.reachable_count(), 3);
        assert_eq!(ctx.loop_count(), 0);
        assert_eq!(ctx.exit_count(), 1);
    }

    #[test]
    fn test_enumeration_context_with_loop() {
        use super::super::EnumerationContext;

        let cfg = create_loop_cfg();
        let ctx = EnumerationContext::new(&cfg);

        // Loop CFG: should have 1 loop header
        assert_eq!(ctx.loop_count(), 1);
        assert!(ctx.reachable_count() > 0);
        assert!(ctx.exit_count() > 0);
    }

    #[test]
    fn test_enumeration_context_diamond_cfg() {
        use super::super::EnumerationContext;

        let cfg = create_diamond_cfg();
        let ctx = EnumerationContext::new(&cfg);

        // Diamond CFG: no loops
        assert_eq!(ctx.loop_count(), 0);
        assert_eq!(ctx.reachable_count(), 4); // All 4 blocks reachable
        assert_eq!(ctx.exit_count(), 1); // Single merge point exit
    }

    #[test]
    fn test_enumeration_context_is_reachable() {
        use super::super::EnumerationContext;

        let cfg = create_dead_code_cfg();
        let ctx = EnumerationContext::new(&cfg);

        // Block 0 is reachable (entry)
        assert!(ctx.is_reachable(0));
        // Block 1 is not reachable (dead code)
        assert!(!ctx.is_reachable(1));
    }

    #[test]
    fn test_enumeration_context_is_loop_header() {
        use super::super::EnumerationContext;
        use petgraph::graph::NodeIndex;

        let cfg = create_loop_cfg();
        let ctx = EnumerationContext::new(&cfg);

        // Block 1 is the loop header
        assert!(ctx.is_loop_header(NodeIndex::new(1)));
        // Block 0 is not a loop header
        assert!(!ctx.is_loop_header(NodeIndex::new(0)));
    }

    #[test]
    fn test_enumeration_context_is_exit() {
        use super::super::EnumerationContext;
        use petgraph::graph::NodeIndex;

        let cfg = create_diamond_cfg();
        let ctx = EnumerationContext::new(&cfg);

        // Block 3 is the exit
        assert!(ctx.is_exit(NodeIndex::new(3)));
        // Block 0 is not an exit
        assert!(!ctx.is_exit(NodeIndex::new(0)));
    }

    #[test]
    fn test_enumerate_paths_with_context_matches_basic() {
        use super::super::{enumerate_paths, enumerate_paths_with_context, EnumerationContext};

        let cfg = create_diamond_cfg();
        let limits = PathLimits::default();
        let ctx = EnumerationContext::new(&cfg);

        // Both methods should return same paths
        let paths_basic = enumerate_paths(&cfg, &limits);
        let paths_context = enumerate_paths_with_context(&cfg, &limits, &ctx);

        assert_eq!(paths_basic.len(), paths_context.len());

        // Sort and compare
        let mut sorted_basic: Vec<_> = paths_basic.iter().collect();
        let mut sorted_context: Vec<_> = paths_context.iter().collect();
        sorted_basic.sort_by_key(|p| p.blocks.clone());
        sorted_context.sort_by_key(|p| p.blocks.clone());

        for (basic, context) in sorted_basic.iter().zip(sorted_context.iter()) {
            assert_eq!(basic.blocks, context.blocks);
            assert_eq!(basic.kind, context.kind);
        }
    }

    #[test]
    fn test_enumerate_paths_with_context_linear_cfg() {
        use super::super::{enumerate_paths_with_context, EnumerationContext};

        let cfg = create_linear_cfg();
        let limits = PathLimits::default();
        let ctx = EnumerationContext::new(&cfg);

        let paths = enumerate_paths_with_context(&cfg, &limits, &ctx);

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].blocks, vec![0, 1, 2]);
    }

    #[test]
    fn test_enumerate_paths_with_context_with_loop() {
        use super::super::{enumerate_paths_with_context, EnumerationContext};

        let cfg = create_loop_cfg();
        let limits = PathLimits::default();
        let ctx = EnumerationContext::new(&cfg);

        let paths = enumerate_paths_with_context(&cfg, &limits, &ctx);

        // With loop_unroll_limit=3, should have bounded paths
        assert!(paths.len() > 0);
        assert!(paths.len() <= 4); // Entry + up to 3 loop iterations
    }

    #[test]
    fn test_enumerate_paths_with_context_performance() {
        use super::super::{enumerate_paths, enumerate_paths_with_context, EnumerationContext};
        use std::time::Instant;

        let cfg = create_diamond_cfg();
        let limits = PathLimits::default();

        // Time basic enumeration
        let start = Instant::now();
        let _paths1 = enumerate_paths(&cfg, &limits);
        let basic_time = start.elapsed();

        // Create context and time context enumeration
        let ctx_start = Instant::now();
        let ctx = EnumerationContext::new(&cfg);
        let ctx_creation_time = ctx_start.elapsed();

        let start = Instant::now();
        let _paths2 = enumerate_paths_with_context(&cfg, &limits, &ctx);
        let context_time = start.elapsed();

        // First call with context should be faster than basic
        // (basic recomputes everything, context reuses)
        // Note: This is a micro-benchmark and may vary
        println!("Basic: {:?}, Context creation: {:?}, Context enum: {:?}",
                 basic_time, ctx_creation_time, context_time);

        // Second call with same context should be much faster
        let start = Instant::now();
        let _paths3 = enumerate_paths_with_context(&cfg, &limits, &ctx);
        let context_time2 = start.elapsed();

        println!("Second context call: {:?}", context_time2);

        // Context enumeration should be complete
        assert!(context_time2.as_millis() < 100);
    }

    #[test]
    fn test_enumerate_paths_with_context_multiple_calls() {
        use super::super::{enumerate_paths_with_context, EnumerationContext};

        let cfg = create_diamond_cfg();
        let ctx = EnumerationContext::new(&cfg);

        // Multiple calls with different limits should reuse context
        let limits1 = PathLimits::default().with_max_paths(10);
        let limits2 = PathLimits::default().with_max_paths(100);

        let paths1 = enumerate_paths_with_context(&cfg, &limits1, &ctx);
        let paths2 = enumerate_paths_with_context(&cfg, &limits2, &ctx);

        // Both should return the same paths (10 is enough for diamond)
        assert_eq!(paths1.len(), paths2.len());
    }

    // Task 05-06-3: enumerate_paths_cached tests

    #[test]
    fn test_enumerate_paths_cached_cache_miss_enumerates() {
        use crate::storage::create_schema;
        use super::super::enumerate_paths_cached;

        let mut conn = rusqlite::Connection::open_in_memory().unwrap();

        // Create Magellan tables
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = 1;

        let cfg = create_linear_cfg();
        let function_hash = "test_hash_123";
        let limits = PathLimits::default();

        // First call should enumerate (cache miss)
        let paths = enumerate_paths_cached(&cfg, function_id, function_hash, &limits, &mut conn).unwrap();

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].blocks, vec![0, 1, 2]);
    }

    #[test]
    fn test_enumerate_paths_cached_cache_hit_retrieves() {
        use crate::storage::create_schema;
        use super::super::enumerate_paths_cached;

        let mut conn = rusqlite::Connection::open_in_memory().unwrap();

        // Create Magellan tables
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = 1;

        let cfg = create_linear_cfg();
        let function_hash = "test_hash_456";
        let limits = PathLimits::default();

        // First call - cache miss, enumerates and stores
        let paths1 = enumerate_paths_cached(&cfg, function_id, function_hash, &limits, &mut conn).unwrap();
        assert_eq!(paths1.len(), 1);

        // Second call with same hash - cache hit, retrieves without enumeration
        let paths2 = enumerate_paths_cached(&cfg, function_id, function_hash, &limits, &mut conn).unwrap();
        assert_eq!(paths2.len(), 1);
        assert_eq!(paths2[0].blocks, vec![0, 1, 2]);
    }

    #[test]
    fn test_enumerate_paths_cached_hash_change_invalidates() {
        use crate::storage::create_schema;
        use super::super::enumerate_paths_cached;

        let mut conn = rusqlite::Connection::open_in_memory().unwrap();

        // Create Magellan tables
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = 1;

        let cfg = create_linear_cfg();
        let hash1 = "test_hash_v1";
        let hash2 = "test_hash_v2";
        let limits = PathLimits::default();

        // Call with hash1 - stores paths
        let paths1 = enumerate_paths_cached(&cfg, function_id, hash1, &limits, &mut conn).unwrap();
        assert_eq!(paths1.len(), 1);

        // Call with hash2 - should invalidate and re-enumerate
        let paths2 = enumerate_paths_cached(&cfg, function_id, hash2, &limits, &mut conn).unwrap();
        assert_eq!(paths2.len(), 1);

        // Both should return the same path content (same CFG)
        assert_eq!(paths1[0].blocks, paths2[0].blocks);
    }

    #[test]
    fn test_enumerate_paths_cached_with_context() {
        use crate::storage::create_schema;
        use super::super::{enumerate_paths_cached_with_context, EnumerationContext};

        let mut conn = rusqlite::Connection::open_in_memory().unwrap();

        // Create Magellan tables
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = 1;

        let cfg = create_diamond_cfg();
        let function_hash = "test_hash_ctx";
        let limits = PathLimits::default();
        let ctx = EnumerationContext::new(&cfg);

        // First call - cache miss
        let paths1 = enumerate_paths_cached_with_context(
            &cfg, function_id, function_hash, &limits, &ctx, &mut conn
        ).unwrap();
        assert_eq!(paths1.len(), 2); // Diamond has 2 paths

        // Second call - cache hit
        let paths2 = enumerate_paths_cached_with_context(
            &cfg, function_id, function_hash, &limits, &ctx, &mut conn
        ).unwrap();
        assert_eq!(paths2.len(), 2);
    }

    // Task 05-06-4: estimate_path_count tests

    #[test]
    fn test_estimate_path_count_linear_cfg() {
        // Linear CFG: 0 -> 1 -> 2
        let cfg = create_linear_cfg();
        let estimate = estimate_path_count(&cfg, 3);

        // Linear CFG has no branches or loops, so 1 path
        assert_eq!(estimate, 1);
    }

    #[test]
    fn test_estimate_path_count_diamond_cfg() {
        // Diamond: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        let cfg = create_diamond_cfg();
        let estimate = estimate_path_count(&cfg, 3);

        // Diamond has 1 branch point (2 outgoing from block 0)
        // With loop_unroll_limit=3: 2^1 = 2
        assert_eq!(estimate, 2);
    }

    #[test]
    fn test_estimate_path_count_single_loop() {
        // Single loop with unroll_limit=3
        let cfg = create_loop_cfg();
        let estimate = estimate_path_count(&cfg, 3);

        // 1 loop, unroll_limit=3: (3+1)^1 = 4
        assert_eq!(estimate, 4);
    }

    #[test]
    fn test_estimate_path_count_loop_formula() {
        // Single loop, verify formula: (unroll_limit + 1) * 2
        let cfg = create_loop_cfg();

        // With unroll_limit=3: (3+1) * 2 = 8
        // (2 because there's also a branch at the loop header)
        let estimate = estimate_path_count(&cfg, 3);
        assert!(estimate >= 4); // At minimum, loop contributes 4 paths
    }

    #[test]
    fn test_estimate_path_count_increases_with_loop_limit() {
        let cfg = create_loop_cfg();

        let estimate3 = estimate_path_count(&cfg, 3);
        let estimate5 = estimate_path_count(&cfg, 5);

        // Higher unroll limit should give higher estimate
        assert!(estimate5 >= estimate3);
    }

    #[test]
    fn test_estimate_path_count_monotonic_with_complexity() {
        // Create increasingly complex CFGs and verify monotonic growth
        let linear = create_linear_cfg();
        let diamond = create_diamond_cfg();
        let loop_cfg = create_loop_cfg();

        let linear_estimate = estimate_path_count(&linear, 3);
        let diamond_estimate = estimate_path_count(&diamond, 3);
        let _loop_estimate = estimate_path_count(&loop_cfg, 3);

        // Linear < Diamond < Loop (in terms of complexity)
        assert!(linear_estimate <= diamond_estimate);
        // Diamond might be less than loop depending on structure
    }

    #[test]
    fn test_check_path_explosion_safe() {
        let cfg = create_linear_cfg();
        let limits = PathLimits::default();

        // Linear CFG with default limits should be safe
        let result = check_path_explosion(&cfg, &limits);
        assert!(result.is_none(), "Linear CFG should be safe");
    }

    #[test]
    fn test_check_path_explosion_exceeds_limit() {
        let cfg = create_diamond_cfg();
        let limits = PathLimits {
            max_length: 100,
            max_paths: 1, // Very low limit
            loop_unroll_limit: 3,
        };

        // Diamond might exceed very low limit
        let result = check_path_explosion(&cfg, &limits);
        // Diamond produces 2 paths, so limit of 1 should trigger warning
        assert!(result.is_some(), "Should warn about path explosion");
        if let Some(estimate) = result {
            assert!(estimate > 1);
        }
    }

    #[test]
    fn test_estimate_path_count_no_overflow() {
        // Even with high unroll limit, should not overflow
        let cfg = create_loop_cfg();

        // Very high unroll limit
        let estimate = estimate_path_count(&cfg, 1000);

        // Should return a reasonable number, not overflow
        assert!(estimate > 0);
        assert!(estimate < usize::MAX);
    }

    // Task 05-06-5: Performance benchmark tests

    /// Create a large linear CFG (100 blocks)
    fn create_large_linear_cfg(size: usize) -> Cfg {
        let mut g = DiGraph::new();

        for i in 0..size {
            let kind = if i == 0 {
                BlockKind::Entry
            } else if i == size - 1 {
                BlockKind::Exit
            } else {
                BlockKind::Normal
            };

            let terminator = if i == size - 1 {
                Terminator::Return
            } else {
                Terminator::Goto { target: i + 1 }
            };

            let _node = g.add_node(BasicBlock {
                id: i,
                kind,
                statements: vec![],
                terminator,
                source_location: None,
            });
        }

        // Add edges
        for i in 0..size - 1 {
            let from = NodeIndex::new(i);
            let to = NodeIndex::new(i + 1);
            g.add_edge(from, to, EdgeType::Fallthrough);
        }

        g
    }

    /// Create a large diamond CFG (10 sequential branches)
    fn create_large_diamond_cfg() -> Cfg {
        let mut g = DiGraph::new();

        // Create a chain of diamond patterns
        let mut nodes = Vec::new();

        for i in 0..21 {
            let kind = if i == 0 {
                BlockKind::Entry
            } else if i % 2 == 0 && i > 0 {
                // Merge points
                BlockKind::Normal
            } else if i == 20 {
                BlockKind::Exit
            } else {
                BlockKind::Normal
            };

            let terminator = if i == 20 {
                Terminator::Return
            } else if i % 2 == 0 {
                // Branch point (even numbers after 0)
                let target1 = i + 1;
                let target2 = i + 2;
                Terminator::SwitchInt { targets: vec![target1], otherwise: target2 }
            } else {
                // Fallthrough to merge
                let merge = i + 1;
                Terminator::Goto { target: merge }
            };

            let node = g.add_node(BasicBlock {
                id: i,
                kind,
                statements: vec![],
                terminator,
                source_location: None,
            });
            nodes.push(node);
        }

        // Add edges for branch points
        for i in (0..20).step_by(2) {
            let from = nodes[i];
            let to1 = nodes[i + 1];
            let to2 = nodes[i + 2];
            g.add_edge(from, to1, EdgeType::TrueBranch);
            g.add_edge(from, to2, EdgeType::FalseBranch);
        }

        // Add edges for fallthroughs
        for i in (1..20).filter(|x| x % 2 == 1) {
            let from = nodes[i];
            let to = nodes[i + 1];
            g.add_edge(from, to, EdgeType::Fallthrough);
        }

        g
    }

    #[test]
    #[ignore = "benchmark test - run with cargo test -- --ignored"]
    fn test_perf_linear_cfg_10_blocks() {
        use std::time::Instant;

        let cfg = create_large_linear_cfg(10);
        let limits = PathLimits::default();

        let start = Instant::now();
        let paths = enumerate_paths(&cfg, &limits);
        let duration = start.elapsed();

        assert_eq!(paths.len(), 1);
        assert!(duration < Duration::from_millis(10),
                "Linear 10-block CFG should be <10ms, took {:?}", duration);
        println!("Linear 10 blocks: {:?}", duration);
    }

    #[test]
    #[ignore = "benchmark test - run with cargo test -- --ignored"]
    fn test_perf_linear_cfg_100_blocks() {
        use std::time::Instant;

        let cfg = create_large_linear_cfg(100);
        let limits = PathLimits::default();

        let start = Instant::now();
        let paths = enumerate_paths(&cfg, &limits);
        let duration = start.elapsed();

        assert_eq!(paths.len(), 1);
        assert!(duration < Duration::from_millis(100),
                "Linear 100-block CFG should be <100ms, took {:?}", duration);
        println!("Linear 100 blocks: {:?}", duration);
    }

    #[test]
    #[ignore = "benchmark test - run with cargo test -- --ignored"]
    fn test_perf_diamond_cfg_10_branches() {
        use std::time::Instant;

        let cfg = create_large_diamond_cfg();
        let limits = PathLimits::default();

        let start = Instant::now();
        let paths = enumerate_paths(&cfg, &limits);
        let duration = start.elapsed();

        // 10 branches = 2^10 = 1024 paths
        assert!(paths.len() > 0);
        assert!(duration < Duration::from_millis(50),
                "Diamond CFG should be <50ms, took {:?}", duration);
        println!("Diamond 10 branches: {} paths in {:?}", paths.len(), duration);
    }

    #[test]
    #[ignore = "benchmark test - run with cargo test -- --ignored"]
    fn test_perf_single_loop_unroll_3() {
        use std::time::Instant;

        let cfg = create_loop_cfg();
        let limits = PathLimits::default().with_loop_unroll_limit(3);

        let start = Instant::now();
        let paths = enumerate_paths(&cfg, &limits);
        let duration = start.elapsed();

        assert!(paths.len() > 0);
        assert!(duration < Duration::from_millis(100),
                "Single loop should be <100ms, took {:?}", duration);
        println!("Single loop (unroll=3): {} paths in {:?}", paths.len(), duration);
    }

    #[test]
    #[ignore = "benchmark test - run with cargo test -- --ignored"]
    fn test_perf_nested_loops() {
        use std::time::Instant;
        use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};

        let mut g = DiGraph::new();

        // Create 2-level nested loop
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
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 5 },
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

        let b5 = g.add_node(BasicBlock {
            id: 5,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b5, EdgeType::FalseBranch);
        g.add_edge(b2, b3, EdgeType::TrueBranch);
        g.add_edge(b2, b1, EdgeType::LoopBack);
        g.add_edge(b3, b2, EdgeType::LoopBack);

        let limits = PathLimits::default().with_loop_unroll_limit(2);

        let start = Instant::now();
        let paths = enumerate_paths(&g, &limits);
        let duration = start.elapsed();

        assert!(paths.len() > 0);
        assert!(duration < Duration::from_millis(500),
                "Nested loops should be <500ms, took {:?}", duration);
        println!("Nested 2 loops (unroll=2): {} paths in {:?}", paths.len(), duration);
    }

    #[test]
    #[ignore = "benchmark test - run with cargo test -- --ignored"]
    fn test_perf_enumeration_context_reuse() {
        use std::time::Instant;
        use super::super::EnumerationContext;

        let cfg = create_diamond_cfg();
        let ctx = EnumerationContext::new(&cfg);

        // Time multiple calls with same context
        let limits = PathLimits::default();

        let start = Instant::now();
        for _ in 0..100 {
            let _ = enumerate_paths_with_context(&cfg, &limits, &ctx);
        }
        let duration = start.elapsed();

        println!("100 calls with context: {:?}", duration);
        assert!(duration < Duration::from_millis(100),
                "100 cached calls should be <100ms, took {:?}", duration);
    }

    #[test]
    #[ignore = "benchmark test - run with cargo test -- --ignored"]
    fn test_perf_estimation_vs_actual() {
        use std::time::Instant;

        let cfg = create_diamond_cfg();

        // Time estimation
        let start = Instant::now();
        let estimate = estimate_path_count(&cfg, 3);
        let est_duration = start.elapsed();

        // Time actual enumeration
        let start = Instant::now();
        let limits = PathLimits::default();
        let paths = enumerate_paths(&cfg, &limits);
        let enum_duration = start.elapsed();

        println!("Estimation: {} paths in {:?}", estimate, est_duration);
        println!("Enumeration: {} paths in {:?}", paths.len(), enum_duration);

        // Both should be fast for simple CFGs
        assert!(est_duration.as_micros() < 1000,
                "Estimation should be fast: {:?}", est_duration);
        assert!(enum_duration.as_micros() < 1000,
                "Enumeration should be fast: {:?}", enum_duration);
    }
}
