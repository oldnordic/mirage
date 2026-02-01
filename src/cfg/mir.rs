//! Convert ULLBC to Mirage CFG

use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};
use crate::mir::UllbcBody;

/// Convert ULLBC body to Mirage CFG
pub fn ullbc_to_cfg(body: &UllbcBody) -> Cfg {
    let mut graph = Cfg::new();

    // Map block IDs to graph node indices
    let mut node_indices = Vec::with_capacity(body.blocks.len());

    // Add all blocks as nodes
    for block in &body.blocks {
        let kind = if block.id == 0 {
            BlockKind::Entry
        } else {
            match &block.terminator {
                crate::mir::UllbcTerminator::Return
                | crate::mir::UllbcTerminator::Unreachable => BlockKind::Exit,
                _ => BlockKind::Normal,
            }
        };

        let basic_block = BasicBlock {
            id: block.id,
            kind,
            statements: block.statements.clone(),
            terminator: convert_terminator(&block.terminator),
        };

        node_indices.push(graph.add_node(basic_block));
    }

    // Add edges from terminators
    for (block_idx, block) in body.blocks.iter().enumerate() {
        let from = node_indices[block_idx];
        let edges = classify_ullbc_terminator(&block.terminator);

        for (target_id, edge_type) in edges {
            if target_id < node_indices.len() {
                let to = node_indices[target_id];
                graph.add_edge(from, to, edge_type);
            }
        }
    }

    graph
}

/// Convert ULLBC terminator to Mirage terminator
fn convert_terminator(term: &crate::mir::UllbcTerminator) -> Terminator {
    match term {
        crate::mir::UllbcTerminator::Goto { target } => Terminator::Goto { target: *target },
        crate::mir::UllbcTerminator::SwitchInt { targets, otherwise } => Terminator::SwitchInt {
            targets: targets.clone(),
            otherwise: *otherwise,
        },
        crate::mir::UllbcTerminator::Return => Terminator::Return,
        crate::mir::UllbcTerminator::Unreachable => Terminator::Unreachable,
        crate::mir::UllbcTerminator::Call { target, unwind } => Terminator::Call {
            target: *target,
            unwind: *unwind,
        },
        crate::mir::UllbcTerminator::Abort { message } => Terminator::Abort(message.clone()),
    }
}

/// Classify edges from ULLBC terminator
fn classify_ullbc_terminator(
    term: &crate::mir::UllbcTerminator,
) -> Vec<(usize, EdgeType)> {
    match term {
        crate::mir::UllbcTerminator::Goto { target } => {
            vec![(*target, EdgeType::Fallthrough)]
        }
        crate::mir::UllbcTerminator::SwitchInt { targets, otherwise } => {
            let mut edges = targets
                .iter()
                .map(|&t| (t, EdgeType::TrueBranch))
                .collect::<Vec<_>>();
            edges.push((*otherwise, EdgeType::FalseBranch));
            edges
        }
        crate::mir::UllbcTerminator::Return => vec![],
        crate::mir::UllbcTerminator::Unreachable => vec![],
        crate::mir::UllbcTerminator::Call { target: Some(t), unwind } => {
            let mut edges = vec![(*t, EdgeType::Call)];
            if let Some(uw) = unwind {
                edges.push((*uw, EdgeType::Exception));
            }
            edges
        }
        crate::mir::UllbcTerminator::Call { target: None, unwind: Some(uw) } => {
            vec![(**uw, EdgeType::Exception)]
        }
        crate::mir::UllbcTerminator::Call { target: None, unwind: None } => vec![],
        crate::mir::UllbcTerminator::Abort { .. } => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{UllbcBlock, UllbcBody, UllbcTerminator};

    fn create_simple_body() -> UllbcBody {
        UllbcBody {
            blocks: vec![
                UllbcBlock {
                    id: 0,
                    statements: vec!["let x = 1".to_string()],
                    terminator: UllbcTerminator::Goto { target: 1 },
                },
                UllbcBlock {
                    id: 1,
                    statements: vec![],
                    terminator: UllbcTerminator::Return,
                },
            ],
        }
    }

    #[test]
    fn test_ullbc_to_cfg_simple() {
        let body = create_simple_body();
        let cfg = ullbc_to_cfg(&body);

        // Should have 2 nodes
        assert_eq!(cfg.node_count(), 2);

        // Should have 1 edge
        assert_eq!(cfg.edge_count(), 1);

        // Entry block should be id 0
        let entry = cfg.node_weight(petgraph::graph::NodeIndex::new(0)).unwrap();
        assert_eq!(entry.id, 0);
        assert_eq!(entry.kind, BlockKind::Entry);

        // Exit block should be id 1
        let exit = cfg.node_weight(petgraph::graph::NodeIndex::new(1)).unwrap();
        assert_eq!(exit.id, 1);
        assert_eq!(exit.kind, BlockKind::Exit);
    }

    #[test]
    fn test_ullbc_to_cfg_with_unwind() {
        let body = UllbcBody {
            blocks: vec![
                UllbcBlock {
                    id: 0,
                    statements: vec![],
                    terminator: UllbcTerminator::Call {
                        target: Some(1),
                        unwind: Some(2),
                    },
                },
                UllbcBlock {
                    id: 1,
                    statements: vec![],
                    terminator: UllbcTerminator::Return,
                },
                UllbcBlock {
                    id: 2,
                    statements: vec![],
                    terminator: UllbcTerminator::Return,
                },
            ],
        };

        let cfg = ullbc_to_cfg(&body);

        // Should have 3 nodes, 2 edges
        assert_eq!(cfg.node_count(), 3);
        assert_eq!(cfg.edge_count(), 2);

        // Check edge types
        let edges: Vec<_> = cfg
            .edge_indices()
            .map(|e| cfg.edge_weight(e).unwrap())
            .collect();

        assert!(edges.iter().any(|&e| e == EdgeType::Call));
        assert!(edges.iter().any(|&e| e == EdgeType::Exception));
    }

    #[test]
    fn test_ullbc_to_cfg_switch() {
        let body = UllbcBody {
            blocks: vec![
                UllbcBlock {
                    id: 0,
                    statements: vec![],
                    terminator: UllbcTerminator::SwitchInt {
                        targets: vec![1, 2],
                        otherwise: 3,
                    },
                },
                UllbcBlock {
                    id: 1,
                    statements: vec![],
                    terminator: UllbcTerminator::Return,
                },
                UllbcBlock {
                    id: 2,
                    statements: vec![],
                    terminator: UllbcTerminator::Return,
                },
                UllbcBlock {
                    id: 3,
                    statements: vec![],
                    terminator: UllbcTerminator::Return,
                },
            ],
        };

        let cfg = ullbc_to_cfg(&body);

        // Should have 4 nodes, 3 edges
        assert_eq!(cfg.node_count(), 4);
        assert_eq!(cfg.edge_count(), 3);

        // Check edge types - 2 TrueBranch (targets) + 1 FalseBranch (otherwise)
        let edges: Vec<_> = cfg
            .edge_indices()
            .map(|e| cfg.edge_weight(e).unwrap())
            .collect();

        let true_count = edges.iter().filter(|&&e| *e == EdgeType::TrueBranch).count();
        let false_count = edges
            .iter()
            .filter(|&&e| *e == EdgeType::FalseBranch)
            .count();

        assert_eq!(true_count, 2, "Should have 2 TrueBranch edges");
        assert_eq!(false_count, 1, "Should have 1 FalseBranch edge");
    }

    #[test]
    fn test_ullbc_goto_produces_fallthrough() {
        let body = UllbcBody {
            blocks: vec![
                UllbcBlock {
                    id: 0,
                    statements: vec![],
                    terminator: UllbcTerminator::Goto { target: 1 },
                },
                UllbcBlock {
                    id: 1,
                    statements: vec![],
                    terminator: UllbcTerminator::Return,
                },
            ],
        };

        let cfg = ullbc_to_cfg(&body);

        let edges: Vec<_> = cfg
            .edge_indices()
            .map(|e| cfg.edge_weight(e).unwrap())
            .collect();

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0], EdgeType::Fallthrough);
    }

    #[test]
    fn test_ullbc_unreachable_is_exit() {
        let body = UllbcBody {
            blocks: vec![
                UllbcBlock {
                    id: 0,
                    statements: vec![],
                    terminator: UllbcTerminator::Goto { target: 1 },
                },
                UllbcBlock {
                    id: 1,
                    statements: vec![],
                    terminator: UllbcTerminator::Unreachable,
                },
            ],
        };

        let cfg = ullbc_to_cfg(&body);

        // Block 1 should be Exit kind
        let block = cfg.node_weight(petgraph::graph::NodeIndex::new(1)).unwrap();
        assert_eq!(block.kind, BlockKind::Exit);
        assert!(matches!(block.terminator, Terminator::Unreachable));
    }

    #[test]
    fn test_convert_terminator_goto() {
        let term = UllbcTerminator::Goto { target: 5 };
        let converted = convert_terminator(&term);
        assert!(matches!(converted, Terminator::Goto { target: 5 }));
    }

    #[test]
    fn test_convert_terminator_return() {
        let term = UllbcTerminator::Return;
        let converted = convert_terminator(&term);
        assert!(matches!(converted, Terminator::Return));
    }

    #[test]
    fn test_convert_terminator_call_with_unwind() {
        let term = UllbcTerminator::Call {
            target: Some(2),
            unwind: Some(3),
        };
        let converted = convert_terminator(&term);
        assert!(matches!(converted, Terminator::Call { target: Some(2), unwind: Some(3) }));
    }

    #[test]
    fn test_convert_terminator_abort() {
        let term = UllbcTerminator::Abort {
            message: "panic".to_string(),
        };
        let converted = convert_terminator(&term);
        match converted {
            Terminator::Abort(msg) => assert_eq!(msg, "panic"),
            _ => panic!("Expected Abort terminator"),
        }
    }
}
