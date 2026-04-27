use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[cfg(feature = "backend-geometric")]
pub mod geometric;
pub mod sqlite;

#[cfg(feature = "backend-geometric")]
pub use geometric::GeometricRouter;
#[cfg(feature = "backend-sqlite")]
pub use sqlite::SqliteRouter;

// Priority-based Router type alias:
// - Geometric takes priority if enabled
// - Otherwise SQLite if enabled
#[cfg(feature = "backend-geometric")]
pub type Router = GeometricRouter;
#[cfg(all(not(feature = "backend-geometric"), feature = "backend-sqlite"))]
pub type Router = SqliteRouter;

pub trait BackendRouter: Sized {
    fn open(db_path: &Path) -> Result<Self>;
    fn status(&self) -> Result<DatabaseStatus>;
    fn load_cfg(&self, function_id: i64) -> Result<crate::cfg::Cfg>;
    fn resolve_function(&self, name_or_id: &str) -> Result<i64>;
    fn get_function_name(&self, function_id: i64) -> Option<String>;
    fn get_function_file(&self, function_id: i64) -> Option<String>;
    fn function_exists(&self, function_id: i64) -> bool;
    fn enumerate_paths(&self, function_id: i64, max_paths: usize) -> Result<Vec<ExecutionPath>>;
    fn get_cfg_blocks(&self, function_id: i64) -> Result<Vec<CfgBlockInfo>>;
    fn get_dominators(&self, function_id: i64) -> Result<DominatorTree>;
    fn get_loops(&self, function_id: i64) -> Result<Vec<NaturalLoop>>;
    fn find_unreachable(&self, within_functions: bool) -> Result<Vec<UnreachableCode>>;
    fn get_patterns(&self, function_id: i64) -> Result<Vec<BranchPattern>>;
    fn get_frontiers(&self, function_id: i64) -> Result<DominanceFrontiers>;
    fn find_cycles(&self) -> Result<Vec<CallCycle>>;
    fn get_blast_zone(&self, function_id: i64, block_id: Option<i64>) -> Result<BlastZone>;
    fn slice(&self, symbol: &str, direction: SliceDirection) -> Result<SliceResult>;
    fn get_hotspots(&self) -> Result<Vec<Hotspot>>;
    fn get_hotpaths(&self, function_id: Option<i64>) -> Result<Vec<HotPath>>;
    fn verify_path(&self, path_id: &str) -> Result<PathVerification>;
    fn get_icfg(&self, function_id: i64) -> Result<InterProceduralCfg>;
    fn get_call_graph(&self) -> Result<CallGraph>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatus {
    pub cfg_blocks: i64,
    pub cfg_paths: i64,
    pub cfg_dominators: i64,
    pub mirage_schema_version: i32,
    pub magellan_schema_version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPath {
    pub path_id: String,
    pub blocks: Vec<i64>,
    pub length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgBlockInfo {
    pub id: i64,
    pub kind: String,
    pub terminator: Option<String>,
    pub byte_start: u64,
    pub byte_end: u64,
    pub start_line: u64,
    pub start_col: u64,
    pub end_line: u64,
    pub end_col: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DominatorTree {
    pub function_id: i64,
    pub dominators: std::collections::HashMap<i64, Vec<i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NaturalLoop {
    pub header: i64,
    pub blocks: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreachableCode {
    pub function_id: i64,
    pub block_id: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchPattern {
    pub pattern_id: String,
    pub kind: String,
    pub blocks: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DominanceFrontiers {
    pub function_id: i64,
    pub frontiers: std::collections::HashMap<i64, Vec<i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallCycle {
    pub cycle_id: String,
    pub functions: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastZone {
    pub center_function: i64,
    pub center_block: Option<i64>,
    pub affected_functions: Vec<i64>,
    pub affected_blocks: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceResult {
    pub symbol: String,
    pub direction: String,
    pub affected_symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotspot {
    pub function_id: i64,
    pub complexity: f64,
    pub frequency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPath {
    pub path: Vec<i64>,
    pub frequency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathVerification {
    pub path_id: String,
    pub valid: bool,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterProceduralCfg {
    pub entry_function: i64,
    pub nodes: Vec<IcfgNode>,
    pub edges: Vec<IcfgEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcfgNode {
    pub id: i64,
    pub function_id: i64,
    pub block_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcfgEdge {
    pub from_node: i64,
    pub to_node: i64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraph {
    pub nodes: Vec<CallGraphNode>,
    pub edges: Vec<CallGraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraphNode {
    pub id: i64,
    pub function_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraphEdge {
    pub caller_id: i64,
    pub callee_id: i64,
}

pub enum SliceDirection {
    Forward,
    Backward,
    Both,
}
