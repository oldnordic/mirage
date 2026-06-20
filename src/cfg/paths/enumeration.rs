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
        }
    }

    fn enumerate(mut self, entry: NodeIndex) -> Vec<Path> {
        self.dfs(entry);
        self.paths
    }

    fn dfs(&mut self, current: NodeIndex) {
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
            self.current_path.pop();
            return;
        }

        if self.paths.len() >= self.limits.max_paths {
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
        }
    }

    fn enumerate(mut self, entry: NodeIndex) -> Vec<Path> {
        self.dfs(entry);
        self.paths
    }

    fn dfs(&mut self, current: NodeIndex) {
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
            self.current_path.pop();
            return;
        }

        if self.paths.len() >= self.limits.max_paths {
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

                if self.paths.len() >= self.limits.max_paths {
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

pub fn enumerate_paths_with_context(
    cfg: &Cfg,
    limits: &PathLimits,
    ctx: &EnumerationContext,
) -> Vec<Path> {
    let entry = match crate::cfg::analysis::find_entry(cfg) {
        Some(e) => e,
        None => return vec![],
    };

    if ctx.exits.is_empty() {
        return vec![];
    }

    ContextPathEnumerator::new(cfg, limits, ctx).enumerate(entry)
}

pub fn enumerate_paths(cfg: &Cfg, limits: &PathLimits) -> Vec<Path> {
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
    PathEnumerator::new(cfg, exits, limits, loop_headers, reachable_blocks).enumerate(entry)
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
