//! Find complex blocks by coordinate thresholds
//!
//! This command filters CFG blocks based on their 4D coordinates,
//! allowing developers to identify the most complex code regions.

use crate::cli::{ComplexBlocksArgs, Cli};
use crate::cfg::{Cfg, BlockKind};
use crate::storage::load_cfg_from_db;
use anyhow::Result;

/// Find complex blocks by coordinate thresholds
pub fn complex_blocks(args: &ComplexBlocksArgs, cli: &Cli) -> Result<()> {
    // Load CFG from database
    let cfg = load_cfg_from_db(&cli.backend, &args.function, None)?;

    // Filter blocks based on coordinate thresholds
    let complex_blocks = find_complex_blocks(
        &cfg,
        args.min_depth,
        args.min_loops,
        args.min_branches,
        args.all,
    );

    // Generate output based on format
    match cli.output {
        crate::cli::OutputFormat::Human => {
            print_complex_blocks_human(&complex_blocks, &args.function, args.all)
        }
        crate::cli::OutputFormat::Json => {
            print_complex_blocks_json(&complex_blocks, &args.function)
        }
        crate::cli::OutputFormat::Pretty => {
            let json = print_complex_blocks_json(&complex_blocks, &args.function)?;
            println!("{}", serde_json::to_string_pretty(&json)?);
            Ok(())
        }
    }
}

/// Find blocks that meet coordinate complexity criteria
fn find_complex_blocks(
    cfg: &Cfg,
    min_depth: i64,
    min_loops: i64,
    min_branches: i64,
    require_all: bool,
) -> Vec<(usize, &crate::cfg::BasicBlock)> {
    let mut results = Vec::new();

    for node_idx in cfg.node_indices() {
        if let Some(block) = cfg.node_weight(node_idx) {
            let meets_depth = block.coord_x >= min_depth;
            let meets_loops = block.coord_y >= min_loops;
            let meets_branches = block.coord_z >= min_branches;

            let passes = if require_all {
                meets_depth && meets_loops && meets_branches
            } else {
                meets_depth || meets_loops || meets_branches
            };

            if passes {
                results.push((node_idx.index(), block));
            }
        }
    }

    // Sort by complexity score (sum of coordinates)
    results.sort_by(|a, b| {
        let score_a = a.1.coord_x + a.1.coord_y + a.1.coord_z;
        let score_b = b.1.coord_x + b.1.coord_y + b.1.coord_z;
        score_b.cmp(&score_a) // Descending order
    });

    results
}

/// Print complex blocks in human-readable format
fn print_complex_blocks_human(
    blocks: &[(usize, &crate::cfg::BasicBlock)],
    function_name: &str,
    require_all: bool,
) -> Result<()> {
    println!("Complex Blocks in '{}'", function_name);
    println!("Criteria: depth>={}, loops>={}, branches>={}, logic={}",
        0, 0, 0, if require_all { "AND" } else { "OR" });
    println!("{}", "=".repeat(60));

    if blocks.is_empty() {
        println!("No complex blocks found matching the criteria.");
        return Ok(());
    }

    println!("\nFound {} complex block(s):\n", blocks.len());

    for (i, (node_idx, block)) in blocks.iter().enumerate() {
        let complexity_score = block.coord_x + block.coord_y + block.coord_z;

        println!("{}. Block {} [{}]", i + 1, block.id, format_block_kind(&block.kind));
        println!("   Node Index: {}", node_idx);
        println!("   Coordinates: X={}, Y={}, Z={}", block.coord_x, block.coord_y, block.coord_z);
        println!("   Complexity Score: {}", complexity_score);
        println!("   Terminator: {}", format_terminator(&block.terminator));

        // Add complexity insights
        let mut insights = Vec::new();
        if block.coord_x > 2 {
            insights.push(format!("{} levels deep in control flow", block.coord_x));
        }
        if block.coord_y > 1 {
            insights.push(format!("nested in {} loops", block.coord_y));
        }
        if block.coord_z > 3 {
            insights.push(format!("{} conditional branches from entry", block.coord_z));
        }

        if !insights.is_empty() {
            println!("   Insights: {}", insights.join(", "));
        }

        println!();
    }

    Ok(())
}

/// Print complex blocks in JSON format
fn print_complex_blocks_json(
    blocks: &[(usize, &crate::cfg::BasicBlock)],
    function_name: &str,
) -> Result<serde_json::Value> {
    let blocks_json: Vec<serde_json::Value> = blocks
        .iter()
        .map(|(node_idx, block)| {
            let complexity_score = block.coord_x + block.coord_y + block.coord_z;
            serde_json::json!({
                "node_index": node_idx,
                "block_id": block.id,
                "kind": format_block_kind(&block.kind),
                "coordinates": {
                    "x": block.coord_x,
                    "y": block.coord_y,
                    "z": block.coord_z,
                },
                "complexity_score": complexity_score,
                "terminator": format_terminator(&block.terminator),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "function": function_name,
        "total_complex_blocks": blocks.len(),
        "blocks": blocks_json,
    }))
}

fn format_block_kind(kind: &BlockKind) -> &'static str {
    match kind {
        BlockKind::Entry => "ENTRY",
        BlockKind::Normal => "NORMAL",
        BlockKind::Exit => "EXIT",
    }
}

fn format_terminator(terminator: &crate::cfg::Terminator) -> String {
    match terminator {
        crate::cfg::Terminator::Goto { target } => format!("goto {}", target),
        crate::cfg::Terminator::SwitchInt { targets, otherwise } => {
            format!("switch({} targets, otherwise {})", targets.len(), otherwise)
        }
        crate::cfg::Terminator::Return => "return".to_string(),
        crate::cfg::Terminator::Unreachable => "unreachable".to_string(),
        crate::cfg::Terminator::Call { target, unwind } => {
            format!("call {:?}, unwind {:?}", target, unwind)
        }
        crate::cfg::Terminator::Abort(msg) => format!("abort({})", msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, Terminator};
    use petgraph::graph::DiGraph;

    fn create_test_cfg() -> Cfg {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            db_id: None,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
            coord_x: 3, // Deep
            coord_y: 1, // In loop
            coord_z: 2, // Branching
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            db_id: None,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        g.add_edge(b0, b1, crate::cfg::EdgeType::Fallthrough);
        g.add_edge(b1, b2, crate::cfg::EdgeType::Fallthrough);

        g
    }

    #[test]
    fn test_find_complex_blocks_or_logic() {
        let cfg = create_test_cfg();

        // Test OR logic (default): find blocks with any coordinate >= 1
        let blocks = find_complex_blocks(&cfg, 1, 1, 1, false);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].1.id, 1); // Block 1 meets criteria
    }

    #[test]
    fn test_find_complex_blocks_and_logic() {
        let cfg = create_test_cfg();

        // Test AND logic: find blocks with all coordinates >= 1
        let blocks = find_complex_blocks(&cfg, 1, 1, 1, true);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].1.id, 1); // Block 1 meets all criteria
    }

    #[test]
    fn test_find_complex_blocks_sorting() {
        let cfg = create_test_cfg();

        let blocks = find_complex_blocks(&cfg, 1, 0, 0, false);

        // Should be sorted by complexity score (descending)
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].1.coord_x + blocks[0].1.coord_y + blocks[0].1.coord_z, 6);
    }

    #[test]
    fn test_find_complex_blocks_empty_result() {
        let cfg = create_test_cfg();

        // No blocks should meet such high thresholds
        let blocks = find_complex_blocks(&cfg, 10, 10, 10, false);

        assert_eq!(blocks.len(), 0);
    }
}
