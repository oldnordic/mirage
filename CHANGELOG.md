# Changelog

All notable changes to Mirage are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-02-03

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
- Structured JSON for LLM/agent consumption
- Natural language summaries for execution paths

### Integration
- Magellan v2.0.0 call graph integration
- Charon MIR extraction pipeline
- Compatible with SQLiteGraph database format

### Documentation
- Comprehensive README with quick start guide
- Detailed user manual (MANUAL.md)
- This changelog

[0.1.0]: https://github.com/oldnordic/mirage/releases/tag/v0.1.0
