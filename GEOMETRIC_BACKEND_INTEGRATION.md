# Geometric Backend Integration Guide for Mirage

This document describes how to integrate the Magellan geometric backend into Mirage, based on the implementation done in llmgrep.

## Overview

The geometric backend (`.geo` files) provides spatial indexing and CFG analysis capabilities. This guide covers:
1. Cargo.toml configuration
2. Backend module structure
3. Backend detection and routing
4. Data mapping from Magellan to Mirage types
5. CLI command integration
6. Testing strategy

## 1. Cargo.toml Configuration

### Add magellan dependency with geometric feature

```toml
[dependencies]
# Change from:
# magellan = "2.4.8"
# To:
magellan = { path = "../magellan", features = ["geometric-backend"] }
```

### Key points:
- Use local path dependency to Magellan repo (not crates.io)
- Enable `geometric-backend` feature flag
- This pulls in `geographdb-core` and all geometric dependencies

## 2. Backend Module Structure

### Create src/backend/geometric.rs

This file wraps Magellan's `GeometricBackend` and implements your `BackendTrait`.

```rust
//! Geometric backend implementation for .geo database files.

use crate::error::YourError;
use crate::output::{YourOutputTypes};
use crate::query::SearchOptions;
use magellan::graph::geometric_backend::{
    GeometricBackend as MagellanGeometricBackend, SymbolInfo,
};
use std::path::Path;

pub struct GeometricBackend {
    inner: MagellanGeometricBackend,
    db_path: std::path::PathBuf,
}

impl std::fmt::Debug for GeometricBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeometricBackend")
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl GeometricBackend {
    pub fn open(db_path: &Path) -> Result<Self, YourError> {
        // Validate file exists
        if !db_path.exists() {
            return Err(YourError::DatabaseNotFound {
                path: db_path.display().to_string(),
            });
        }

        // Validate .geo extension
        match db_path.extension().and_then(|e| e.to_str()) {
            Some("geo") => {}
            _ => {
                return Err(YourError::BackendDetectionFailed {
                    path: db_path.display().to_string(),
                    reason: "File does not have .geo extension".to_string(),
                })
            }
        }

        // Open Magellan geometric backend
        let inner = MagellanGeometricBackend::open(db_path).map_err(|e| {
            YourError::BackendDetectionFailed {
                path: db_path.display().to_string(),
                reason: format!("Failed to open geometric backend: {}", e),
            }
        })?;

        Ok(Self {
            inner,
            db_path: db_path.to_path_buf(),
        })
    }
}
```

## 3. Backend Detection and Routing

### Update src/backend/mod.rs

Add Geometric variant to Backend enum:

```rust
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Backend {
    Sqlite(SqliteBackend),
    #[cfg(feature = "native-v3")]
    NativeV3(NativeV3Backend),
    Geometric(GeometricBackend),  // ADD THIS
}
```

Update `detect_and_open()`:

```rust
pub fn detect_and_open(db_path: &Path) -> Result<Self, YourError> {
    // Check if file exists
    if !db_path.exists() {
        return Err(YourError::DatabaseNotFound {
            path: db_path.display().to_string(),
        });
    }

    // First check file extension for geometric backend
    let is_geometric = db_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "geo")
        .unwrap_or(false);

    if is_geometric {
        return GeometricBackend::open(db_path).map(Backend::Geometric);
    }

    // Read first 16 bytes to detect format for other backends
    let mut file = File::open(db_path).map_err(|e| YourError::BackendDetectionFailed {
        path: db_path.display().to_string(),
        reason: format!("Cannot open file: {}", e),
    })?;

    let mut header = [0u8; 16];
    file.read_exact(&mut header)
        .map_err(|e| YourError::BackendDetectionFailed {
            path: db_path.display().to_string(),
            reason: format!("Cannot read file header: {}", e),
        })?;

    // Check for V3 format magic: "SQLTGF"
    let is_v3 = &header[0..6] == b"SQLTGF";

    // Check for SQLite format: "SQLite format 3\0"
    let is_sqlite = &header[0..16] == b"SQLite format 3\0";

    if is_v3 {
        #[cfg(feature = "native-v3")]
        return NativeV3Backend::open(db_path).map(Backend::NativeV3);

        #[cfg(not(feature = "native-v3"))]
        return Err(YourError::NativeV3BackendNotSupported {
            path: db_path.display().to_string(),
        });
    } else if is_sqlite {
        SqliteBackend::open(db_path).map(Backend::Sqlite)
    } else {
        // Unknown format, try SQLite as fallback
        SqliteBackend::open(db_path).map(Backend::Sqlite)
    }
}
```

### Add delegation methods for Geometric variant

For each BackendTrait method, add Geometric match arm:

```rust
pub fn search_symbols(
    &self,
    options: SearchOptions,
) -> Result<(SearchResponse, bool, bool), YourError> {
    match self {
        Backend::Sqlite(b) => b.search_symbols(options),
        #[cfg(feature = "native-v3")]
        Backend::NativeV3(b) => b.search_symbols(options),
        Backend::Geometric(b) => b.search_symbols(options),  // ADD THIS
    }
}
```

Do this for ALL BackendTrait methods:
- `search_symbols`
- `search_references`
- `search_calls`
- `ast`
- `find_ast`
- `complete`
- `lookup`
- `search_by_label`

## 4. Data Mapping

### Map Magellan SymbolInfo to your SymbolMatch

```rust
fn symbol_info_to_match(&self, info: SymbolInfo, query: &str) -> SymbolMatch {
    SymbolMatch {
        match_id: format!("match_{}", info.id),
        span: Span {
            span_id: format!("span_{}", info.id),
            file_path: info.file_path.clone(),
            byte_start: info.byte_start,
            byte_end: info.byte_end,
            start_line: info.start_line,
            start_col: info.start_col,
            end_line: info.end_line,
            end_col: info.end_col,
            context: None,
        },
        name: info.name.clone(),
        kind: format!("{:?}", info.kind),
        parent: None,
        symbol_id: Some(format!("{:016x}", info.id)),
        score: Some(100),
        fqn: Some(info.fqn.clone()),
        canonical_fqn: Some(info.fqn.clone()),
        display_fqn: Some(info.fqn.clone()),
        content_hash: None,
        symbol_kind_from_chunk: None,
        snippet: None,
        snippet_truncated: None,
        language: Some(format!("{:?}", info.language).to_lowercase()),
        kind_normalized: Some(format!("{:?}", info.kind).to_lowercase()),
        complexity_score: None,
        fan_in: None,
        fan_out: None,
        cyclomatic_complexity: None,
        ast_context: None,
        supernode_id: None,
    }
}
```

### Implement BackendTrait methods

```rust
impl YourBackendTrait for GeometricBackend {
    fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), YourError> {
        let symbols = self.inner.find_symbols_by_name_info(options.query);
        
        let total_count = symbols.len() as u64;
        let partial = symbols.len() > options.limit;
        
        let matches: Vec<SymbolMatch> = symbols
            .into_iter()
            .take(options.limit)
            .map(|info| self.symbol_info_to_match(info, options.query))
            .collect();
        
        let response = SearchResponse {
            query: options.query.to_string(),
            total_count,
            results: matches,
            path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            kind_filter: options.kind_filter.map(|k| k.to_string()),
            notice: None,
        };
        
        Ok((response, partial, false))
    }

    fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, YourError> {
        // Use geometric backend's FQN prefix completion
        let completions = self.inner.complete_fqn_prefix(prefix, limit);
        Ok(completions)
    }

    fn lookup(&self, fqn: &str, db_path: &str) -> Result<SymbolMatch, YourError> {
        match self.inner.find_symbol_by_fqn(fqn) {
            Some(info) => Ok(self.symbol_info_to_match(info, fqn)),
            None => Err(YourError::SymbolNotFound {
                fqn: fqn.to_string(),
                db: db_path.to_string(),
                partial: fqn.split("::").last().unwrap_or(fqn).to_string(),
            }),
        }
    }

    // ... implement other methods
}
```

## 5. CLI Command Integration

### Update require_native_v3() to allow Geometric

If you have a function that restricts commands to native-v3 only:

```rust
fn require_native_v3(backend: &Backend, command: &str, db_path: &Path) -> Result<(), YourError> {
    match backend {
        #[cfg(feature = "native-v3")]
        Backend::NativeV3(_) => Ok(()),
        Backend::Geometric(_) => {
            // Geometric backend now supports most commands
            Ok(())
        }
        Backend::Sqlite(_) => Err(YourError::RequiresNativeV3Backend {
            command: command.to_string(),
            path: db_path.display().to_string(),
        }),
    }
}
```

### Update --detect-backend flag

The default Magellan `detect_backend_format()` doesn't know about geometric. Update your detection:

```rust
if cli.detect_backend {
    let db_path = cli.db.as_ref().ok_or(YourError::DatabaseNotFound {
        path: "none".to_string(),
    })?;
    
    // Check extension first for geometric
    let is_geometric = db_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "geo")
        .unwrap_or(false);
    
    let backend_str = if is_geometric {
        "geometric"
    } else {
        // Use Magellan's detection for other backends
        use magellan::migrate_backend_cmd::{detect_backend_format, BackendFormat};
        let format = detect_backend_format(&db_path)?;
        match format {
            BackendFormat::Sqlite => "sqlite",
        }
    };
    
    // Output backend info
    println!("{{\"backend\":\"{}\",\"database\":\"{}\"}}", 
        backend_str, 
        db_path.display()
    );
    return Ok(());
}
```

## 6. Testing Strategy

### Layer 1: Unit Tests (in src/backend/geometric.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_geo_db() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let geo_path = temp_dir.path().join("test.geo");
        
        // Create valid geometric database
        let _backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
            .expect("Failed to create test geo database");
        
        (temp_dir, geo_path)
    }

    #[test]
    fn test_geometric_backend_open_valid_file() {
        let (_temp_dir, geo_path) = create_test_geo_db();
        let result = GeometricBackend::open(&geo_path);
        assert!(result.is_ok(), "Should open valid .geo file");
    }

    #[test]
    fn test_geometric_backend_search_symbols_empty() {
        let (_temp_dir, geo_path) = create_test_geo_db();
        let backend = GeometricBackend::open(&geo_path).unwrap();
        
        let options = SearchOptions {
            db_path: &geo_path,
            query: "test",
            // ... other fields
        };
        
        let result = backend.search_symbols(options);
        assert!(result.is_ok());
        let (response, _, _) = result.unwrap();
        assert_eq!(response.total_count, 0);
    }
}
```

### Layer 2: Integration Tests (tests/unit_integration_geometric.rs)

```rust
//! Layer 2: Unit Integration Tests

use yourcrate::backend::{Backend, BackendTrait};
use tempfile::TempDir;

fn create_temp_geo_file() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test.geo");
    
    let _backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");
    
    (temp_dir, geo_path)
}

#[test]
fn test_backend_enum_delegates_search_symbols_to_geometric() {
    let (_temp, geo_path) = create_temp_geo_file();
    let backend = Backend::detect_and_open(&geo_path).unwrap();
    
    match &backend {
        Backend::Geometric(_) => {}
        _ => panic!("Expected Geometric backend"),
    }
    
    let options = create_test_search_options(&geo_path);
    let result = backend.search_symbols(options);
    assert!(result.is_ok());
}
```

### Layer 3: Component Tests (tests/component_integration_geometric.rs)

```rust
//! Layer 3: Component Integration Tests

use yourcrate::backend::Backend;
use tempfile::TempDir;

fn create_temp_geo_file() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test.geo");
    
    let _backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");
    
    (temp_dir, geo_path)
}

#[test]
fn test_full_workflow_open_and_search() {
    let (_temp, geo_path) = create_temp_geo_file();
    
    // Step 1: Open the database
    let backend = Backend::detect_and_open(&geo_path).unwrap();
    
    // Step 2: Perform symbol search
    let options = create_test_search_options(&geo_path, "test_function");
    let result = backend.search_symbols(options);
    assert!(result.is_ok());
    
    // Step 3: Perform reference search
    let options = create_test_search_options(&geo_path, "test_function");
    let result = backend.search_references(options);
    assert!(result.is_ok());
    
    // Step 4: Perform call search
    let options = create_test_search_options(&geo_path, "test_function");
    let result = backend.search_calls(options);
    assert!(result.is_ok());
    
    // Step 5: Query AST
    let result = backend.ast(std::path::Path::new("test.rs"), None, 100);
    assert!(result.is_ok());
}
```

## 7. Manual Testing

### Build and test with real data:

```bash
# Build release binary
cargo build --release

# Create test geometric database
cd /path/to/magellan
cargo run --bin magellan-geometric --features geometric-backend -- create --db /tmp/test.geo

# Index a project
cargo run --bin magellan-geometric --features geometric-backend -- index --root /path/to/project --db /tmp/test.geo

# Test with your tool
/path/to/your/tool --db /tmp/test.geo search --query "main" --output json

# Verify backend detection
/path/to/your/tool --db /tmp/test.geo --detect-backend
```

## 8. Common Issues

### Issue 1: "backend":"sqlite" in --detect-backend
**Cause**: Using Magellan's old detect_backend_format() which doesn't know about geometric
**Fix**: Check .geo extension first before calling Magellan's detection

### Issue 2: GeometricBackend doesn't implement Debug
**Fix**: Implement Debug manually:
```rust
impl std::fmt::Debug for GeometricBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeometricBackend")
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}
```

### Issue 3: Missing methods on GeometricBackend
**Check available methods**:
```bash
grep -n "pub fn" /path/to/magellan/src/graph/geometric_backend.rs
```

Common methods:
- `find_symbols_by_name_info(name: &str) -> Vec<SymbolInfo>`
- `find_symbol_by_fqn(fqn: &str) -> Option<SymbolInfo>`
- `complete_fqn_prefix(prefix: &str, limit: usize) -> Vec<String>`
- `symbols_in_file(file_path: &str) -> Result<Vec<SymbolInfo>>`
- `get_callees(symbol_id: u64) -> Vec<u64>`
- `get_callers(symbol_id: u64) -> Vec<u64>`
- `get_references_bidirectional(symbol_id: u64) -> Result<(Vec<u64>, Vec<u64>)>`

## 9. Verification Checklist

Before considering integration complete:

- [ ] Cargo.toml uses local magellan with geometric-backend feature
- [ ] GeometricBackend struct created with proper Debug impl
- [ ] Backend enum has Geometric variant
- [ ] detect_and_open() checks .geo extension first
- [ ] All BackendTrait methods delegate to Geometric variant
- [ ] SymbolInfo mapping to your SymbolMatch type
- [ ] require_native_v3() allows Geometric backend
- [ ] --detect-backend reports "geometric" for .geo files
- [ ] Layer 1 unit tests pass
- [ ] Layer 2 integration tests pass
- [ ] Layer 3 component tests pass
- [ ] Manual CLI testing with real .geo database works
- [ ] JSON output works for all commands

## 10. Files Modified in llmgrep

For reference, these files were modified:

1. `Cargo.toml` - Added local magellan dependency with geometric-backend feature
2. `src/backend/mod.rs` - Added Geometric variant and detection logic
3. `src/backend/geometric.rs` - New file with complete implementation
4. `src/main.rs` - Updated require_native_v3() and --detect-backend logic
5. `tests/unit_integration_geometric.rs` - New test file
6. `tests/component_integration_geometric.rs` - New test file
7. `tests/native_v3_commands_test.rs` - Added Geometric match arm
8. `tests/backend_parity_test.rs` - Added Geometric match arm
9. `tests/backend_parity_extended_test.rs` - Added Geometric match arm

## Questions?

Refer to the llmgrep implementation at `/home/feanor/Projects/llmgrep` for working examples.
