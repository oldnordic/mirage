use crate::cfg::{BlockId, Cfg, Terminator};
use std::collections::HashSet;

use super::find_node_by_block_id;
use super::PathKind;

pub fn is_feasible_path(cfg: &Cfg, blocks: &[BlockId]) -> bool {
    use crate::cfg::BlockKind;

    if blocks.is_empty() {
        return false;
    }

    let first_idx = match find_node_by_block_id(cfg, blocks[0]) {
        Some(idx) => idx,
        None => return false,
    };
    if cfg[first_idx].kind != BlockKind::Entry {
        return false;
    }

    let last_idx = match find_node_by_block_id(cfg, *blocks.last().unwrap()) {
        Some(idx) => idx,
        None => return false,
    };

    match &cfg[last_idx].terminator {
        Terminator::Return => {}
        Terminator::Abort(_) => {}
        Terminator::Call { unwind: None, .. } => {}
        Terminator::Call {
            unwind: Some(_),
            target: Some(_),
        } => {}
        Terminator::Unreachable
        | Terminator::Goto { .. }
        | Terminator::SwitchInt { .. }
        | Terminator::Call {
            unwind: Some(_),
            target: None,
        } => {
            return false;
        }
    }

    for &block_id in blocks.iter().skip(1).take(blocks.len().saturating_sub(2)) {
        if find_node_by_block_id(cfg, block_id).is_none() {
            return false;
        }
    }

    true
}

/// Check if a path is statically feasible using pre-computed reachable set
///
/// This is an optimized version of `is_feasible_path` for batch operations.
/// Instead of calling `is_reachable_from_entry` for each block (which is O(n)
/// per call), we use a pre-computed HashSet of reachable block IDs, making
/// reachability checks O(1).
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `blocks` - Block IDs in execution order
/// * `reachable_blocks` - Pre-computed set of reachable BlockIds
///
/// # Returns
///
/// `true` if the path is statically feasible, `false` otherwise
pub fn is_feasible_path_precomputed(
    cfg: &Cfg,
    blocks: &[BlockId],
    reachable_blocks: &HashSet<BlockId>,
) -> bool {
    use crate::cfg::BlockKind;

    if blocks.is_empty() {
        return false;
    }

    let first_idx = match find_node_by_block_id(cfg, blocks[0]) {
        Some(idx) => idx,
        None => return false,
    };
    if cfg[first_idx].kind != BlockKind::Entry {
        return false;
    }

    for &block_id in blocks {
        if !reachable_blocks.contains(&block_id) {
            return false;
        }
    }

    let last_idx = match find_node_by_block_id(cfg, *blocks.last().unwrap()) {
        Some(idx) => idx,
        None => return false,
    };

    match &cfg[last_idx].terminator {
        Terminator::Return => {}
        Terminator::Abort(_) => {}
        Terminator::Call { unwind: None, .. } => {}
        Terminator::Call {
            unwind: Some(_),
            target: Some(_),
        } => {}
        Terminator::Unreachable
        | Terminator::Goto { .. }
        | Terminator::SwitchInt { .. }
        | Terminator::Call {
            unwind: Some(_),
            target: None,
        } => {
            return false;
        }
    }

    for &block_id in blocks.iter().skip(1).take(blocks.len().saturating_sub(2)) {
        if find_node_by_block_id(cfg, block_id).is_none() {
            return false;
        }
    }

    true
}

/// Classify a path based on its terminators and reachability
///
/// **Classification rules (in priority order):**
///
/// 1. **Unreachable:** Any block in path is unreachable from entry
/// 2. **Error:** Path contains error terminators (Abort, Call with unwind)
/// 3. **Degenerate:** Path ends abnormally or has unreachable terminator
/// 4. **Normal:** Default classification (entry -> return path)
pub fn classify_path(cfg: &Cfg, blocks: &[BlockId]) -> PathKind {
    use crate::cfg::reachability::is_reachable_from_entry;

    if blocks.is_empty() {
        return PathKind::Degenerate;
    }

    for &block_id in blocks {
        let node_idx = match find_node_by_block_id(cfg, block_id) {
            Some(idx) => idx,
            None => return PathKind::Degenerate,
        };

        if !is_reachable_from_entry(cfg, node_idx) {
            return PathKind::Unreachable;
        }

        let terminator = &cfg[node_idx].terminator;

        match terminator {
            Terminator::Abort(_) => return PathKind::Error,
            Terminator::Call {
                unwind: Some(_), ..
            } => return PathKind::Error,
            _ => {}
        }

        if matches!(terminator, Terminator::Unreachable) {
            return PathKind::Degenerate;
        }
    }

    if let Some(&last_block_id) = blocks.last() {
        if let Some(node_idx) = find_node_by_block_id(cfg, last_block_id) {
            let terminator = &cfg[node_idx].terminator;
            if !matches!(terminator, Terminator::Return) {
                return PathKind::Degenerate;
            }
        }
    }

    PathKind::Normal
}

/// Classify a path using a pre-computed reachable set for O(n) batch classification
///
/// # Arguments
///
/// * `cfg` - Control flow graph to analyze
/// * `blocks` - Block IDs in execution order
/// * `reachable_blocks` - Pre-computed set of reachable BlockIds
///
/// # Returns
///
/// The classified PathKind for this path
pub fn classify_path_precomputed(
    cfg: &Cfg,
    blocks: &[BlockId],
    reachable_blocks: &HashSet<BlockId>,
) -> PathKind {
    if blocks.is_empty() {
        return PathKind::Degenerate;
    }

    for &block_id in blocks {
        if !reachable_blocks.contains(&block_id) {
            return PathKind::Unreachable;
        }
    }

    for &block_id in blocks {
        let node_idx = match find_node_by_block_id(cfg, block_id) {
            Some(idx) => idx,
            None => return PathKind::Degenerate,
        };

        let terminator = &cfg[node_idx].terminator;

        match terminator {
            Terminator::Abort(_) => return PathKind::Error,
            Terminator::Call {
                unwind: Some(_), ..
            } => return PathKind::Error,
            _ => {}
        }

        if matches!(terminator, Terminator::Unreachable) {
            return PathKind::Degenerate;
        }
    }

    if !is_feasible_path_precomputed(cfg, blocks, reachable_blocks) {
        return PathKind::Degenerate;
    }

    PathKind::Normal
}
