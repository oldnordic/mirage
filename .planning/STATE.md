# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-01)

**Core value:** An agent may only speak if it can reference a graph artifact. No artifact -> no output.
**Current focus:** Phase 11: Inter-procedural Dominance, Hotspots & Smart Re-indexing
**Last focus:** Phase 10: Magellan v2 Integration & Bugfixes (complete)

## Current Position

Phase: 11 of 11 (Inter-procedural Dominance, Hotspots & Smart Re-indexing)
Plan: 5 of 6
Next Phase: None (final phase)
Status: Phase 11 in progress, plan 11-05 complete
Last activity: 2026-02-03 - Completed 11-05 (Smart Re-indexing)

Progress: [█████████████░] 98% (10 phases complete, 55/56 plans done)

## Performance Metrics

**Velocity:**
- Total plans completed: 55
- Average duration: 5.3 min
- Total execution time: 4.8 hours

**By Phase:**

| Phase | Plans | Complete | Avg/Plan |
|-------|-------|----------|----------|
| 01-database-foundation | 3 | 3/3 | 4 min |
| 02-cfg-construction | 6 | 6/6 | 5 min |
| 03-reachability-control | 4 | 4/4 | 4.5 min |
| 04-dominance-analysis | 3 | 3/3 | 3.7 min |
| 05-path-enumeration | 6 | 6/6 | 4.6 min |
| 06-cli-interface | 7 | 7/7 | 5.6 min |
| 07-llm-integration | 4 | 4/4 | 4.5 min |
| 08-drift-remediation | 6 | 6/6 | 11.3 min |
| 09-mir-integration-database-loading | 4 | 4/4 | 5.5 min |
| 10-magellan-v2-integration-and-bugfixes | 5 | 5/5 | 4.0 min |
| 11-inter-procedural-dominance-hotspots-smart-reindexing | 5 | 5/6 | 4.5 min |

**Recent Trend:**
- Last 5 plans: 3.8 min
- Trend: Improved

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

**From 05-02 (Path Classification):**
- Helper function find_node_by_block_id for BlockId -> NodeIndex conversion
- Classification priority: Unreachable (reachability) > Error (terminator) > Degenerate (abnormal exit) > Normal (default)
- classify_path uses is_reachable_from_entry per block; classify_path_precomputed uses pre-computed HashSet for O(1) lookups
- O(n) batch classification: 1000 paths classified in <10ms
- enumerate_paths now uses classify_path_precomputed instead of hardcoding PathKind::Normal

**From 05-03 (Configurable Path Bounding):**
- Loop iteration counting: increment on header entry, decrement on backtrack
- Loop iteration counter allows (limit-1) actual loop iterations due to increment on first visit
- Independent counters per loop header enable O(k^n) bounding for nested loops (k=limit, n=depth)
- PathLimits presets: quick_analysis (100, 1000, 2) for fast results; thorough (10000, 100000, 5) for completeness
- Self-loops handled via loop_headers check - bounded like regular back-edges
- Builder pattern with method chaining for convenient PathLimits configuration

**From 05-04 (Static Feasibility Checking):**
- Static feasibility only: No symbolic execution (>100x slower, too complex for interactive use)
- Feasibility criteria: Entry kind + Exit terminator + Reachability + No dead-ends
- Valid exit terminators: Return, Abort, Call with target; Infeasible: Unreachable, Goto, SwitchInt, Call without target
- Batch optimization: is_feasible_path_precomputed with pre-computed reachable HashSet for O(1) reachability
- Classification priority updated: Unreachable > Error > Feasibility > Normal
- Limitations documented: Does NOT check conflicting conditions (x > 5 && x < 3), data-dependent constraints, runtime panics

**From 05-05 (Path Caching with BLAKE3 Content Addressing):**
- Path caching in database using BLAKE3 content-addressed IDs for automatic deduplication
- Schema fix: Removed FK constraints from entry_block/exit_block/block_id to cfg_blocks(id)
- These fields store conceptual BlockId values from CFG, not database primary keys
- BLAKE3 path_id provides deduplication and integrity verification instead of FK
- BEGIN IMMEDIATE TRANSACTION for atomic path storage operations
- Hash-based incremental cache updates via function_hash comparison in cfg_blocks
- get_or_enumerate_paths bridges caching layer to enumeration for cached path retrieval

**From 05-06 (Performance Optimization):**
- Batch inserts with UNION ALL use 20-row batches for balance between round-trips vs statement prep
- EnumerationContext pre-computes reachability, loops, exits once for O(v+e) vs O(n*(v+e)) for n enumerations
- enumerate_paths_cached integrates hash check, enumeration with context, and batch storage in one function
- estimate_path_count uses 2^branches * (unroll+1)^loops with saturating arithmetic to prevent overflow
- Performance benchmarks verify targets: 100 paths in <100ms, 100-block CFG in <100ms, nested loops <500ms
- PRAGMA cache_size = -64000 (64MB) improves bulk insert performance
- Context reuse enables 100 enumeration calls in <1ms

**From 06-01 (Path Query Commands):**
- paths() command uses test CFG until MIR extraction complete (consistent with other CLI commands)
- PathSummary struct for JSON serialization with path_id, kind, length, blocks fields
- Human format shows path count, error path count, and optional block sequences
- Error path filtering via --show-errors flag with retention pattern
- Format resolution: args.format overrides cli.output for consistency

**From 06-02 (CFG Command):**
- cfg() command connects to database with error handling following status command pattern
- JsonResponse wrapper for JSON output ensures consistency across all commands
- Format handling: CfgFormat::Human and CfgFormat::Dot both output DOT (same behavior)
- CfgFormat::Json uses export_json() and wraps in JsonResponse
- create_test_cfg() helper marked pub(crate) for test access
- TODO comment points to Phase 02-01 MIR extraction for real CFG loading

**From 06-06 (Status Command Verification):**
- status() command was already correctly implemented in Phase 1 (01-02)
- All three output formats (human/json/pretty) work correctly
- Error handling provides helpful hints for missing database
- Tests added for all output formats, empty database, and error cases
- Pattern matching used in tests to avoid Debug trait requirement on MirageDb

**From 06-03 (Dominators Command):**
- dominators() command uses DominatorTree and PostDominatorTree from Phase 4
- --post flag switches to post-dominator analysis
- --must-pass-through query finds all nodes dominated by a given block
- Human format prints tree structure with indentation for parent-child relationships
- JSON output includes DominanceResponse with immediate_dominator and dominated lists
- Print functions (print_dominator_tree_human, print_post_dominator_tree_human) handle recursive tree display

**From 06-04 (Unreachable Command):**
- unreachable() command uses find_unreachable from Phase 3 reachability analysis
- --within-functions flag groups output by function (for single function in test CFG)
- --show-branches flag reserved for future edge detail implementation
- Empty unreachable results handled gracefully with info message
- UnreachableResponse includes function, total_functions, functions_with_unreachable, unreachable_count metadata
- Exported unreachable_block_ids from cfg module for future use

**From 06-05 (Path Verification Command):**
- verify() command checks if cached paths still exist after code changes
- Verification pattern: cache lookup -> re-enumerate -> compare path_ids for existence
- Uses test CFG until MIR extraction complete (Phase 02-01)
- VerifyResult struct includes path_id, valid, found_in_cache, function_id, reason, current_paths
- OptionalExtension trait required for optional query results in rusqlite

**From 06-07 (Output Format Standardization):**
- All CLI commands support three output formats: human (readable text), json (compact), pretty (formatted JSON)
- JsonResponse wrapper provides consistent metadata: schema_version, execution_id, tool, timestamp
- All response structs derive serde::Serialize for JSON compatibility
- snake_case field naming convention for JSON output
- Output format pattern: match cli.output { Human => println!, Json => JsonResponse.to_json(), Pretty => JsonResponse.to_pretty_json() }
- Comprehensive test suite (12 tests) ensures output format consistency across all commands

**From 07-01 (LLM-Optimized JSON Response Structs):**
- PathBlock struct with block_id (usize) and terminator (String) for LLM-optimized path data
- SourceRange struct with file_path, start_line, end_line for future source location integration
- PathSummary.blocks changed from Vec<usize> to Vec<PathBlock> for explicit field names
- Optional summary and source_range fields added to PathSummary (populated in future plans)
- Explicit nulls in JSON output (no skip_serializing_if) helps LLMs distinguish missing vs empty data
- String terminator field provides flexible text representation instead of enum
- Placeholder "Unknown" terminator values in From impl; full data in from_with_cfg

**From 07-02 (Source Location Inclusion in Path Output):**
- from_with_cfg method signature takes Path by value, borrows CFG (needs CFG reference to look up terminators)
- source_range is Option<SourceRange> for AST CFGs without source locations (graceful None handling)
- PathBlock.terminator stores Debug-formatted string for JSON compatibility (not enum directly)
- PathSummary::from_with_cfg provides actual terminator types (Goto, SwitchInt, Return) instead of "Unknown"
- calculate_source_range helper derives path-level span from first and last block source locations
- Metadata enrichment pattern: separate from_with_cfg method rather than replacing From trait

**From 07-03 (Error Remediation Suggestions):**
- Error code format uses "E###" prefix for machine parsing (e.g., E001, E002, E003)
- Remediation hints stored as constants for consistency and maintainability
- Block not found errors don't include remediation (less actionable - block IDs are internal)
- All database errors consistently suggest running 'mirage index' command
- JSON-aware error handling pattern: matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty)
- Human mode retains colored output with separate info() calls for hints

**From 07-04 (Control Flow Natural Language Summaries):**
- Template-based NL generation chosen over external LLM API calls (zero dependency, always works)
- Path truncation at 5 blocks keeps summaries concise for LLM consumption
- Terminator descriptions use simple text format (e.g., "goto b1", "if b2|b3", "switch (3 targets)")
- describe_block() made public for external tool/testing access
- PathSummary.summary field now populated with natural language descriptions via summarize_path()

**From 08-01 (Wire Natural Loops Command):**
- CLI command `mirage loops --function FUNC` displays natural loops detected in function CFGs
- LoopsResponse and LoopInfo structs for JSON output with header, back_edge_from, body_size, nesting_level, body_blocks
- --verbose flag shows detailed loop body block IDs
- Nesting level calculation uses recursive header containment check (0 for outermost, increments for inner loops)
- Integration with detect_natural_loops() from Phase 3 via dominance-based back-edge detection
- Commented out Patterns and Frontiers dispatch in main.rs (to be implemented in 08-02, 08-03)
- Followed dominators() command pattern for consistency

**From 08-04 (Fix Doctest Variable Names):**
- Use no_run flag for incomplete documentation examples instead of full executable code
- Rename cfg variable to graph in all doctests to avoid collision with Rust's built-in cfg! macro
- Provide complete imports and type annotations in doctest examples for self-contained documentation
- Fixed all 34 failing doctests across 7 files (loops, patterns, dominance_frontiers, reachability, paths, dominators, post_dominators)

**From 08-02 (Wire Branching Patterns Command):**
- Filter logic: --if-else excludes match patterns and vice versa; default shows both
- Separate IfElseInfo and MatchInfo structs rather than unified enum for clearer JSON serialization
- Pattern detection functions (detect_if_else_patterns, detect_match_patterns) from Phase 3 now exposed via CLI

**From 08-03 (Wire Dominance Frontiers Command):**
- Three query modes for dominance frontiers: default (all frontiers), --node N (single node), --iterated (iterated frontier for phi placement)
- CLI analysis command pattern: database resolution → test CFG → computation → formatted output
- FrontiersResponse, NodeFrontier, IteratedFrontierResponse structs for JSON serialization
- Follow established CLI patterns from dominators/loops commands for consistency
- Reuse compute_dominance_frontiers() from Phase 4 rather than reimplementing algorithm

**From 08-05 (Implement --show-branches Edge Details):**
- --show-branches flag now shows incoming edge details for unreachable blocks
- IncomingEdge struct with from_block and edge_type fields for JSON serialization
- Edge traversal uses petgraph EdgeRef trait (edge.target(), edge.source(), edge.id())
- Conditional edge collection: only compute incoming_edges when --show-branches is true (performance optimization)
- serde::skip_serializing_if hides empty incoming_edges arrays in JSON output
- Fixed blocking bug: removed non-existent max_count field in paths() command
- Pattern: use petgraph::visit::EdgeRef for edge methods in graph traversal

**From 08-06 (Wire Path Caching to CLI):**
- paths() command changed from enumerate_paths() to get_or_enumerate_paths() for automatic caching
- Test function_id (-1) and hash ("test_cfg") used for test CFG database integration
- Foreign key constraint handling: tests must insert graph_entities entry before storing cfg_paths
- Cache behavior verified: first call enumerates, second call returns cached paths, hash changes trigger invalidation
- Test database setup pattern: Magellan schema → Mirage schema → insert test entity → enable FK → run test
- Database connection changed from _db to mut db for cache operations

**From 09-01 (mirage index command):**
- BLAKE3 function_hash for incremental update detection - only re-index changed functions
- Auto-install Charon with helpful error message when binary not found
- store_cfg() clears existing blocks before insert for atomic updates
- index() uses --incremental flag to skip unchanged functions
- External tool integration: spawn binary, capture stdout, parse JSON
- Incremental indexing pattern: compute hash, compare, skip if unchanged
- Database atomic updates: BEGIN IMMEDIATE TRANSACTION, clear existing, insert new
- Progress indication: show processed, updated, skipped, errors counts

**From 09-02 (Database Loading Utilities):**
- Function resolution accepts both numeric IDs and name strings for CLI flexibility
- Block IDs mapped from database AUTOINCREMENT to sequential graph indices for consistency
- NULL terminators default to Unreachable instead of failing
- Re-export pattern: cfg module re-exports storage functions for convenience
- Helper function pattern: create_test_db_with_schema() for test database setup

**From 09-03 (Wire Database Loading to CLI Commands):**
- All CLI analysis commands now use resolve_function_name() and load_cfg_from_db()
- Error handling pattern: check cli.output format for JSON-aware error messages
- Error remediation: "Run 'mirage index' to index your code" hint for function not found
- unreachable() command scans all functions from database (no function argument)
- Tests simplified to argument parsing rather than full command execution

**From 09-04 (blast-zone Command):**
- BFS traversal with depth tracking instead of DFS for more predictable impact scope
- Block-based and path-based analysis share same reachability core (find_reachable_from_block)
- max_depth defaults to 100 (effectively unlimited) for practical use
- Error path filtering via --include-errors flag for targeted analysis
- BlockImpact and PathImpact structs for structured impact results

**From 10-01 (Magellan v2.0.0 Integration):**
- Magellan v2.0.0 integrated as local path dependency (../magellan)
- rusqlite downgraded from 0.32 to 0.31 to match Magellan's dependency (prevents libsqlite3-sys link conflicts)
- MagellanBridge wrapper provides convenience methods for inter-procedural analysis
- All Magellan algorithm result types re-exported for ergonomic API (CodeGraph, SymbolInfo, Cycle, Slice, etc.)
- Analysis module (src/analysis/mod.rs) provides bridge pattern for combining inter-procedural (Magellan) and intra-procedural (Mirage) analysis

**From 10-04 (Combined Cycle Detection):**
- `--both` flag as default behavior for cycles command shows both call graph and function loops
- Cycle types clearly separated: call graph cycles (architectural coupling) vs function loops (control flow structure)
- HashMap<String, Vec<LoopInfo>> mapping for function loops (multiple loops per function due to nesting)
- Graceful degradation: cycles command continues with function loops if Magellan database unavailable

**From 10-05 (Program Slicing):**
- SliceWrapper and SliceStats provide JSON serialization for Magellan's non-serializable SliceResult type
- backward_slice/forward_slice return SliceWrapper instead of raw SliceResult for CLI compatibility
- --direction flag with Backward/Forward enum for clear intent (what affects vs what is affected)
- Program slicing uses call-graph reachability as fallback (CFG-based slicing future enhancement)

**From 10-03 (Enhanced Blast Zone):**
- Use function name as symbol identifier for call graph queries (not symbol_id) for simpler CLI usage
- Graceful degradation with warning messages when Magellan database unavailable for --use-call-graph flag
- Separate "Inter-Procedural Impact" and "Intra-Procedural Impact" in human output for clarity
- Optional JSON fields with skip_serializing_if for clean output when call graph data is None

**From 11-01 (Call Graph Condensation for Inter-Procedural Dominance):**
- CondensationJson and SupernodeJson wrappers provide CLI serialization for SCC-based condensation results
- From<&CondensationResult> trait implementation enables clean conversion API
- condense_call_graph_json() convenience method returns JSON-serializable output
- Supernode.id converted to String (from i64) for JSON compatibility
- largest_scc_size computed for quick tight coupling assessment
- JSON wrapper pattern follows existing SliceWrapper/SliceStats approach for consistency

**From 11-04 (Inter-procedural Dominance Command):**
- --inter-procedural flag on dominators command enables call graph dominance analysis
- Inter-procedural dominance uses SCC condensation DAG: upstream SCCs dominate downstream SCCs
- can_reach_scc helper function for SCC reachability queries in condensation DAG
- InterProceduralDominanceResponse JSON struct with function, kind, dominator_count, dominators fields
- Graceful degradation with helpful hints when Magellan database unavailable
- Flag-based mode switching pattern: args.inter_procedural routes to call graph analysis early

**From 11-03 (Hotspots CLI Command):**
- Risk score formula differs between modes: inter-procedural uses path_count + dominance*2, intra-procedural uses path_count*0.5 + complexity*0.1
- Graceful degradation: When --inter-procedural set but Magellan DB unavailable, log warning and fall back to intra-procedural
- Database query joins graph_entities: Function names from graph_entities table, not cfg_blocks
- HotspotEntry derives Clone for vector operations in response construction

**From 11-05 (Smart Re-indexing):**
- Git diff as pre-filter for early user feedback, not authoritative (hash comparison remains definitive)
- Non-fatal git detection: graceful fallback to hash-only comparison on git errors
- Helper functions (hash_changed, get_changed_functions) are public for testability and reusability
- Dual-strategy change detection: git diff for broad file changes, hash comparison for precise function changes

### Roadmap Evolution

**Phase 8 added (2026-02-02):** Drift Remediation - Wire Unimplemented Features
- Trigger: Code drift analysis found 6 categories of gaps
- Completed: Path caching, loops detection, patterns detection, dominance frontiers, doctest fixes, --show-branches flag
- Deferred: `mirage index`, `mirage blast-zone` (moved to Phase 9)

**Phase 9 added (2026-02-02):** MIR Integration & Database Loading - COMPLETE
- Trigger: Code drift analysis after Phase 8 completion
- Completed: `mirage index` (MIR extraction via Charon), `mirage blast-zone` (path-based impact analysis), database loading for all CLI commands
- Database loading: All 7 analysis commands now load CFG from database (paths, cfg, dominators, loops, unreachable, patterns, frontiers)
- Requirements delivered: MIR-01 (Charon integration), MIR-02 (ULLBC parsing), MIR-03 (CFG storage), CLI-DB-01 (load CFG from DB), CLI-DB-02 (function lookup), BLAST-01 (block impact), BLAST-02 (path impact)

**Phase 10 added (2026-02-03):** Magellan v2 Integration & Bugfixes - COMPLETE
- Trigger: Integration opportunities identified with Magellan v2.0.0 + compilation errors need fixing
- Completed: Magellan dependency added, analysis module with MagellanBridge wrapper created
- 10-01 complete: Enhanced unreachable command with --include-uncalled flag
- 10-02 complete: unreachable command with --include-uncalled flag
- 10-03 complete: blast zone enhancement with call graph integration
- 10-04 complete: combined cycle detection (call graph + function loops)
- 10-05 complete: program slicing command (backward/forward)
- Deferred: SC 8 (Inter-procedural dominance), SC 9 (Path-based hotspot analysis), SC 10 (Smart re-indexing)

**Phase 11 added (2026-02-03):** Inter-procedural Dominance, Hotspots & Smart Re-indexing - Ready to plan
- Trigger: Phase 10 completion left unused Magellan imports (CondensationGraph, CondensationResult, Supernode, ExecutionPath, PathEnumerationResult, PathStatistics)
- Purpose: Complete Magellan v2 integration, implement deferred SC 8-10 features, achieve zero-warning build
- Target features:
  * SC 8: Inter-procedural dominance analysis using call graph condensation
  * SC 9: Path-based hotspot analysis with risk scoring
  * SC 10: Smart re-indexing using Magellan's graph diff

### Pending Todos

**All Phase 10 plans complete.** No pending todos.

### Blockers/Concerns

- **sccache corruption**: Build cache occasionally returns stale results. Workaround: `RUSTC_WRAPPER=""` env var to bypass. Not blocking but noted.
- **Charon external dependency**: Users must install Charon binary separately. Documented in SUMMARY.md but not enforced.
- **CLI module size**: src/cli/mod.rs is ~6300 lines, making edits difficult. Consider refactoring into smaller modules.

## Session Continuity

Last session: 2026-02-03
Stopped at: Completed 11-05 (Smart Re-indexing)
Resume file: .planning/phases/11-inter-procedural-dominance-hotspots-and-smart-reindexing/11-05-SUMMARY.md
Phase 11 in progress (5/6 plans complete). Overall: 55/56 plans done.
