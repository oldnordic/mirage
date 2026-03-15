//! Mirage - Path-Aware Code Intelligence Engine
//!
//! A control-flow and logic graph engine for multi-language codebases.
//!
//! # Getting Started
//!
//! ```rust,no_run
//! use mirage_analyzer::Backend;
//! use std::path::Path;
//!
//! // Auto-detect and open the database backend
//! let backend = Backend::detect_and_open(Path::new("codegraph.db"))?;
//!
//! // Query CFG blocks (works with both SQLite and native-v3)
//! let blocks = backend.get_cfg_blocks(123)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Backend Support
//!
//! Mirage supports two storage backends:
//! - **SQLite**: Default backend, backward compatible with Magellan v7+
//! - **Native-V3**: High-performance KV backend (requires `backend-native-v3` feature)
//!
//! The backend is automatically detected from the database file format.
//!
//! # Public API
//!
//! - [`Backend`] - Enum wrapping storage backends with auto-detection
//! - [`StorageTrait`] - Backend-agnostic storage interface
//! - [`MirageDb`] - Legacy database connection (wraps Backend internally)

#![allow(dead_code)]

// Compile-time guard: prevent enabling both backends simultaneously
// This must be at the lib level since storage/mod.rs is compiled first
#[cfg(all(feature = "backend-sqlite", feature = "backend-native-v3"))]
compile_error!(
    "Features 'backend-sqlite' and 'backend-native-v3' are mutually exclusive. \
     Enable only one backend feature. Use either: \
     \
     SQLite (default): cargo build \
     Native-V3: cargo build --features backend-native-v3 --no-default-features"
);

pub mod analysis;
pub mod cli;
pub mod cfg;
pub mod integrations;
pub mod mir;
pub mod output;
pub mod router;
pub mod storage;

// Public API exports
pub use storage::{MirageDb, create_schema, DatabaseStatus, Backend, StorageTrait, CfgBlockData};
