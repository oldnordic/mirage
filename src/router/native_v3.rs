//! Native V3 Backend Router Implementation
//!
//! This module implements the BackendRouter trait for Native V3 databases.

use super::*;
use anyhow::Result;
use std::path::Path;

/// Native V3 backend router
pub struct NativeV3Router;

impl BackendRouter for NativeV3Router {
    fn open(_db_path: &Path) -> Result<Self> {
        anyhow::bail!("Native V3 backend not yet implemented")
    }

    fn status(&self) -> Result<DatabaseStatus> {
        anyhow::bail!("Not implemented")
    }

    fn load_cfg(&self, _function_id: i64) -> Result<crate::cfg::Cfg> {
        anyhow::bail!("Not implemented")
    }

    fn resolve_function(&self, _name_or_id: &str) -> Result<i64> {
        anyhow::bail!("Not implemented")
    }

    fn get_function_name(&self, _function_id: i64) -> Option<String> {
        None
    }

    fn get_function_file(&self, _function_id: i64) -> Option<String> {
        None
    }

    fn function_exists(&self, _function_id: i64) -> bool {
        false
    }

    fn enumerate_paths(&self, _function_id: i64, _max_paths: usize) -> Result<Vec<ExecutionPath>> {
        anyhow::bail!("Not implemented")
    }

    fn get_cfg_blocks(&self, _function_id: i64) -> Result<Vec<CfgBlockInfo>> {
        anyhow::bail!("Not implemented")
    }

    fn get_dominators(&self, _function_id: i64) -> Result<DominatorTree> {
        anyhow::bail!("Not implemented")
    }

    fn get_loops(&self, _function_id: i64) -> Result<Vec<NaturalLoop>> {
        anyhow::bail!("Not implemented")
    }

    fn find_unreachable(&self, _within_functions: bool) -> Result<Vec<UnreachableCode>> {
        anyhow::bail!("Not implemented")
    }

    fn get_patterns(&self, _function_id: i64) -> Result<Vec<BranchPattern>> {
        anyhow::bail!("Not implemented")
    }

    fn get_frontiers(&self, _function_id: i64) -> Result<DominanceFrontiers> {
        anyhow::bail!("Not implemented")
    }

    fn find_cycles(&self) -> Result<Vec<CallCycle>> {
        anyhow::bail!("Not implemented")
    }

    fn get_blast_zone(&self, _function_id: i64, _block_id: Option<i64>) -> Result<BlastZone> {
        anyhow::bail!("Not implemented")
    }

    fn slice(&self, _symbol: &str, _direction: SliceDirection) -> Result<SliceResult> {
        anyhow::bail!("Not implemented")
    }

    fn get_hotspots(&self) -> Result<Vec<Hotspot>> {
        anyhow::bail!("Not implemented")
    }

    fn get_hotpaths(&self, _function_id: Option<i64>) -> Result<Vec<HotPath>> {
        anyhow::bail!("Not implemented")
    }

    fn verify_path(&self, _path_id: &str) -> Result<PathVerification> {
        anyhow::bail!("Not implemented")
    }

    fn get_icfg(&self, _function_id: i64) -> Result<InterProceduralCfg> {
        anyhow::bail!("Not implemented")
    }

    fn get_call_graph(&self) -> Result<CallGraph> {
        anyhow::bail!("Not implemented")
    }
}
