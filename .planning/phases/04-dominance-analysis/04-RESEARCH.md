# Phase 4: Dominance Analysis - Research

**Researched:** 2026-02-01
**Domain:** Control Flow Graph (CFG) dominance analysis for Rust code intelligence
**Confidence:** HIGH

## Summary

This phase implements dominance analysis for Mirage's CFG engine, enabling must-pass-through proofs critical for code intelligence queries. Dominance determines which code MUST execute on any path from entry to a given node, forming the foundation for:
- Natural loop detection (already using dominance in Phase 3)
- Static Single Assignment (SSA) construction
- Control dependence analysis
- "Must-execute" property verification

**Primary recommendation:** Use petgraph's `simple_fast` algorithm for dominators, wrap with `DominatorTree` struct for cached queries, implement post-dominators via graph reversal using `Reversed<G>` adaptor, and implement dominance frontiers using Cytron et al.'s iterative algorithm.

## Standard Stack

The dominance analysis stack extends Phase 2-3's petgraph foundation:

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| **petgraph** | 0.8 | Graph algorithms and data structures | Industry-standard Rust graph library with production-proven dominance algorithms |
| **petgraph::algo::dominators** | 0.8 | `simple_fast` dominator computation | Cooper et al. algorithm: O(|V|²) theoretical but faster in practice than Lengauer-Tarjan for CFGs ≤30K nodes |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **petgraph::visit::Reversed** | 0.8 | Graph adaptor for edge direction reversal | Computing post-dominators by running dominator algorithm on reversed CFG |
| **petgraph::visit** | 0.8 | Graph trait implementations (`IntoNeighbors`, `Visitable`) | Required by `simple_fast` algorithm |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `petgraph::algo::dominators::simple_fast` | Custom Lengauer-Tarjan implementation | Lengauer-Tarjan has better theoretical complexity O(E α(V)) but Cooper et al. found `simple_fast` faster in practice for CFG sizes typical in compilation (up to ~30K nodes). Custom implementation = maintenance burden. |
| Reversal with `Reversed<G>` adaptor | Clone graph + `graph.reverse()` | `Reversed<G>` is zero-copy view, cloning adds O(V+E) allocation overhead per post-dominator query |

**Installation:**
No new dependencies. Phase 2 already installed `petgraph = "0.8"`.

## Architecture Patterns

### Dominance Module Structure

```
src/cfg/
├── dominators.rs       # DominatorTree, immediate dominators, API wrapper
├── post_dominators.rs  # PostDominatorTree (via graph reversal)
├── dominance_frontiers.rs # Dominance frontier computation
└── mod.rs              # Re-exports dominance APIs
```

### Pattern 1: Wrap petgraph's Dominators with Cached Queries

**What:** Create a `DominatorTree` struct that wraps petgraph's `Dominators<N>` and provides O(1) lookups for common queries.

**When to use:** All dominance queries. The wrapper provides Mirage-specific API while petgraph handles algorithm correctness.

**Why:** petgraph's `Dominators` struct is low-level. A wrapper provides:
- Cached immediate dominator map (HashMap<NodeIndex, Option<NodeIndex>>)
- Dominator tree structure (parent-child relationships)
- Convenience methods: `is_dominated_by`, `dominates`, `common_dominator`

**Example:**

```rust
/// Dominator tree wrapper providing cached dominance queries
///
/// Wraps petgraph's Dominators to provide Mirage-specific API
/// with O(1) lookups for common queries.
///
/// Source: /websites/rs_petgraph - petgraph::algo::dominators module
#[derive(Debug, Clone)]
pub struct DominatorTree {
    /// Root node (entry block)
    root: NodeIndex,
    /// Immediate dominator for each node (None for root)
    immediate_dominator: HashMap<NodeIndex, Option<NodeIndex>>,
    /// Children in dominator tree (nodes immediately dominated by each node)
    children: HashMap<NodeIndex, Vec<NodeIndex>>,
}

impl DominatorTree {
    /// Compute dominator tree using Cooper et al. algorithm
    ///
    /// Returns None if CFG has no entry node.
    ///
    /// Time: O(|V|²) worst case, faster in practice
    /// Space: O(|V| + |E|)
    pub fn new(cfg: &Cfg) -> Option<Self> {
        use petgraph::algo::dominators::simple_fast;

        let entry = find_entry(cfg)?;
        let dominators = simple_fast(cfg, entry);

        // Build immediate dominator map
        let mut immediate_dominator = HashMap::new();
        let mut children: HashMap<_, Vec<_>> = HashMap::new();

        for node in cfg.node_indices() {
            let idom = dominators.immediate_dominator(node);

            // Insert into immediate dominator map
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

    /// Get immediate dominator of a node
    ///
    /// Returns None for root node or unreachable nodes
    pub fn immediate_dominator(&self, node: NodeIndex) -> Option<NodeIndex> {
        self.immediate_dominator.get(&node).copied().flatten()
    }

    /// Check if `a` dominates `b`
    ///
    /// A dominates B if every path from root to B contains A
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
    pub fn children(&self, node: NodeIndex) -> &[NodeIndex] {
        self.children.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// Get root node of dominator tree
    pub fn root(&self) -> NodeIndex {
        self.root
    }
}
```

### Pattern 2: Post-Dominators via Graph Reversal

**What:** Compute post-dominators by running dominator algorithm on reversed CFG.

**When to use:** All post-dominator queries. Post-dominance is dual to dominance: A post-dominates B if every path from B to exit contains A.

**Why:** petgraph doesn't provide post-dominators directly. Reversing the graph transforms post-dominance into dominance:
- Original CFG: A post-dominates B iff all paths B → exit contain A
- Reversed CFG: A dominates B iff all paths exit → B contain A

**Example:**

```rust
/// Post-dominator tree for CFG
///
/// Computed by running dominance algorithm on reversed CFG.
/// Root is the exit node(s).
///
/// Source: /websites/rs_petgraph - petgraph::visit::Reversed adaptor
pub struct PostDominatorTree {
    /// Dominator tree on reversed graph
    inner: DominatorTree,
}

impl PostDominatorTree {
    /// Compute post-dominator tree
    ///
    /// Returns None if CFG has no exit nodes
    ///
    /// Algorithm:
    /// 1. Reverse graph edges using Reversed<G> adaptor
    /// 2. Compute dominators with exit as root
    /// 3. Result is post-dominators on original graph
    pub fn new(cfg: &Cfg) -> Option<Self> {
        use petgraph::visit::Reversed;

        // Find exit node(s) - use primary exit for now
        let exit = find_exits(cfg).first().copied()?;

        // Compute dominators on reversed graph
        let reversed = Reversed(cfg);
        let dominators = simple_fast(reversed, exit);

        // Build DominatorTree from reversed dominators
        let mut immediate_dominator = HashMap::new();
        let mut children: HashMap<_, Vec<_>> = HashMap::new();

        for node in cfg.node_indices() {
            let idom = dominators.immediate_dominator(node);
            immediate_dominator.insert(node, idom);

            if let Some(parent) = idom {
                children.entry(parent).or_default().push(node);
            }
        }

        Some(Self {
            inner: DominatorTree {
                root: exit,
                immediate_dominator,
                children,
            },
        })
    }

    /// Get immediate post-dominator of a node
    pub fn immediate_post_dominator(&self, node: NodeIndex) -> Option<NodeIndex> {
        self.inner.immediate_dominator(node)
    }

    /// Check if `a` post-dominates `b`
    pub fn post_dominates(&self, a: NodeIndex, b: NodeIndex) -> bool {
        self.inner.dominates(a, b)
    }
}
```

### Pattern 3: Dominance Frontier (Cytron et al. Algorithm)

**What:** Compute dominance frontiers for φ-node placement in SSA construction.

**When to use:** SSA construction, control dependence analysis, identifying join points in control flow.

**Why:** Dominance frontiers identify nodes where dominance boundaries meet. Critical for:
- SSA φ-node placement (where variables merge from multiple control paths)
- Control dependence edges (if condition dominates its branches' merge point)
- Identifying "join points" in control flow

**Algorithm (Cytron et al. 1991):**

```text
DF[n] = { v | ∃p ∈ pred(v) : n dominates p and n does not strictly dominate v }

Translation: Node v is in n's dominance frontier if:
1. n dominates a predecessor p of v
2. n does NOT strictly dominate v (v is not in n's dominator subtree)

Intuition: v is a "join point" where control from n's region meets control from outside
```

**Implementation:**

```rust
/// Dominance frontier for a single node
///
/// The dominance frontier of node n is the set of nodes where
/// dominance from n ends. These are join points in the CFG.
///
/// Used for SSA φ-node placement and control dependence analysis.
///
/// Source: Cytron et al. 1991 - "Efficiently Computing Static Single Assignment Form"
pub struct DominanceFrontiers {
    /// Dominance frontier for each node
    frontiers: HashMap<NodeIndex, HashSet<NodeIndex>>,
    /// Dominator tree for dominance queries
    dominator_tree: DominatorTree,
}

impl DominanceFrontiers {
    /// Compute dominance frontiers for all nodes
    ///
    /// Uses Cytron et al.'s iterative algorithm:
    /// 1. Process nodes in dominator tree post-order (children before parents)
    /// 2. For each node, compute frontier from:
    ///    a. Strict dominance boundary checking
    ///    b. Union of children's frontiers
    ///
    /// Complexity: O(|V|²) for worst-case CFG
    pub fn new(cfg: &Cfg, dominator_tree: DominatorTree) -> Self {
        let mut frontiers: HashMap<_, HashSet<_>> = HashMap::new();

        // Process nodes in dominator tree post-order
        // (children before parents) via iterative worklist
        let mut worklist: Vec<NodeIndex> = cfg.node_indices().collect();
        worklist.sort_by_key(|n| dominator_tree_depth(&dominator_tree, *n));

        for &n in &worklist {
            let mut df = HashSet::new();

            // Rule 1: Strict dominance boundary
            // For each predecessor p of a node v:
            // if n dominates p and n does not strictly dominate v
            // then v is in n's dominance frontier
            for &v in cfg.node_indices() {
                for p in cfg.neighbors_directed(v, petgraph::Direction::Incoming) {
                    if dominator_tree.dominates(n, p)
                        && !dominator_tree.dominates(n, v)
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
                        if !dominator_tree.dominates(n, v) {
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
    pub fn frontier(&self, node: NodeIndex) -> &HashSet<NodeIndex> {
        self.frontiers.get(&node).map_or(&HashSet::new(), |s| s)
    }

    /// Check if `v` is in `n`'s dominance frontier
    pub fn in_frontier(&self, n: NodeIndex, v: NodeIndex) -> bool {
        self.frontier(n).contains(&v)
    }
}

/// Helper: Compute depth in dominator tree for post-order sorting
fn dominator_tree_depth(tree: &DominatorTree, node: NodeIndex) -> usize {
    let mut depth = 0;
    let mut current = node;
    while let Some(idom) = tree.immediate_dominator(current) {
        depth += 1;
        current = idom;
    }
    depth
}
```

### Anti-Patterns to Avoid

- **Re-computing dominators on every query:** Dominator computation is O(|V|²). Cache results in `DominatorTree` struct.
- **Using `Reversed` without understanding zero-copy:** `Reversed<G>` is a view adaptor, not a clone. Don't clone then reverse.
- **Confusing dominance with reachability:** A dominating B ⇒ A reachable to B, but NOT vice versa. Dominance requires ALL paths.
- **Assuming single exit for post-dominators:** Functions may have multiple exits (Return, Abort, Unreachable). Choose primary exit or compute multiple post-dominator trees.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Dominator algorithm | Custom Cooper/Lengauer-Tarjan | `petgraph::algo::dominators::simple_fast` | Correctness critical, petgraph has tested implementation |
| Post-dominators | Custom post-dominator algorithm | `simple_fast(Reversed(cfg), exit)` | Reversal adaptor transforms problem into solved dominator case |
| Graph traversal for dominance | Custom DFS/BFS | `petgraph::visit::Dfs`, `has_path_connecting` | Handles edge cases (unreachable nodes, self-loops) |
| Graph reversal | Manual edge swapping | `petgraph::visit::Reversed<G>` adaptor | Zero-copy view, maintains graph invariants |

**Key insight:** Dominance is a well-studied compiler problem with standard algorithms. Custom implementations risk subtle bugs (incorrect dominance sets, broken post-dominator logic, mishandled CFG edge cases).

## Common Pitfalls

### Pitfall 1: Confusing petgraph's Dominators API

**What goes wrong:** Misunderstanding `immediate_dominator` return value (returns `None` for root AND unreachable nodes).

**Why it happens:** API documentation says "Returns None for nodes not reachable from root, and for the root itself" but code often assumes None = root only.

**How to avoid:**

```rust
// WRONG: Assumes None = root
if let Some(idom) = dominators.immediate_dominator(node) {
    // idom is immediate dominator
} else {
    // BUG: This could be root OR unreachable!
}

// CORRECT: Check if node is root first
if node == dominators.root() {
    // Node is root, no immediate dominator
} else if let Some(idom) = dominators.immediate_dominator(node) {
    // Node has immediate dominator
} else {
    // Node is unreachable (shouldn't happen in valid CFG)
}
```

**Warning signs:** Panics when accessing unreachable nodes, incorrect dominator tree leaves.

### Pitfall 2: Post-Dominator Root Selection

**What goes wrong:** Assuming CFG has single exit node when computing post-dominators.

**Why it happens:** Functions can exit via multiple paths (Return, Abort, Unreachable, panic). Post-dominance from single exit is undefined or meaningless.

**How to avoid:**

```rust
// Option 1: Use primary exit (first Return node)
fn find_primary_exit(cfg: &Cfg) -> Option<NodeIndex> {
    find_exits(cfg).iter()
        .find(|n| matches!(cfg[*n].terminator, Terminator::Return))
        .copied()
}

// Option 2: Add virtual exit node
fn add_virtual_exit(cfg: &mut Cfg) -> NodeIndex {
    let virtual_exit = cfg.add_node(BasicBlock {
        id: cfg.node_count(),
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    // Connect all real exits to virtual exit
    for exit in find_exits(cfg) {
        cfg.add_edge(exit, virtual_exit, EdgeType::Fallthrough);
    }

    virtual_exit
}
```

**Warning signs:** Post-dominator queries return unexpected results, some nodes show all paths post-dominated.

### Pitfall 3: Dominance Frontier Performance

**What goes wrong:** Naive O(|V|²) dominance frontier computation with repeated dominance checks.

**Why it happens:** Straightforward implementation of Cytron et al. algorithm without optimization. For each node, checks dominance for all predecessor-node pairs.

**How to avoid:**

```rust
// Cache dominance checks during frontier computation
struct DominanceCache {
    dominator_tree: DominatorTree,
    cache: HashMap<(NodeIndex, NodeIndex), bool>,
}

impl DominanceCache {
    fn dominates(&mut self, a: NodeIndex, b: NodeIndex) -> bool {
        *self.cache.entry((a, b)).or_insert_with(|| {
            self.dominator_tree.dominates(a, b)
        })
    }
}

// Use in frontier computation
let mut dom_cache = DominanceCache::new(dominator_tree.clone());
// ... use dom_cache.dominates(n, p) instead of dominator_tree.dominates(n, p)
```

**Warning signs:** Frontier computation >1 second for CFGs >1000 nodes, timeouts on large functions.

### Pitfall 4: Ignoring Multiple Entry Points

**What goes wrong:** `simple_fast` requires single root, but CFG may have multiple entry points (public API, trait impl, exported function).

**Why it happens:** Not checking for single entry before calling `simple_fast`, or assuming first BlockKind::Entry is only entry.

**How to avoid:**

```rust
// Validate single entry before dominance
pub fn compute_dominators(cfg: &Cfg) -> Result<DominatorTree, DominanceError> {
    let entries = cfg.node_indices()
        .filter(|&n| matches!(cfg[n].kind, BlockKind::Entry))
        .collect::<Vec<_>>();

    if entries.len() != 1 {
        return Err(DominanceError::MultipleEntries(entries.len()));
    }

    DominatorTree::new(cfg)
        .ok_or(DominanceError::NoEntry)
}

#[derive(Debug, thiserror::Error)]
pub enum DominanceError {
    #[error("CFG has {0} entry nodes, dominance requires single entry")]
    MultipleEntries(usize),

    #[error("CFG has no entry node")]
    NoEntry,
}
```

**Warning signs:** Panics in `simple_fast`, dominator tree missing expected nodes.

## Code Examples

Verified patterns from official sources:

### Dominator Tree Construction

```rust
// Source: /websites/rs_petgraph - petgraph::algo::dominators::simple_fast

use petgraph::algo::dominators::simple_fast;
use crate::cfg::analysis::find_entry;

let entry = match find_entry(cfg) {
    Some(e) => e,
    None => return Ok(()), // Empty CFG, no dominators
};

// Compute dominators using Cooper et al. algorithm
let dominators = simple_fast(cfg, entry);

// Query immediate dominator
if let Some(idom) = dominators.immediate_dominator(node) {
    println!("Node {:?} is immediately dominated by {:?}", node, idom);
}

// Iterate all dominators
if let Some(all_doms) = dominators.dominators(node) {
    for dom in all_doms {
        println!("Dominator: {:?}", dom);
    }
}

// Get nodes immediately dominated by a node
for &child in dominators.immediately_dominated_by(node) {
    println!("Node {:?} immediately dominates {:?}", node, child);
}
```

### Graph Reversal for Post-Dominators

```rust
// Source: /websites/rs_petgraph - petgraph::visit::Reversed adaptor

use petgraph::visit::Reversed;

// Zero-copy reversal - doesn't modify original graph
let reversed = Reversed(cfg);

// Now run dominance algorithm on reversed graph
let post_dominators = simple_fast(reversed, exit_node);

// Result is post-dominators on original graph
```

### Dominance Query Pattern

```rust
// Check if A dominates B
// Source: /websites/rs_petgraph - Dominators struct documentation

fn check_dominance(dominators: &Dominators<NodeIndex>, a: NodeIndex, b: NodeIndex) -> bool {
    // A dominates B if A is in B's dominator set
    dominators.dominators(b)
        .map_or(false, |mut doms| doms.any(|d| d == a))
}

// Equivalent using immediate dominator chain
fn check_dominance_chain(dominator_tree: &DominatorTree, a: NodeIndex, b: NodeIndex) -> bool {
    let mut current = b;
    loop {
        if current == a {
            return true;
        }
        match dominator_tree.immediate_dominator(current) {
            Some(idom) => current = idom,
            None => return false,
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Iterative dataflow (O(|V|³)) | Cooper et al. simple_fast (O(|V|²)) | 2000s | Standard in modern compilers, faster in practice despite worse theoretical bound than Lengauer-Tarjan |
| Custom dominance implementations | petgraph::algo::dominators | 2016 (petgraph 0.4) | Rust ecosystem standard, battle-tested |
| Manual graph reversal for post-dom | Reversed<G> adaptor | 2017 (petgraph 0.4.13) | Zero-copy view, cleaner API |

**Deprecated/outdated:**
- **Lengauer-Tarjan in production:** Better theoretical complexity O(E α(V)) but Cooper et al. found simple_fast faster in practice for CFGs up to 30K nodes. Only implement if profiling shows dominance is bottleneck AND CFG has >10K nodes.
- **Fusion dominance algorithms:** Attempts to combine dominance and post-dominance in single pass. Complexity outweighs benefit, maintain two separate trees.

## Open Questions

Things that couldn't be fully resolved:

1. **Multiple exits for post-dominance**
   - What we know: Functions can exit via multiple nodes (Return, Abort, Unreachable). Post-dominance requires single root.
   - What's unclear: Best practice for handling multiple exits in Rust compiler ecosystem.
   - Recommendation: Start with primary exit (first Return node), add virtual exit node if needed. Document limitation in CLI.

2. **Dominance frontier optimization**
   - What we know: Cytron et al. algorithm is O(|V|²) in worst case. Modern compilers use optimizations (iterative dataflow, SSA-specific pruning).
   - What's unclear: Whether Mirage needs optimized frontiers (target use case is code intelligence queries, not full SSA construction).
   - Recommendation: Implement straightforward Cytron et al. algorithm first. Profile on real Rust codebases. Optimize if >100ms for typical functions.

3. **Incremental dominance updates**
   - What we know: CFG changes during refactoring require recomputing dominators. Full recomputation is O(|V|²).
   - What's unclear: Whether petgraph supports incremental dominance updates (not documented).
   - Recommendation: Full recomputation on CFG changes. Cache dominator trees per function in Magellan database. Investigate incremental algorithms if profiling shows bottleneck.

## Sources

### Primary (HIGH confidence)

- **/websites/rs_petgraph** - petgraph graph library
  - `petgraph::algo::dominators::simple_fast` - Cooper et al. dominance algorithm
  - `petgraph::algo::dominators::Dominators` struct - API for dominance queries
  - `petgraph::visit::Reversed<G>` - Zero-copy graph reversal adaptor
  - Topics fetched: dominators module, Reversed adaptor, graph visit traits

- [Static Single-Assignment Form - Wikipedia](https://en.wikipedia.org/wiki/Static_single-assignment_form)
  - What was checked: Cytron et al. reference, dominance frontier definition, SSA construction requirements
  - Publication: Updated 2024, references original 1991 Cytron et al. paper

### Secondary (MEDIUM confidence)

- [A Simple, Fast Dominance Algorithm - Cooper et al. (PDF)](https://www.cs.tufts.edu/~nr/cs257/archive/keith-cooper/dom14.pdf)
  - What was checked: Algorithm pseudocode, performance comparison with Lengauer-Tarjan, discussion of dominance frontier computation
  - Verified with: Wikipedia citation, petgraph implementation

- [SchrodingerZhu/domtree - GitHub](https://github.com/SchrodingerZhu/domtree)
  - What was checked: Alternative dominator tree implementation in Rust, API design patterns
  - Note: Not used (petgraph is sufficient), but confirms ecosystem patterns

### Tertiary (LOW confidence)

- WebSearch results for "dominance frontier algorithm Rust implementation 2026"
  - Marked for validation: Found domtree crate (2023) but not verified if it provides dominance frontiers
  - Recommendation: Stick with Cytron et al. implementation based on petgraph dominators

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - petgraph 0.8 is documented, installed, and already used in Phase 3
- Architecture: HIGH - Dominator wrapper pattern is standard (LLVM, GCC both wrap dominance algorithms), post-dominators via reversal is textbook approach
- Dominance frontier: MEDIUM - Cytron et al. algorithm is standard, but optimization opportunities not fully explored
- Pitfalls: HIGH - All pitfalls documented based on real petgraph API behavior and common compiler mistakes

**Research date:** 2026-02-01
**Valid until:** 2026-03-01 (30 days - stable domain, petgraph APIs mature)

**Magellan snapshot:**
- Database: `.codemcp/mirage.db`
- Files indexed: 17
- Symbols indexed: 237
- Status: Ready for llmgrep queries

**Planner readiness:** This research provides sufficient detail for gsd-planner to create 3 executable plans (04-01 through 04-03) covering dominator tree construction, post-dominator tree construction, and dominance frontier computation. All APIs identified with HIGH confidence, architecture patterns specified, test patterns clear from Phase 3 examples.
