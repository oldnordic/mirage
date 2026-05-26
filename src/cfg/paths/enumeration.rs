use crate::cfg::{BlockId, Cfg};
use petgraph::graph::NodeIndex;
use std::collections::{BTreeSet, HashMap, HashSet};

use super::{classify_path_precomputed, EnumerationContext, Path, PathLimits};

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

    let mut paths = Vec::new();
    let mut current_path = Vec::new();
    let mut visited = HashSet::new();
    let mut loop_iterations: HashMap<NodeIndex, usize> = HashMap::new();

    dfs_enumerate_with_context(
        cfg,
        entry,
        limits,
        &mut paths,
        &mut current_path,
        &mut visited,
        ctx,
        &mut loop_iterations,
    );

    paths
}

#[allow(clippy::too_many_arguments)]
fn dfs_enumerate_with_context(
    cfg: &Cfg,
    current: NodeIndex,
    limits: &PathLimits,
    paths: &mut Vec<Path>,
    current_path: &mut Vec<BlockId>,
    visited: &mut HashSet<NodeIndex>,
    ctx: &EnumerationContext,
    loop_iterations: &mut HashMap<NodeIndex, usize>,
) {
    let block_id = match cfg.node_weight(current) {
        Some(block) => block.id,
        None => return,
    };

    current_path.push(block_id);

    if current_path.len() > limits.max_length {
        current_path.pop();
        return;
    }

    if ctx.is_exit(current) {
        let kind = classify_path_precomputed(cfg, current_path, &ctx.reachable_blocks);
        let path = Path::new(current_path.clone(), kind);
        paths.push(path);
        current_path.pop();
        return;
    }

    if paths.len() >= limits.max_paths {
        current_path.pop();
        return;
    }

    if visited.contains(&current) && !ctx.is_loop_header(current) {
        current_path.pop();
        return;
    }

    visited.insert(current);

    let is_loop_header = ctx.is_loop_header(current);
    if is_loop_header {
        let count = loop_iterations.entry(current).or_insert(0);
        if *count >= limits.loop_unroll_limit {
            visited.remove(&current);
            current_path.pop();
            return;
        }
        *count += 1;
    }

    let neighbors: Vec<_> = cfg.neighbors(current).collect();

    for next in neighbors {
        dfs_enumerate_with_context(
            cfg,
            next,
            limits,
            paths,
            current_path,
            visited,
            ctx,
            loop_iterations,
        );
    }

    if is_loop_header {
        if let Some(count) = loop_iterations.get_mut(&current) {
            *count = count.saturating_sub(1);
        }
    }

    visited.remove(&current);
    current_path.pop();
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

    let mut paths = Vec::new();
    let mut current_path = Vec::new();
    let mut visited = HashSet::new();

    let loop_headers = crate::cfg::loops::find_loop_headers(cfg);
    let mut loop_iterations: HashMap<NodeIndex, usize> = HashMap::new();

    dfs_enumerate(
        cfg,
        entry,
        &exits,
        limits,
        &mut paths,
        &mut current_path,
        &mut visited,
        &loop_headers,
        &mut loop_iterations,
        &reachable_blocks,
    );

    paths
}

#[allow(clippy::too_many_arguments)]
fn dfs_enumerate(
    cfg: &Cfg,
    current: NodeIndex,
    exits: &HashSet<NodeIndex>,
    limits: &PathLimits,
    paths: &mut Vec<Path>,
    current_path: &mut Vec<BlockId>,
    visited: &mut HashSet<NodeIndex>,
    loop_headers: &HashSet<NodeIndex>,
    loop_iterations: &mut HashMap<NodeIndex, usize>,
    reachable_blocks: &HashSet<BlockId>,
) {
    let block_id = match cfg.node_weight(current) {
        Some(block) => block.id,
        None => return,
    };

    current_path.push(block_id);

    if current_path.len() > limits.max_length {
        current_path.pop();
        return;
    }

    if exits.contains(&current) {
        let kind = classify_path_precomputed(cfg, current_path, reachable_blocks);
        let path = Path::new(current_path.clone(), kind);
        paths.push(path);
        current_path.pop();
        return;
    }

    if paths.len() >= limits.max_paths {
        current_path.pop();
        return;
    }

    let is_loop_header = loop_headers.contains(&current);
    if is_loop_header {
        let count = loop_iterations.entry(current).or_insert(0);
        if *count >= limits.loop_unroll_limit {
            current_path.pop();
            return;
        }
        *count += 1;
    }

    let was_visited = visited.insert(current);

    let mut successors: Vec<NodeIndex> = cfg.neighbors(current).collect();
    successors.sort_by_key(|n| n.index());

    if successors.is_empty() {
        let kind = classify_path_precomputed(cfg, current_path, reachable_blocks);
        let path = Path::new(current_path.clone(), kind);
        paths.push(path);
    } else {
        for succ in successors {
            let is_back_edge = loop_headers.contains(&succ) && loop_iterations.contains_key(&succ);
            if visited.contains(&succ) && !is_back_edge {
                continue;
            }

            if is_back_edge {
                let count = loop_iterations.get(&succ).copied().unwrap_or(0);
                if count >= limits.loop_unroll_limit {
                    continue;
                }
            }

            dfs_enumerate(
                cfg,
                succ,
                exits,
                limits,
                paths,
                current_path,
                visited,
                loop_headers,
                loop_iterations,
                reachable_blocks,
            );

            if paths.len() >= limits.max_paths {
                break;
            }
        }
    }

    if was_visited {
        visited.remove(&current);
    }

    if is_loop_header {
        loop_iterations.entry(current).and_modify(|c| *c -= 1);
    }

    current_path.pop();
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
