//! Reachability analysis for CFGs

use crate::cfg::{BlockId, Cfg};
use crate::cfg::analysis::find_entry;
use petgraph::algo::has_path_connecting;
use petgraph::graph::NodeIndex;
use petgraph::visit::Dfs;
use petgraph::algo::DfsSpace;
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
/// # use mirage::cfg::reachability::find_unreachable;
/// # use mirage::cfg::Cfg;
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
/// # use mirage::cfg::reachability::can_reach;
/// # use mirage::cfg::analysis::find_entry;
/// # use mirage::cfg::Cfg;
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
/// # use mirage::cfg::reachability::can_reach_cached;
/// # use petgraph::algo::DfsSpace;
/// # use mirage::cfg::Cfg;
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
/// # use mirage::cfg::reachability::ReachabilityCache;
/// # use mirage::cfg::analysis::find_entry;
/// # use mirage::cfg::Cfg;
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
/// # use mirage::cfg::reachability::find_reachable_from_block;
/// # use mirage::cfg::Cfg;
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
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    fn create_test_cfg_with_unreachable() -> Cfg {
        let mut g = DiGraph::new();

        // Block 0: entry, goes to 1
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Block 1: normal, goes to 2
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
        });

        // Block 2: exit
        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // Block 3: unreachable (no edges to it)
        let _b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec!["unreachable code".to_string()],
            terminator: Terminator::Unreachable,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::Fallthrough);

        g
    }

    #[test]
    fn test_find_unreachable() {
        let cfg = create_test_cfg_with_unreachable();
        let unreachable = find_unreachable(&cfg);

        assert_eq!(unreachable.len(), 1);
        let block_id = cfg.node_weight(unreachable[0]).unwrap().id;
        assert_eq!(block_id, 3);
    }

    #[test]
    fn test_find_reachable() {
        let cfg = create_test_cfg_with_unreachable();
        let reachable = find_reachable(&cfg);

        assert_eq!(reachable.len(), 3);
        let ids: Vec<_> = reachable.iter()
            .map(|&idx| cfg.node_weight(idx).unwrap().id)
            .collect();
        assert!(ids.contains(&0));
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        assert!(!ids.contains(&3));
    }

    #[test]
    fn test_is_reachable_from_entry() {
        let cfg = create_test_cfg_with_unreachable();

        let b0 = NodeIndex::new(0);
        let b3 = NodeIndex::new(3);

        assert!(is_reachable_from_entry(&cfg, b0));
        assert!(!is_reachable_from_entry(&cfg, b3));
    }

    #[test]
    fn test_empty_cfg() {
        let cfg: Cfg = DiGraph::new();
        assert!(find_unreachable(&cfg).is_empty());
        assert!(find_reachable(&cfg).is_empty());
    }

    #[test]
    fn test_fully_reachable_cfg() {
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
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        assert!(find_unreachable(&g).is_empty());
    }

    #[test]
    fn test_can_reach_simple() {
        let mut g = DiGraph::new();

        // Create: 0 -> 1 -> 2
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

        // All nodes can reach themselves
        assert!(can_reach(&g, b0, b0));
        assert!(can_reach(&g, b1, b1));
        assert!(can_reach(&g, b2, b2));

        // Forward reachability
        assert!(can_reach(&g, b0, b1));
        assert!(can_reach(&g, b0, b2));
        assert!(can_reach(&g, b1, b2));

        // No backward reachability
        assert!(!can_reach(&g, b1, b0));
        assert!(!can_reach(&g, b2, b0));
        assert!(!can_reach(&g, b2, b1));
    }

    #[test]
    fn test_can_reach_diamond() {
        let mut g = DiGraph::new();

        // Diamond: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![1], otherwise: 2 },
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

        // All nodes reachable from entry
        assert!(can_reach(&g, b0, b1));
        assert!(can_reach(&g, b0, b2));
        assert!(can_reach(&g, b0, b3));

        // Branches can't reach each other
        assert!(!can_reach(&g, b1, b2));
        assert!(!can_reach(&g, b2, b1));
    }

    #[test]
    fn test_can_reach_cached() {
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
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        let mut space = DfsSpace::new(&g);

        // First query
        assert!(can_reach_cached(&g, b0, b1, &mut space));

        // Second query (space is reset internally by has_path_connecting)
        assert!(!can_reach_cached(&g, b1, b0, &mut space));
    }

    #[test]
    fn test_reachability_cache() {
        let mut g = DiGraph::new();

        // Create a linear chain: 0 -> 1 -> 2 -> 3
        let nodes = (0..4).map(|i| {
            g.add_node(BasicBlock {
                id: i,
                kind: if i == 0 { BlockKind::Entry } else if i == 3 { BlockKind::Exit } else { BlockKind::Normal },
                statements: vec![],
                terminator: if i < 3 { Terminator::Goto { target: i + 1 } } else { Terminator::Return },
                source_location: None,
            })
        }).collect::<Vec<_>>();

        for i in 0..3 {
            g.add_edge(nodes[i], nodes[i + 1], EdgeType::Fallthrough);
        }

        let mut cache = ReachabilityCache::new(&g);

        // Multiple queries using same cache
        assert!(cache.can_reach(&g, nodes[0], nodes[3]));
        assert!(cache.can_reach(&g, nodes[1], nodes[3]));
        assert!(!cache.can_reach(&g, nodes[3], nodes[0]));
    }

    #[test]
    fn test_find_reachable_from_block_linear() {
        let mut g = DiGraph::new();

        // Create: 0 -> 1 -> 2 -> 3
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
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::Fallthrough);
        g.add_edge(b2, b3, EdgeType::Fallthrough);

        // From block 0, can reach 1, 2, 3
        let impact = find_reachable_from_block(&g, 0, None);
        assert_eq!(impact.source_block_id, 0);
        assert_eq!(impact.reachable_count, 3);
        assert!(impact.reachable_blocks.contains(&1));
        assert!(impact.reachable_blocks.contains(&2));
        assert!(impact.reachable_blocks.contains(&3));
        assert!(!impact.has_cycles);
    }

    #[test]
    fn test_find_reachable_from_block_diamond() {
        let mut g = DiGraph::new();

        // Diamond: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![1], otherwise: 2 },
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

        // From block 0, can reach 1, 2, 3
        let impact = find_reachable_from_block(&g, 0, None);
        assert_eq!(impact.source_block_id, 0);
        assert_eq!(impact.reachable_count, 3);
        assert!(impact.reachable_blocks.contains(&1));
        assert!(impact.reachable_blocks.contains(&2));
        assert!(impact.reachable_blocks.contains(&3));
    }

    #[test]
    fn test_find_reachable_from_block_max_depth() {
        let mut g = DiGraph::new();

        // Create: 0 -> 1 -> 2 -> 3
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
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::Fallthrough);
        g.add_edge(b2, b3, EdgeType::Fallthrough);

        // With max_depth=1, from block 0 can only reach block 1
        let impact = find_reachable_from_block(&g, 0, Some(1));
        assert_eq!(impact.source_block_id, 0);
        assert_eq!(impact.reachable_count, 1);
        assert!(impact.reachable_blocks.contains(&1));
        assert!(!impact.reachable_blocks.contains(&2));
        assert!(!impact.reachable_blocks.contains(&3));
        assert_eq!(impact.max_depth_reached, 1);
    }

    #[test]
    fn test_find_reachable_from_block_not_found() {
        let g = DiGraph::new();

        // Block 99 doesn't exist
        let impact = find_reachable_from_block(&g, 99, None);
        assert_eq!(impact.source_block_id, 99);
        assert_eq!(impact.reachable_count, 0);
        assert!(impact.reachable_blocks.is_empty());
    }

    #[test]
    fn test_find_reachable_from_block_with_loop() {
        let mut g = DiGraph::new();

        // Create a loop: 0 -> 1 -> 2 -> 1 (back edge)
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
            terminator: Terminator::SwitchInt { targets: vec![1], otherwise: 3 },
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
        g.add_edge(b1, b2, EdgeType::Fallthrough);
        g.add_edge(b2, b1, EdgeType::LoopBack);  // Back edge
        g.add_edge(b2, b3, EdgeType::LoopExit);

        // From block 1, should detect cycle
        let impact = find_reachable_from_block(&g, 1, Some(10));
        assert_eq!(impact.source_block_id, 1);
        assert!(impact.has_cycles);
    }

    #[test]
    fn test_compute_path_impact() {
        let mut g = DiGraph::new();

        // Diamond: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![1], otherwise: 2 },
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

        // Path: 0 -> 1 -> 3
        // From 0: reaches 1, 2, 3
        // From 1: reaches 3
        // From 3: reaches nothing (exit)
        // Combined impact: {1, 2, 3} U {3} U {} = {1, 2, 3}
        // Minus path blocks {0, 1, 3} = {2}
        let impact = compute_path_impact(&g, &[0, 1, 3], None);
        assert_eq!(impact.path_length, 3);
        // Block 2 is the only block not in the path but reachable from it
        assert!(impact.unique_blocks_affected.contains(&2));
    }
}
