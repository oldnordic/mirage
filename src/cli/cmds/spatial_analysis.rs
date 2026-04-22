//! Perform 4D spatial analysis on function CFG
//!
//! This command provides comprehensive spatial analysis using 4D coordinates,
//! including coordinate distribution, hot path detection, and complexity metrics.

use crate::cli::{SpatialAnalysisArgs, Cli};
use crate::cfg::{Cfg, export::calculate_coordinate_statistics};
use crate::storage::load_cfg_from_db;
use anyhow::Result;

/// Perform 4D spatial analysis on function
pub fn spatial_analysis(args: &SpatialAnalysisArgs, cli: &Cli) -> Result<()> {
    // Load CFG from database
    let cfg = load_cfg_from_db(&cli.backend, &args.function, None)?;

    // Calculate coordinate statistics
    let stats = calculate_coordinate_statistics(&cfg);

    // Generate output based on format
    match cli.output {
        crate::cli::OutputFormat::Human => {
            print_spatial_analysis_human(&cfg, &args.function, &stats, args.distribution, args.hot_paths)
        }
        crate::cli::OutputFormat::Json => {
            print_spatial_analysis_json(&cfg, &args.function, &stats, args.distribution, args.hot_paths)
        }
        crate::cli::OutputFormat::Pretty => {
            let json = print_spatial_analysis_json(&cfg, &args.function, &stats, args.distribution, args.hot_paths)?;
            println!("{}", serde_json::to_string_pretty(&json)?);
            Ok(())
        }
    }
}

/// Print spatial analysis in human-readable format
fn print_spatial_analysis_human(
    cfg: &Cfg,
    function_name: &str,
    stats: &crate::cfg::export::CoordinateStatistics,
    show_distribution: bool,
    show_hot_paths: bool,
) -> Result<()> {
    println!("4D Spatial Analysis: {}", function_name);
    println!("{}", "=".repeat(60));

    // Overview statistics
    println!("\n📊 Coordinate Statistics:");
    println!("  Total Blocks: {}", stats.total_blocks);
    println!("  Dominator Depth (X):    max={}, avg={:.2}, min={}",
        stats.max_coord_x, stats.avg_coord_x, find_min_coord_x(cfg));
    println!("  Loop Nesting (Y):       max={}, avg={:.2}, min={}",
        stats.max_coord_y, stats.avg_coord_y, find_min_coord_y(cfg));
    println!("  Branch Distance (Z):    max={}, avg={:.2}, min={}",
        stats.max_coord_z, stats.avg_coord_z, find_min_coord_z(cfg));

    // Complexity assessment
    println!("\n🎯 Complexity Assessment:");
    assess_complexity(stats);

    if show_distribution {
        println!("\n📈 Coordinate Distribution:");
        show_coordinate_distribution(cfg);
    }

    if show_hot_paths {
        println!("\n🔥 Hot Path Analysis:");
        show_hot_path_analysis(cfg, stats);
    }

    // Spatial insights
    println!("\n💡 Spatial Insights:");
    generate_spatial_insights(cfg, stats);

    Ok(())
}

/// Find minimum coord_x value
fn find_min_coord_x(cfg: &Cfg) -> i64 {
    cfg.node_indices()
        .filter_map(|n| cfg.node_weight(n))
        .map(|b| b.coord_x)
        .min()
        .unwrap_or(0)
}

/// Find minimum coord_y value
fn find_min_coord_y(cfg: &Cfg) -> i64 {
    cfg.node_indices()
        .filter_map(|n| cfg.node_weight(n))
        .map(|b| b.coord_y)
        .min()
        .unwrap_or(0)
}

/// Find minimum coord_z value
fn find_min_coord_z(cfg: &Cfg) -> i64 {
    cfg.node_indices()
        .filter_map(|n| cfg.node_weight(n))
        .map(|b| b.coord_z)
        .min()
        .unwrap_or(0)
}

/// Assess overall complexity based on coordinates
fn assess_complexity(stats: &crate::cfg::export::CoordinateStatistics) {
    let depth_score = stats.avg_coord_x;
    let loop_score = stats.avg_coord_y;
    let branch_score = stats.avg_coord_z;

    let overall_complexity = (depth_score + loop_score * 2.0 + branch_score) / 4.0;

    let (level, color) = if overall_complexity < 1.0 {
        ("Low", "🟢")
    } else if overall_complexity < 2.5 {
        ("Medium", "🟡")
    } else if overall_complexity < 4.0 {
        ("High", "🟠")
    } else {
        ("Very High", "🔴")
    };

    println!("  Overall Complexity: {} ({})", level, color);
    println!("  Score: {:.2}/5.0", overall_complexity);

    // Specific assessments
    if depth_score > 2.0 {
        println!("  → Deep control flow nesting detected");
    }
    if loop_score > 0.5 {
        println!("  → Significant loop complexity");
    }
    if branch_score > 2.0 {
        println!("  → High conditional branching");
    }
}

/// Show coordinate distribution
fn show_coordinate_distribution(cfg: &Cfg) {
    let mut depth_dist = std::collections::HashMap::new();
    let mut loop_dist = std::collections::HashMap::new();

    for node_idx in cfg.node_indices() {
        if let Some(block) = cfg.node_weight(node_idx) {
            *depth_dist.entry(block.coord_x).or_insert(0) += 1;
            *loop_dist.entry(block.coord_y).or_insert(0) += 1;
        }
    }

    println!("  Dominator Depth Distribution:");
    let mut depths: Vec<_> = depth_dist.iter().collect();
    depths.sort_by_key(|&(k, _)| k);
    for (depth, count) in depths {
        let bar = "█".repeat((*count as usize) * 2);
        println!("    X={}: {} ({})", depth, bar, count);
    }

    println!("  Loop Nesting Distribution:");
    let mut loops: Vec<_> = loop_dist.iter().collect();
    loops.sort_by_key(|&(k, _)| k);
    for (depth, count) in loops {
        let bar = "█".repeat((*count as usize) * 2);
        println!("    Y={}: {} ({})", depth, bar, count);
    }
}

/// Show hot path analysis
fn show_hot_path_analysis(cfg: &Cfg, stats: &crate::cfg::export::CoordinateStatistics) {
    // Find blocks with highest combined complexity
    let mut blocks: Vec<_> = cfg.node_indices()
        .filter_map(|n| cfg.node_weight(n).map(|b| (n, b)))
        .collect();

    blocks.sort_by(|a, b| {
        let score_a = a.1.coord_x + a.1.coord_y + a.1.coord_z;
        let score_b = b.1.coord_x + b.1.coord_y + b.1.coord_z;
        score_b.cmp(&score_a)
    });

    println!("  Top 5 Most Complex Blocks:");
    for (i, (node_idx, block)) in blocks.iter().take(5).enumerate() {
        let score = block.coord_x + block.coord_y + block.coord_z;
        println!("    {}. Block {}: X={}, Y={}, Z={}, Score={}",
            i + 1, block.id, block.coord_x, block.coord_y, block.coord_z, score);
    }
}

/// Generate spatial insights
fn generate_spatial_insights(cfg: &Cfg, stats: &crate::cfg::export::CoordinateStatistics) {
    // Find "spatial outliers" - blocks that are unusually complex
    let avg_complexity = stats.avg_coord_x + stats.avg_coord_y + stats.avg_coord_z;

    let mut outliers = Vec::new();
    for node_idx in cfg.node_indices() {
        if let Some(block) = cfg.node_weight(node_idx) {
            let complexity = block.coord_x + block.coord_y + block.coord_z;
            if complexity as f64 > avg_complexity * 1.5 {
                outliers.push((block.id, complexity));
            }
        }
    }

    if !outliers.is_empty() {
        println!("  Spatial Outliers (>1.5x avg complexity):");
        outliers.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        for (block_id, score) in outliers.iter().take(3) {
            println!("    → Block {}: complexity {}", block_id, score);
        }
    }

    // Check for "deep blocks" - high dominator depth
    if stats.max_coord_x > 3 {
        let deep_blocks: Vec<_> = cfg.node_indices()
            .filter_map(|n| cfg.node_weight(n))
            .filter(|b| b.coord_x >= stats.max_coord_x - 1)
            .map(|b| b.id)
            .collect();

        if !deep_blocks.is_empty() {
            println!("  Deep Nesting Zones (depth={}):", stats.max_coord_x);
            println!("    → Blocks: {:?}", deep_blocks);
        }
    }

    // Check for "loop-intensive" regions
    if stats.max_coord_y > 1 {
        let loop_blocks: Vec<_> = cfg.node_indices()
            .filter_map(|n| cfg.node_weight(n))
            .filter(|b| b.coord_y > 0)
            .map(|b| b.id)
            .collect();

        if !loop_blocks.is_empty() {
            println!("  Loop-Intensive Regions: {} blocks in loops", loop_blocks.len());
        }
    }
}

/// Print spatial analysis in JSON format
fn print_spatial_analysis_json(
    cfg: &Cfg,
    function_name: &str,
    stats: &crate::cfg::export::CoordinateStatistics,
    _show_distribution: bool,
    _show_hot_paths: bool,
) -> Result<serde_json::Value> {
    // Find most complex blocks
    let mut blocks: Vec<_> = cfg.node_indices()
        .filter_map(|n| cfg.node_weight(n).map(|b| (n, b)))
        .collect();

    blocks.sort_by(|a, b| {
        let score_a = a.1.coord_x + a.1.coord_y + a.1.coord_z;
        let score_b = b.1.coord_x + b.1.coord_y + b.1.coord_z;
        score_b.cmp(&score_a)
    });

    let top_blocks: Vec<_> = blocks.iter().take(10).map(|(node_idx, block)| {
        serde_json::json!({
            "block_id": block.id,
            "node_index": node_idx.index(),
            "coordinates": {
                "x": block.coord_x,
                "y": block.coord_y,
                "z": block.coord_z,
            },
            "complexity_score": block.coord_x + block.coord_y + block.coord_z,
        })
    }).collect();

    Ok(serde_json::json!({
        "function": function_name,
        "statistics": {
            "total_blocks": stats.total_blocks,
            "dominator_depth": {
                "max": stats.max_coord_x,
                "avg": stats.avg_coord_x,
                "min": find_min_coord_x(cfg),
            },
            "loop_nesting": {
                "max": stats.max_coord_y,
                "avg": stats.avg_coord_y,
                "min": find_min_coord_y(cfg),
            },
            "branch_distance": {
                "max": stats.max_coord_z,
                "avg": stats.avg_coord_z,
                "min": find_min_coord_z(cfg),
            },
        },
        "top_complex_blocks": top_blocks,
        "overall_complexity": (stats.avg_coord_x + stats.avg_coord_y * 2.0 + stats.avg_coord_z) / 4.0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, Terminator, BlockKind};
    use petgraph::graph::DiGraph;

    fn create_complex_test_cfg() -> Cfg {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
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
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
            coord_x: 3, // Deep
            coord_y: 2, // Nested loops
            coord_z: 4, // High branching
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
            coord_x: 1,
            coord_y: 0,
            coord_z: 1,
        });

        g.add_edge(b0, b1, crate::cfg::EdgeType::Fallthrough);
        g.add_edge(b1, b2, crate::cfg::EdgeType::Fallthrough);

        g
    }

    #[test]
    fn test_find_min_coord_x() {
        let cfg = create_complex_test_cfg();
        assert_eq!(find_min_coord_x(&cfg), 0);
    }

    #[test]
    fn test_find_min_coord_y() {
        let cfg = create_complex_test_cfg();
        assert_eq!(find_min_coord_y(&cfg), 0);
    }

    #[test]
    fn test_find_min_coord_z() {
        let cfg = create_complex_test_cfg();
        assert_eq!(find_min_coord_z(&cfg), 0);
    }

    #[test]
    fn test_spatial_analysis_json_structure() {
        let cfg = create_complex_test_cfg();
        let stats = calculate_coordinate_statistics(&cfg);

        let json = print_spatial_analysis_json(&cfg, "test", &stats, true, true).unwrap();

        assert!(json.is_object());
        assert_eq!(json["function"], "test");
        assert!(json["statistics"].is_object());
        assert!(json["top_complex_blocks"].is_array());
        assert!(json["overall_complexity"].is_f64());
    }
}
