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
//! use mirage::cfg::icfg::{build_icfg, IcfgOptions, to_dot};
//! use mirage::storage::MirageDb;
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
//!     db.path(),
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
use petgraph::visit::EdgeRef;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use sqlitegraph::{GraphBackend, NeighborQuery, SnapshotId};

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
    IntraProcedural { edge_type: String },
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
    db_path: &Path,
    entry_function: i64,
    options: IcfgOptions,
) -> Result<Icfg> {
    let snapshot = SnapshotId::current();
    let mut magellan_graph = magellan::CodeGraph::open(db_path)?;

    let mut icfg = Icfg {
        graph: DiGraph::new(),
        node_map: HashMap::new(),
        entry_function,
    };

    let mut queue = vec![(entry_function, 0)];
    let mut visited = HashSet::new();

    // Collect caller->callee relationships for second-pass edge insertion.
    // We defer inter-procedural edges until after all functions have been
    // processed so that callee nodes are guaranteed to exist.
    let mut call_relations: Vec<PreciseCallRelation> = Vec::new();

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
            block_id: -1,
            node_type: IcfgNodeType::FunctionEntry,
        });
        icfg.node_map.insert((function_id, -1), entry_idx);

        let exit_idx = icfg.graph.add_node(IcfgNode {
            function_id,
            function_name: get_function_name(backend, function_id)?,
            block_id: -2,
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
                    add_intra_edge(
                        &mut icfg,
                        function_id,
                        &blocks,
                        idx + 1,
                        from_idx,
                        "fallthrough",
                    );
                }
                "conditional" => {
                    add_intra_edge(&mut icfg, function_id, &blocks, idx + 1, from_idx, "true");
                    add_intra_edge(&mut icfg, function_id, &blocks, idx + 2, from_idx, "false");
                }
                "return" | "panic" | "break" | "continue" => {}
                _ => {}
            }
        }

        // Connect entry to first block
        if let Some(first_block) = blocks.first() {
            let entry = icfg.node_map[&(function_id, -1)];
            let first = icfg.node_map[&(function_id, first_block.id)];
            icfg.graph.add_edge(
                entry,
                first,
                IcfgEdge::IntraProcedural {
                    edge_type: "entry".to_string(),
                },
            );
        }

        // Discover callees for this function via call graph
        let query = NeighborQuery {
            edge_type: Some("CALLS".to_string()),
            ..Default::default()
        };
        let calls_result = backend.neighbors(snapshot, function_id, query);

        let mut callee_ids = calls_result.unwrap_or_default();
        let precise_relations = get_precise_call_relations(
            &mut magellan_graph,
            backend,
            db_path,
            function_id,
            options.include_return_edges,
        )?;
        if !precise_relations.is_empty() {
            callee_ids = precise_relations.iter().map(|rel| rel.callee_id).collect();
        }

        // Fallback: if GraphBackend returns empty, try storage.get_callees.
        if callee_ids.is_empty() {
            if let Ok(ids) = storage.get_callees(function_id) {
                callee_ids = ids;
            }
        }

        // Queue callees for expansion
        for callee_id in &callee_ids {
            if depth < options.max_depth && !visited.contains(callee_id) {
                queue.push((*callee_id, depth + 1));
            }
        }

        // Record caller->callee relationship for second-pass edge insertion
        if !precise_relations.is_empty() {
            call_relations.extend(precise_relations);
        } else if !callee_ids.is_empty() {
            call_relations.extend(callee_ids.into_iter().map(|callee_id| PreciseCallRelation {
                caller_id: function_id,
                callee_id,
                caller_call_block_id: None,
                caller_resume_block_id: None,
                callee_return_block_ids: Vec::new(),
            }));
        }
    }

    // Second pass: add inter-procedural edges after all nodes exist.
    for relation in &call_relations {
        let caller_edge_from = relation
            .caller_call_block_id
            .and_then(|block_id| icfg.node_map.get(&(relation.caller_id, block_id)).copied())
            .unwrap_or_else(|| icfg.node_map[&(relation.caller_id, -1)]);

        if let Some(&callee_entry_idx) = icfg.node_map.get(&(relation.callee_id, -1)) {
            icfg.graph.add_edge(
                caller_edge_from,
                callee_entry_idx,
                IcfgEdge::Call {
                    from_function: relation.caller_id,
                    to_function: relation.callee_id,
                },
            );
        }

        if options.include_return_edges {
            if !relation.callee_return_block_ids.is_empty() {
                if let Some(caller_resume_idx) =
                    relation.caller_resume_block_id.and_then(|block_id| {
                        icfg.node_map.get(&(relation.caller_id, block_id)).copied()
                    })
                {
                    for return_block_id in &relation.callee_return_block_ids {
                        if let Some(&callee_return_idx) =
                            icfg.node_map.get(&(relation.callee_id, *return_block_id))
                        {
                            icfg.graph.add_edge(
                                callee_return_idx,
                                caller_resume_idx,
                                IcfgEdge::Return {
                                    from_function: relation.callee_id,
                                    to_function: relation.caller_id,
                                },
                            );
                        }
                    }
                    continue;
                }
            }

            if let (Some(&callee_exit_idx), Some(&caller_exit_idx)) = (
                icfg.node_map.get(&(relation.callee_id, -2)),
                icfg.node_map.get(&(relation.caller_id, -2)),
            ) {
                icfg.graph.add_edge(
                    callee_exit_idx,
                    caller_exit_idx,
                    IcfgEdge::Return {
                        from_function: relation.callee_id,
                        to_function: relation.caller_id,
                    },
                );
            }
        }
    }

    Ok(icfg)
}

/// Enumerate execution paths through an ICFG by projecting it into the shared
/// CFG path engine with synthetic node IDs equal to ICFG node indices.
pub fn enumerate_icfg_paths(icfg: &Icfg, limits: &crate::cfg::PathLimits) -> Vec<crate::cfg::Path> {
    let cfg = project_icfg_to_cfg(icfg);
    crate::cfg::enumerate_paths(&cfg, limits)
}

/// Project an ICFG onto Mirage's CFG core so existing loop-aware path
/// enumeration can reason across call and return edges without duplicating the
/// DFS machinery.
pub fn project_icfg_to_cfg(icfg: &Icfg) -> crate::cfg::Cfg {
    use crate::cfg::{BasicBlock, BlockKind, EdgeType};

    let mut cfg = DiGraph::new();
    let mut node_map = HashMap::new();

    for node_idx in icfg.graph.node_indices() {
        let node = &icfg.graph[node_idx];
        let synthetic_id = node_idx.index();
        let outgoing = sorted_icfg_successors(icfg, node_idx);
        let terminator = synthetic_terminator(node, &outgoing);
        let kind = match node.node_type {
            IcfgNodeType::FunctionEntry => BlockKind::Entry,
            IcfgNodeType::FunctionExit => BlockKind::Exit,
            _ if outgoing.is_empty() => BlockKind::Exit,
            _ => BlockKind::Normal,
        };

        let statements = vec![match (&node.function_name, node.block_id) {
            (Some(function_name), block_id) if block_id >= 0 => {
                format!("{}::block_{}", function_name, block_id)
            }
            (Some(function_name), -1) => format!("{}::entry", function_name),
            (Some(function_name), -2) => format!("{}::exit", function_name),
            (Some(function_name), block_id) => format!("{}::block_{}", function_name, block_id),
            (None, block_id) => format!("block_{}", block_id),
        }];

        let cfg_idx = cfg.add_node(BasicBlock {
            id: synthetic_id,
            db_id: None,
            kind,
            statements,
            terminator,
            source_location: None,
        });
        node_map.insert(node_idx, cfg_idx);
    }

    for edge_idx in icfg.graph.edge_indices() {
        let (from, to) = icfg
            .graph
            .edge_endpoints(edge_idx)
            .expect("invariant: edge from graph.edge_indices()");
        let edge_type = match &icfg.graph[edge_idx] {
            IcfgEdge::IntraProcedural { edge_type } => match edge_type.as_str() {
                "true" => EdgeType::TrueBranch,
                "false" => EdgeType::FalseBranch,
                "loop" => EdgeType::LoopBack,
                _ => EdgeType::Fallthrough,
            },
            IcfgEdge::Call { .. } => EdgeType::Call,
            IcfgEdge::Return { .. } => EdgeType::Return,
        };
        cfg.add_edge(node_map[&from], node_map[&to], edge_type);
    }

    cfg
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

#[derive(Debug, Clone)]
struct PreciseCallRelation {
    caller_id: i64,
    callee_id: i64,
    caller_call_block_id: Option<i64>,
    caller_resume_block_id: Option<i64>,
    callee_return_block_ids: Vec<i64>,
}

fn get_precise_call_relations(
    magellan_graph: &mut magellan::CodeGraph,
    backend: &dyn GraphBackend,
    db_path: &Path,
    function_id: i64,
    include_return_edges: bool,
) -> Result<Vec<PreciseCallRelation>> {
    let snapshot = SnapshotId::current();
    let entity = match backend.get_node(snapshot, function_id) {
        Ok(entity) => entity,
        Err(_) => return Ok(Vec::new()),
    };
    let Some(file_path) = entity.file_path.as_deref() else {
        return Ok(Vec::new());
    };

    let stitched = magellan_graph.direct_call_icfg_edges(file_path, &entity.name)?;
    let caller_block_ids = load_cfg_block_ids_in_magellan_order(db_path, function_id)?;
    let mut relations = Vec::new();
    for edge in stitched {
        if edge.caller_symbol_id != function_id {
            continue;
        }

        let caller_call_block_id = caller_block_ids.get(edge.caller_block_idx).copied();
        let caller_resume_block_id = edge
            .caller_resume_block_idx
            .and_then(|idx| caller_block_ids.get(idx).copied());

        let callee_return_block_ids = if include_return_edges {
            let callee_block_ids =
                load_cfg_block_ids_in_magellan_order(db_path, edge.callee_symbol_id)?;
            edge.callee_return_block_indices
                .iter()
                .filter_map(|idx| callee_block_ids.get(*idx).copied())
                .collect()
        } else {
            Vec::new()
        };

        relations.push(PreciseCallRelation {
            caller_id: function_id,
            callee_id: edge.callee_symbol_id,
            caller_call_block_id,
            caller_resume_block_id,
            callee_return_block_ids,
        });
    }

    Ok(relations)
}

fn load_cfg_block_ids_in_magellan_order(db_path: &Path, function_id: i64) -> Result<Vec<i64>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id
         FROM cfg_blocks
         WHERE function_id = ?1
         ORDER BY byte_start, id",
    )?;
    let ids = stmt
        .query_map(rusqlite::params![function_id], |row| row.get(0))?
        .collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

fn add_intra_edge(
    icfg: &mut Icfg,
    function_id: i64,
    blocks: &[crate::storage::CfgBlockData],
    target_idx: usize,
    from_idx: NodeIndex,
    edge_type: &str,
) {
    if let Some(target_block) = blocks.get(target_idx) {
        let to_idx = icfg.node_map[&(function_id, target_block.id)];
        icfg.graph.add_edge(
            from_idx,
            to_idx,
            IcfgEdge::IntraProcedural {
                edge_type: edge_type.to_string(),
            },
        );
    }
}

fn sorted_icfg_successors(icfg: &Icfg, node_idx: NodeIndex) -> Vec<(NodeIndex, &IcfgEdge)> {
    let mut successors: Vec<_> = icfg
        .graph
        .edges(node_idx)
        .map(|edge| (edge.target(), edge.weight()))
        .collect();
    successors.sort_by_key(|(target, edge)| (icfg_edge_sort_key(edge), target.index()));
    successors
}

fn icfg_edge_sort_key(edge: &IcfgEdge) -> usize {
    match edge {
        IcfgEdge::IntraProcedural { edge_type } if edge_type == "true" => 0,
        IcfgEdge::IntraProcedural { edge_type } if edge_type == "false" => 1,
        IcfgEdge::IntraProcedural { .. } => 2,
        IcfgEdge::Call { .. } => 3,
        IcfgEdge::Return { .. } => 4,
    }
}

fn synthetic_terminator(
    node: &IcfgNode,
    outgoing: &[(NodeIndex, &IcfgEdge)],
) -> crate::cfg::Terminator {
    use crate::cfg::Terminator;

    if matches!(node.node_type, IcfgNodeType::FunctionExit) || outgoing.is_empty() {
        return Terminator::Return;
    }

    if outgoing.len() == 1 {
        let (target, edge) = outgoing[0];
        return match edge {
            IcfgEdge::Call { .. } => Terminator::Call {
                target: Some(target.index()),
                unwind: None,
            },
            _ => Terminator::Goto {
                target: target.index(),
            },
        };
    }

    let targets: Vec<_> = outgoing
        .iter()
        .take(outgoing.len() - 1)
        .map(|(target, _)| target.index())
        .collect();
    let otherwise = outgoing
        .last()
        .map(|(target, _)| target.index())
        .expect("non-empty outgoing when building SwitchInt");

    Terminator::SwitchInt { targets, otherwise }
}

/// Export ICFG to DOT format for visualization
pub fn to_dot(icfg: &Icfg) -> String {
    let mut dot = String::from("digraph ICFG {\n");
    dot.push_str("  rankdir=TB;\n");
    dot.push_str("  node [shape=box];\n\n");

    // Add nodes
    for node in icfg.graph.node_indices() {
        let node_data = &icfg.graph[node];
        let _label = format!(
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
        let (from, to) = icfg
            .graph
            .edge_endpoints(edge)
            .expect("invariant: edge from graph.edge_indices()");
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

        dot.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\"{}];\n",
            from.index(),
            to.index(),
            label,
            style
        ));
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

        let nodes: Vec<IcfgNodeJson> = icfg
            .graph
            .node_indices()
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

        let edges: Vec<IcfgEdgeJson> = icfg
            .graph
            .edge_indices()
            .map(|idx| {
                let (from, to) = icfg
                    .graph
                    .edge_endpoints(idx)
                    .expect("invariant: idx from graph.edge_indices()");
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
    use crate::cfg::PathLimits;
    use crate::storage::MirageDb;
    use petgraph::visit::EdgeRef;
    use tempfile::TempDir;

    /// Copy the checked-in caller/callee fixture DB into a fresh temp dir and
    /// return `(TempDir, db_path)`. The fixture was generated once with
    /// `magellan::CodeGraph::index_file` (magellan 4.13.1 + sqlitegraph 3.7.0)
    /// indexing exactly:
    ///
    /// ```rust
    /// fn callee(x: i32) -> i32 { return x + 1; }
    /// fn caller() -> i32 { let y = callee(41); return y + 1; }
    /// ```
    ///
    /// It is checked in as a binary because magellan's write path
    /// (`index_file`, pool_size = 1) deadlocks against sqlitegraph >= 3.9
    /// (nested pooled-connection checkout in `insert_entity` ->
    /// `bump_authoritative_version`), while mirage only ever *reads* magellan
    /// databases in production.
    fn copy_icfg_fixture(name: &str) -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/icfg_caller_callee.db");
        let db_path = temp_dir.path().join(name);
        std::fs::copy(&src, &db_path).unwrap();
        (temp_dir, db_path)
    }

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

    #[test]
    fn test_build_icfg_stitches_real_callsite_and_resume_blocks() {
        let (_temp_dir, db_path) = copy_icfg_fixture("icfg.db");

        let db = MirageDb::open(&db_path).unwrap();
        let caller_id = db.resolve_function_name("caller").unwrap();
        let caller_blocks = db.storage().get_cfg_blocks(caller_id).unwrap();
        let callee_id = db.resolve_function_name("callee").unwrap();
        let callee_blocks = db.storage().get_cfg_blocks(callee_id).unwrap();

        let call_block_id = caller_blocks
            .iter()
            .find(|block| block.kind == "call" && block.terminator == "call")
            .map(|block| block.id)
            .expect("caller should have a call block");
        let caller_resume_block_id = caller_blocks
            .iter()
            .find(|block| block.kind == "return")
            .map(|block| block.id)
            .expect("caller should have a return continuation block");
        let callee_return_block_id = callee_blocks
            .iter()
            .find(|block| block.kind == "return")
            .map(|block| block.id)
            .expect("callee should have a return block");

        let icfg = build_icfg(
            db.storage(),
            db.backend(),
            db.path(),
            caller_id,
            IcfgOptions::default(),
        )
        .unwrap();

        let caller_callsite_idx = icfg.node_map[&(caller_id, call_block_id)];
        let callee_entry_idx = icfg.node_map[&(callee_id, -1)];
        let callee_return_idx = icfg.node_map[&(callee_id, callee_return_block_id)];
        let caller_resume_idx = icfg.node_map[&(caller_id, caller_resume_block_id)];

        let has_precise_call_edge = icfg.graph.edges(caller_callsite_idx).any(|edge| {
            edge.target() == callee_entry_idx && matches!(edge.weight(), IcfgEdge::Call { .. })
        });
        assert!(
            has_precise_call_edge,
            "ICFG should connect caller callsite to callee entry"
        );

        let has_precise_return_edge = icfg.graph.edges(callee_return_idx).any(|edge| {
            edge.target() == caller_resume_idx && matches!(edge.weight(), IcfgEdge::Return { .. })
        });
        assert!(
            has_precise_return_edge,
            "ICFG should connect callee return block to caller continuation"
        );
    }

    #[test]
    fn test_enumerate_icfg_paths_crosses_call_and_return_edges() {
        let (_temp_dir, db_path) = copy_icfg_fixture("icfg-paths.db");

        let db = MirageDb::open(&db_path).unwrap();
        let caller_id = db.resolve_function_name("caller").unwrap();
        let callee_id = db.resolve_function_name("callee").unwrap();
        let caller_blocks = db.storage().get_cfg_blocks(caller_id).unwrap();
        let callee_blocks = db.storage().get_cfg_blocks(callee_id).unwrap();

        let caller_call_block_id = caller_blocks
            .iter()
            .find(|block| block.kind == "call" && block.terminator == "call")
            .map(|block| block.id)
            .expect("caller should have a call block");
        let caller_resume_block_id = caller_blocks
            .iter()
            .find(|block| block.kind == "return")
            .map(|block| block.id)
            .expect("caller should have a return continuation block");
        let callee_return_block_id = callee_blocks
            .iter()
            .find(|block| block.kind == "return")
            .map(|block| block.id)
            .expect("callee should have a return block");

        let icfg = build_icfg(
            db.storage(),
            db.backend(),
            db.path(),
            caller_id,
            IcfgOptions::default(),
        )
        .unwrap();

        let paths = enumerate_icfg_paths(&icfg, &PathLimits::default());

        assert!(
            paths.iter().any(|path| {
                let caller_call_pos = path.blocks.iter().position(|block_id| {
                    *block_id == icfg.node_map[&(caller_id, caller_call_block_id)].index()
                });
                let callee_return_pos = path.blocks.iter().position(|block_id| {
                    *block_id == icfg.node_map[&(callee_id, callee_return_block_id)].index()
                });
                let caller_resume_pos = path.blocks.iter().position(|block_id| {
                    *block_id == icfg.node_map[&(caller_id, caller_resume_block_id)].index()
                });

                matches!(
                    (caller_call_pos, callee_return_pos, caller_resume_pos),
                    (Some(call_pos), Some(return_pos), Some(resume_pos))
                        if call_pos < return_pos && return_pos < resume_pos
                )
            }),
            "ICFG path enumeration should include caller -> callee -> caller continuation flow"
        );
    }
}
