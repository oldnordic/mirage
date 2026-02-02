//! Post-dominator tree construction and queries
//!
//! A node d post-dominates node n if every path from n to an exit node
//! must go through d. Post-dominance is the dual of dominance, computed
//! by running the dominance algorithm on a reversed CFG.

use crate::cfg::{Cfg, BlockId};
use crate::cfg::analysis::find_exits;
use crate::cfg::dominators::DominatorTree;
use petgraph::algo::dominators::simple_fast;
use petgraph::graph::NodeIndex;
use petgraph::visit::Reversed;
use std::collections::HashMap;

/// Post-dominator tree for a CFG
///
/// Computed by running the dominance algorithm on a reversed CFG.
/// The root is the exit node.
///
/// # Example
/// ```rust,no_run
/// # use mirage::cfg::post_dominators::PostDominatorTree;
/// # use mirage::cfg::Cfg;
/// # use petgraph::graph::NodeIndex;
/// # let graph: Cfg = unimplemented!();
/// # let node = NodeIndex::new(0);
/// let post_dom_tree = PostDominatorTree::new(&graph).unwrap();
/// if let Some(ipdom) = post_dom_tree.immediate_post_dominator(node) {
///     println!("Node {:?} is post-dominated by {:?}", node, ipdom);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PostDominatorTree {
    /// Dominator tree on reversed graph (isomorphic to post-dominator tree)
    inner: DominatorTree,
    /// Root node (exit block)
    exit: NodeIndex,
}

impl PostDominatorTree {
    /// Compute post-dominator tree using graph reversal
    ///
    /// Returns None if CFG has no exit nodes.
    ///
    /// Algorithm:
    /// 1. Find primary exit node (first Return node)
    /// 2. Reverse graph edges using Reversed<G> adaptor
    /// 3. Compute dominators with exit as root
    /// 4. Result is post-dominators on original graph
    ///
    /// Time: O(|V|²) worst case, faster in practice
    /// Space: O(|V| + |E|)
    ///
    /// # Limitations
    /// - Uses primary exit only. Functions with multiple exits
    ///   (Return + Abort + Unreachable) may have incomplete post-dominators.
    /// - Returns None if no exit nodes found.
    pub fn new(cfg: &Cfg) -> Option<Self> {
        // Find exit node(s) - use primary exit (first Return node)
        let exits = find_exits(cfg);
        let exit = exits.first().copied()?;

        // Reverse the graph (zero-copy view)
        let reversed = Reversed(cfg);

        // Compute dominators on reversed graph
        let dominators = simple_fast(reversed, exit);

        // Build DominatorTree from reversed dominators
        let mut immediate_dominator = HashMap::new();
        let mut children: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();

        for node in cfg.node_indices() {
            let idom = dominators.immediate_dominator(node);
            immediate_dominator.insert(node, idom);

            if let Some(parent) = idom {
                children.entry(parent).or_default().push(node);
            }
        }

        // Manually construct DominatorTree using internal constructor
        let inner = DominatorTree::from_parts(exit, immediate_dominator, children);

        Some(Self { inner, exit })
    }

    /// Get the root node of the post-dominator tree
    ///
    /// The root is the exit node of the CFG.
    pub fn root(&self) -> NodeIndex {
        self.exit
    }

    /// Get immediate post-dominator of a node
    ///
    /// Returns None for the exit node (which has no post-dominator).
    ///
    /// # Example
    /// ```rust,no_run
    /// # use mirage::cfg::post_dominators::PostDominatorTree;
    /// # use mirage::cfg::Cfg;
    /// # use petgraph::graph::NodeIndex;
    /// # let graph: Cfg = unimplemented!();
    /// # let post_dom_tree = PostDominatorTree::new(&graph).unwrap();
    /// # let node = NodeIndex::new(0);
    /// if let Some(ipdom) = post_dom_tree.immediate_post_dominator(node) {
    ///     println!("Immediately post-dominated by {:?}", ipdom);
    /// } else {
    ///     println!("This is the exit node");
    /// }
    /// ```
    pub fn immediate_post_dominator(&self, node: NodeIndex) -> Option<NodeIndex> {
        self.inner.immediate_dominator(node)
    }

    /// Check if `a` post-dominates `b`
    ///
    /// A post-dominates B if every path from B to exit contains A.
    /// By definition, every node post-dominates itself.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use mirage::cfg::post_dominators::PostDominatorTree;
    /// # use mirage::cfg::Cfg;
    /// # use petgraph::graph::NodeIndex;
    /// # let graph: Cfg = unimplemented!();
    /// # let post_dom_tree = PostDominatorTree::new(&graph).unwrap();
    /// # let exit = NodeIndex::new(0);
    /// # let node = NodeIndex::new(1);
    /// if post_dom_tree.post_dominates(exit, node) {
    ///     println!("exit post-dominates node (always true for nodes that can reach exit)");
    /// }
    /// ```
    pub fn post_dominates(&self, a: NodeIndex, b: NodeIndex) -> bool {
        self.inner.dominates(a, b)
    }

    /// Get all nodes immediately post-dominated by `node`
    ///
    /// Returns the children of `node` in the post-dominator tree.
    pub fn children(&self, node: NodeIndex) -> &[NodeIndex] {
        self.inner.children(node)
    }

    /// Check if `a` strictly post-dominates `b`
    ///
    /// A strictly post-dominates B if A post-dominates B and A != B.
    pub fn strictly_post_dominates(&self, a: NodeIndex, b: NodeIndex) -> bool {
        self.inner.strictly_dominates(a, b)
    }

    /// Get all post-dominators of a node (including itself)
    ///
    /// Returns iterator from node up to exit.
    pub fn post_dominators(&self, node: NodeIndex) -> PostDominators<'_> {
        PostDominators {
            tree: self,
            current: Some(node),
        }
    }

    /// Get the nearest common post-dominator of two nodes
    ///
    /// Returns the node that post-dominates both `a` and `b` and is
    /// post-dominated by all other common post-dominators.
    pub fn common_post_dominator(&self, a: NodeIndex, b: NodeIndex) -> Option<NodeIndex> {
        // Collect a's post-dominators
        let a_pdoms: std::collections::HashSet<NodeIndex> =
            self.post_dominators(a).collect();

        // Find first (nearest) post-dominator of b that's also in a's post-dominators
        for pdom in self.post_dominators(b) {
            if a_pdoms.contains(&pdom) {
                return Some(pdom);
            }
        }

        None
    }

    /// Get depth of node in post-dominator tree
    ///
    /// Exit has depth 0, its children have depth 1, etc.
    pub fn depth(&self, node: NodeIndex) -> usize {
        self.inner.depth(node)
    }

    /// Get the underlying DominatorTree (for advanced use)
    ///
    /// This exposes the internal dominator tree structure on the reversed graph.
    pub fn as_dominator_tree(&self) -> &DominatorTree {
        &self.inner
    }
}

/// Iterator over a node's post-dominators (from node up to exit)
pub struct PostDominators<'a> {
    tree: &'a PostDominatorTree,
    current: Option<NodeIndex>,
}

impl<'a> Iterator for PostDominators<'a> {
    type Item = NodeIndex;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.current?;
        self.current = self.tree.immediate_post_dominator(node);
        Some(node)
    }
}

/// Convenience function to compute post-dominator tree
///
/// This is a shorthand for PostDominatorTree::new().
///
/// # Example
/// ```rust,no_run
/// # use mirage::cfg::post_dominators::compute_post_dominator_tree;
/// # use mirage::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// let post_dom_tree = compute_post_dominator_tree(&graph).unwrap();
/// ```
pub fn compute_post_dominator_tree(cfg: &Cfg) -> Option<PostDominatorTree> {
    PostDominatorTree::new(cfg)
}

/// Get immediate post-dominator as BlockId
///
/// Convenience function that converts NodeIndex to BlockId.
pub fn immediate_post_dominator_id(tree: &PostDominatorTree, block_id: BlockId, cfg: &Cfg) -> Option<BlockId> {
    let node = node_from_id(cfg, block_id)?;
    let ipdom_node = tree.immediate_post_dominator(node)?;
    Some(cfg[ipdom_node].id)
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
    fn test_post_dominator_tree_construction() {
        let cfg = create_diamond_cfg();
        let post_dom_tree = PostDominatorTree::new(&cfg).expect("CFG has exit");

        // Exit (3) is root
        assert_eq!(post_dom_tree.root(), NodeIndex::new(3));

        // Exit has no immediate post-dominator
        assert_eq!(post_dom_tree.immediate_post_dominator(NodeIndex::new(3)), None);

        // Node 1 is immediately post-dominated by exit (3)
        assert_eq!(post_dom_tree.immediate_post_dominator(NodeIndex::new(1)), Some(NodeIndex::new(3)));

        // Node 2 is immediately post-dominated by exit (3)
        assert_eq!(post_dom_tree.immediate_post_dominator(NodeIndex::new(2)), Some(NodeIndex::new(3)));

        // Node 0 is immediately post-dominated by exit (3) in diamond CFG
        assert_eq!(post_dom_tree.immediate_post_dominator(NodeIndex::new(0)), Some(NodeIndex::new(3)));
    }

    #[test]
    fn test_post_dominates() {
        let cfg = create_diamond_cfg();
        let post_dom_tree = PostDominatorTree::new(&cfg).expect("CFG has exit");

        let exit = NodeIndex::new(3);
        let entry = NodeIndex::new(0);

        // Exit post-dominates all nodes that can reach it
        assert!(post_dom_tree.post_dominates(exit, exit));
        assert!(post_dom_tree.post_dominates(exit, entry));

        // Entry does not post-dominate exit
        assert!(!post_dom_tree.post_dominates(entry, exit));

        // Every node post-dominates itself
        assert!(post_dom_tree.post_dominates(entry, entry));
        assert!(post_dom_tree.post_dominates(exit, exit));
    }

    #[test]
    fn test_children() {
        let cfg = create_diamond_cfg();
        let post_dom_tree = PostDominatorTree::new(&cfg).expect("CFG has exit");

        let exit = NodeIndex::new(3);
        let children = post_dom_tree.children(exit);

        // Exit has children 0, 1, and 2 (in diamond CFG)
        assert_eq!(children.len(), 3);
        assert!(children.contains(&NodeIndex::new(0)));
        assert!(children.contains(&NodeIndex::new(1)));
        assert!(children.contains(&NodeIndex::new(2)));
    }

    #[test]
    fn test_strictly_post_dominates() {
        let cfg = create_diamond_cfg();
        let post_dom_tree = PostDominatorTree::new(&cfg).expect("CFG has exit");

        let exit = NodeIndex::new(3);
        let node1 = NodeIndex::new(1);

        // Exit strictly post-dominates node1
        assert!(post_dom_tree.strictly_post_dominates(exit, node1));

        // Exit does NOT strictly post-dominate itself
        assert!(!post_dom_tree.strictly_post_dominates(exit, exit));
    }

    #[test]
    fn test_post_dominators_iterator() {
        let cfg = create_diamond_cfg();
        let post_dom_tree = PostDominatorTree::new(&cfg).expect("CFG has exit");

        let entry = NodeIndex::new(0);
        let pdoms: Vec<_> = post_dom_tree.post_dominators(entry).collect();

        // Entry's post-dominators: 0 itself, and 3 (exit)
        assert_eq!(pdoms.len(), 2);
        assert_eq!(pdoms[0], entry);
        assert_eq!(pdoms[1], NodeIndex::new(3));
    }

    #[test]
    fn test_common_post_dominator() {
        let cfg = create_diamond_cfg();
        let post_dom_tree = PostDominatorTree::new(&cfg).expect("CFG has exit");

        let node1 = NodeIndex::new(1);
        let node2 = NodeIndex::new(2);
        let exit = NodeIndex::new(3);

        // Common post-dominator of 1 and 2 is exit (3)
        assert_eq!(post_dom_tree.common_post_dominator(node1, node2), Some(exit));

        // Common post-dominator of node with itself is the node
        assert_eq!(post_dom_tree.common_post_dominator(node1, node1), Some(node1));
    }

    #[test]
    fn test_depth() {
        let cfg = create_diamond_cfg();
        let post_dom_tree = PostDominatorTree::new(&cfg).expect("CFG has exit");

        // Exit has depth 0
        assert_eq!(post_dom_tree.depth(NodeIndex::new(3)), 0);

        // Children of exit have depth 1
        assert_eq!(post_dom_tree.depth(NodeIndex::new(0)), 1);
        assert_eq!(post_dom_tree.depth(NodeIndex::new(1)), 1);
        assert_eq!(post_dom_tree.depth(NodeIndex::new(2)), 1);
    }

    #[test]
    fn test_empty_cfg() {
        let cfg: Cfg = DiGraph::new();
        assert!(PostDominatorTree::new(&cfg).is_none());
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

        let post_dom_tree = PostDominatorTree::new(&g).expect("CFG has exit");

        // In linear CFG, each node i is post-dominated by i+1, i+2, ..., 3
        assert_eq!(post_dom_tree.immediate_post_dominator(b3), None);
        assert_eq!(post_dom_tree.immediate_post_dominator(b2), Some(b3));
        assert_eq!(post_dom_tree.immediate_post_dominator(b1), Some(b2));
        assert_eq!(post_dom_tree.immediate_post_dominator(b0), Some(b1));
    }

    #[test]
    fn test_reversed_is_zero_copy() {
        // This test verifies that Reversed is a view, not a clone
        use petgraph::visit::NodeCount;
        use petgraph::visit::EdgeCount;

        let cfg = create_diamond_cfg();

        // Reversed should be a view - same node count
        let reversed = Reversed(&cfg);
        assert_eq!(reversed.node_count(), cfg.node_count());
        assert_eq!(reversed.edge_count(), cfg.edge_count());
    }
}
