use crate::cfg::{BlockId, Cfg};
use petgraph::graph::NodeIndex;
use std::collections::{BTreeSet, HashMap, HashSet};

use super::{classify_path_precomputed, EnumerationContext, Path, PathLimits};

struct ContextPathEnumerator<'a> {
    cfg: &'a Cfg,
    limits: &'a PathLimits,
    ctx: &'a EnumerationContext,
    paths: Vec<Path>,
    current_path: Vec<BlockId>,
    visited: HashSet<NodeIndex>,
    loop_iterations: HashMap<NodeIndex, usize>,
    /// Set once `limits.max_paths` is reached. Checked at DFS entry and in the
    /// successor loop so post-saturation recursion unwinds in O(depth) instead
    /// of walking the full exponential recursion tree (the 570s `risk` hang).
    saturated: bool,
    /// DFS node visits so far; compared against `limits.max_visits`.
    visits: usize,
    /// Set when saturation was caused by the work budget, not the path cap.
    budget_hit: bool,
}

impl<'a> ContextPathEnumerator<'a> {
    fn new(cfg: &'a Cfg, limits: &'a PathLimits, ctx: &'a EnumerationContext) -> Self {
        Self {
            cfg,
            limits,
            ctx,
            paths: Vec::new(),
            current_path: Vec::new(),
            visited: HashSet::new(),
            loop_iterations: HashMap::new(),
            saturated: false,
            visits: 0,
            budget_hit: false,
        }
    }

    fn enumerate(mut self, entry: NodeIndex) -> (Vec<Path>, bool, bool) {
        self.dfs(entry);
        (self.paths, self.saturated, self.budget_hit)
    }

    fn dfs(&mut self, current: NodeIndex) {
        if self.saturated {
            return;
        }
        self.visits += 1;
        if self.visits > self.limits.max_visits {
            // Work budget exhausted: stop immediately. `max_paths` alone does
            // not bound runtime when complete paths are rare among walks.
            self.saturated = true;
            self.budget_hit = true;
            return;
        }
        let block_id = match self.cfg.node_weight(current) {
            Some(block) => block.id,
            None => return,
        };

        self.current_path.push(block_id);

        if self.current_path.len() > self.limits.max_length {
            self.current_path.pop();
            return;
        }

        if self.ctx.is_exit(current) {
            let kind =
                classify_path_precomputed(self.cfg, &self.current_path, &self.ctx.reachable_blocks);
            self.paths.push(Path::new(self.current_path.clone(), kind));
            if self.paths.len() >= self.limits.max_paths {
                self.saturated = true;
            }
            self.current_path.pop();
            return;
        }

        if self.saturated {
            self.current_path.pop();
            return;
        }

        if self.visited.contains(&current) && !self.ctx.is_loop_header(current) {
            self.current_path.pop();
            return;
        }

        self.visited.insert(current);

        let is_loop_header = self.ctx.is_loop_header(current);
        if is_loop_header {
            let count = self.loop_iterations.entry(current).or_insert(0);
            if *count >= self.limits.loop_unroll_limit {
                self.visited.remove(&current);
                self.current_path.pop();
                return;
            }
            *count += 1;
        }

        let neighbors: Vec<_> = self.cfg.neighbors(current).collect();
        for next in neighbors {
            if self.saturated {
                break;
            }
            self.dfs(next);
        }

        if is_loop_header {
            if let Some(count) = self.loop_iterations.get_mut(&current) {
                *count = count.saturating_sub(1);
            }
        }

        self.visited.remove(&current);
        self.current_path.pop();
    }
}

struct PathEnumerator<'a> {
    cfg: &'a Cfg,
    exits: HashSet<NodeIndex>,
    limits: &'a PathLimits,
    loop_headers: HashSet<NodeIndex>,
    reachable_blocks: HashSet<BlockId>,
    paths: Vec<Path>,
    current_path: Vec<BlockId>,
    visited: HashSet<NodeIndex>,
    loop_iterations: HashMap<NodeIndex, usize>,
    /// Set once `limits.max_paths` is reached; short-circuits the remaining DFS.
    saturated: bool,
    /// DFS node visits so far; compared against `limits.max_visits`.
    visits: usize,
    /// Set when saturation was caused by the work budget, not the path cap.
    budget_hit: bool,
}

impl<'a> PathEnumerator<'a> {
    fn new(
        cfg: &'a Cfg,
        exits: HashSet<NodeIndex>,
        limits: &'a PathLimits,
        loop_headers: HashSet<NodeIndex>,
        reachable_blocks: HashSet<BlockId>,
    ) -> Self {
        Self {
            cfg,
            exits,
            limits,
            loop_headers,
            reachable_blocks,
            paths: Vec::new(),
            current_path: Vec::new(),
            visited: HashSet::new(),
            loop_iterations: HashMap::new(),
            saturated: false,
            visits: 0,
            budget_hit: false,
        }
    }

    fn enumerate(mut self, entry: NodeIndex) -> (Vec<Path>, bool, bool) {
        self.dfs(entry);
        (self.paths, self.saturated, self.budget_hit)
    }

    fn dfs(&mut self, current: NodeIndex) {
        if self.saturated {
            return;
        }
        self.visits += 1;
        if self.visits > self.limits.max_visits {
            self.saturated = true;
            self.budget_hit = true;
            return;
        }
        let block_id = match self.cfg.node_weight(current) {
            Some(block) => block.id,
            None => return,
        };

        self.current_path.push(block_id);

        if self.current_path.len() > self.limits.max_length {
            self.current_path.pop();
            return;
        }

        if self.exits.contains(&current) {
            let kind =
                classify_path_precomputed(self.cfg, &self.current_path, &self.reachable_blocks);
            self.paths.push(Path::new(self.current_path.clone(), kind));
            if self.paths.len() >= self.limits.max_paths {
                self.saturated = true;
            }
            self.current_path.pop();
            return;
        }

        if self.saturated {
            self.current_path.pop();
            return;
        }

        let is_loop_header = self.loop_headers.contains(&current);
        if is_loop_header {
            let count = self.loop_iterations.entry(current).or_insert(0);
            if *count >= self.limits.loop_unroll_limit {
                self.current_path.pop();
                return;
            }
            *count += 1;
        }

        let was_visited = self.visited.insert(current);

        let mut successors: Vec<NodeIndex> = self.cfg.neighbors(current).collect();
        successors.sort_by_key(|n| n.index());

        if successors.is_empty() {
            let kind =
                classify_path_precomputed(self.cfg, &self.current_path, &self.reachable_blocks);
            self.paths.push(Path::new(self.current_path.clone(), kind));
            if self.paths.len() >= self.limits.max_paths {
                self.saturated = true;
            }
        } else {
            for succ in successors {
                let is_back_edge =
                    self.loop_headers.contains(&succ) && self.loop_iterations.contains_key(&succ);
                if self.visited.contains(&succ) && !is_back_edge {
                    continue;
                }

                if is_back_edge {
                    let count = self.loop_iterations.get(&succ).copied().unwrap_or(0);
                    if count >= self.limits.loop_unroll_limit {
                        continue;
                    }
                }

                self.dfs(succ);

                if self.saturated {
                    break;
                }
            }
        }

        if was_visited {
            self.visited.remove(&current);
        }

        if is_loop_header {
            self.loop_iterations
                .entry(current)
                .and_modify(|count| *count = count.saturating_sub(1));
        }

        self.current_path.pop();
    }
}

/// Result of a bounded path enumeration.
///
/// `truncated` is true when enumeration stopped early because
/// `PathLimits::max_paths` was reached — in that case `paths` is a prefix
/// sample, not the complete set, and callers must not present its length as
/// the true path count.
#[derive(Debug, Clone)]
pub struct PathEnumeration {
    pub paths: Vec<Path>,
    /// True when enumeration stopped early for any reason (path cap or work
    /// budget); `paths` is then a sample, not the complete set.
    pub truncated: bool,
    /// True when the `max_visits` work budget (not the `max_paths` cap) caused
    /// the truncation — i.e. the CFG is too dense/loopy for deeper exact
    /// enumeration within the budget.
    pub budget_exhausted: bool,
}

pub fn enumerate_paths_with_context(
    cfg: &Cfg,
    limits: &PathLimits,
    ctx: &EnumerationContext,
) -> Vec<Path> {
    enumerate_paths_with_context_outcome(cfg, limits, ctx).paths
}

/// Bounded enumeration with honest truncation reporting.
///
/// Always terminates: once `max_paths` paths have been collected the DFS
/// unwinds immediately (a `saturated` flag short-circuits every remaining
/// recursive call), so even a 2^N-path CFG costs only as much as collecting
/// the first `max_paths` paths.
pub fn enumerate_paths_with_context_outcome(
    cfg: &Cfg,
    limits: &PathLimits,
    ctx: &EnumerationContext,
) -> PathEnumeration {
    let entry = match crate::cfg::analysis::find_entry(cfg) {
        Some(e) => e,
        None => {
            return PathEnumeration {
                paths: vec![],
                truncated: false,
                budget_exhausted: false,
            }
        }
    };

    if ctx.exits.is_empty() {
        return PathEnumeration {
            paths: vec![],
            truncated: false,
            budget_exhausted: false,
        };
    }

    let (paths, truncated, budget_exhausted) =
        ContextPathEnumerator::new(cfg, limits, ctx).enumerate(entry);
    PathEnumeration {
        paths,
        truncated,
        budget_exhausted,
    }
}

pub fn enumerate_paths(cfg: &Cfg, limits: &PathLimits) -> Vec<Path> {
    enumerate_paths_outcome(cfg, limits).paths
}

/// Bounded enumeration (no pre-computed context) with truncation reporting.
pub fn enumerate_paths_outcome(cfg: &Cfg, limits: &PathLimits) -> PathEnumeration {
    let entry = match crate::cfg::analysis::find_entry(cfg) {
        Some(e) => e,
        None => {
            return PathEnumeration {
                paths: vec![],
                truncated: false,
                budget_exhausted: false,
            }
        }
    };

    let mut exits: HashSet<NodeIndex> = crate::cfg::analysis::find_exits(cfg).into_iter().collect();

    if exits.is_empty() {
        for node in cfg.node_indices() {
            if cfg.neighbors(node).next().is_none() {
                exits.insert(node);
            }
        }
    }

    if exits.is_empty() {
        return PathEnumeration {
            paths: vec![],
            truncated: false,
            budget_exhausted: false,
        };
    }

    let reachable_nodes = crate::cfg::reachability::find_reachable(cfg);
    let reachable_blocks: HashSet<BlockId> =
        reachable_nodes.iter().map(|&idx| cfg[idx].id).collect();

    let loop_headers = crate::cfg::loops::find_loop_headers(cfg);
    let (paths, truncated, budget_exhausted) =
        PathEnumerator::new(cfg, exits, limits, loop_headers, reachable_blocks).enumerate(entry);
    PathEnumeration {
        paths,
        truncated,
        budget_exhausted,
    }
}

/// Iterative DFS path enumeration (stack-based, no recursion)
///
/// Improved version of `enumerate_paths` that:
/// - Uses an explicit stack instead of recursion (prevents stack overflow)
/// - Performs early path deduplication (no duplicate paths stored)
/// - Tracks more path metadata during enumeration
pub fn enumerate_paths_iterative(cfg: &Cfg, limits: &PathLimits) -> Vec<Path> {
    let entry = match crate::cfg::analysis::find_entry(cfg) {
        Some(e) => e,
        None => return vec![],
    };

    let mut exits: HashSet<NodeIndex> = crate::cfg::analysis::find_exits(cfg).into_iter().collect();

    if exits.is_empty() {
        for node in cfg.node_indices() {
            if cfg.neighbors(node).next().is_none() {
                exits.insert(node);
            }
        }
    }

    if exits.is_empty() {
        return vec![];
    }

    let reachable_nodes = crate::cfg::reachability::find_reachable(cfg);
    let reachable_blocks: HashSet<BlockId> =
        reachable_nodes.iter().map(|&idx| cfg[idx].id).collect();
    let loop_headers = crate::cfg::loops::find_loop_headers(cfg);

    let mut seen_paths: BTreeSet<Vec<BlockId>> = BTreeSet::new();

    struct StackFrame {
        node: NodeIndex,
        path: Vec<BlockId>,
        visited: HashSet<NodeIndex>,
        loop_iterations: HashMap<NodeIndex, usize>,
    }

    let mut stack = Vec::new();
    let mut paths = Vec::new();
    let mut visits: usize = 0;

    let entry_block_id = cfg[entry].id;
    let mut initial_visited = HashSet::new();
    initial_visited.insert(entry);

    stack.push(StackFrame {
        node: entry,
        path: vec![entry_block_id],
        visited: initial_visited,
        loop_iterations: HashMap::new(),
    });

    while let Some(frame) = stack.pop() {
        let StackFrame {
            node: current,
            path: current_path,
            visited: current_visited,
            mut loop_iterations,
        } = frame;

        if current_path.len() > limits.max_length {
            continue;
        }

        visits += 1;
        if visits > limits.max_visits {
            // Work budget exhausted: stop; results are a truncated sample.
            break;
        }

        if exits.contains(&current) {
            if seen_paths.insert(current_path.clone()) {
                let kind = classify_path_precomputed(cfg, &current_path, &reachable_blocks);
                let path = Path::new(current_path, kind);
                paths.push(path);
            }
            continue;
        }

        if paths.len() >= limits.max_paths {
            break;
        }

        let mut successors: Vec<NodeIndex> = cfg.neighbors(current).collect();
        successors.sort_by_key(|n| n.index());

        for succ in successors {
            let is_back_edge = loop_headers.contains(&succ)
                && loop_iterations.get(&succ).copied().unwrap_or(0) < limits.loop_unroll_limit;

            if current_visited.contains(&succ) && !is_back_edge {
                continue;
            }

            if is_back_edge {
                let count = loop_iterations.entry(succ).or_insert(0);
                if *count >= limits.loop_unroll_limit {
                    continue;
                }
                *count += 1;
            }

            let mut new_path = current_path.clone();
            let block_id = cfg[succ].id;
            new_path.push(block_id);

            let mut new_visited = current_visited.clone();
            new_visited.insert(succ);

            stack.push(StackFrame {
                node: succ,
                path: new_path,
                visited: new_visited,
                loop_iterations: loop_iterations.clone(),
            });
        }
    }

    paths
}
