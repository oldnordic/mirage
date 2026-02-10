//! Mirage - Path-Aware Code Intelligence Engine
//!
//! A control-flow and logic graph engine for Rust codebases.
//!
//! # Getting Started
//!
//! ```rust,no_run
//! use mirage_analyzer::Backend;
//!
//! // Auto-detect and open the database backend
//! let backend = Backend::detect_and_open("codegraph.db")?;
//!
//! // Query CFG blocks (works with both SQLite and native-v2)
//! let blocks = backend.get_cfg_blocks(123)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Backend Support
//!
//! Mirage supports two storage backends:
//! - **SQLite**: Default backend, backward compatible with Magellan v7+
//! - **Native-V2**: High-performance KV backend (requires `backend-native-v2` feature)
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
#[cfg(all(feature = "sqlite", feature = "native-v2"))]
compile_error!(
    "Features 'sqlite' and 'native-v2' are mutually exclusive. \
     Enable only one backend feature. Remove one of: \
     --features sqlite \
     --features native-v2 \
     \
     Default is SQLite, so use `cargo build` with no features, or \
     `cargo build --features native-v2` for the native-v2 backend."
);

pub mod analysis;
pub mod cli;
pub mod cfg;
pub mod mir;
pub mod output;
pub mod storage;

// Public API exports
pub use storage::{MirageDb, create_schema, DatabaseStatus, Backend, StorageTrait, CfgBlockData};
