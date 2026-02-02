//! Dominance frontier computation
//!
//! The dominance frontier of a node n is the set of nodes where
//! dominance from n ends. These are join points in the CFG where
//! control flow from n's dominated region meets control from outside.
//!
//! Dominance frontiers are used for:
//! - SSA phi-node placement (where variables merge from multiple paths)
//! - Control dependence analysis
//! - Identifying join points in control flow

use crate::cfg::Cfg;
use crate::cfg::dominators::DominatorTree;
use petgraph::graph::NodeIndex;
use std::collections::{HashSet, HashMap};

/// Dominance frontiers for all nodes in a CFG
///
/// The dominance frontier of node n is the set of nodes v such that:
/// - n dominates a predecessor of v, AND
/// - n does NOT strictly dominate v
///
/// Intuitively, these are the "join points" where control from n's
/// region meets control from outside n's region.
///
/// # Example
/// ```rust,no_run
/// # use mirage::cfg::dominance_frontiers::DominanceFrontiers;
/// # use mirage::cfg::dominators::DominatorTree;
/// # use mirage::cfg::Cfg;
/// # use petgraph::graph::NodeIndex;
/// # let graph: Cfg = unimplemented!();
/// # let dom_tree = DominatorTree::new(&graph).unwrap();
/// # let frontiers = DominanceFrontiers::new(&graph, dom_tree);
/// # let some_node = NodeIndex::new(0);
/// for node in frontiers.frontier(some_node) {
///     println!("{:?} is in {:?}'s dominance frontier", node, some_node);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct DominanceFrontiers {
    /// Dominance frontier for each node
    frontiers: HashMap<NodeIndex, HashSet<NodeIndex>>,
    /// Dominator tree for dominance queries
    dominator_tree: DominatorTree,
}

impl DominanceFrontiers {
    /// Compute dominance frontiers for all nodes using Cytron et al. algorithm
    ///
    /// Algorithm (Cytron et al. 1991):
    /// 1. Process nodes in reverse post-order of dominator tree
    /// 2. For each node n:
    ///    a. Add v where n dominates pred(v) but not v
    ///    b. Union with children's frontiers (excluding strict dominators)
    ///
    /// Complexity: O(|V|²) for worst-case CFG
    ///
    /// # Example
    /// ```rust,no_run
    /// # use mirage::cfg::dominance_frontiers::DominanceFrontiers;
    /// # use mirage::cfg::dominators::DominatorTree;
    /// # use mirage::cfg::Cfg;
    /// # let graph: Cfg = unimplemented!();
    /// # let dom_tree = DominatorTree::new(&graph).unwrap();
    /// let frontiers = DominanceFrontiers::new(&graph, dom_tree);
    /// ```
    pub fn new(cfg: &Cfg, dominator_tree: DominatorTree) -> Self {
        let mut frontiers: HashMap<NodeIndex, HashSet<NodeIndex>> = HashMap::new();

        // Process nodes in reverse post-order (deep nodes first)
        // We'll use depth in dominator tree as approximation
        let mut nodes: Vec<NodeIndex> = cfg.node_indices().collect();
        nodes.sort_by_key(|&n| std::cmp::Reverse(dominator_tree.depth(n)));

        // Compute frontier for each node
        for &n in &nodes {
            let mut df = HashSet::new();

            // Rule 1: Strict dominance boundary
            // For each node v, check if n dominates a predecessor of v
            // but does NOT strictly dominate v itself
            for &v in &nodes {
                for p in cfg.neighbors_directed(v, petgraph::Direction::Incoming) {
                    if dominator_tree.dominates(n, p)
                        && !dominator_tree.strictly_dominates(n, v)
                    {
                        df.insert(v);
                    }
                }
            }

            // Rule 2: Propagate children's frontiers
            // If child c's frontier contains v and n does not strictly dominate v
            // then v is in n's dominance frontier
            for &child in dominator_tree.children(n) {
                if let Some(child_df) = frontiers.get(&child) {
                    for &v in child_df {
                        if !dominator_tree.strictly_dominates(n, v) {
                            df.insert(v);
                        }
                    }
                }
            }

            frontiers.insert(n, df);
        }

        Self { frontiers, dominator_tree }
    }

    /// Get dominance frontier for a node
    ///
    /// Returns empty set if node has no dominance frontier
    /// (e.g., node that doesn't dominate any branching code).
    ///
    /// # Example
    /// ```rust,no_run
    /// # use mirage::cfg::dominance_frontiers::DominanceFrontiers;
    /// # use mirage::cfg::dominators::DominatorTree;
    /// # use mirage::cfg::Cfg;
    /// # use petgraph::graph::NodeIndex;
    /// # let graph: Cfg = unimplemented!();
    /// # let dom_tree = DominatorTree::new(&graph).unwrap();
    /// # let frontiers = DominanceFrontiers::new(&graph, dom_tree);
    /// # let node = NodeIndex::new(0);
    /// let df = frontiers.frontier(node);
    /// println!("Node {:?} has {} nodes in its dominance frontier", node, df.len());
    /// ```
    pub fn frontier(&self, node: NodeIndex) -> HashSet<NodeIndex> {
        self.frontiers.get(&node).cloned().unwrap_or_default()
    }

    /// Check if `v` is in `n`'s dominance frontier
    ///
    /// Returns true if v is a join point where n's dominance ends.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use mirage::cfg::dominance_frontiers::DominanceFrontiers;
    /// # use mirage::cfg::dominators::DominatorTree;
    /// # use mirage::cfg::Cfg;
    /// # use petgraph::graph::NodeIndex;
    /// # let graph: Cfg = unimplemented!();
    /// # let dom_tree = DominatorTree::new(&graph).unwrap();
    /// # let frontiers = DominanceFrontiers::new(&graph, dom_tree);
    /// # let n = NodeIndex::new(0);
    /// # let v = NodeIndex::new(1);
    /// if frontiers.in_frontier(n, v) {
    ///     println!("{:?} is where {:?}'s dominance ends", v, n);
    /// }
    /// ```
    pub fn in_frontier(&self, n: NodeIndex, v: NodeIndex) -> bool {
        self.frontiers
            .get(&n)
            .map(|set| set.contains(&v))
            .unwrap_or(false)
    }

    /// Get reference to the underlying dominator tree
    ///
    /// Useful for additional dominance queries.
    pub fn dominator_tree(&self) -> &DominatorTree {
        &self.dominator_tree
    }

    /// Get all nodes with non-empty dominance frontiers
    ///
    /// These are the nodes that have join points in their dominated regions.
    pub fn nodes_with_frontiers(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.frontiers.iter()
            .filter(|(_, df)| !df.is_empty())
            .map(|(&n, _)| n)
    }

    /// Find the iterated dominance frontier
    ///
    /// The iterated dominance frontier is the closure of the dominance
    /// frontier under the dominance relation. Used for placing phi nodes
    /// in SSA construction.
    ///
    /// Algorithm: Iteratively add frontier nodes until fixed point.
    pub fn iterated_frontier(&self, nodes: &[NodeIndex]) -> HashSet<NodeIndex> {
        let mut result = HashSet::new();
        let mut worklist: Vec<NodeIndex> = nodes.to_vec();

        while let Some(n) = worklist.pop() {
            for v in self.frontier(n) {
                if result.insert(v) {
                    worklist.push(v);
                }
            }
        }

        result
    }

    /// Compute dominance frontier for a set of nodes
    ///
    /// Returns the union of each node's dominance frontier.
    pub fn union_frontier(&self, nodes: &[NodeIndex]) -> HashSet<NodeIndex> {
        nodes.iter()
            .flat_map(|&n| self.frontier(n))
            .collect()
    }
}

/// Convenience function to compute dominance frontiers
///
/// This is a shorthand for DominanceFrontiers::new().
///
/// # Example
/// ```rust,no_run
/// # use mirage::cfg::dominance_frontiers::compute_dominance_frontiers;
/// # use mirage::cfg::dominators::DominatorTree;
/// # use mirage::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// # let dom_tree = DominatorTree::new(&graph).unwrap();
/// let frontiers = compute_dominance_frontiers(&graph, dom_tree);
/// ```
pub fn compute_dominance_frontiers(cfg: &Cfg, dominator_tree: DominatorTree) -> DominanceFrontiers {
    DominanceFrontiers::new(cfg, dominator_tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, BlockKind, Terminator, EdgeType};
    use crate::cfg::dominators::DominatorTree;
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

    /// Create CFG with loop:
    ///     0 (entry)
    ///     |
    ///     1 (header: if condition)
    ///    / \
    ///   2   3 (exit)
    ///   |
    ///   1 (back edge)
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
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 3 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec!["loop body".to_string()],
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
    fn test_dominance_frontiers_diamond() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");
        let frontiers = DominanceFrontiers::new(&cfg, dom_tree);

        // In diamond CFG:
        // Entry (0) dominates all nodes (0, 1, 2, 3)
        // Entry (0) strictly dominates 1, 2, 3
        // So DF[0] is empty (0 strictly dominates every node it dominates)
        let df0 = frontiers.frontier(NodeIndex::new(0));
        assert!(df0.is_empty());

        // Node 1 dominates itself, and pred(3) includes 1
        // 1 does NOT strictly dominate 3
        // So DF[1] = {3}
        let df1 = frontiers.frontier(NodeIndex::new(1));
        assert!(df1.contains(&NodeIndex::new(3)));
        assert_eq!(df1.len(), 1);

        // Similarly, DF[2] = {3}
        let df2 = frontiers.frontier(NodeIndex::new(2));
        assert!(df2.contains(&NodeIndex::new(3)));
        assert_eq!(df2.len(), 1);
    }

    #[test]
    fn test_dominance_frontiers_loop() {
        let cfg = create_loop_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");
        let frontiers = DominanceFrontiers::new(&cfg, dom_tree);

        // Entry (0) dominates everything, DF[0] is empty
        assert!(frontiers.frontier(NodeIndex::new(0)).is_empty());

        // Loop header (1) dominates 2 (loop body)
        // DF[1] contains 1 because:
        // - pred(1) includes 2 (back edge from loop body)
        // - 1 dominates 2
        // - 1 does NOT strictly dominate 1 (itself)
        // This is the "self-frontier" that characterizes loop headers
        let df1 = frontiers.frontier(NodeIndex::new(1));
        assert!(df1.contains(&NodeIndex::new(1)));

        // Loop body (2) dominates itself
        // DF[2] should contain 1 because:
        // - pred(1) includes 2 (back edge)
        // - 2 dominates 2 (itself)
        // - 2 does NOT strictly dominate 1 (2 doesn't dominate 1 at all)
        let df2 = frontiers.frontier(NodeIndex::new(2));
        assert!(df2.contains(&NodeIndex::new(1)));
    }

    #[test]
    fn test_in_frontier() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");
        let frontiers = DominanceFrontiers::new(&cfg, dom_tree);

        // 3 is in 1's dominance frontier (1 dominates itself, pred of 3)
        assert!(frontiers.in_frontier(NodeIndex::new(1), NodeIndex::new(3)));

        // 3 is in 2's dominance frontier (2 dominates itself, pred of 3)
        assert!(frontiers.in_frontier(NodeIndex::new(2), NodeIndex::new(3)));

        // 3 is NOT in 0's dominance frontier (0 strictly dominates 3)
        assert!(!frontiers.in_frontier(NodeIndex::new(0), NodeIndex::new(3)));
    }

    #[test]
    fn test_nodes_with_frontiers() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");
        let frontiers = DominanceFrontiers::new(&cfg, dom_tree);

        let nodes: Vec<_> = frontiers.nodes_with_frontiers().collect();
        // Nodes 1 and 2 should have non-empty frontiers in diamond CFG
        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains(&NodeIndex::new(1)));
        assert!(nodes.contains(&NodeIndex::new(2)));
    }

    #[test]
    fn test_iterated_frontier() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");
        let frontiers = DominanceFrontiers::new(&cfg, dom_tree);

        // Iterated frontier of {1} should include {3}
        let idf = frontiers.iterated_frontier(&[NodeIndex::new(1)]);
        assert!(idf.contains(&NodeIndex::new(3)));

        // Iterated frontier of {2} should include {3}
        let idf = frontiers.iterated_frontier(&[NodeIndex::new(2)]);
        assert!(idf.contains(&NodeIndex::new(3)));

        // Iterated frontier of {0} should be empty (0 has no frontier)
        let idf = frontiers.iterated_frontier(&[NodeIndex::new(0)]);
        assert!(idf.is_empty());
    }

    #[test]
    fn test_union_frontier() {
        let cfg = create_diamond_cfg();
        let dom_tree = DominatorTree::new(&cfg).expect("CFG has entry");
        let frontiers = DominanceFrontiers::new(&cfg, dom_tree);

        // Union frontier of {0, 1} should include {3}
        let union = frontiers.union_frontier(&[NodeIndex::new(0), NodeIndex::new(1)]);
        assert!(union.contains(&NodeIndex::new(3)));
    }

    #[test]
    fn test_linear_cfg() {
        // Linear CFG: 0 -> 1 -> 2 -> 3
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
        let frontiers = DominanceFrontiers::new(&g, dom_tree);

        // Linear CFG has no dominance frontiers (no join points)
        for node in g.node_indices() {
            assert!(frontiers.frontier(node).is_empty(),
                "Linear CFG should have empty dominance frontiers");
        }
    }

    #[test]
    fn test_complex_join() {
        // Create CFG with multiple join points:
        //       0
        //      /|\
        //     1 2 3
        //     \|/ |
        //      4  |
        //      \ /
        //       5
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![1, 2], otherwise: 3 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 4 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 4 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 5 },
            source_location: None,
        });

        let b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 5 },
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
        g.add_edge(b0, b2, EdgeType::Fallthrough);
        g.add_edge(b0, b3, EdgeType::Fallthrough);
        g.add_edge(b1, b4, EdgeType::Fallthrough);
        g.add_edge(b2, b4, EdgeType::Fallthrough);
        g.add_edge(b3, b5, EdgeType::Fallthrough);
        g.add_edge(b4, b5, EdgeType::Fallthrough);

        let dom_tree = DominatorTree::new(&g).expect("CFG has entry");
        let frontiers = DominanceFrontiers::new(&g, dom_tree);

        // In this complex CFG:
        // Node 0 dominates all nodes (0, 1, 2, 3, 4, 5)
        // So DF[0] is empty (0 strictly dominates all nodes)

        // Node 1 dominates itself, and pred(4) includes 1
        // 1 does NOT strictly dominate 4
        // So DF[1] = {4}
        let df1 = frontiers.frontier(b1);
        assert!(df1.contains(&b4));

        // Similarly, DF[2] = {4}
        let df2 = frontiers.frontier(b2);
        assert!(df2.contains(&b4));

        // DF[3] = {5} (3 dominates itself, pred of 5, and doesn't strictly dominate 5)
        let df3 = frontiers.frontier(b3);
        assert!(df3.contains(&b5));

        // DF[4] = {5} (4 dominates itself, pred of 5, and doesn't strictly dominate 5)
        let df4 = frontiers.frontier(b4);
        assert!(df4.contains(&b5));
    }

    #[test]
    fn test_empty_cfg() {
        let cfg: Cfg = DiGraph::new();
        let dom_tree = DominatorTree::new(&cfg);
        assert!(dom_tree.is_none());
    }
}
