# Changelog

All notable changes to Mirage are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
