//! Inter-procedural Control Flow Graph (ICFG) construction
//!
//! This module provides functionality for building ICFGs that connect
//! multiple function CFGs via call/return edges from Magellan's CALLS graph.
//!
//! # ICFG Construction
//!
//! The ICFG combines individual function CFGs into a single graph that shows
//! both intra-procedural control flow (within a function) and inter-procedural
//! flow (function calls and returns).
//!
//! # Algorithm
//!
//! 1. Start from entry function CFG
//! 2. For each call block in current CFG:
//!    a. Query Magellan's CALLS edges to find callee
//!    b. Load callee CFG and add to ICFG
//!    c. Add call edge from call site to function entry
//!    d. Add return edge from function exit back to call site
//! 3. Repeat for depth limit (prevent infinite recursion)
//!
//! # Examples
//!
//! ```no_run
//! use mirage_analyzer::cfg::icfg::{build_icfg, IcfgOptions, to_dot};
//! use mirage_analyzer::storage::MirageDb;
//!
//! # fn main() -> anyhow::Result<()> {
//! let db = MirageDb::open("codegraph.db")?;
//! let function_id = 123; // Entry function ID
//!
//! let options = IcfgOptions {
//!     max_depth: 3,
//!     include_return_edges: true,
//! };
//!
//! let icfg = build_icfg(
//!     db.storage(),
//!     db.backend(),
//!     function_id,
//!     options,
//! )?;
//!
//! println!("{}", to_dot(&icfg));
//! # Ok(())
//! # }
//! ```

use anyhow::Result;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use sqlitegraph::{GraphBackend, NeighborQuery, SnapshotId};

use crate::storage::CfgBlockData;

/// Inter-procedural Control Flow Graph
///
/// Combines multiple function CFGs with call/return edges.
#[derive(Debug, Clone)]
pub struct Icfg {
    /// Combined graph with all function CFGs
    pub graph: DiGraph<IcfgNode, IcfgEdge>,
    /// Mapping from (function_id, block_id) to node index
    pub node_map: HashMap<(i64, i64), NodeIndex>,
    /// Entry function ID
    pub entry_function: i64,
}

/// Node in the ICFG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcfgNode {
    pub function_id: i64,
    pub function_name: Option<String>,
    pub block_id: i64,
    pub node_type: IcfgNodeType,
}

/// Type of ICFG node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IcfgNodeType {
    /// Normal basic block
    BasicBlock,
    /// Block containing a function call
    CallSite,
    /// Function entry point
    FunctionEntry,
    /// Function exit point
    FunctionExit,
}

/// Edge in the ICFG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IcfgEdge {
    /// Intra-procedural edge (within a function)
    IntraProcedural {
        edge_type: String,
    },
    /// Call edge from call site to function entry
    Call {
        from_function: i64,
        to_function: i64,
    },
    /// Return edge from function exit back to call site
    Return {
        from_function: i64,
        to_function: i64,
    },
}

/// ICFG construction options
#[derive(Debug, Clone)]
pub struct IcfgOptions {
    pub max_depth: usize,
    pub include_return_edges: bool,
}

impl Default for IcfgOptions {
    fn default() -> Self {
        Self {
            max_depth: 3,
            include_return_edges: true,
        }
    }
}

/// Build inter-procedural CFG starting from entry function
///
/// # Algorithm
///
/// 1. Start from entry function CFG
/// 2. For each call block in current CFG:
///    a. Query Magellan's CALLS edges to find callee
///    b. Load callee CFG and add to ICFG
///    c. Add call edge from call site to function entry
///    d. Add return edge from function exit back to call site
/// 3. Repeat for depth limit (prevent infinite recursion)
///
/// # Arguments
///
/// * `storage` - Storage trait implementation for CFG data
/// * `backend` - GraphBackend for CALLS queries
/// * `entry_function` - Entry function ID
/// * `options` - Construction options
///
/// # Returns
///
/// * `Ok(Icfg)` - Constructed inter-procedural CFG
pub fn build_icfg(
    storage: &dyn crate::storage::StorageTrait,
    backend: &dyn GraphBackend,
    entry_function: i64,
    options: IcfgOptions,
) -> Result<Icfg> {
    let snapshot = SnapshotId::current();

    let mut icfg = Icfg {
        graph: DiGraph::new(),
        node_map: HashMap::new(),
        entry_function,
    };

    let mut queue = vec![(entry_function, 0)];
    let mut visited = HashSet::new();

    // Track call sites for return edges
    let mut call_sites: HashMap<(i64, i64), NodeIndex> = HashMap::new();

    while let Some((function_id, depth)) = queue.pop() {
        if depth > options.max_depth || visited.contains(&function_id) {
            continue;
        }
        visited.insert(function_id);

        // Load CFG for this function
        let blocks = storage.get_cfg_blocks(function_id)?;

        if blocks.is_empty() {
            // No CFG data - skip this function
            continue;
        }

        // Add entry/exit nodes
        let entry_idx = icfg.graph.add_node(IcfgNode {
            function_id,
            function_name: get_function_name(backend, function_id)?,
            block_id: -1, // Special ID for entry
            node_type: IcfgNodeType::FunctionEntry,
        });
        icfg.node_map.insert((function_id, -1), entry_idx);

        let exit_idx = icfg.graph.add_node(IcfgNode {
            function_id,
            function_name: get_function_name(backend, function_id)?,
            block_id: -2, // Special ID for exit
            node_type: IcfgNodeType::FunctionExit,
        });
        icfg.node_map.insert((function_id, -2), exit_idx);

        // Add all blocks to ICFG
        for block in &blocks {
            let node_idx = icfg.graph.add_node(IcfgNode {
                function_id,
                function_name: get_function_name(backend, function_id)?,
                block_id: block.id,
                node_type: if block.terminator == "call" {
                    IcfgNodeType::CallSite
                } else {
                    IcfgNodeType::BasicBlock
                },
            });
            icfg.node_map.insert((function_id, block.id), node_idx);
        }

        // Add intra-procedural edges
        for (idx, block) in blocks.iter().enumerate() {
            let from_idx = icfg.node_map[&(function_id, block.id)];

            match block.terminator.as_str() {
                "fallthrough" | "goto" | "call" => {
                    if idx + 1 < blocks.len() {
                        let to_idx = icfg.node_map[&(function_id, blocks[idx + 1].id)];
                        icfg.graph.add_edge(from_idx, to_idx, IcfgEdge::IntraProcedural {
                            edge_type: "fallthrough".to_string(),
                        });
                    }
                }
                "conditional" => {
                    // Two edges: true and false branches
                    if idx + 1 < blocks.len() {
                        let to_idx = icfg.node_map[&(function_id, blocks[idx + 1].id)];
                        icfg.graph.add_edge(from_idx, to_idx, IcfgEdge::IntraProcedural {
                            edge_type: "true".to_string(),
                        });
                    }
                    if idx + 2 < blocks.len() {
                        let to_idx = icfg.node_map[&(function_id, blocks[idx + 2].id)];
                        icfg.graph.add_edge(from_idx, to_idx, IcfgEdge::IntraProcedural {
                            edge_type: "false".to_string(),
                        });
                    }
                }
                "return" | "panic" | "break" | "continue" => {
                    // No outgoing edges
                }
                _ => {
                    // Unknown terminator - no edge
                }
            }

            // Track call sites for inter-procedural edges
            if block.terminator == "call" {
                call_sites.insert((function_id, block.id), from_idx);
            }
        }

        // Connect entry to first block
        if let Some(first_block) = blocks.first() {
            let entry = icfg.node_map[&(function_id, -1)];
            let first = icfg.node_map[&(function_id, first_block.id)];
            icfg.graph.add_edge(entry, first, IcfgEdge::IntraProcedural {
                edge_type: "entry".to_string(),
            });
        }

        // Query CALLS graph to find callees for call sites
        for (block_idx, block) in blocks.iter().enumerate() {
            if block.terminator != "call" {
                continue;
            }

            // Query GraphBackend for CALLS neighbors from this function
            let query = NeighborQuery {
                edge_type: Some("CALLS".to_string()),
                ..Default::default()
            };
            let calls_result = backend.neighbors(snapshot, function_id, query);

            let callee_ids = match calls_result {
                Ok(ids) => ids,
                Err(_) => continue, // Skip if CALLS query fails
            };

            for callee_id in callee_ids {
                // Check if we should visit this callee
                if depth + 1 <= options.max_depth && !visited.contains(&callee_id) {
                    queue.push((callee_id, depth + 1));
                }

                // Add call edge from call site to callee entry
                let call_site_idx = call_sites[&(function_id, block.id)];
                let callee_entry = icfg.node_map.get(&(callee_id, -1));

                if let Some(&entry_idx) = callee_entry {
                    icfg.graph.add_edge(call_site_idx, entry_idx, IcfgEdge::Call {
                        from_function: function_id,
                        to_function: callee_id,
                    });

                    // Add return edge from callee exit back to call site's successor
                    if options.include_return_edges {
                        if let Some(exit_idx) = icfg.node_map.get(&(callee_id, -2)) {
                            // Find successor to call site (next block after call)
                            if block_idx + 1 < blocks.len() {
                                let successor_idx = icfg.node_map[&(function_id, blocks[block_idx + 1].id)];
                                icfg.graph.add_edge(*exit_idx, successor_idx, IcfgEdge::Return {
                                    from_function: callee_id,
                                    to_function: function_id,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(icfg)
}

/// Get function name from entity ID
///
/// Queries the GraphBackend to retrieve the function name for a given entity ID.
fn get_function_name(backend: &dyn GraphBackend, entity_id: i64) -> Result<Option<String>> {
    let snapshot = SnapshotId::current();
    match backend.get_node(snapshot, entity_id) {
        Ok(entity) => Ok(Some(entity.name)),
        Err(_) => Ok(None),
    }
}

/// Export ICFG to DOT format for visualization
pub fn to_dot(icfg: &Icfg) -> String {
    let mut dot = String::from("digraph ICFG {\n");
    dot.push_str("  rankdir=TB;\n");
    dot.push_str("  node [shape=box];\n\n");

    // Add nodes
    for node in icfg.graph.node_indices() {
        let node_data = &icfg.graph[node];
        let label = format!(
            "F{}_B{}",
            node_data.function_id,
            if node_data.block_id < 0 {
                match node_data.node_type {
                    IcfgNodeType::FunctionEntry => "entry".to_string(),
                    IcfgNodeType::FunctionExit => "exit".to_string(),
                    _ => "unknown".to_string(),
                }
            } else {
                node_data.block_id.to_string()
            }
        );

        let style = match node_data.node_type {
            IcfgNodeType::CallSite => " [style=dashed]",
            IcfgNodeType::FunctionEntry => " [style=bold]",
            IcfgNodeType::FunctionExit => " [style=bold]",
            _ => "",
        };

        dot.push_str(&format!("  \"{}\"{};\n", node.index(), style));
    }

    // Add edges
    for edge in icfg.graph.edge_indices() {
        let (from, to) = icfg.graph.edge_endpoints(edge).unwrap();
        let edge_data = &icfg.graph[edge];

        let label = match edge_data {
            IcfgEdge::IntraProcedural { edge_type } => edge_type.clone(),
            IcfgEdge::Call { .. } => "call".to_string(),
            IcfgEdge::Return { .. } => "return".to_string(),
        };

        let style = match edge_data {
            IcfgEdge::Call { .. } => " [style=bold,color=blue]",
            IcfgEdge::Return { .. } => " [style=dashed,color=red]",
            _ => "",
        };

        dot.push_str(&format!("  \"{}\" -> \"{}\" [label=\"{}\"{}];\n",
            from.index(), to.index(), label, style));
    }

    dot.push_str("}\n");
    dot
}

/// JSON-serializable representation of ICFG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcfgJson {
    pub entry_function: i64,
    pub nodes: Vec<IcfgNodeJson>,
    pub edges: Vec<IcfgEdgeJson>,
    pub function_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcfgNodeJson {
    pub id: usize,
    pub function_id: i64,
    pub function_name: Option<String>,
    pub block_id: i64,
    pub node_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcfgEdgeJson {
    pub from: usize,
    pub to: usize,
    pub edge_type: String,
    pub label: String,
}

impl IcfgJson {
    pub fn from_icfg(icfg: &Icfg) -> Self {
        use std::collections::HashSet;

        let mut function_ids = HashSet::new();

        let nodes: Vec<IcfgNodeJson> = icfg.graph.node_indices()
            .map(|idx| {
                let node = &icfg.graph[idx];
                function_ids.insert(node.function_id);
                IcfgNodeJson {
                    id: idx.index(),
                    function_id: node.function_id,
                    function_name: node.function_name.clone(),
                    block_id: node.block_id,
                    node_type: format!("{:?}", node.node_type),
                }
            })
            .collect();

        let edges: Vec<IcfgEdgeJson> = icfg.graph.edge_indices()
            .map(|idx| {
                let (from, to) = icfg.graph.edge_endpoints(idx).unwrap();
                let edge = &icfg.graph[idx];
                let (edge_type, label) = match edge {
                    IcfgEdge::IntraProcedural { edge_type } => ("intra", edge_type.clone()),
                    IcfgEdge::Call { .. } => ("call", "call".to_string()),
                    IcfgEdge::Return { .. } => ("return", "return".to_string()),
                };
                IcfgEdgeJson {
                    from: from.index(),
                    to: to.index(),
                    edge_type: edge_type.to_string(),
                    label,
                }
            })
            .collect();

        IcfgJson {
            entry_function: icfg.entry_function,
            nodes,
            edges,
            function_count: function_ids.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icfg_options_default() {
        let options = IcfgOptions::default();
        assert_eq!(options.max_depth, 3);
        assert!(options.include_return_edges);
    }

    #[test]
    fn test_icfg_node_types() {
        let entry = IcfgNodeType::FunctionEntry;
        let exit = IcfgNodeType::FunctionExit;
        let call = IcfgNodeType::CallSite;
        let basic = IcfgNodeType::BasicBlock;

        assert_eq!(entry, IcfgNodeType::FunctionEntry);
        assert_eq!(exit, IcfgNodeType::FunctionExit);
        assert_eq!(call, IcfgNodeType::CallSite);
        assert_eq!(basic, IcfgNodeType::BasicBlock);
    }
}
