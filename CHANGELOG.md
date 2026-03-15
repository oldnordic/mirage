# Changelog

All notable changes to Mirage are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.1] - 2026-03-15

### Fixed
- **CFG path enumeration with SQLite backend** - Fixed `paths` command returning 0 paths
  - Fixed `build_edges_from_terminators` to use byte offsets instead of sequential indices
  - Implemented `enumerate_paths` in SQLite router using CFG module
  - Paths now correctly enumerated from Magellan-indexed SQLite databases

### Changed
- **SQLite backend compatibility** - Now compatible with Magellan 3.1.1 SQLite databases
  - Call graph features (`cycles`, `status`) work correctly
  - CFG features work when database has proper terminator data

## [1.2.0] - 2026-03-10

### Added
- **Geometric Backend Support**: `.geo` file format via Magellan 3.1.0+
- **GeometricRouter**: Full `BackendRouter` trait implementation for `.geo` databases
  - `enumerate_paths()` - Path enumeration through CFG
  - `get_loops()` - Natural loop detection with back edges
  - `get_dominators()` - Dominance tree computation
  - `get_blast_zone()` - Reachability-based impact analysis
  - `find_cycles()` - Call graph cycle detection
  - `slice_forward/backward()` - Program slicing
  - `compute_hotspots()` - Risk scoring analysis
  - `compute_icfg()` - Inter-procedural CFG construction
  - All 17 tests passing in `tests/geometric_router_features_test.rs`
- **MagellanAdapter**: Contract-aware integration with path normalization and ambiguity handling
- **GeometricBridge**: Dedicated bridge type for .geo databases
- **backend-geometric**: Feature flag for .geo backend support

### Changed
- **Magellan dependency**: Updated to 3.1.0+ with geometric-backend feature
- **Documentation**: Updated README, MANUAL for .geo backend support

### Added Documentation
- **ARCHITECTURE.md**: Design documentation and component overview
- **API_INTEGRATION.md**: Magellan contract documentation
- **INVARIANTS.md**: Behavioral guarantees documentation
- **Geometric Router Tests**: `tests/geometric_router_features_test.rs` with 17 comprehensive tests

## [1.1.0] - 2026-02-27

### Added
- **Iterative Path Enumeration:** New `enumerate_paths_iterative()` function using stack-based DFS
  - Prevents stack overflow on deeply nested CFGs
  - Early path deduplication via `BTreeSet` (no duplicate paths stored)
  - Produces identical results to recursive version with better safety

- **Path Metadata API:** New `enumerate_paths_with_metadata()` function
  - Returns `PathEnumerationResult` with paths and detailed statistics
  - `EnumerationStats`: total paths, classification breakdown, avg/max length, loop count
  - `LimitsHit` enum: indicates if enumeration was complete or truncated

- **SQLite Backend Path Caching:** Implemented `get_cached_paths()` for SQLite backend
  - Queries `cfg_paths` and `cfg_path_elements` tables
  - Returns previously enumerated paths with full metadata

- **Source Location Extraction:** AST-based CFG now extracts source locations from tree-sitter nodes
  - `ast_to_cfg()` accepts optional `file_path` parameter
  - `SourceLocation` includes byte range and line/column info

### Changed
- **Magellan dependency:** Updated from 2.4 → 2.5.0 (caller/callee references, batch insert, parent resolution)

### Fixed
- **SQLite Backend:** Fixed path caching implementation to properly query stored paths

## [1.0.6] - 2026-02-22

### Fixed
- **`mirage paths` E007 Error:** Fixed "Function hash not found" error by using `symbol_id` from `graph_entities` as fallback when `cfg_blocks.function_hash` column doesn't exist (Magellan v7+ schema)

## [1.0.5] - 2026-02-22

### Fixed
- **Symbol ID Lookup:** `--function <hex_id>` now correctly resolves Magellan symbol IDs (e.g., `7ca9eebfa98204a5`) in both SQLite and native-v3 backends
- **Remediation Hints:** Fixed broken hint messages
  - Changed `mirage cfg --list-functions` → `magellan find <function_name>`
  - Changed `mirage verify --list` → `mirage paths --function <name>`
- **Status Output:** Added clarification that `cfg_paths` count requires explicit enumeration via `mirage paths --function <name>`

## [1.0.4] - 2026-02-22

### Fixed
- Documentation tests now pass (60 doctests)
- Fixed native-v3 backend KV store implementation
- Removed obsolete native-v2 references
- Cleaned up feature flag naming (backend-sqlite, backend-native-v3)

## [1.0.3] - 2026-02-20

### Updated
- Update sqlitegraph from 2.0.3 to 2.0.7 (bug fixes)

## [1.0.2] - 2026-02-14

### Added
- **Native-V3 Backend Support:** High-performance binary backend with KV store
  - Feature flag: `--features native-v3 --no-default-features`
  - Uses Magellan's native-v3 format (`.v3` files)
  - Full feature parity with SQLite backend
  - Dual backend architecture with runtime detection

### Changed
- **Dependencies:**
  - magellan: 2.2 → 2.4.3 (native-v3 support)
  - sqlitegraph: 1.5 → 2.0.3 (native-v3 support)

### Fixed
- **Tests:** Fixed pre-existing test failures
  - `test_compute_edge_diff` — Added missing block to test data
  - `test_compute_hot_paths_empty` — Added early return for empty paths
  - Integration tests — Fixed binary path detection for `cargo test`

### Documentation
- Rewrote README in concise format (100 lines)
- Added backend comparison table

## [1.0.1] - 2026-02-04

### Added
- **Windows Support (analysis-only):** Cross-platform compatibility via explicit feature flag
  - Use `--features windows` to enable Windows builds
  - Default: `--features unix` (Linux/macOS)
  - Platform detection centralized in `platform.rs` module
  - Users are warned about Windows limitations on startup

### Changed
- Replaced `atty` with `is-terminal` for terminal detection
- Minimum Rust version: 1.70+ (for `std::io::IsTerminal`)
- Feature model: `default = ["unix"]`, `windows` opt-in

### Windows Limitations
Windows builds are supported for analysis and exploration. Some Unix-only capabilities are intentionally disabled:
- No file watching (use manual reindex via Magellan)
- No auto-index
- No background processes

**One sentence for the docs:**
> Windows support is opt-in via `--features windows`. Analysis-only; no watchers, auto-index, or background processes.

## [1.0.0] - 2026-02-03

### Added
- **MIR Extraction:** Extract control-flow graphs from Rust MIR via Charon integration
- **Path Enumeration:** Enumerate all execution paths through functions with caching (BLAKE3)
- **Dominance Analysis:** Compute dominators, post-dominators, and dominance frontiers
- **Loop Detection:** Identify natural loops within functions
- **Dead Code Detection:** Find unreachable code blocks
- **Branching Patterns:** Detect if/else and match patterns
- **Path Verification:** Verify cached paths after code changes
- **Impact Analysis:** Blast zone analysis using path traversal
- **Cycle Detection:** Combined call graph SCCs and function-level loops
- **Program Slicing:** Backward and forward slicing using Magellan call graph
- **Hotspots Analysis:** Risk scoring combining path counts, dominance, and complexity
- **Inter-procedural Analysis:** Call graph condensation and dominance
- **Incremental Indexing:** Git diff-based smart re-indexing

### CLI Commands (14 total)
- `index` - Index Rust projects via MIR extraction
- `status` - Database statistics
- `paths` - Execution path enumeration and queries
- `cfg` - Control-flow graph visualization (human/dot/json)
- `dominators` - Dominance and post-dominance analysis
- `loops` - Natural loop detection
- `unreachable` - Dead code detection (with Magellan uncalled functions)
- `patterns` - Branching pattern detection (if/else, match)
- `frontiers` - Dominance frontier computation
- `verify` - Path verification after code changes
- `blast-zone` - Path-based impact analysis
- `cycles` - Combined cycle detection (call graph + CFG)
- `slice` - Program slicing (backward/forward)
- `hotspots` - High-risk function identification

### Database Schema
- SQLite-based storage with Magellan compatibility
- Tables: `graph_entities`, `cfg_blocks`, `cfg_edges`, `cfg_paths`, `cfg_dominators`
- BLAKE3 content-addressed path storage for automatic deduplication
- Function-level hash tracking for incremental updates

### Output Formats
- Three output modes: `human`, `json`, `pretty`
