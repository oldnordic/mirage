//! Natural language summaries of control flow structures

use crate::cfg::{Cfg, Path, PathKind, BlockId, Terminator, BlockKind};

/// Generate a natural language summary of a path
///
/// Produces concise descriptions like:
/// - "Entry → validate → return success (3 blocks)"
/// - "Entry → validate → (error_path) → return (5 blocks)"
pub fn summarize_path(cfg: &Cfg, path: &Path) -> String {
    if path.blocks.is_empty() {
        return "Empty path".to_string();
    }

    let block_descs: Vec<String> = path.blocks.iter()
        .map(|&bid| describe_block(cfg, bid))
        .collect();

    // Truncate long paths for readability
    let flow = if block_descs.len() <= 5 {
        block_descs.join(" → ")
    } else {
        format!(
            "{} → ... → {} ({} blocks)",
            block_descs.first().unwrap_or(&"?".to_string()),
            block_descs.last().unwrap_or(&"?".to_string()),
            path.len()
        )
    };

    // Add path kind context
    match path.kind {
        PathKind::Normal => format!("{} ({} blocks)", flow, path.len()),
        PathKind::Error => format!("{} → error ({} blocks)", flow, path.len()),
        PathKind::Degenerate => format!("{} → dead end ({} blocks)", flow, path.len()),
        PathKind::Unreachable => format!("Unreachable: {} ({} blocks)", flow, path.len()),
    }
}

/// Describe a single block in natural language
pub fn describe_block(cfg: &Cfg, block_id: BlockId) -> String {
    // Find the node with this block_id
    let node_idx = match cfg.node_indices().find(|&n| cfg[n].id == block_id) {
        Some(idx) => idx,
        None => return format!("b{}(unknown)", block_id),
    };

    let block = &cfg[node_idx];

    // Description based on block kind
    let kind_desc = match block.kind {
        BlockKind::Entry => "entry",
        BlockKind::Exit => "exit",
        BlockKind::Normal => "",
    };

    // Description based on terminator
    let term_desc = match &block.terminator {
        Terminator::Return => "return".to_string(),
        Terminator::Goto { target } => format!("goto b{}", target),
        Terminator::SwitchInt { targets, otherwise } => {
            let count = targets.len();
            if count == 1 {
                format!("if b{}|b{}", otherwise, targets[0])
            } else {
                format!("switch ({} targets)", count)
            }
        }
        Terminator::Call { target, unwind: _ } => {
            target.as_ref().map(|t| format!("call b{}", t)).unwrap_or_else(|| "call".to_string())
        }
        Terminator::Unreachable => "unreachable".to_string(),
        Terminator::Abort(msg) => format!("abort: {}", msg),
    };

    // Combine descriptions
    match (kind_desc, term_desc.is_empty()) {
        ("", true) => format!("b{}", block_id),
        ("", false) => format!("b{}({})", block_id, term_desc),
        (kind, true) => format!("{}", kind),
        (kind, false) => format!("{}({})", kind, term_desc),
    }
}

/// Generate a high-level summary of a CFG
///
/// Returns something like:
/// "Function 'process_request' has 5 blocks, 2 exits. Entry: b0. Loops: 1."
pub fn summarize_cfg(function_name: &str, cfg: &Cfg) -> String {
    use crate::cfg::{find_entry, find_exits, detect_natural_loops};

    let entry = find_entry(cfg)
        .map(|id| format!("b{}", id.index()))
        .unwrap_or_else(|| "unknown".to_string());

    let exits = find_exits(cfg);
    let exit_count = exits.len();

    let loops = detect_natural_loops(cfg);
    let loop_count = loops.len();

    format!(
        "Function '{}' has {} blocks, {} exit(s). Entry: {}. Loops: {}.",
        function_name,
        cfg.node_count(),
        exit_count,
        entry,
        loop_count
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, Terminator, Path, PathKind};
    use crate::cfg::edge::EdgeType;
    use petgraph::graph::DiGraph;

    #[test]
    fn test_summarize_path_linear() {
        let mut cfg: Cfg = DiGraph::new();

        // Create simple linear CFG: b0 -> b1 -> b2
        let b0 = cfg.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = cfg.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
        });

        let b2 = cfg.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        cfg.add_edge(b0, b1, EdgeType::Fallthrough);
        cfg.add_edge(b1, b2, EdgeType::Fallthrough);

        let path = Path {
            path_id: "test".to_string(),
            blocks: vec![0, 1, 2],
            kind: PathKind::Normal,
            entry: 0,
            exit: 2,
        };

        let summary = summarize_path(&cfg, &path);

        assert!(summary.contains("entry"));
        assert!(summary.contains("return"));
        assert!(summary.contains("3 blocks"));
    }

    #[test]
    fn test_summarize_path_truncates_long() {
        let mut cfg: Cfg = DiGraph::new();

        // Create a 10-block path (should truncate)
        for i in 0..10 {
            let kind = if i == 0 { BlockKind::Entry } else if i == 9 { BlockKind::Exit } else { BlockKind::Normal };
            let term = if i == 9 { Terminator::Return } else { Terminator::Goto { target: i + 1 } };

            cfg.add_node(BasicBlock {
                id: i,
                kind,
                statements: vec![],
                terminator: term,
                source_location: None,
            });
        }

        let path = Path {
            path_id: "test".to_string(),
            blocks: (0..10).collect(),
            kind: PathKind::Normal,
            entry: 0,
            exit: 9,
        };

        let summary = summarize_path(&cfg, &path);

        // Should truncate, not show all 10 blocks in flow
        assert!(summary.contains("..."));
        assert!(summary.contains("10 blocks"));
    }

    #[test]
    fn test_describe_block_entry() {
        let cfg: Cfg = DiGraph::new();
        let block_id = 0;

        let desc = describe_block(&cfg, block_id);

        assert!(desc.contains("b0"));
        assert!(desc.contains("(unknown)")); // No block with id=0 in empty CFG
    }

    #[test]
    fn test_summarize_cfg() {
        let cfg: Cfg = DiGraph::new();

        let summary = summarize_cfg("test_func", &cfg);

        assert!(summary.contains("test_func"));
        assert!(summary.contains("0 blocks"));
        assert!(summary.contains("Entry:"));
    }

    #[test]
    fn test_summarize_path_error_kind() {
        let mut cfg: Cfg = DiGraph::new();

        let b0 = cfg.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = cfg.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Abort("panic".to_string()),
            source_location: None,
        });

        cfg.add_edge(b0, b1, EdgeType::Fallthrough);

        let path = Path {
            path_id: "test".to_string(),
            blocks: vec![0, 1],
            kind: PathKind::Error,
            entry: 0,
            exit: 1,
        };

        let summary = summarize_path(&cfg, &path);

        assert!(summary.contains("error"));
        assert!(summary.contains("2 blocks"));
    }

    #[test]
    fn test_summarize_path_unreachable_kind() {
        let cfg: Cfg = DiGraph::new();

        let path = Path {
            path_id: "test".to_string(),
            blocks: vec![0, 1],
            kind: PathKind::Unreachable,
            entry: 0,
            exit: 1,
        };

        let summary = summarize_path(&cfg, &path);

        assert!(summary.contains("Unreachable"));
    }
}
