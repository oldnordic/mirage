//! Natural language summaries of control flow structures
//!
//! Distills raw MIR/AST data into compact "Truth Proofs" for LLM consumption.

use crate::cfg::{BasicBlock, BlockKind, Cfg, Path, PathKind, Terminator};

pub struct PathSummarizer;

impl PathSummarizer {
    /// Generate a compact "Truth Proof" for a sequence of blocks
    pub fn summarize(blocks: &[BasicBlock]) -> String {
        if blocks.is_empty() {
            return "Empty path".to_string();
        }

        let mut flow = Vec::new();
        for block in blocks {
            let summary = Self::summarize_block(block);
            if !summary.is_empty() {
                flow.push(summary);
            }
        }

        if flow.is_empty() {
            return "no logical effects".to_string();
        }

        flow.join(" -> ")
    }

    /// Distill a single block into its primary logical effects
    pub fn summarize_block(block: &BasicBlock) -> String {
        let mut effects = Vec::new();

        // 1. Add Entry/Exit markers
        match block.kind {
            BlockKind::Entry => effects.push("[Entry]".to_string()),
            BlockKind::Exit => {}
            BlockKind::Normal => {}
        }

        // 2. Filter and distill statements
        for stmt in &block.statements {
            if let Some(distilled) = Self::distill_statement(stmt) {
                effects.push(distilled);
            }
        }

        // 3. Add terminator effect
        match &block.terminator {
            Terminator::Return => effects.push("[Return]".to_string()),
            Terminator::Abort(msg) => effects.push(format!("[Abort: {}]", msg)),
            Terminator::Unreachable => effects.push("[Unreachable]".to_string()),
            _ => {}
        }

        effects.join(" -> ")
    }

    fn distill_statement(stmt: &str) -> Option<String> {
        // Noise filters for MIR bookkeeping statements
        if stmt.contains("StorageLive")
            || stmt.contains("StorageDead")
            || stmt.contains("Nop")
            || stmt.contains("FakeRead")
        {
            return None;
        }

        let stmt = stmt.trim();

        // 1. Call Signal: Extracts function action regardless of assignment
        // Prefer Call signals as they represent primary actions
        if let Some(call_start) = stmt.find("Call(") {
            let inner = &stmt[call_start + 5..];
            let mut depth = 0;
            let mut end = None;
            for (i, c) in inner.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        if depth == 0 {
                            end = Some(i);
                            break;
                        }
                        depth -= 1;
                    }
                    ',' if depth == 0 => {
                        end = Some(i);
                        break;
                    }
                    _ => {}
                }
            }
            if let Some(e) = end {
                return Some(format!("[Call: {}]", inner[..e].trim()));
            }
        }

        // 2. State Signal: Extracts assignments like Local(1) = 42
        if stmt.starts_with("Assign(") && stmt.ends_with(')') {
            let inner = &stmt[7..stmt.len() - 1];
            let mut depth = 0;
            let mut comma_pos = None;

            for (i, c) in inner.char_indices() {
                match c {
                    '(' | '[' => depth += 1,
                    ')' | ']' => depth -= 1,
                    ',' if depth == 0 => {
                        comma_pos = Some(i);
                        break;
                    }
                    _ => {}
                }
            }

            if let Some(pos) = comma_pos {
                let lhs = inner[..pos].trim();
                let rhs = inner[pos + 1..].trim();

                // Clean up RHS: Constant(42) -> 42
                let rhs_display = if rhs.starts_with("Constant(") && rhs.ends_with(')') {
                    &rhs[9..rhs.len() - 1].trim()
                } else {
                    rhs
                };

                return Some(format!("[State: {} = {}]", lhs, rhs_display));
            }
        }

        None
    }
}

/// Legacy compatible entry point that uses the new summarization logic
pub fn summarize_path(cfg: &Cfg, path: &Path) -> String {
    let mut blocks = Vec::new();
    for &bid in &path.blocks {
        if let Some(node_idx) = cfg.node_indices().find(|&n| cfg[n].id == bid) {
            blocks.push(cfg[node_idx].clone());
        }
    }

    let summary = PathSummarizer::summarize(&blocks);

    match path.kind {
        PathKind::Normal => format!("{} ({} blocks)", summary, path.len()),
        PathKind::Error => format!("{} -> error ({} blocks)", summary, path.len()),
        PathKind::Degenerate => format!("{} -> dead end ({} blocks)", summary, path.len()),
        PathKind::Unreachable => format!("Unreachable: {} ({} blocks)", summary, path.len()),
    }
}
