//! CFG analysis: entry/exit detection, dominance preparation

use crate::cfg::{BlockKind, Cfg, Terminator};
use petgraph::graph::NodeIndex;

/// Find the entry node of a CFG
///
/// The entry is always the first basic block (id = 0).
/// Returns None if the CFG is empty.
pub fn find_entry(cfg: &Cfg) -> Option<NodeIndex> {
    cfg.node_indices().next()
}

/// Find all exit nodes in a CFG
///
/// Exits are blocks that terminate execution:
/// - Return terminators
/// - Unreachable terminators
/// - Abort terminators (panics)
///
/// Functions can have multiple exits due to:
/// - Early returns
/// - Panic paths
/// - Different exit points from error handling
pub fn find_exits(cfg: &Cfg) -> Vec<NodeIndex> {
    cfg.node_indices()
        .filter(|&idx| is_exit_block(cfg, idx))
        .collect()
}

/// Check if a block is an exit block
pub fn is_exit_block(cfg: &Cfg, block_idx: NodeIndex) -> bool {
    if let Some(block) = cfg.node_weight(block_idx) {
        return matches!(
            &block.terminator,
            Terminator::Return | Terminator::Unreachable | Terminator::Abort(_)
        );
    }
    false
}

/// Get the BlockKind of a node
pub fn get_block_kind(cfg: &Cfg, block_idx: NodeIndex) -> Option<BlockKind> {
    cfg.node_weight(block_idx).map(|b| b.kind)
}

/// Count incoming edges to a node
pub fn in_degree(cfg: &Cfg, block_idx: NodeIndex) -> usize {
    cfg.neighbors_directed(block_idx, petgraph::Direction::Incoming)
        .count()
}

/// Count outgoing edges from a node
pub fn out_degree(cfg: &Cfg, block_idx: NodeIndex) -> usize {
    cfg.neighbors_directed(block_idx, petgraph::Direction::Outgoing)
        .count()
}

/// Check if a node is a merge point (multiple incoming edges)
pub fn is_merge_point(cfg: &Cfg, block_idx: NodeIndex) -> bool {
    in_degree(cfg, block_idx) > 1
}

/// Check if a node is a branch point (multiple outgoing edges)
pub fn is_branch_point(cfg: &Cfg, block_idx: NodeIndex) -> bool {
    out_degree(cfg, block_idx) > 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, EdgeType};
    use petgraph::graph::DiGraph;

    fn create_test_cfg() -> Cfg {
        let mut g = DiGraph::new();

        // Block 0: entry, goes to 1
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Block 1: if statement, goes to 2 (true) or 3 (false)
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

        // Block 2: true branch, returns
        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // Block 3: false branch, returns
        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // Add edges
        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b3, EdgeType::FalseBranch);

        g
    }

    #[test]
    fn test_find_entry() {
        let cfg = create_test_cfg();
        let entry = find_entry(&cfg);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().index(), 0);
    }

    #[test]
    fn test_find_exits() {
        let cfg = create_test_cfg();
        let exits = find_exits(&cfg);
        assert_eq!(exits.len(), 2);

        // Both blocks 2 and 3 are exits
        let exit_ids: Vec<_> = exits
            .iter()
            .map(|&idx| cfg.node_weight(idx).unwrap().id)
            .collect();
        assert!(exit_ids.contains(&2));
        assert!(exit_ids.contains(&3));
    }

    #[test]
    fn test_is_exit_block() {
        let cfg = create_test_cfg();

        let b0 = NodeIndex::new(0);
        let b1 = NodeIndex::new(1);
        let b2 = NodeIndex::new(2);
        let b3 = NodeIndex::new(3);

        assert!(!is_exit_block(&cfg, b0)); // entry
        assert!(!is_exit_block(&cfg, b1)); // branch
        assert!(is_exit_block(&cfg, b2)); // exit
        assert!(is_exit_block(&cfg, b3)); // exit
    }

    #[test]
    fn test_is_branch_point() {
        let cfg = create_test_cfg();

        let b0 = NodeIndex::new(0);
        let b1 = NodeIndex::new(1);
        let b2 = NodeIndex::new(2);

        assert!(!is_branch_point(&cfg, b0)); // 1 outgoing
        assert!(is_branch_point(&cfg, b1)); // 2 outgoing
        assert!(!is_branch_point(&cfg, b2)); // 0 outgoing
    }

    #[test]
    fn test_is_merge_point() {
        let cfg = create_test_cfg();

        let b0 = NodeIndex::new(0);
        let b1 = NodeIndex::new(1);

        assert!(!is_merge_point(&cfg, b0)); // 0 incoming
        assert!(!is_merge_point(&cfg, b1)); // 1 incoming
    }

    #[test]
    fn test_is_merge_point_with_actual_merge() {
        let mut g = DiGraph::new();

        // Create a diamond pattern: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        // Block 3 is a merge point (2 incoming edges)

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

        assert!(!is_merge_point(&g, b0)); // 0 incoming
        assert!(!is_merge_point(&g, b1)); // 1 incoming
        assert!(!is_merge_point(&g, b2)); // 1 incoming
        assert!(is_merge_point(&g, b3)); // 2 incoming - merge point!
    }

    #[test]
    fn test_multiple_exits_with_unwind() {
        let mut g = DiGraph::new();

        // Block 0: entry with call that can unwind
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Call {
                target: Some(1),
                unwind: Some(2),
            },
            source_location: None,
        });

        // Block 1: normal return
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // Block 2: unwind exit
        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Abort("panic".to_string()),
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Call);
        g.add_edge(b0, b2, EdgeType::Exception);

        let exits = find_exits(&g);
        assert_eq!(exits.len(), 2);
    }

    #[test]
    fn test_empty_cfg() {
        let cfg: Cfg = DiGraph::new();
        assert!(find_entry(&cfg).is_none());
        assert!(find_exits(&cfg).is_empty());
    }

    #[test]
    fn test_single_block_cfg() {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // A single block that is both entry and exit
        assert_eq!(find_entry(&g), Some(b0));
        assert_eq!(find_exits(&g), vec![b0]);
    }

    #[test]
    fn test_unreachable_exit() {
        let mut g = DiGraph::new();

        // Block 0: entry
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Block 1: normal path
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        // Block 2: unreachable (dead code)
        let _b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Unreachable,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);

        // Unreachable is still an exit block
        let exits = find_exits(&g);
        assert_eq!(exits.len(), 2);
        let exit_ids: Vec<_> = exits
            .iter()
            .map(|&idx| g.node_weight(idx).unwrap().id)
            .collect();
        assert!(exit_ids.contains(&1)); // Return
        assert!(exit_ids.contains(&2)); // Unreachable
    }
}
