//! Reachability analysis for CFGs

use crate::cfg::{BlockId, Cfg};
use crate::cfg::analysis::find_entry;
use petgraph::algo::reachable_from;
use petgraph::graph::NodeIndex;
use std::collections::HashSet;

/// Find all blocks unreachable from the entry node
///
/// Returns an empty vec if:
/// - CFG has no entry (empty graph)
/// - All blocks are reachable
///
/// # Example
/// ```rust
/// let unreachable = find_unreachable(&cfg);
/// for block_idx in unreachable {
///     println!("Block {:?} is dead code", block_idx);
/// }
/// ```
pub fn find_unreachable(cfg: &Cfg) -> Vec<NodeIndex> {
    let entry = match find_entry(cfg) {
        Some(e) => e,
        None => return vec![],
    };

    // Get all reachable nodes from entry
    let reachable: HashSet<_> = reachable_from(cfg, entry).collect();

    // Unreachable = all nodes - reachable nodes
    cfg.node_indices()
        .filter(|&n| !reachable.contains(&n))
        .collect()
}

/// Find all blocks reachable from the entry node
///
/// Returns all nodes that have a path from the entry block.
/// For empty CFGs, returns an empty vec.
pub fn find_reachable(cfg: &Cfg) -> Vec<NodeIndex> {
    let entry = match find_entry(cfg) {
        Some(e) => e,
        None => return vec![],
    };

    reachable_from(cfg, entry).collect()
}

/// Check if a specific block is reachable from the entry node
pub fn is_reachable_from_entry(cfg: &Cfg, block: NodeIndex) -> bool {
    let entry = match find_entry(cfg) {
        Some(e) => e,
        None => return false,
    };

    let reachable: HashSet<_> = reachable_from(cfg, entry).collect();
    reachable.contains(&block)
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
