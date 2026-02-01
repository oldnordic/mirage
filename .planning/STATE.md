# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-01)

**Core value:** An agent may only speak if it can reference a graph artifact. No artifact -> no output.
**Current focus:** Phase 3: Reachability & Control Structure (in progress)
**Last focus:** Phase 2: CFG Construction (complete)

## Current Position

Phase: 3 of 7 (Reachability & Control Structure) - In progress
Plan: 04 complete (of 4 in this phase)
Status: Branching pattern recovery with if/else diamond detection and match/switch pattern detection
Last activity: 2026-02-01 - Completed 03-04: Branching pattern recovery

Progress: [████████████░] 28% (Phase 3/7 complete, 4/4 plans in phase, 13 total plans)

## Performance Metrics

**Velocity:**
- Total plans completed: 13
- Average duration: 4.7 min
- Total execution time: 1.0 hours

**By Phase:**

| Phase | Plans | Complete | Avg/Plan |
|-------|-------|----------|----------|
| 01-database-foundation | 3 | 3/3 | 4 min |
| 02-cfg-construction | 6 | 6/6 | 5 min |
| 03-reachability-control | 4 | 4/4 | 4.5 min |

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

**From 02-04 (Entry/Exit Detection and CFG Analysis):**
- Exit blocks identified by terminator type: Return, Unreachable, Abort
- Functions support multiple exits (early returns, panic paths, unwind paths)
- Entry detection uses first-node query (id=0 always entry by construction)
- Variable naming: avoid `cfg` as variable name (conflicts with Rust built-in macro)

**From 02-05 (Source Location Mapping):**
- source_location is Optional<SourceLocation> on BasicBlock (AST CFG doesn't have spans yet)
- Charon ULLBC spans provide line:column but not byte offsets (uses 0 placeholder)
- File ID to path mapping deferred (requires ULLBC file table access)
- byte_to_line_column correctly handles UTF-8 multibyte characters for column counting
- SourceLocation::display() produces "file:line:col-line:col" format for IDE integration
- overlaps() method for source range intersection (useful for coverage analysis)

**From 02-06 (DOT and JSON Export):**
- Manual DOT generation instead of petgraph::dot for more control over colors and labels
- JSON export uses serde for easy tool integration
- CLI uses test CFG for now - database loading comes in later plans
- Export pattern: separate export_dot() and export_json() functions for different formats
- CLI format handling: --format flag with fallback to global --output

**From 03-01 (Unreachable Code Detection):**
- Reachability analysis uses petgraph::visit::Dfs for traversal (reachable_from doesn't exist in petgraph 0.8)
- Single-block reachability queries use petgraph::algo::has_path_connecting
- find_unreachable returns all nodes not reachable from entry (dead code detection)
- unreachable_block_ids() helper converts NodeIndex to BlockId for CLI integration
- Empty CFGs handled gracefully (return empty vec, not panic)

**From 03-02 (Reachability Query Engine):**
- can_reach() provides simple path existence queries using has_path_connecting
- can_reach_cached() reuses DfsSpace for repeated queries (better performance)
- ReachabilityCache wraps DfsSpace for cleaner API with interior mutability
- has_path_connecting auto-resets DfsSpace in petgraph 0.8 (no manual reset needed)
- DfsSpace import path: petgraph::algo::DfsSpace (not petgraph::visit)
- Separate APIs for simple vs. cached queries help users choose right approach

**From 03-03 (Natural Loop Detection):**
- Natural loop detection uses dominance-based definition: back-edge (N -> H) where H dominates N
- Dominator computation via petgraph::algo::dominators::simple_fast (Cooper et al. algorithm)
- Loop body computed via predecessor traversal from tail until header (standard algorithm)
- Iterator-based dominator query pattern: dominators.dominators(node).any(|d| d == target)
- DfsSpace moved from petgraph::visit to petgraph::algo in petgraph 0.8 (API migration)
- EdgeRef trait required in scope for edge.source()/edge.target() methods
- Nested loops detected by checking if inner header is in outer loop body
- Loop nesting level calculation via recursive header containment check

**From 03-04 (Branching Pattern Recovery):**
- Diamond pattern detection identifies if/else via 2-way branch with common merge point
- Distinguished if/else from match by SwitchInt target count (1 target vs 2+ targets)
- EdgeType (TrueBranch/FalseBranch) used to order true/false branches in IfElsePattern
- find_common_successor excludes source nodes to find actual merge points
- Pattern detection approach: find branch points → verify structure → extract metadata

### Pending Todos

None yet.

### Blockers/Concerns

- **sccache corruption**: Build cache occasionally returns stale results. Workaround: `RUSTC_WRAPPER=""` env var to bypass. Not blocking but noted.
- **Charon external dependency**: Users must install Charon binary separately. Documented in SUMMARY.md but not enforced.

## Session Continuity

Last session: 2026-02-01
Stopped at: Completed 03-04: Branching pattern recovery
Resume file: None
