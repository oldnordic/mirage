//! MIR extraction and translation

pub mod charon_llbc;
pub mod translator;

use crate::cfg::Cfg;
use anyhow::Result;
use std::path::Path;

/// Extract CFGs from a Rust project using Charon
pub fn extract_rust_cfg(_project_path: &Path) -> Result<Vec<Cfg>> {
    // TODO: Implement orchestrator in Milestone 2
    Ok(vec![])
}
