//! Dominator tree construction and queries
//!
//! A node d dominates node n if every path from the entry node to n
//! must go through d. The immediate dominator of a node is its unique
//! closest strict dominator.
//!
//! This module wraps petgraph's dominance algorithm with a more ergonomic API.

use crate::cfg::{Cfg, BlockId};
use crate::cfg::analysis::find_entry;
use petgraph::algo::dominators::simple_fast;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;

/// Dominator tree for a CFG
///
/// Provides cached dominance queries including immediate dominators,
/// dominance checks, and dominator tree traversal.
///
/// # Example
/// ```rust
/// let dom_tree = DominatorTree::new(&cfg)?;
/// if let Some(idom) = dom_tree.immediate_dominator(node) {
///     println!("Node {:?} is dominated by {:?}", node, idom);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct DominatorTree {
    /// Root node (entry block)
    root: NodeIndex,
    /// Immediate dominator for each node
    /// None indicates the root node (not unreachable - unreachable nodes aren't in the map)
    immediate_dominator: HashMap<NodeIndex, Option<NodeIndex>>,
    /// Children in dominator tree (nodes immediately dominated by each node)
    children: HashMap<NodeIndex, Vec<NodeIndex>>,
}

impl DominatorTree {
    /// Compute dominator tree using Cooper et al. algorithm
    ///
    /// Returns None if CFG has no entry node.
    ///
    /// Time: O(|V|²) worst case, faster in practice for typical CFGs
    /// Space: O(|V| + |E|)
    ///
    /// # Errors
    /// Returns None if:
    /// - CFG is empty (no nodes)
    /// - CFG has no entry node (no BlockKind::Entry)
    pub fn new(cfg: &Cfg) -> Option<Self> {
        let entry = find_entry(cfg)?;

        // Compute dominators using Cooper et al. algorithm
        let dominators = simple_fast(cfg, entry);

        let mut immediate_dominator = HashMap::new();
        let mut children: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();

        // Build immediate dominator map and children lists
        for node in cfg.node_indices() {
            let idom = dominators.immediate_dominator(node);

            // Store immediate dominator (None for root, Some for others)
            immediate_dominator.insert(node, idom);

            // Build dominator tree: node is child of its immediate dominator
            if let Some(parent) = idom {
                children.entry(parent).or_default().push(node);
            }
        }

        Some(Self {
            root: entry,
            immediate_dominator,
            children,
        })
    }

    /// Get the root node of the dominator tree
    ///
    /// The root is the entry node of the CFG.
    pub fn root(&self) -> NodeIndex {
        self.root
    }

    /// Get immediate dominator of a node
    ///
    /// Returns None for the root node (which has no dominator).
    ///
    /// # Example
    /// ```rust
    /// if let Some(idom) = dom_tree.immediate_dominator(node) {
    ///     println!("Immediately dominated by {:?}", idom);
    /// } else {
    ///     println!("This is the root node");
    /// }
    /// ```
    pub fn immediate_dominator(&self, node: NodeIndex) -> Option<NodeIndex> {
        self.immediate_dominator.get(&node).copied().flatten()
    }

    /// Check if `a` dominates `b`
    ///
    /// A dominates B if every path from root to B contains A.
    /// By definition, every node dominates itself.
    ///
    /// # Example
    /// ```rust
    /// if dom_tree.dominates(entry, node) {
    ///     println!("entry dominates node (always true for reachable nodes)");
    /// }
    /// ```
    pub fn dominates(&self, a: NodeIndex, b: NodeIndex) -> bool {
        if a == b {
            return true; // Node dominates itself
        }

        // Walk up b's dominator chain to see if we hit a
        let mut current = b;
        while let Some(idom) = self.immediate_dominator(current) {
            if idom == a {
                return true;
            }
            current = idom;
        }

        false
    }

    /// Get all nodes immediately dominated by `node`
    ///
    /// Returns the children of `node` in the dominator tree.
    ///
    /// # Example
    /// ```rust
    /// for child in dom_tree.children(node) {
    ///     println!("{:?} immediately dominates {:?}", node, child);
    /// }
    /// ```
    pub fn children(&self, node: NodeIndex) -> &[NodeIndex] {
        self.children.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// Check if `a` strictly dominates `b`
    ///
    /// A strictly dominates B if A dominates B and A != B.
    pub fn strictly_dominates(&self, a: NodeIndex, b: NodeIndex) -> bool {
        a != b && self.dominates(a, b)
    }

    /// Get all dominators of a node (including itself)
    ///
    /// Returns iterator from node up to root.
    ///
    /// # Example
    /// ```rust
    /// let doms: Vec<_> = dom_tree.dominators(node).collect();
    /// println!("Node {:?} has {} dominators", node, doms.len());
    /// ```
    pub fn dominators(&self, node: NodeIndex) -> Dominators<'_> {
        Dominators {
            tree: self,
            current: Some(node),
        }
    }

    /// Get the nearest common dominator of two nodes
    ///
    /// Returns the node that dominates both `a` and `b` and is
    /// dominated by all other common dominators.
    ///
    /// Returns None if nodes are not in the same dominance tree
    /// (shouldn't happen in valid CFGs with single entry).
    pub fn common_dominator(&self, a: NodeIndex, b: NodeIndex) -> Option<NodeIndex> {
        // Collect a's dominators
        let a_doms: std::collections::HashSet<NodeIndex> =
            self.dominators(a).collect();

        // Find first (nearest) dominator of b that's also in a's dominators
        for dom in self.dominators(b) {
            if a_doms.contains(&dom) {
                return Some(dom);
            }
        }

        None
    }

    /// Get depth of node in dominator tree
    ///
    /// Root has depth 0, its children have depth 1, etc.
    pub fn depth(&self, node: NodeIndex) -> usize {
        let mut depth = 0;
        let mut current = node;
        while let Some(idom) = self.immediate_dominator(current) {
            depth += 1;
            current = idom;
        }
        depth
    }

    /// Create DominatorTree from pre-computed parts
    ///
    /// This is used internally by PostDominatorTree to construct
    /// a dominator tree on a reversed graph.
    pub(crate) fn from_parts(
        root: NodeIndex,
        immediate_dominator: HashMap<NodeIndex, Option<NodeIndex>>,
        children: HashMap<NodeIndex, Vec<NodeIndex>>,
    ) -> Self {
        Self {
            root,
            immediate_dominator,
            children,
        }
    }
}

/// Iterator over a node's dominators (from node up to root)
pub struct Dominators<'a> {
    tree: &'a DominatorTree,
    current: Option<NodeIndex>,
}

impl<'a> Iterator for Dominators<'a> {
    type Item = NodeIndex;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.current?;
        self.current = self.tree.immediate_dominator(node);
        Some(node)
    }
}

/// Convenience function to compute dominator tree
///
/// This is a shorthand for DominatorTree::new().
///
/// # Example
/// ```rust
/// let dom_tree = compute_dominator_tree(&cfg)?;
/// ```
pub fn compute_dominator_tree(cfg: &Cfg) -> Option<DominatorTree> {
    DominatorTree::new(cfg)
}

/// Get immediate dominator as BlockId
///
/// Convenience function that converts NodeIndex to BlockId.
pub fn immediate_dominator_id(tree: &DominatorTree, block_id: BlockId, cfg: &Cfg) -> Option<BlockId> {
    let node = node_from_id(cfg, block_id)?;
    let idom_node = tree.immediate_dominator(node)?;
    Some(cfg[idom_node].id)
}

/// Helper: find NodeIndex from BlockId
fn node_from_id(cfg: &Cfg, block_id: BlockId) -> Option<NodeIndex> {
    cfg.node_indices()
        .find(|&n| cfg[n].id == block_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, BlockKind, Terminator, EdgeType};
    use petgraph::graph::DiGraph;

    /// Create a simple diamond CFG:
    ///     0 (entry)
    ///    / \
    ///   1   2
    ///    \ /
    ///     3 (exit)
    fn create_diamond_cfg() -> Cfg {
        let mut g = DiGraph::new();

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
            statements: vec!["branch 1".to_string()],
            terminator: Terminator::Goto { target: 3 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec!["branch 2".to_string()],
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

    #[test]
    fn test_dominator_tree_construction() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");

        // Entry (0) is root
        assert_eq!(dom_tree.root(), NodeIndex::new(0));

        // Entry has no immediate dominator
        assert_eq!(dom_tree.immediate_dominator(NodeIndex::new(0)), None);

        // Node 1 is immediately dominated by entry (0)
        assert_eq!(dom_tree.immediate_dominator(NodeIndex::new(1)), Some(NodeIndex::new(0)));

        // Node 2 is immediately dominated by entry (0)
        assert_eq!(dom_tree.immediate_dominator(NodeIndex::new(2)), Some(NodeIndex::new(0)));

        // Node 3 is immediately dominated by entry (0) in diamond CFG
        assert_eq!(dom_tree.immediate_dominator(NodeIndex::new(3)), Some(NodeIndex::new(0)));
    }

    #[test]
    fn test_dominates() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");

        let entry = NodeIndex::new(0);
        let node1 = NodeIndex::new(1);
        let node3 = NodeIndex::new(3);

        // Entry dominates all nodes
        assert!(dom_tree.dominates(entry, entry));
        assert!(dom_tree.dominates(entry, node1));
        assert!(dom_tree.dominates(entry, node3));

        // Non-root doesn't dominate entry
        assert!(!dom_tree.dominates(node1, entry));

        // Every node dominates itself
        assert!(dom_tree.dominates(node1, node1));
        assert!(dom_tree.dominates(node3, node3));
    }

    #[test]
    fn test_children() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");

        let entry = NodeIndex::new(0);
        let children = dom_tree.children(entry);

        // Entry has children 1, 2, and 3 (in diamond CFG)
        assert_eq!(children.len(), 3);
        assert!(children.contains(&NodeIndex::new(1)));
        assert!(children.contains(&NodeIndex::new(2)));
        assert!(children.contains(&NodeIndex::new(3)));
    }

    #[test]
    fn test_strictly_dominates() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");

        let entry = NodeIndex::new(0);
        let node1 = NodeIndex::new(1);

        // Entry strictly dominates node1
        assert!(dom_tree.strictly_dominates(entry, node1));

        // Entry does NOT strictly dominate itself
        assert!(!dom_tree.strictly_dominates(entry, entry));
    }

    #[test]
    fn test_dominators_iterator() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");

        let node3 = NodeIndex::new(3);
        let doms: Vec<_> = dom_tree.dominators(node3).collect();

        // Node 3's dominators: 3 itself, and 0 (entry)
        assert_eq!(doms.len(), 2);
        assert_eq!(doms[0], node3);
        assert_eq!(doms[1], NodeIndex::new(0));
    }

    #[test]
    fn test_common_dominator() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");

        let node1 = NodeIndex::new(1);
        let node2 = NodeIndex::new(2);
        let entry = NodeIndex::new(0);

        // Common dominator of 1 and 2 is entry (0)
        assert_eq!(dom_tree.common_dominator(node1, node2), Some(entry));

        // Common dominator of node with itself is the node
        assert_eq!(dom_tree.common_dominator(node1, node1), Some(node1));
    }

    #[test]
    fn test_depth() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");

        // Entry has depth 0
        assert_eq!(dom_tree.depth(NodeIndex::new(0)), 0);

        // Children of entry have depth 1
        assert_eq!(dom_tree.depth(NodeIndex::new(1)), 1);
        assert_eq!(dom_tree.depth(NodeIndex::new(2)), 1);
        assert_eq!(dom_tree.depth(NodeIndex::new(3)), 1);
    }

    #[test]
    fn test_empty_cfg() {
        let cfg: Cfg = DiGraph::new();
        assert!(DominatorTree::new(&cfg).is_none());
    }

    #[test]
    fn test_linear_cfg() {
        // Linear: 0 -> 1 -> 2 -> 3
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
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::Fallthrough);
        g.add_edge(b2, b3, EdgeType::Fallthrough);

        let dom_tree = DominatorTree::new(&g).expect("CFG has entry");

        // In linear CFG, each node i is dominated by 0, 1, ..., i-1
        assert_eq!(dom_tree.immediate_dominator(b0), None);
        assert_eq!(dom_tree.immediate_dominator(b1), Some(b0));
        assert_eq!(dom_tree.immediate_dominator(b2), Some(b1));
        assert_eq!(dom_tree.immediate_dominator(b3), Some(b2));
    }
}
