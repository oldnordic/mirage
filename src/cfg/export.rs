//! CFG export to DOT and JSON formats

use crate::cfg::{BlockKind, Cfg, EdgeType, Terminator};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Export CFG to DOT format for Graphviz
pub fn export_dot(cfg: &Cfg) -> String {
    let mut dot = String::from("digraph CFG {\n");
    dot.push_str("  rankdir=TB;\n");
    dot.push_str("  node [shape=box, style=rounded];\n\n");

    // Define nodes
    for node_idx in cfg.node_indices() {
        if let Some(block) = cfg.node_weight(node_idx) {
            let label = escape_dot_string(&format!(
                "Block {}\\n{}\\n{}",
                block.id,
                format_block_kind(&block.kind),
                format_terminator(&block.terminator)
            ));

            let style = match block.kind {
                BlockKind::Entry => "fillcolor=lightgreen, style=filled",
                BlockKind::Exit => "fillcolor=lightcoral, style=filled",
                BlockKind::Normal => "",
            };

            writeln!(dot, "  \"{}\" [label=\"{}\" {}];", node_idx.index(), label, style).ok();
        }
    }

    // Define edges
    dot.push_str("\n");
    for edge_idx in cfg.edge_indices() {
        let (from, to) = cfg.edge_endpoints(edge_idx).unwrap();
        if let Some(edge_type) = cfg.edge_weight(edge_idx) {
            let color = edge_type.dot_color();
            let label = edge_type.dot_label();
            let label_attr = if label.is_empty() {
                String::new()
            } else {
                format!(", label=\"{}\"", label)
            };

            writeln!(
                dot,
                "  \"{}\" -> \"{}\" [color={}, style={}{}];",
                from.index(),
                to.index(),
                color,
                if *edge_type == EdgeType::Fallthrough {
                    "dashed"
                } else {
                    "solid"
                },
                label_attr
            )
            .ok();
        }
    }

    dot.push_str("}\n");
    dot
}

fn escape_dot_string(s: &str) -> String {
    s.replace('"', "\\\"")
}

fn format_block_kind(kind: &BlockKind) -> &'static str {
    match kind {
        BlockKind::Entry => "ENTRY",
        BlockKind::Normal => "NORMAL",
        BlockKind::Exit => "EXIT",
    }
}

fn format_terminator(term: &Terminator) -> String {
    match term {
        Terminator::Goto { target } => format!("goto {}", target),
        Terminator::SwitchInt { targets, otherwise } => {
            format!("switch({} targets, otherwise {})", targets.len(), otherwise)
        }
        Terminator::Return => "return".to_string(),
        Terminator::Unreachable => "unreachable".to_string(),
        Terminator::Call { target, unwind } => {
            format!("call {:?}, unwind {:?}", target, unwind)
        }
        Terminator::Abort(msg) => format!("abort({})", msg),
    }
}

/// Complete CFG export for JSON serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFGExport {
    pub function_name: String,
    pub entry: Option<usize>,
    pub exits: Vec<usize>,
    pub blocks: Vec<BlockExport>,
    pub edges: Vec<EdgeExport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExport {
    pub id: usize,
    pub kind: String,
    pub statements: Vec<String>,
    pub terminator: String,
    pub source_location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeExport {
    pub from: usize,
    pub to: usize,
    pub kind: String,
}

/// Export CFG to JSON format
pub fn export_json(cfg: &Cfg, function_name: &str) -> CFGExport {
    use crate::cfg::analysis;

    let entry = analysis::find_entry(cfg).map(|idx| idx.index());
    let exits = analysis::find_exits(cfg)
        .iter()
        .map(|idx| idx.index())
        .collect();

    let blocks: Vec<_> = cfg
        .node_indices()
        .map(|idx| {
            let block = cfg.node_weight(idx).unwrap();
            BlockExport {
                id: block.id,
                kind: format_block_kind(&block.kind).to_string(),
                statements: block.statements.clone(),
                terminator: format_terminator(&block.terminator),
                source_location: block
                    .source_location
                    .as_ref()
                    .map(|loc| loc.display()),
            }
        })
        .collect();

    let edges: Vec<_> = cfg
        .edge_indices()
        .map(|idx| {
            let (from, to) = cfg.edge_endpoints(idx).unwrap();
            let edge_type = cfg.edge_weight(idx).unwrap();
            EdgeExport {
                from: from.index(),
                to: to.index(),
                kind: format!("{:?}", edge_type),
            }
        })
        .collect();

    CFGExport {
        function_name: function_name.to_string(),
        entry,
        exits,
        blocks,
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::BasicBlock;
    use petgraph::graph::DiGraph;

    fn create_test_cfg() -> Cfg {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec!["x = 1".to_string()],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec!["if x > 0".to_string()],
            terminator: Terminator::SwitchInt {
                targets: vec![2],
                otherwise: 3,
            },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec!["return true".to_string()],
            terminator: Terminator::Return,
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec!["return false".to_string()],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b3, EdgeType::FalseBranch);

        g
    }

    #[test]
    fn test_export_dot() {
        let cfg = create_test_cfg();
        let dot = export_dot(&cfg);

        assert!(dot.contains("digraph CFG"));
        assert!(dot.contains("Block 0"));
        assert!(dot.contains("ENTRY"));
        assert!(dot.contains("color=green")); // TrueBranch
        assert!(dot.contains("color=red")); // FalseBranch
    }

    #[test]
    fn test_export_json() {
        let cfg = create_test_cfg();
        let export = export_json(&cfg, "test_function");

        assert_eq!(export.function_name, "test_function");
        assert_eq!(export.entry, Some(0));
        assert_eq!(export.exits.len(), 2); // blocks 2 and 3
        assert_eq!(export.blocks.len(), 4);
        assert_eq!(export.edges.len(), 3);

        // Check block kinds
        assert_eq!(export.blocks[0].kind, "ENTRY");
        assert_eq!(export.blocks[2].kind, "EXIT");

        // Check edge types
        assert!(export.edges.iter().any(|e| e.kind == "TrueBranch"));
        assert!(export.edges.iter().any(|e| e.kind == "FalseBranch"));
    }

    #[test]
    fn test_dot_is_valid_graphviz() {
        let cfg = create_test_cfg();
        let dot = export_dot(&cfg);

        // Basic validation: starts correctly, ends correctly
        assert!(dot.starts_with("digraph CFG {"));
        assert!(dot.ends_with("}\n"));

        // Check that edges section starts after newline following nodes
        // Edges start with "  \"" followed by number and " ->"
        // Nodes end with "];" before the "\n\n" separator
        let first_edge_pos = dot.find("->").unwrap();
        let section_separator = dot.find("\n\n").unwrap();
        assert!(section_separator < first_edge_pos, "Node section should end before edges start");

        // Verify basic DOT structure elements
        assert!(dot.contains("rankdir=TB;"));
        assert!(dot.contains("node [shape=box"));
    }
}
