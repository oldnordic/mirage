# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-01)

**Core value:** An agent may only speak if it can reference a graph artifact. No artifact -> no output.
**Current focus:** Phase 2: CFG Construction (in progress)

## Current Position

Phase: 2 of 7 (CFG Construction) - In Progress
Plan: 02 complete (of 6 in this phase)
Status: AST-based CFG construction complete, leader detection and edge handling implemented
Last activity: 2026-02-01 - Completed 02-02: AST-based CFG construction

Progress: [██████████░] 33% (Phase 2/7, Plan 3/6 in phase)

## Performance Metrics

**Velocity:**
- Total plans completed: 6
- Average duration: 5 min
- Total execution time: 0.5 hours

**By Phase:**

| Phase | Plans | Complete | Avg/Plan |
|-------|-------|----------|----------|
| 01-database-foundation | 3 | 3/3 | 4 min |
| 02-cfg-construction | 6 | 3/6 | 6 min |

**Recent Trend:**
- Last 5 plans: 5 min
- Trend: Consistent

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

**From 01-01 (Incremental Update Tracking and Migration Framework):**
- Migration framework uses in-place upgrades (no separate migration files needed for v1)
- Foreign key enforcement requires explicit PRAGMA in tests (SQLite default: OFF)
- function_hash uses TEXT type (not BLOB) for easier debugging and human-readability

**From 01-02 (Status Command):**
- Database path resolution follows Magellan pattern: CLI arg > MIRAGE_DB env > default "./codemcp.db"
- Output formats use JsonResponse wrapper for consistency across all commands
- CLI commands needing global context receive (args, &Cli) signature

**From 01-03 (Database Integration Tests):**
- Tests use real Magellan database schema (version 4) not mocks for authentic validation
- OptionalExtension trait required for nullable query results in rusqlite
- Error tests use if-let pattern matching to avoid Debug trait requirement on MirageDb

**From 02-03 (Core CFG Data Structures):**
- petgraph DiGraph as backing store for CFG (de facto standard for Rust graph algorithms)
- Simplified Terminator enum for initial release, MIR-specific variants added later
- EdgeType includes visualization metadata (dot_color, dot_label) for graphviz output
- BlockId uses usize for simple integer indexing within functions
- All domain types derive Serialize/Deserialize for JSON export capability

**From 02-01 (MIR Extraction via Charon):**
- Charon used as external binary (not linked) to avoid nightly Rust requirement
- ULLBC structures simplified for CFG needs - full Charon types are much larger
- EdgeType classification matches MIR terminator semantics (Call/Exception for unwind)
- BlockKind inference: Entry (id=0), Exit (Return/Unreachable), Normal (others)
- External tool integration pattern: spawn binary, capture stdout, parse JSON

**From 02-02 (AST-based CFG Construction):**
- Leader-based algorithm for CFG construction: first statement, branch targets, post-branch statements
- tree-sitter 0.22 used for language-agnostic AST parsing (language grammars feature-gated)
- Terminator enum derives PartialEq/Eq for test assertions
- Edge types encode semantic meaning: TrueBranch/FalseBranch for conditionals, LoopBack/LoopExit for loops
- CFGBuilder pattern: find_leaders → build_blocks → connect_edges

### Pending Todos

None yet.

### Blockers/Concerns

- **sccache corruption**: Build cache occasionally returns stale results. Workaround: `RUSTC_WRAPPER=""` env var to bypass. Not blocking but noted.
- **Charon external dependency**: Users must install Charon binary separately. Documented in SUMMARY.md but not enforced.

## Session Continuity

Last session: 2026-02-01
Stopped at: Completed 02-02: AST-based CFG construction
Resume file: None
