//! Translator from Charon LLBC to Mirage CFG

use crate::cfg::{
    BasicBlock, BlockKind, Cfg, EdgeType, SourceLocation, Terminator as MirageTerminator,
};
use crate::mir::charon_llbc::{File, FunDecl, Terminator as CharonTerminator};
use std::collections::HashMap;
use std::path::PathBuf;

/// Translate a Charon function declaration into a Mirage CFG
pub fn translate_function(decl: &FunDecl, files: &[File]) -> Cfg {
    let mut cfg = Cfg::new();
    let body = match &decl.body {
        Some(b) => b,
        None => return cfg,
    };

    let unstructured = match &body.unstructured {
        Some(u) => u,
        None => return cfg,
    };

    let file_path = files
        .get(decl.meta.span.file_id as usize)
        .map(|f| PathBuf::from(&f.path))
        .unwrap_or_else(|| PathBuf::from("unknown"));

    // 1. Create all nodes
    let mut block_map = HashMap::with_capacity(unstructured.blocks.len());
    for block_data in &unstructured.blocks {
        let kind = if block_data.id == 0 {
            BlockKind::Entry
        } else if matches!(block_data.terminator, CharonTerminator::Return) {
            BlockKind::Exit
        } else {
            BlockKind::Normal
        };

        // Translate statements to strings for now
        let statements = block_data
            .statements
            .iter()
            .map(|s| format!("{:?}", s))
            .collect();

        let block = BasicBlock {
            id: block_data.id as usize,
            db_id: None,
            kind,
            statements,
            terminator: translate_terminator(&block_data.terminator),
            source_location: Some(SourceLocation::new(
                file_path.clone(),
                0,
                0, // Byte offsets unknown from LLBC top-level
                decl.meta.span.beg.line as usize,
                decl.meta.span.beg.col as usize,
                decl.meta.span.end.line as usize,
                decl.meta.span.end.col as usize,
            )),
            // MIR-based CFG doesn't compute spatial coordinates (use 0 defaults)
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        };

        let node_idx = cfg.add_node(block);
        block_map.insert(block_data.id, node_idx);
    }

    // 2. Create edges
    for block_data in &unstructured.blocks {
        let source_idx = match block_map.get(&block_data.id) {
            Some(&idx) => idx,
            None => continue,
        };

        match &block_data.terminator {
            CharonTerminator::Goto(target)
            | CharonTerminator::Assert { target, .. }
            | CharonTerminator::Drop { target, .. } => {
                if let Some(&target_idx) = block_map.get(target) {
                    cfg.add_edge(source_idx, target_idx, EdgeType::Fallthrough);
                }
            }
            CharonTerminator::Call { target, .. } => {
                if let Some(t) = target {
                    if let Some(&target_idx) = block_map.get(t) {
                        cfg.add_edge(source_idx, target_idx, EdgeType::Call);
                    }
                }
            }
            CharonTerminator::SwitchInt {
                targets, otherwise, ..
            } => {
                // True branches
                for target in targets {
                    if let Some(&target_idx) = block_map.get(&target.target) {
                        cfg.add_edge(source_idx, target_idx, EdgeType::TrueBranch);
                    }
                }
                // False/Otherwise branch
                if let Some(&otherwise_idx) = block_map.get(otherwise) {
                    cfg.add_edge(source_idx, otherwise_idx, EdgeType::FalseBranch);
                }
            }
            CharonTerminator::Return | CharonTerminator::Panic => {
                // No outgoing edges
            }
        }
    }

    cfg
}

fn translate_terminator(t: &CharonTerminator) -> MirageTerminator {
    match t {
        CharonTerminator::Goto(target)
        | CharonTerminator::Assert { target, .. }
        | CharonTerminator::Drop { target, .. } => MirageTerminator::Goto {
            target: *target as usize,
        },
        CharonTerminator::Return => MirageTerminator::Return,
        CharonTerminator::Panic => MirageTerminator::Abort("Panic".to_string()),
        CharonTerminator::Call { target, .. } => {
            MirageTerminator::Call {
                target: target.map(|t| t as usize),
                unwind: None, // LLBC doesn't explicitly expose unwind block per-call easily in this format
            }
        }
        CharonTerminator::SwitchInt {
            targets, otherwise, ..
        } => {
            let mirage_targets = targets.iter().map(|t| t.target as usize).collect();
            MirageTerminator::SwitchInt {
                targets: mirage_targets,
                otherwise: *otherwise as usize,
            }
        }
    }
}
