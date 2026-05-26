# Changelog

All notable changes to Mirage are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.5.0] - 2026-05-26

### Added
- **`risk` command** — Compute risk scores for functions based on CFG analysis:
  - `mirage risk --function <name>` — Scores by cyclomatic complexity, path count, error path ratio, nesting depth, block count
  - Reports risk level (low/medium/high/critical)
- **`suggest` command** — Suggest refactoring actions for a symbol:
  - `mirage suggest --symbol <name>` — Detects high complexity, deep nesting, excessive paths, dead code
  - Severity-tagged suggestions with detail messages
- **`stats` command** — Show aggregate code statistics from the database:
  - Function counts, block counts, path counts, complexity distribution
  - Dead code blocks and coverage gaps
- **Opt-in telemetry** — `--record` flag or `MIRAGE_TELEMETRY=1` env var writes to `~/.magellan/mirage-telemetry.db`

## [1.4.6] - 2026-05-21

### Changed
- **sqlitegraph 3.0.2 → 3.0.3** — Picks up AVX-512 SIMD performance improvements.

## [1.4.5] - 2026-05-19

### Changed

- **sqlitegraph 3.0.1 → 3.0.2** — Picks up V3Backend flush-error handling, PersistentHeaderV3 panic fixes, CliClient stack optimization, and HNSW docstring corrections.

## [1.4.3] - 2026-05-18

### Changed
- Updated Magellan dependency to `3.3.9` (sqlitegraph 3.0.1).
- Updated sqlitegraph dependency to `3.0.1`.

## [1.3.1] - 2026-05-10

### Added
- **`docs` command** — List source documents from Magellan's graph memory tables:
  - `mirage docs --db code.db` — List all source documents
  - `--kind <kind>` — Filter by source kind (wiki, code, message, etc.)
  - `--tag <tag>` — Filter by tag
  - `--limit <n>` — Maximum results (default: 50)
  - Supports `--output human`, `--output json`, `--output pretty`
  - Graceful degradation: returns empty when `source_documents` table is missing

- **DocumentInfo and FactInfo structs** — New data types in `storage::StorageTrait`:
  - `DocumentInfo` — path, kind, title, tags, wikilinks
  - `FactInfo` — candidate_id, subject, predicate, object, status
  - `get_documents_for_function()`, `get_facts_for_function()`, `list_source_documents()` — StorageTrait methods with SQLite implementations
  - Default implementations return empty (no breakage for geometric backend)

### Changed
- Updated Magellan dependency to `3.3.3` (schema 14).

## [1.3.0] - 2026-05-06

### Added
- **`cycles --type` filter** — Call graph cycle detection now supports filtering by cycle type:
  - `--type all` (default) — All cycles
  - `--type inter-function` — Cycles spanning multiple functions
  - `--type self-loop` — Functions that call themselves

### Fixed
- **Path enumeration for implicit returns** — Functions without explicit `return` terminators (magellan stores these as "fallthrough") caused `find_exits()` to return empty, producing 0 paths for ~90% of functions:
  - Added dead-end block fallback in `enumerate_paths()`, `enumerate_paths_iterative()`, and `EnumerationContext::new()`
  - When no exit blocks found via terminators, blocks with no outgoing edges are treated as implicit exits
  - Verified with `?` operator self-loop test and implicit return dead-end test

### Changed
- Updated Magellan dependency to `3.2.0`.

## [1.2.7] - 2026-05-06

### Changed
- Updated Magellan dependency to `3.2.0`.

## [1.2.6] - 2026-05-06

### Fixed
- Eliminated compiler warning: removed unused `resolve_function_name_with_conn` re-export.

## [1.2.5] - 2026-05-04

### Changed
- Updated Magellan dependency to `3.1.9`.
- Removed local path patch for Magellan; now uses published crate from crates.io.

## [1.2.4] - 2026-04-27

### Fixed
- **Magellan v11 compatibility** — Use `cfg_hash` column consistently across all path enumeration
  - Fixed `get_or_enumerate_paths()` to use `cfg_hash` instead of `function_hash` for Magellan v11 schema
  - Fixed `enumerate_paths_cached()` for Magellan v11 schema with cfg_hash column
  - Fixed `enumerate_paths_cached_with_context()` for cfg_hash compatibility
  - Fixed `hash_changed()` to use cfg_hash column for detecting CFG changes
  - Removed divergent `cfg_edges` table creation (Magellan v11 manages it)
  - **Impact**: Path caching now works correctly with Magellan v11 databases; automatic cache invalidation when CFG changes
- **ICFG (inter-procedural CFG) across supported backends** — `mirage icfg` now correctly discovers callees and emits call/return edges
  - Added `get_callees()` to `StorageTrait` with implementations for SQLite and geometric backends (`src/storage/mod.rs`, `src/storage/sqlite_backend.rs`, `src/storage/geometric.rs`)
  - Fixed `build_icfg()` in `src/cfg/icfg.rs` to use a two-pass algorithm: first pass builds all nodes, second pass adds inter-procedural edges after callee nodes exist
  - Added fallback to `storage.get_callees()` when `GraphBackend::neighbors()` returns empty, fixing callee discovery for both geometric (stub GraphBackend) and SQLite (CALLER→CALLS two-hop Magellan schema)
  - Completed geometric router `get_icfg()` (`src/router/geometric.rs`) — now populates intra-procedural, call, and return edges instead of returning an empty edges vector
  - Implemented SQLite router `get_icfg()` (`src/router/sqlite.rs`) — delegates to `build_icfg()` and converts to `InterProceduralCfg`
  - Added `IcfgJson::to_inter_procedural_cfg()` conversion in `src/cfg/icfg.rs` to bridge the petgraph-based `Icfg` and the flat `router::InterProceduralCfg` models
- **CFG edge loading from Magellan's `cfg_edges` table** — Mirage now reads actual control-flow edges instead of guessing them from terminator strings
  - Added `build_edges_from_cfg_edges()` in `src/cfg/mod.rs` to construct edges from `cfg_edges` (source_idx, target_idx, edge_type)
  - Modified `src/storage/mod.rs` to query `cfg_edges` in both `load_cfg_from_sqlite()` and `MirageDb::load_cfg()`
  - Fixed `get_or_enumerate_paths()` to re-enumerate when cached paths table is empty (prevents stale 0-path caches)
  - Falls back to `build_edges_from_terminators()` for pre-Magellan-v11 databases without `cfg_edges`
  - **Impact**: Path enumeration now returns > 0 paths for functions with proper edge data; loop detection works; dominator trees are correct
- **Geometric backend edge type mapping** in `src/router/geometric.rs` — maps `edge_type` u32 discriminants to Mirage `EdgeType` enum instead of hardcoding `Fallthrough` for all edges

### Changed
- **Database schema requirement** — Now requires Magellan v11+ (or v10 with 4D coordinate columns)
  - Added `coord_t` column support for temporal/type metadata (Magellan v11)
  - Migration guide available in MANUAL.md for users upgrading from v10
- **Documentation updates**:
  - `MANUAL.md`: Updated version to 1.2.4, added `icfg`, `hotpaths`, `diff`, and `migrate` commands; added `--file` option to all function-disambiguating commands; updated all database path examples from `.codemcp/project.db` to `.magellan/mirage.db`; simplified function name examples (use simple names, not crate-qualified)
  - `README.md`: Fixed version to 1.2.4 and removed internal automation references

## [1.3.0] - 2026-04-12

### Added
- **Truth-Based AI Engineering Vision**: Established a new paradigm for code intelligence centered on deterministic, graph-based reasoning.
  - Created **`THE_VISION.md`**: Outlines the 4D Voxel World and Minecraft-style block streaming to reduce context usage by 90%.
  - Created **`ECOSYSTEM_SYNERGY.md`**: Documents the integration with GeoMetriDB 4D research (Sparse Inference as Cognition).
  - Created **`MIR_INTEGRATION_PLAN.md`**: File-by-file roadmap for high-fidelity MIR extraction.
  - Created **`PATH_SUMMARIZATION_PLAN.md`**: TDD plan for distilling raw MIR into "Truth Proofs."
- **High-Fidelity MIR Integration**:
  - Implemented **`src/mir/`** module for parsing Charon LLBC (Low-Level Borrow Calculus) JSON format.
  - Implemented TDD-verified translation from MIR basic blocks to Mirage Control Flow Graphs.
  - Supported high-fidelity instructions (Assign, Call, Drop, Assert) to eliminate guesswork.
- **Path Summarization (Truth Proofs)**:
  - Implemented **`src/cfg/summary.rs`**: A depth-aware distillation engine that filters compiler noise (StorageLive, Nop) and highlights logical effects.
  - Added **`--summarize`** flag to the `paths` command, providing compact logic sequences in both Human and JSON formats.
  - Example output: `[Entry] -> [Call: auth::verify] -> [State: Local(0) = true] -> [Return]`.

### Changed
- **Ecosystem Synchronization**:
  - Updated **Magellan** to Schema Version 9, adding the `statements` column to the `cfg_blocks` table for instruction persistence.
  - Updated **geographdb-core** to support binary serialization of MIR fields (`cfg_hash`, `statements`), ensuring data preservation in `.geo` files.
  - Updated **`GEMINI.md`**: Documented a cohesive workflow for discovery (Magellan), search (llmgrep), and reasoning (Mirage).
- **Architecture**:
  - Refactored `src/cfg/summary.rs` into a robust `PathSummarizer` struct with better error resilience and fallback handling for AST-only data.

### Verified
- **Strict TDD**: All new features verified with passing unit and integration tests (`tests/path_summary_tdd.rs`, `tests/mir_to_db_integration.rs`).
- **Subagent Audits**: Completed soundness, security, and performance reviews using specialized AI subagents.
- **Database Integrity**: Proved physical storage of MIR JSON in the `statements` column via direct SQL audit.

## [1.2.4] - 2026-03-20

### Added
- **`--file` option to disambiguate functions with duplicate names**
  - Added to `cfg`, `paths`, `dominators`, `loops`, `patterns`, `frontiers` commands
  - Allows selecting the correct function when multiple functions share the same name
  - Example: `mirage cfg --function index_file --file ops.rs`
  - Location: `src/cli/mod.rs`

### Fixed
- **Function resolution ambiguity**
  - `resolve_function_name_sqlite()` now supports optional file path filtering
  - Added `resolve_function_name_with_file()` for backend-agnostic API
  - Uses LIKE pattern matching for partial file paths

## [1.2.2] - 2026-03-19

### Fixed
- **Compiler warnings** - Fixed 7 warnings across the codebase
  - Fixed unused variable `is_geo` in storage/mod.rs (added proper cfg attribute)
  - Fixed unused variables `original_idx` in cfg/mod.rs (prefixed with underscore)
  - Fixed unused variable `label` in cfg/icfg.rs (prefixed with underscore)
  - Fixed unused variables `db`, `input_db`, `output_db` in cli/mod.rs (added cfg attributes)
  - Fixed unreachable pattern warning in `is_sqlite()` function
  - Build now completes with only minor unused import warnings

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
- **Symbol ID Lookup:** `--function <hex_id>` now correctly resolves Magellan symbol IDs (e.g., `7ca9eebfa98204a5`) in both SQLite and retired-binary-backend backends
- **Remediation Hints:** Fixed broken hint messages
  - Changed `mirage cfg --list-functions` → `magellan find <function_name>`
  - Changed `mirage verify --list` → `mirage paths --function <name>`
- **Status Output:** Added clarification that `cfg_paths` count requires explicit enumeration via `mirage paths --function <name>`

## [1.0.4] - 2026-02-22

### Fixed
- Documentation tests now pass (60 doctests)
- Fixed retired-binary-backend backend KV store implementation
- Removed obsolete native-v2 references
- Cleaned up feature flag naming (backend-sqlite, backend-retired-binary-backend)

## [1.0.3] - 2026-02-20

### Updated
- Update sqlitegraph from 2.0.3 to 2.0.7 (bug fixes)

## [1.0.2] - 2026-02-14

### Added
- **retired binary backend Backend Support:** High-performance binary backend with KV store
  - Feature flag: `--features retired-binary-backend --no-default-features`
  - Uses Magellan's retired-binary-backend format (`.db` files)
  - Full feature parity with SQLite backend
  - Dual backend architecture with runtime detection

### Changed
- **Dependencies:**
  - magellan: 2.2 → 2.4.3 (retired-binary-backend support)
  - sqlitegraph: 1.5 → 2.0.3 (retired-binary-backend support)

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
