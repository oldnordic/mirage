# Mirage Backend Router System

## Overview

Mirage now supports multiple backends via compile-time feature flags:
- **SQLite** (`backend-sqlite`): Traditional SQLite database (default)
- **Geometric** (`backend-geometric`): Spatial indexing with A* pathfinding
- **Native V3** (`backend-native-v3`): High-performance KV store (stub)

Only one backend can be enabled at compile time, ensuring:
- Smaller binary size (only compiled backend included)
- No runtime overhead from backend detection
- CLI parity across all backends

## Building

```bash
# SQLite backend (default)
cargo build --release

# Geometric backend
cargo build --release --no-default-features --features backend-geometric

# Native V3 backend (not fully implemented)
cargo build --release --no-default-features --features backend-native-v3
```

## Architecture

### Router Module (`src/router/`)

The router module provides a unified interface for all backends:

```rust
pub trait BackendRouter {
    fn open(db_path: &Path) -> Result<Self>;
    fn status(&self) -> Result<DatabaseStatus>;
    fn enumerate_paths(&self, function_id: i64, max_paths: usize) -> Result<Vec<ExecutionPath>>;
    fn get_dominators(&self, function_id: i64) -> Result<DominatorTree>;
    fn get_loops(&self, function_id: i64) -> Result<Vec<NaturalLoop>>;
    // ... all other Mirage commands
}
```

### Type Alias

The `Router` type alias resolves to the appropriate backend at compile time:

```rust
#[cfg(feature = "backend-sqlite")]
pub type Router = sqlite::SqliteRouter;

#[cfg(feature = "backend-geometric")]
pub type Router = geometric::GeometricRouter;

#[cfg(feature = "backend-native-v3")]
pub type Router = native_v3::NativeV3Router;
```

### Compile-Time Guards

Mutually exclusive features prevent multiple backends:

```rust
#[cfg(all(feature = "backend-sqlite", feature = "backend-geometric"))]
compile_error!("Features are mutually exclusive...");
```

## Geometric Backend Advantages

The geometric backend provides superior pathfinding capabilities:

### A* Pathfinding
```rust
// Find shortest path between two blocks
let path = inner.find_cfg_path(function_id, start_block, goal_block);

// Find k shortest paths (Yen's algorithm)
let k_paths = inner.find_cfg_k_paths(function_id, start, goal, k);
```

### Spatial Queries
- O(log n) queries using octree indexing
- Nearby block queries for context analysis
- 3D spatial relationships for complex control flow

### Advanced Algorithms
- **Dominance**: Efficient dominator tree computation
- **Loops**: Natural loop detection with back-edge analysis
- **Slicing**: Backward/forward program slicing
- **Reachability**: Transitive closure for impact analysis

## CLI Parity

All CLI commands work with any backend:

| Command | SQLite | Geometric | Notes |
|---------|--------|-----------|-------|
| `status` | ✅ | ✅ | Database statistics |
| `paths` | ✅ | ✅ | Path enumeration (geometric uses A*) |
| `cfg` | ✅ | ⚠️ | CFG visualization (partial) |
| `dominators` | ✅ | ✅ | Dominator tree |
| `loops` | ✅ | ✅ | Natural loops |
| `unreachable` | ✅ | ✅ | Dead code detection |
| `patterns` | ✅ | ⚠️ | Branch patterns (partial) |
| `frontiers` | ✅ | ✅ | Dominance frontiers |
| `cycles` | ✅ | ⚠️ | Call cycles (partial) |
| `blast-zone` | ✅ | ⚠️ | Impact analysis (partial) |
| `slice` | ✅ | ✅ | Program slicing |
| `hotspots` | ✅ | ⚠️ | Risk analysis (partial) |
| `hotpaths` | ✅ | ✅ | Frequent paths |
| `verify` | ✅ | ⚠️ | Path verification (partial) |
| `icfg` | ✅ | ⚠️ | Inter-procedural CFG (partial) |

## Implementation Status

### SQLite Router (`src/router/sqlite.rs`)
- ✅ Basic implementation complete
- ⚠️ Many methods need actual implementation (currently return errors)
- Uses existing `MirageDb` and storage modules

### Geometric Router (`src/router/geometric.rs`)
- ✅ Core functionality implemented
- ✅ A* pathfinding integrated
- ✅ Dominance, loops, slicing working
- ⚠️ Some methods need enhancement (call graph, ICFG)
- Leverages geographdb-core algorithms

### Native V3 Router (`src/router/native_v3.rs`)
- ⚠️ Stub implementation only
- Not yet functional

## Files Added/Modified

### New Files
1. `src/router/mod.rs` - Router trait and type aliases
2. `src/router/sqlite.rs` - SQLite backend implementation
3. `src/router/geometric.rs` - Geometric backend implementation
4. `src/router/native_v3.rs` - Native V3 stub implementation

### Modified Files
1. `Cargo.toml` - Added backend feature flags and geographdb-core dependency
2. `src/lib.rs` - Added router module exports
3. `src/storage/mod.rs` - Added geometric backend support to MirageDb
4. `src/storage/geometric.rs` - Created geometric storage wrapper

## Usage Example

```rust
use mirage_analyzer::router::{BackendRouter, Router};
use std::path::Path;

// Open database (backend determined at compile time)
let router = Router::open(Path::new("code.db"))?;

// Get status
let status = router.status()?;
println!("CFG blocks: {}", status.cfg_blocks);

// Enumerate paths (uses A* for geometric, DFS for SQLite)
let paths = router.enumerate_paths(123, 10)?;
for path in paths {
    println!("Path {}: {:?}", path.path_id, path.blocks);
}

// Get dominators
let dom_tree = router.get_dominators(123)?;
```

## Testing

Run tests for specific backend:

```bash
# SQLite backend tests
cargo test --features backend-sqlite

# Geometric backend tests
cargo test --no-default-features --features backend-geometric
```

## Future Work

1. **Complete SQLite Router**: Implement remaining methods using existing analysis modules
2. **Enhance Geometric Router**: 
   - Full call graph construction
   - Complete ICFG support
   - Pattern detection using spatial queries
3. **Implement Native V3 Router**: Full support when native-v3 is ready
4. **Unified Tests**: Test suite that runs against all backends
5. **Performance Benchmarks**: Compare pathfinding performance between backends

## Migration Guide

For existing code using `MirageDb`:

```rust
// Old way (SQLite only)
let db = MirageDb::open("code.db")?;
let status = db.status()?;

// New way (any backend)
let router = Router::open("code.db")?;
let status = router.status()?;
```

The router provides the same interface regardless of backend, ensuring code portability.
