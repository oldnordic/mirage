# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-01)

**Core value:** An agent may only speak if it can reference a graph artifact. No artifact -> no output.
**Current focus:** Phase 5: Path Enumeration (Plan 1/6 complete)
**Last focus:** Phase 4: Dominance Analysis (complete)

## Current Position

Phase: 5 of 7 (Path Enumeration) - In progress
Plan: 03 complete (of 6 in this phase)
Status: Path bounding enforcement with self-loop detection and nested loop bounding
Last activity: 2026-02-01 - Completed 05-03: Configurable path bounding with presets

Progress: [████████████░] 60% (Phase 5/7 in progress, 18/30 plans complete)

## Performance Metrics

**Velocity:**
- Total plans completed: 18
- Average duration: 4.5 min
- Total execution time: 1.4 hours

**By Phase:**

| Phase | Plans | Complete | Avg/Plan |
|-------|-------|----------|----------|
| 01-database-foundation | 3 | 3/3 | 4 min |
| 02-cfg-construction | 6 | 6/6 | 5 min |
| 03-reachability-control | 4 | 4/4 | 4.5 min |
| 04-dominance-analysis | 3 | 3/3 | 3.7 min |
| 05-path-enumeration | 6 | 3/6 | 5 min |

**Recent Trend:**
- Last 5 plans: 4.2 min
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

**From 04-01 (Dominator Tree):**
- DominatorTree wraps petgraph's simple_fast instead of reimplementing Cooper et al. algorithm
- immediate_dominator() returns None for root node (unreachable nodes excluded from map)
- dominates() walks up dominator chain (O(depth) instead of O(|V|) set iteration)
- Children HashMap provides O(1) reverse dominator tree traversal
- Dominators iterator provides ergonomic upward traversal from node to root
- common_dominator() uses HashSet for O(min(|a|,|b|)) intersection finding

**From 04-02 (Post-Dominator Tree):**
- PostDominatorTree uses petgraph::visit::Reversed for zero-copy graph reversal instead of cloning
- Post-dominance computed as dominance on reversed graph (dual relationship)
- DominatorTree::from_parts() added as pub(crate) for internal construction by PostDominatorTree
- Primary exit only (first Return node) - multiple exits noted as limitation
- PostDominators iterator walks from node up to exit (mirror of Dominators)

**From 04-03 (Dominance Frontiers):**
- DominanceFrontiers implements Cytron et al. O(|V|²) algorithm with two rules
- frontier() returns owned HashSet instead of reference to avoid lifetime issues with empty set
- Self-frontier pattern (n in DF[n]) characterizes loop headers due to back edges
- Corrected test expectations: entry node strictly dominates all nodes in diamond CFG, so DF[entry] is empty

**From 05-01 (DFS Path Enumeration Core):**
- Path struct with BLAKE3-based path_id for deterministic identification
- hash_path() includes length in hash to prevent collision between [1,2,3] and [1,2,3,0]
- DFS enumeration with visited set + backtracking for cycle detection
- Loop headers exempt from visited check; back-edges bounded by loop_iterations counter
- PathLimits default: max_length=1000, max_paths=10000, loop_unroll_limit=3
- PathKind::Normal default; classification deferred to plan 05-02

**From 05-03 (Configurable Path Bounding):**
- Loop iteration counting: increment on header entry, decrement on backtrack
- Loop iteration counter allows (limit-1) actual loop iterations due to increment on first visit
- Independent counters per loop header enable O(k^n) bounding for nested loops (k=limit, n=depth)
- PathLimits presets: quick_analysis (100, 1000, 2) for fast results; thorough (10000, 100000, 5) for completeness
- Self-loops handled via loop_headers check - bounded like regular back-edges
- Builder pattern with method chaining for convenient PathLimits configuration

### Pending Todos

None yet.

### Blockers/Concerns

- **sccache corruption**: Build cache occasionally returns stale results. Workaround: `RUSTC_WRAPPER=""` env var to bypass. Not blocking but noted.
- **Charon external dependency**: Users must install Charon binary separately. Documented in SUMMARY.md but not enforced.

## Session Continuity

Last session: 2026-02-01
Stopped at: Completed 05-03: Configurable path bounding with presets
Resume file: None
