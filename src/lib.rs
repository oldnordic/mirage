// Mirage: Path-Aware Code Intelligence Engine
//
// A control-flow and logic graph engine for Rust codebases.

#![allow(dead_code)]

pub mod analysis;
pub mod cli;
pub mod cfg;
pub mod mir;
pub mod output;
pub mod storage;

pub use storage::{MirageDb, create_schema, DatabaseStatus};
