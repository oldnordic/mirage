//! CFG export to DOT and JSON formats with 4D coordinate visualization

use crate::cfg::{BlockKind, Cfg, EdgeType, Terminator};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Export CFG to DOT format for Graphviz with 4D coordinate visualization
pub fn export_dot(cfg: &Cfg) -> String {
    export_dot_with_coords(cfg)
}

/// Export CFG to DOT format with 4D coordinate-based coloring
///
/// This function creates a GraphViz DOT visualization where:
/// - Node colors represent dominator depth (coord_x)
/// - Node borders represent loop nesting (coord_y)
/// - Node labels include all 4D coordinates
pub fn export_dot_with_coords(cfg: &Cfg) -> String {
    let mut dot = String::from("digraph CFG {\n");
    dot.push_str("  rankdir=TB;\n");
    dot.push_str("  node [shape=box, style=rounded];\n\n");

    // Calculate coordinate ranges for color mapping
    let (max_coord_x, max_coord_y, _max_coord_z) = calculate_coordinate_ranges(cfg);

    // Define nodes with coordinate-based styling
    for node_idx in cfg.node_indices() {
        if let Some(block) = cfg.node_weight(node_idx) {
            let label = escape_dot_string(&format!(
                "Block {}\\n{}\\n{}\\nCoords: X={}, Y={}, Z={}",
                block.id,
                format_block_kind(&block.kind),
                format_terminator(&block.terminator),
                block.coord_x,
                block.coord_y,
                block.coord_z
            ));

            // Color based on dominator depth (coord_x) - deeper = darker
            let depth_color = get_depth_color(block.coord_x, max_coord_x);

            // Border style based on loop nesting (coord_y)
            let border_style = get_loop_border_style(block.coord_y, max_coord_y);

            // Base style for block kind
            let base_style = match block.kind {
                BlockKind::Entry => "fillcolor=lightgreen, style=filled",
                BlockKind::Exit => "fillcolor=lightcoral, style=filled",
                BlockKind::Normal => "",
            };

            writeln!(
                dot,
                "  \"{}\" [label=\"{}\" {} {} {}];",
                node_idx.index(),
                label,
                base_style,
                depth_color,
                border_style
            )
            .ok();
        }
    }

    // Define edges
    dot.push('\n');
    for edge_idx in cfg.edge_indices() {
        let (from, to) = cfg
            .edge_endpoints(edge_idx)
            .expect("invariant: edge_idx from cfg.edge_indices()");
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

/// Calculate maximum coordinate values for color mapping
fn calculate_coordinate_ranges(cfg: &Cfg) -> (i64, i64, i64) {
    let mut max_x = 0;
    let mut max_y = 0;
    let mut max_z = 0;

    for node_idx in cfg.node_indices() {
        if let Some(block) = cfg.node_weight(node_idx) {
            max_x = max_x.max(block.coord_x);
            max_y = max_y.max(block.coord_y);
            max_z = max_z.max(block.coord_z);
        }
    }

    (max_x, max_y, max_z)
}

/// Get color based on dominator depth (coord_x)
fn get_depth_color(coord_x: i64, max_x: i64) -> String {
    if max_x == 0 {
        return String::new();
    }

    // Map coord_x to a color gradient (light to dark blue)
    let ratio = coord_x as f64 / max_x as f64;
    let intensity = (ratio * 255.0) as u8;

    // Light blue to dark blue gradient
    let r = 200u8.saturating_sub(intensity / 2);
    let g = 220u8.saturating_sub(intensity / 2);
    let b = 255u8.saturating_sub(intensity / 4);

    format!("fillcolor=\"#{:02x}{:02x}{:02x}\", style=filled", r, g, b)
}

/// Get border style based on loop nesting (coord_y)
fn get_loop_border_style(coord_y: i64, _max_y: i64) -> String {
    if coord_y == 0 {
        return String::new(); // No border for non-loop nodes
    }

    // Thicker borders for deeper nesting
    let width = 1 + coord_y.min(4); // Cap at width 5
    format!("penwidth={}, color=red", width)
}

/// Export CFG to human-readable format with 4D coordinate statistics
///
/// This function creates a detailed human-readable representation that includes:
/// - Block information with coordinates
/// - Coordinate statistics (max, average, etc.)
/// - Spatial analysis insights
pub fn export_human_with_coords(cfg: &Cfg, function_name: &str) -> String {
    let mut output = String::new();

    // Header with function name
    writeln!(output, "Control Flow Graph: {}", function_name).ok();
    writeln!(output, "{}", "=".repeat(60)).ok();

    // Calculate coordinate statistics
    let stats = calculate_coordinate_statistics(cfg);

    // Display statistics
    writeln!(output, "\n4D Coordinate Statistics:").ok();
    writeln!(
        output,
        "  Dominator Depth (X):    max={}, avg={:.1}",
        stats.max_coord_x, stats.avg_coord_x
    )
    .ok();
    writeln!(
        output,
        "  Loop Nesting (Y):       max={}, avg={:.1}",
        stats.max_coord_y, stats.avg_coord_y
    )
    .ok();
    writeln!(
        output,
        "  Branch Distance (Z):    max={}, avg={:.1}",
        stats.max_coord_z, stats.avg_coord_z
    )
    .ok();

    // Display blocks with coordinates
    writeln!(output, "\nBlocks ({} total):", cfg.node_count()).ok();
    for node_idx in cfg.node_indices() {
        if let Some(block) = cfg.node_weight(node_idx) {
            writeln!(
                output,
                "\n  Block {} [{}]:",
                block.id,
                format_block_kind(&block.kind)
            )
            .ok();
            writeln!(
                output,
                "    Terminator: {}",
                format_terminator(&block.terminator)
            )
            .ok();
            writeln!(
                output,
                "    Coordinates: X={}, Y={}, Z={}",
                block.coord_x, block.coord_y, block.coord_z
            )
            .ok();

            // Add spatial insights
            if block.coord_x > 0 {
                writeln!(
                    output,
                    "    → {} levels deep in control flow",
                    block.coord_x
                )
                .ok();
            }
            if block.coord_y > 0 {
                writeln!(output, "    → Inside {} loop(s)", block.coord_y).ok();
            }
            if block.coord_z > 1 {
                writeln!(
                    output,
                    "    → {} conditional branches from entry",
                    block.coord_z
                )
                .ok();
            }
        }
    }

    output
}

/// Coordinate statistics for a CFG
#[derive(Debug, Clone)]
pub struct CoordinateStatistics {
    pub max_coord_x: i64,
    pub max_coord_y: i64,
    pub max_coord_z: i64,
    pub avg_coord_x: f64,
    pub avg_coord_y: f64,
    pub avg_coord_z: f64,
    pub total_blocks: usize,
}

/// Calculate comprehensive coordinate statistics for a CFG
pub fn calculate_coordinate_statistics(cfg: &Cfg) -> CoordinateStatistics {
    let mut sum_x = 0i64;
    let mut sum_y = 0i64;
    let mut sum_z = 0i64;
    let mut max_x = 0i64;
    let mut max_y = 0i64;
    let mut max_z = 0i64;
    let mut count = 0usize;

    for node_idx in cfg.node_indices() {
        if let Some(block) = cfg.node_weight(node_idx) {
            sum_x += block.coord_x;
            sum_y += block.coord_y;
            sum_z += block.coord_z;
            max_x = max_x.max(block.coord_x);
            max_y = max_y.max(block.coord_y);
            max_z = max_z.max(block.coord_z);
            count += 1;
        }
    }

    let avg_x = if count > 0 {
        sum_x as f64 / count as f64
    } else {
        0.0
    };
    let avg_y = if count > 0 {
        sum_y as f64 / count as f64
    } else {
        0.0
    };
    let avg_z = if count > 0 {
        sum_z as f64 / count as f64
    } else {
        0.0
    };

    CoordinateStatistics {
        max_coord_x: max_x,
        max_coord_y: max_y,
        max_coord_z: max_z,
        avg_coord_x: avg_x,
        avg_coord_y: avg_y,
        avg_coord_z: avg_z,
        total_blocks: count,
    }
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

/// Coverage data for a single block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockCoverage {
    /// Number of times this block was executed
    pub hit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExport {
    pub id: usize,
    pub kind: String,
    pub statements: Vec<String>,
    pub terminator: String,
    pub source_location: Option<String>,
    /// 4D Spatial Coordinates
    /// X coordinate: dominator depth (control flow hierarchy level)
    pub coord_x: i64,
    /// Y coordinate: loop nesting depth (how many loops surround this block)
    pub coord_y: i64,
    /// Z coordinate: branch distance from entry point
    pub coord_z: i64,
    /// Coverage data (only present when coverage is available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<BlockCoverage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeExport {
    pub from: usize,
    pub to: usize,
    pub kind: String,
}

/// Export CFG to JSON format
///
/// Optionally includes per-block coverage data keyed by the original database block ID
/// (`cfg_blocks.id`). If `coverage` is `Some`, each `BlockExport` will include a
/// `coverage` field when its `db_id` matches an entry in the map.
pub fn export_json(
    cfg: &Cfg,
    function_name: &str,
    coverage: Option<&std::collections::HashMap<i64, i64>>,
) -> CFGExport {
    use crate::cfg::analysis;

    let entry = analysis::find_entry(cfg).map(|idx| idx.index());
    let exits = analysis::find_exits(cfg)
        .iter()
        .map(|idx| idx.index())
        .collect();

    let blocks: Vec<_> = cfg
        .node_indices()
        .map(|idx| {
            let block = cfg
                .node_weight(idx)
                .expect("invariant: idx from cfg.node_indices()");
            let block_coverage = coverage.and_then(|cov_map| {
                block
                    .db_id
                    .and_then(|db_id| cov_map.get(&db_id))
                    .map(|&hit_count| BlockCoverage { hit_count })
            });
            BlockExport {
                id: block.id,
                kind: format_block_kind(&block.kind).to_string(),
                statements: block.statements.clone(),
                terminator: format_terminator(&block.terminator),
                source_location: block.source_location.as_ref().map(|loc| loc.display()),
                coord_x: block.coord_x,
                coord_y: block.coord_y,
                coord_z: block.coord_z,
                coverage: block_coverage,
            }
        })
        .collect();

    let edges: Vec<_> = cfg
        .edge_indices()
        .map(|idx| {
            let (from, to) = cfg
                .edge_endpoints(idx)
                .expect("invariant: idx from cfg.edge_indices()");
            let edge_type = cfg
                .edge_weight(idx)
                .expect("invariant: idx from cfg.edge_indices()");
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
            db_id: None,
            kind: BlockKind::Entry,
            statements: vec!["x = 1".to_string()],
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
            statements: vec!["if x > 0".to_string()],
            terminator: Terminator::SwitchInt {
                targets: vec![2],
                otherwise: 3,
            },
            source_location: None,
            coord_x: 1,
            coord_y: 0,
            coord_z: 1,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            db_id: None,
            kind: BlockKind::Exit,
            statements: vec!["return true".to_string()],
            terminator: Terminator::Return,
            source_location: None,
            coord_x: 2,
            coord_y: 0,
            coord_z: 2,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            db_id: None,
            kind: BlockKind::Exit,
            statements: vec!["return false".to_string()],
            terminator: Terminator::Return,
            source_location: None,
            coord_x: 2,
            coord_y: 0,
            coord_z: 3,
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
        let export = export_json(&cfg, "test_function", None);

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
        assert!(
            section_separator < first_edge_pos,
            "Node section should end before edges start"
        );

        // Verify basic DOT structure elements
        assert!(dot.contains("rankdir=TB;"));
        assert!(dot.contains("node [shape=box"));
    }

    #[test]
    fn test_export_dot_with_coords_includes_coordinates() {
        // Given: A CFG with coordinate data
        let mut cfg = create_test_cfg();

        // Set some coordinates for testing
        for node_idx in cfg.node_indices() {
            if let Some(block) = cfg.node_weight_mut(node_idx) {
                block.coord_x = node_idx.index() as i64;
                block.coord_y = if node_idx.index() > 1 { 1 } else { 0 };
                block.coord_z = node_idx.index() as i64 / 2;
            }
        }

        // When: Exporting to DOT with coordinates
        let dot = export_dot_with_coords(&cfg);

        // Then: DOT output should include coordinate information
        assert!(
            dot.contains("Coords:"),
            "DOT should include coordinate labels"
        );
        assert!(dot.contains("X="), "DOT should include X coordinate");
        assert!(dot.contains("Y="), "DOT should include Y coordinate");
        assert!(dot.contains("Z="), "DOT should include Z coordinate");
    }

    #[test]
    fn test_export_human_with_coords_includes_statistics() {
        // Given: A CFG with coordinate data
        let mut cfg = create_test_cfg();

        // Set specific coordinates for testing
        for node_idx in cfg.node_indices() {
            if let Some(block) = cfg.node_weight_mut(node_idx) {
                block.coord_x = node_idx.index() as i64;
                block.coord_y = (node_idx.index() / 2) as i64;
                block.coord_z = node_idx.index() as i64;
            }
        }

        // When: Exporting to human-readable format with coordinates
        let output = export_human_with_coords(&cfg, "test_function");

        // Then: Output should include coordinate statistics
        assert!(output.contains("Control Flow Graph: test_function"));
        assert!(output.contains("4D Coordinate Statistics"));
        assert!(output.contains("Dominator Depth (X)"));
        assert!(output.contains("Loop Nesting (Y)"));
        assert!(output.contains("Branch Distance (Z)"));
        assert!(output.contains("max="));
        assert!(output.contains("avg="));
    }

    #[test]
    fn test_calculate_coordinate_statistics() {
        // Given: A CFG with varying coordinate values
        let mut cfg = create_test_cfg();

        // Set specific coordinates
        for (i, node_idx) in cfg.node_indices().enumerate() {
            if let Some(block) = cfg.node_weight_mut(node_idx) {
                block.coord_x = i as i64;
                block.coord_y = (i / 2) as i64;
                block.coord_z = (i * 2) as i64;
            }
        }

        // When: Calculating statistics
        let stats = calculate_coordinate_statistics(&cfg);

        // Then: Statistics should be calculated correctly
        assert_eq!(stats.total_blocks, 4);
        assert_eq!(stats.max_coord_x, 3);
        assert_eq!(stats.max_coord_y, 1);
        assert_eq!(stats.max_coord_z, 6);

        // Check averages (should be (0+1+2+3)/4 = 1.5 for X)
        assert!((stats.avg_coord_x - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_coordinate_statistics_empty_cfg() {
        // Given: An empty CFG
        let cfg: Cfg = petgraph::graph::DiGraph::new();

        // When: Calculating statistics
        let stats = calculate_coordinate_statistics(&cfg);

        // Then: Should return zero statistics
        assert_eq!(stats.total_blocks, 0);
        assert_eq!(stats.max_coord_x, 0);
        assert_eq!(stats.max_coord_y, 0);
        assert_eq!(stats.max_coord_z, 0);
        assert_eq!(stats.avg_coord_x, 0.0);
        assert_eq!(stats.avg_coord_y, 0.0);
        assert_eq!(stats.avg_coord_z, 0.0);
    }

    #[test]
    fn test_export_json_includes_coordinates() {
        // Given: A CFG with coordinate data
        let mut cfg = create_test_cfg();

        // Set specific coordinates
        for node_idx in cfg.node_indices() {
            if let Some(block) = cfg.node_weight_mut(node_idx) {
                block.coord_x = 5;
                block.coord_y = 2;
                block.coord_z = 3;
            }
        }

        // When: Exporting to JSON
        let export = export_json(&cfg, "test_function", None);

        // Then: JSON export should include coordinate fields
        assert!(!export.blocks.is_empty());
        for block in &export.blocks {
            assert_eq!(block.coord_x, 5, "Block should have coord_x set");
            assert_eq!(block.coord_y, 2, "Block should have coord_y set");
            assert_eq!(block.coord_z, 3, "Block should have coord_z set");
        }
    }

    #[test]
    fn test_export_human_spatial_insights() {
        // Given: A CFG with complex coordinate data
        let mut cfg = create_test_cfg();

        // Create a block with deep nesting
        if let Some(block) = cfg.node_weight_mut(petgraph::graph::NodeIndex::new(2)) {
            block.coord_x = 4; // Deep dominator depth
            block.coord_y = 2; // Nested in 2 loops
            block.coord_z = 5; // Far from entry
        }

        // When: Exporting to human-readable format
        let output = export_human_with_coords(&cfg, "complex_function");

        // Then: Output should include spatial insights
        assert!(output.contains("4 levels deep in control flow"));
        assert!(output.contains("Inside 2 loop(s)"));
        assert!(output.contains("5 conditional branches from entry"));
    }

    #[test]
    fn test_get_depth_color_gradient() {
        // Test color gradient generation for different depths
        let color_0 = get_depth_color(0, 10);
        let color_5 = get_depth_color(5, 10);
        let color_10 = get_depth_color(10, 10);

        // Check that colors are generated
        assert!(color_0.contains("fillcolor"));
        assert!(color_5.contains("fillcolor"));
        assert!(color_10.contains("fillcolor"));

        // Check that deeper depths produce different colors (darker)
        assert_ne!(color_0, color_10);
    }

    #[test]
    fn test_get_loop_border_style() {
        // Test border styles for different nesting levels
        let style_0 = get_loop_border_style(0, 5);
        let style_1 = get_loop_border_style(1, 5);
        let style_3 = get_loop_border_style(3, 5);

        // No border for non-loop nodes
        assert!(style_0.is_empty());

        // Red borders for loop nodes
        assert!(style_1.contains("red"));
        assert!(style_3.contains("red"));

        // Different widths for different nesting levels
        assert!(style_1.contains("penwidth=2"));
        assert!(style_3.contains("penwidth=4"));
    }
}
