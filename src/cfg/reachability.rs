//! Reachability analysis for CFGs

use crate::cfg::{BlockId, Cfg};
use crate::cfg::analysis::find_entry;
use petgraph::algo::has_path_connecting;
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
/// ```rust
/// let unreachable = find_unreachable(&cfg);
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
}
