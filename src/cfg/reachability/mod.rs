//! Reachability analysis for CFGs

use crate::cfg::analysis::find_entry;
use crate::cfg::{BlockId, Cfg};
use petgraph::algo::has_path_connecting;
use petgraph::algo::DfsSpace;
use petgraph::graph::NodeIndex;
use petgraph::visit::Dfs;
use std::collections::HashSet;

/// Find all blocks reachable from the entry node
///
/// Returns all nodes that have a path from the entry block.
/// For empty CFGs, returns an empty vec.
pub fn find_reachable(cfg: &Cfg) -> Vec<NodeIndex> {
    let entry = match find_entry(cfg) {
        Some(e) => e,
        None => return vec![],
    };

    // Use DFS to collect all reachable nodes
    let mut dfs = Dfs::new(cfg, entry);
    let mut reachable = Vec::new();

    while let Some(node) = dfs.next(cfg) {
        reachable.push(node);
    }

    reachable
}

/// Find all blocks unreachable from the entry node
///
/// Returns an empty vec if:
/// - CFG has no entry (empty graph)
/// - All blocks are reachable
///
/// # Example
/// ```rust,no_run
/// # use mirage_analyzer::cfg::reachability::find_unreachable;
/// # use mirage_analyzer::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// let unreachable = find_unreachable(&graph);
/// for block_idx in unreachable {
///     println!("Block {:?} is dead code", block_idx);
/// }
/// ```
pub fn find_unreachable(cfg: &Cfg) -> Vec<NodeIndex> {
    // Ensure CFG has an entry (not empty)
    if find_entry(cfg).is_none() {
        return vec![];
    }

    // Get all reachable nodes from entry
    let reachable: HashSet<_> = find_reachable(cfg).into_iter().collect();

    // Unreachable = all nodes - reachable nodes
    cfg.node_indices()
        .filter(|&n| !reachable.contains(&n))
        .collect()
}

/// Check if a specific block is reachable from the entry node
pub fn is_reachable_from_entry(cfg: &Cfg, block: NodeIndex) -> bool {
    let entry = match find_entry(cfg) {
        Some(e) => e,
        None => return false,
    };

    has_path_connecting(cfg, entry, block, None)
}

/// Get unreachable block IDs for reporting
///
/// Returns BlockIds (usize) instead of NodeIndex for easier
/// integration with CLI reporting and database queries.
pub fn unreachable_block_ids(cfg: &Cfg) -> Vec<BlockId> {
    find_unreachable(cfg)
        .iter()
        .filter_map(|&idx| cfg.node_weight(idx))
        .map(|block| block.id)
        .collect()
}

/// Check if node `from` can reach node `to`
///
/// Returns true if there exists any path from `from` to `to`.
/// This is a simple yes/no query - it does not enumerate paths.
///
/// # Performance Note
/// For single queries, this allocates a new DFS visitor.
/// Use `can_reach_cached` or `ReachabilityCache` for repeated queries.
///
/// # Example
/// ```rust,no_run
/// # use mirage_analyzer::cfg::reachability::can_reach;
/// # use mirage_analyzer::cfg::analysis::find_entry;
/// # use mirage_analyzer::cfg::Cfg;
/// # use petgraph::graph::NodeIndex;
/// # let graph: Cfg = unimplemented!();
/// let entry = find_entry(&graph).unwrap();
/// let exit = NodeIndex::new(5);
/// if can_reach(&graph, entry, exit) {
///     println!("Exit is reachable from entry");
/// }
/// ```
pub fn can_reach(cfg: &Cfg, from: NodeIndex, to: NodeIndex) -> bool {
    has_path_connecting(cfg, from, to, None)
}

/// Check if node `from` can reach node `to` using cached DFS state
///
/// This version reuses the provided DfsSpace for better performance
/// when making multiple reachability queries.
///
/// # Example
/// ```rust,no_run
/// # use mirage_analyzer::cfg::reachability::can_reach_cached;
/// # use petgraph::algo::DfsSpace;
/// # use mirage_analyzer::cfg::Cfg;
/// # use petgraph::graph::NodeIndex;
/// # let graph: Cfg = unimplemented!();
/// # let queries: Vec<(NodeIndex, NodeIndex)> = vec![];
/// let mut space = DfsSpace::new(&graph);
/// for (from, to) in queries {
///     if can_reach_cached(&graph, from, to, &mut space) {
///         // ...
///     }
/// }
/// ```
pub fn can_reach_cached(
    cfg: &Cfg,
    from: NodeIndex,
    to: NodeIndex,
    space: &mut DfsSpace<NodeIndex, <Cfg as petgraph::visit::Visitable>::Map>,
) -> bool {
    has_path_connecting(cfg, from, to, Some(space))
}

/// Cache for repeated reachability queries
///
/// Holds reusable DFS state to avoid allocation on each query.
/// Create once, reuse for many queries on the same CFG.
///
/// # Example
/// ```rust,no_run
/// # use mirage_analyzer::cfg::reachability::ReachabilityCache;
/// # use mirage_analyzer::cfg::analysis::find_entry;
/// # use mirage_analyzer::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// # let entry = find_entry(&graph).unwrap();
/// let mut cache = ReachabilityCache::new(&graph);
/// for node in graph.node_indices() {
///     if cache.can_reach(&graph, entry, node) {
///         println!("Node {:?} is reachable", node);
///     }
/// }
/// ```
pub struct ReachabilityCache {
    space: DfsSpace<NodeIndex, <Cfg as petgraph::visit::Visitable>::Map>,
}

impl ReachabilityCache {
    /// Create a new cache for the given CFG
    ///
    /// The cache can be reused for multiple queries on the same CFG.
    pub fn new(cfg: &Cfg) -> Self {
        Self {
            space: DfsSpace::new(cfg),
        }
    }

    /// Check if `from` can reach `to` using cached state
    pub fn can_reach(&mut self, cfg: &Cfg, from: NodeIndex, to: NodeIndex) -> bool {
        can_reach_cached(cfg, from, to, &mut self.space)
    }
}

/// Result of block impact analysis
///
/// Describes the "blast zone" - all blocks reachable from a given source block.
/// This is useful for understanding the impact scope of code changes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BlockImpact {
    /// The source block from which impact was analyzed
    pub source_block_id: BlockId,
    /// All blocks reachable from the source (by BlockId, not NodeIndex)
    pub reachable_blocks: Vec<BlockId>,
    /// Total count of reachable blocks
    pub reachable_count: usize,
    /// Maximum traversal depth reached during analysis
    pub max_depth_reached: usize,
    /// Whether the impact contains cycles (loops)
    pub has_cycles: bool,
}

/// Find all blocks reachable from a specific starting block
///
/// Unlike `find_reachable` which starts from entry, this starts from any block.
/// Useful for impact analysis: "what happens if I change this block?"
///
/// # Arguments
///
/// * `cfg` - The control flow graph
/// * `start_block_id` - The BlockId (not NodeIndex) to start from
/// * `max_depth` - Maximum depth to traverse (None for unlimited)
///
/// # Returns
///
/// * `BlockImpact` struct with all reachable blocks and metadata
///
/// # Example
/// ```rust,no_run
/// # use mirage_analyzer::cfg::reachability::find_reachable_from_block;
/// # use mirage_analyzer::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// let impact = find_reachable_from_block(&graph, 5, Some(10));
/// println!("Block 5 affects {} blocks", impact.reachable_count);
/// ```
pub fn find_reachable_from_block(
    cfg: &Cfg,
    start_block_id: BlockId,
    max_depth: Option<usize>,
) -> BlockImpact {
    use std::collections::{HashSet, VecDeque};

    // Find the NodeIndex for the start BlockId
    let start_node = match cfg.node_indices().find(|&n| cfg[n].id == start_block_id) {
        Some(n) => n,
        None => {
            // Block not found in CFG - return empty impact
            return BlockImpact {
                source_block_id: start_block_id,
                reachable_blocks: vec![],
                reachable_count: 0,
                max_depth_reached: 0,
                has_cycles: false,
            };
        }
    };

    let max_depth = max_depth.unwrap_or(usize::MAX);

    // BFS traversal with depth tracking
    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::new();
    let mut reachable_blocks = Vec::new();
    let mut max_depth_reached = 0;
    let mut has_cycles = false;

    queue.push_back((start_node, 0));
    visited.insert(start_node);

    while let Some((node, depth)) = queue.pop_front() {
        max_depth_reached = max_depth_reached.max(depth);

        // Add the block's ID to reachable blocks
        let block_id = cfg[node].id;
        reachable_blocks.push(block_id);

        // Stop at max_depth
        if depth >= max_depth {
            continue;
        }

        // Explore neighbors
        for neighbor in cfg.neighbors(node) {
            if visited.contains(&neighbor) {
                // We've seen this node before - indicates a cycle
                has_cycles = true;
            } else {
                visited.insert(neighbor);
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    // Remove the source block from reachable blocks (it's not "impact", it's the source)
    reachable_blocks.retain(|&id| id != start_block_id);

    let reachable_count = reachable_blocks.len();

    BlockImpact {
        source_block_id: start_block_id,
        reachable_blocks,
        reachable_count,
        max_depth_reached,
        has_cycles,
    }
}

/// Result of path impact analysis
///
/// Aggregates impact across all blocks in a path.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PathImpact {
    /// The path ID analyzed
    pub path_id: String,
    /// Number of blocks in the path
    pub path_length: usize,
    /// Unique blocks affected by this path (union of all block blast zones)
    pub unique_blocks_affected: Vec<BlockId>,
    /// Count of unique blocks affected
    pub impact_count: usize,
}

/// Compute impact analysis for a path by aggregating block impacts
///
/// This function computes the union of all blocks reachable from any block
/// in the given path. This represents the full "blast zone" of the path.
///
/// # Arguments
///
/// * `cfg` - The control flow graph
/// * `path_block_ids` - BlockIds in the path (in order)
/// * `max_depth` - Maximum depth to traverse from each block
///
/// # Returns
///
/// * `PathImpact` struct with aggregated impact data
pub fn compute_path_impact(
    cfg: &Cfg,
    path_block_ids: &[BlockId],
    max_depth: Option<usize>,
) -> PathImpact {
    use std::collections::HashSet;

    let mut all_affected: HashSet<BlockId> = HashSet::new();

    // For each block in the path, compute its impact
    for &block_id in path_block_ids {
        let impact = find_reachable_from_block(cfg, block_id, max_depth);
        all_affected.extend(impact.reachable_blocks);
    }

    // Remove path blocks themselves from affected (they're the source)
    for &block_id in path_block_ids {
        all_affected.remove(&block_id);
    }

    let mut affected_vec: Vec<BlockId> = all_affected.into_iter().collect();
    affected_vec.sort();

    let impact_count = affected_vec.len();

    PathImpact {
        path_id: "[computed]".to_string(), // Will be set by caller
        path_length: path_block_ids.len(),
        unique_blocks_affected: affected_vec,
        impact_count,
    }
}

#[cfg(test)]
mod tests;
