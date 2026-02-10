# Roadmap: Mirage

## Overview

Mirage transforms code into verifiable graph artifacts. Starting from database schema and CFG extraction, we build control flow graphs, analyze dominance relationships, enumerate execution paths, and expose everything through a CLI that produces structured outputs for LLM consumption. Every phase delivers something that can verified against the graph.

## v1.0 Milestone (Completed 2026-02-04)

**Status:** Released
**Archive:** [v1.0-ROADMAP.md](.planning/milestones/v1.0-ROADMAP.md) | [v1.0-REQUIREMENTS.md](.planning/milestones/v1.0-REQUIREMENTS.md) | [v1.0-AUDIT.md](.planning/v1-MILESTONE-AUDIT.md)

**12 Phases | 60 Plans | 51 Requirements | 22,027 LOC**

The v1.0 milestone delivers a complete path-aware code intelligence engine for Rust projects. Mirage integrates with Magellan's AST-based CFG extraction to provide control flow analysis, dominance relationships, path enumeration, and inter-procedural analysis through a CLI interface optimized for LLM consumption.

### Completed Phases

| Phase | Status | Completed |
|-------|--------|-----------|
| 01 - Database Foundation | Complete | 2026-02-01 |
| 02 - CFG Construction | Complete | 2026-02-01 |
| 03 - Reachability & Control Structure | Complete | 2026-02-01 |
| 04 - Dominance Analysis | Complete | 2026-02-01 |
| 05 - Path Enumeration | Complete | 2026-02-01 |
| 06 - CLI Interface | Complete | 2026-02-01 |
| 07 - LLM Integration | Complete | 2026-02-01 |
| 08 - Drift Remediation | Complete | 2026-02-02 |
| 09 - MIR Integration & Database Loading | Complete | 2026-02-02 |
| 10 - Magellan v2 Integration | Complete | 2026-02-03 |
| 11 - Inter-procedural Dominance & Hotspots | Complete | 2026-02-03 |
| 12 - Magellan CFG Integration | Complete | 2026-02-04 |

### Key Capabilities Delivered

- **CFG Construction**: AST-based control flow graphs from Magellan integration
- **Path Enumeration**: Execution path analysis with BLAKE3 caching
- **Dominance Analysis**: Immediate dominators, post-dominators, dominance frontiers
- **CLI Interface**: 13 commands for all analysis types with JSON/human/pretty output
- **LLLM Integration**: Structured outputs optimized for agent consumption
- **Inter-procedural Analysis**: Call graph condensation, hotspots, program slicing

### 13 CLI Commands

`status`, `paths`, `cfg`, `dominators`, `loops`, `unreachable`, `patterns`, `frontiers`, `verify`, `blast-zone`, `cycles`, `slice`, `hotspots`

---

## v1.1 Milestone (Completed 2026-02-10)

**Status:** Complete
**Completed:** 2026-02-10

The v1.1 milestone adds dual backend support for sqlitegraph, enabling Mirage to work with both SQLite (default) and native-v2 (newer I/O layer) backends.

### Completed Phases

| Phase | Status | Completed |
|-------|--------|-----------|
| 13 - Dual Backend Support | Complete | 2026-02-10 |
| 14 - Native-v2 I/O Integration | Complete | 2026-02-10 |

### Key Capabilities Delivered (Phase 13)

- **Dual Backend Support**: Users can choose between sqlite-backend (default) and native-v2 (newer I/O layer)
- **Compile-Time Guard**: Prevents building with both backends simultaneously
- **Runtime Detection**: Backend format detected from file header
- **Helpful Errors**: Clear rebuild instructions when backend mismatches occur

### Key Capabilities Delivered (Phase 14)

- **GraphBackend Integration**: MirageDb wraps Box<dyn GraphBackend> for dual backend support
- **Native-v2 KV Operations**: cfg_blocks loading via Magellan's get_cfg_blocks_kv()
- **Backend-agnostic Metadata**: status() and resolve_function_name() work with both backends
- **All CLI Commands Working**: All 13 commands verified with both SQLite and native-v2 backends
- **Graceful Degradation**: Path caching falls back to direct enumeration for native-v2

### Phase 14: Native-v2 I/O Integration

**Goal:** Mirage built with native-v2 works normally (same functionality as sqlite backend)
**Status:** COMPLETE
**Plans:** 3 plans

Plans:
- [x] 14-01 — GraphBackend wrapper refactoring (10 min)
- [x] 14-02 — Native-v2 KV operations for cfg_blocks (15 min)
- [x] 14-03 — Backend-agnostic metadata queries and CLI verification (50 min)

---

## Future Work

v1.1 and beyond: Advanced features deferred from original requirements. See v2 Requirements in archived requirements document.

### Potential Enhancements

- Statement-level CFG detail (currently block-level only)
- Path summaries stored in database (currently post-hoc generation)
- Full CFG-based program slicing (currently uses call-graph reachability)
- Interactive graph visualization
- Async/await state machine analysis
- Borrow checker integration via Polonius
