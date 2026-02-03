# Roadmap: Mirage

## Overview

Mirage transforms code into verifiable graph artifacts. Starting from database schema and MIR extraction, we build control flow graphs, analyze dominance relationships, enumerate execution paths, and expose everything through a CLI that produces structured outputs for LLM consumption. Every phase delivers something that can verified against the graph.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Database Foundation** - Extend Magellan schema with CFG tables ✓ Completed 2026-02-01
- [x] **Phase 2: CFG Construction** - Build control flow graphs from MIR and AST ✓ Completed 2026-02-01
- [x] **Phase 3: Reachability & Control Structure** - Analyze what code can execute ✓ Completed 2026-02-01
- [x] **Phase 4: Dominance Analysis** - Compute must-pass-through relationships ✓ Completed 2026-02-01
- [x] **Phase 5: Path Enumeration** - Enumerate and classify execution paths ✓ Completed 2026-02-01
- [x] **Phase 6: CLI Interface** - User-facing commands for all analysis types ✓ Completed 2026-02-01
- [x] **Phase 7: LLM Integration** - Structured outputs for agent consumption ✓ Completed 2026-02-01
- [x] **Phase 8: Drift Remediation** - Wire unimplemented features and fix gaps ✓ Completed 2026-02-02
- [x] **Phase 9: MIR Integration & Database Loading** - Implement index command, database loading, and blast zone ✓ Completed 2026-02-02
- [x] **Phase 10: Magellan v2 Integration & Bugfixes** - Integrate Magellan v2.0.0 graph algorithms and fix compilation issues ✓ Completed 2026-02-03
- [ ] **Phase 11: Inter-procedural Dominance, Hotspots & Smart Re-indexing** - Complete Magellan integration with condensation, hotspot analysis, and incremental re-indexing

## Phase Details

### Phase 1: Database Foundation

**Goal**: Mirage extends the Magellan database with tables for storing control flow graphs, paths, and dominance relationships, enabling incremental updates as code changes.

**Depends on**: Nothing (first phase)

**Requirements**: DB-01, DB-02, DB-03, DB-04, DB-05, DB-06

**Success Criteria** (what must be TRUE):
1. Database migration creates cfg_blocks, cfg_edges, cfg_paths, and cfg_dominators tables in Magellan database
2. cfg_blocks.function_id correctly foreign keys to graph_entities.id
3. Code changes trigger function-level re-analysis (not whole-program)
4. Schema version is tracked for future migrations

**Status**: ✓ Completed 2026-02-01

**Plans**: 3 plans in 3 waves

Plans:
- [x] 01-01 — Database schema and migrations (function_hash tracking, migration framework)
- [x] 01-02 — Wire up status command to database statistics
- [x] 01-03 — Database integration tests (foreign keys, incremental updates, migrations)

### Phase 2: CFG Construction

**Goal**: Mirage builds control flow graphs from MIR (Rust) and AST (other languages), identifying basic blocks, control flow edges, entry/exit nodes, and mapping everything back to source locations.

**Depends on**: Phase 1

**Requirements**: CFG-01, CFG-02, CFG-03, CFG-04, CFG-05, CFG-06, MIR-01, MIR-02, MIR-03

**Success Criteria** (what must be TRUE):
1. System identifies basic blocks as maximal straight-line sequences in functions
2. System discovers all control flow edges (conditional, unconditional, exceptional)
3. Entry and exit nodes are correctly identified for each function
4. CFG can be exported in DOT format for visualization
5. CFG can be exported in JSON format for tool integration
6. Each CFG node maps back to source location (file, line, column)
7. MIR extraction works via Charon for Rust code
8. AST-based CFG works as fallback for non-Rust code

**Plans**: 6 plans in 4 waves

Plans:
- [x] 02-03 — Core CFG data structures (Cfg, BasicBlock, EdgeType) with petgraph
- [x] 02-01 — MIR extraction via Charon (ULLBC parsing and conversion)
- [x] 02-02 — AST-based CFG construction (tree-sitter, leader algorithm)
- [x] 02-04 — Entry/exit node detection (analysis functions)
- [x] 02-05 — Source location mapping (spans to line/column)
- [x] 02-06 — DOT and JSON export (visualization and tool integration)

### Phase 3: Reachability & Control Structure

**Goal**: Given a CFG, Mirage determines which code blocks are reachable, recovers natural loops, and identifies branching patterns.

**Depends on**: Phase 2

**Requirements**: REACH-01, REACH-02, REACH-03, CTRL-01, CTRL-02, CTRL-03, CTRL-04

**Success Criteria** (what must be TRUE):
1. System detects unreachable code blocks (no path from entry)
2. System answers reachability queries (can node A reach node B?)
3. System determines if path exists between two points
4. Natural loops are detected (back-edge where head dominates tail)
5. Loop header nodes are identified
6. If/else branching patterns are recovered
7. Match/expression branching patterns are recovered

**Plans**: 4 plans in 2 waves

Plans:
- [x] 03-01 — Unreachable code detection (find_unreachable, find_reachable)
- [x] 03-02 — Reachability query engine (can_reach, ReachabilityCache)
- [x] 03-03 — Natural loop detection (detect_natural_loops, find_loop_headers)
- [x] 03-04 — Branching pattern recovery (detect_if_else, detect_match)

**Status**: ✓ Completed 2026-02-01

### Phase 4: Dominance Analysis

**Goal**: Mirage computes dominance relationships that determine which code MUST execute on any path from entry to exit, enabling must-pass-through proofs.

**Depends on**: Phase 2

**Requirements**: DOM-01, DOM-02, DOM-03, DOM-04

**Success Criteria** (what must be TRUE):
1. System computes immediate dominators for all nodes
2. System computes dominator tree
3. System computes post-dominators
4. System computes dominance frontiers

**Status**: ✓ Completed 2026-02-01

**Plans**: 3 plans in 3 waves

Plans:
- [x] 04-01 — Dominator tree construction (DominatorTree wrapper with simple_fast)
- [x] 04-02 — Post-dominator tree construction (PostDominatorTree via Reversed adaptor)
- [x] 04-03 — Dominance frontier computation (DominanceFrontiers using Cytron et al. algorithm)

### Phase 5: Path Enumeration

**Goal**: Mirage enumerates execution paths through functions, classifying them by type (normal, error, degenerate, unreachable) and caching results with deterministic IDs.

**Depends on**: Phase 2

**Requirements**: PATH-01, PATH-02, PATH-03, PATH-04, PATH-05, PATH-06, PERF-01, PERF-02, PERF-03

**Success Criteria** (what must be TRUE):
1. System enumerates all feasible execution paths through a function
2. Paths are classified (normal, error, degenerate, unreachable)
3. Path length bounding prevents explosion (configurable limit)
4. Feasible paths are distinguished from infeasible paths
5. Enumerated paths are cached with BLAKE3 IDs
6. Each path has unique path ID for reference
7. CFG construction completes in O(n) time relative to code size
8. Path enumeration respects configurable depth and count limits
9. Database updates are incremental at function level

**Plans**: 6 plans in 4 waves

Plans:
- [x] 05-01-PLAN.md — DFS-based path enumeration core (Path, PathKind, PathLimits, enumerate_paths, hash_path)
- [x] 05-02-PLAN.md — Path classification (classify_path, classify_path_precomputed)
- [x] 05-03-PLAN.md — Loop bounding and limits (PathLimits enforcement, cycle detection, nested loop handling)
- [x] 05-04-PLAN.md — Feasibility checking (is_feasible_path, static analysis)
- [x] 05-05-PLAN.md — Path caching with BLAKE3 (store_paths, get_cached_paths, invalidate_function_paths)
- [x] 05-06-PLAN.md — Performance optimization (batch inserts, EnumerationContext, enumerate_paths_cached)

**Status**: ✓ Completed 2026-02-01

### Phase 6: CLI Interface

**Goal**: Users interact with Mirage through a CLI that follows Magellan patterns, providing commands for paths, CFG visualization, dominators, unreachable code, and verification.

**Depends on**: Phase 2, Phase 3, Phase 4, Phase 5

**Requirements**: CLI-01, CLI-02, CLI-03, CLI-04, CLI-05, CLI-06, CLI-07, CLI-08, CLI-09, CLI-10, CLI-11, CLI-12

**Success Criteria** (what must be TRUE):
1. `mirage paths --function SYMBOL` shows all paths for function
2. `mirage paths --show-errors` shows only error paths
3. `mirage paths --max-length N` bounds path exploration
4. `mirage cfg --function SYMBOL` shows human-readable CFG
5. `mirage cfg --format dot` exports Graphviz DOT
6. `mirage cfg --format json` exports JSON
7. `mirage dominators --function SYMBOL` shows dominance tree
8. `mirage dominators --must-pass-through BLOCK` proves mandatory execution
9. `mirage unreachable` finds unreachable code blocks
10. `mirage verify --path-id ID` verifies path still valid
11. `mirage status` shows database statistics
12. All commands support `--output json|pretty|human`

**Status**: ✓ Completed 2026-02-01

**Plans**: 7 plans in 4 waves

Plans:
- [x] 06-01-PLAN.md — Path query commands (paths --function, --show-errors, --max-length)
- [x] 06-02-PLAN.md — CFG visualization commands (cfg --format dot/json/human)
- [x] 06-03-PLAN.md — Dominator commands (--must-pass-through, --post)
- [x] 06-04-PLAN.md — Unreachable code command (--show-branches)
- [x] 06-05-PLAN.md — Verification command (verify --path-id)
- [x] 06-06-PLAN.md — Status command verification (already implemented)
- [x] 06-07-PLAN.md — Output format standardization across all commands

**Note:** CLI-09 (--show-branches flag for unreachable command) is implemented but shows a placeholder message. This is a minor enhancement that can be added in a future update.

### Phase 7: LLM Integration

**Goal**: Mirage produces structured JSON outputs with path IDs, source locations, and natural language summaries that enable LLMs to reason about control flow without hallucination.

**Depends on**: Phase 5, Phase 6

**Requirements**: LLM-01, LLM-02, LLM-03, LLM-04

**Success Criteria** (what must be TRUE):
1. Path queries return structured JSON with path IDs
2. Path results include block sequence and source locations
3. Error responses include remediation suggestions
4. System provides natural language summaries of control flow

**Status**: ✓ Completed 2026-02-01

**Plans**: 4 plans in 3 waves

Plans:
- [x] 07-01-PLAN.md — Structured JSON output format (PathBlock, SourceRange structs)
- [x] 07-02-PLAN.md — Source location inclusion (from_with_cfg with terminator and range)
- [x] 07-03-PLAN.md — Error remediation suggestions (error codes, JsonError helpers)
- [x] 07-04-PLAN.md — Control flow natural language summaries (summarize_path, describe_block)

### Phase 8: Drift Remediation

**Goal**: Wire up all unimplemented features, fix stub commands, enable path caching, add CLI commands for unused analysis capabilities, fix doctests, and implement placeholder flags.

**Depends on**: Phase 5, Phase 6

**Requirements**: DRIFT-01, DRIFT-02, DRIFT-03, DRIFT-04, DRIFT-05, DRIFT-06

**Success Criteria** (what must be TRUE):
1. `mirage index` command implements MIR extraction (no longer stub)
2. `mirage blast-zone` command implements path-based impact analysis (no longer stub)
3. Path caching functions wired to CLI (cached queries actually use database)
4. `mirage loops` command shows natural loops in CFG
5. `mirage patterns` command shows if-else/match patterns
6. `mirage frontiers` command shows dominance frontiers
7. All doctests pass (34 currently failing due to cfg! macro collision)
8. `--show-branches` flag shows actual branch edge details

**Plans**: 6 plans in 2 waves

Plans:
- [x] 08-01 — Wire natural loops to CLI (mirage loops command)
- [x] 08-02 — Wire pattern detection to CLI (mirage patterns command)
- [x] 08-03 — Wire dominance frontiers to CLI (mirage frontiers command)
- [x] 08-04 — Fix doctests (cfg! macro collision)
- [x] 08-05 — Implement --show-branches (incoming edge details)
- [x] 08-06 — Wire path caching to CLI (get_or_enumerate_paths integration)

**Status**: ✓ Complete 2026-02-02

### Phase 9: MIR Integration & Database Loading

**Goal**: Implement the `mirage index` command for MIR extraction via Charon, wire database loading for all analysis commands, and implement blast zone impact analysis.

**Depends on**: Phase 2 (MIR extraction infrastructure exists but not wired), Phase 5 (path enumeration), Phase 6 (CLI interface)

**Requirements**: MIR-01, MIR-02, MIR-03, CLI-DB-01, CLI-DB-02, BLAST-01, BLAST-02

**Success Criteria** (what must be TRUE):
1. `mirage index --project PATH` extracts MIR via Charon and stores CFGs in database
2. `mirage index --crate NAME` indexes specific crate with function-level CFGs
3. `mirage index --incremental` only re-indexes changed functions (function_hash comparison)
4. All analysis commands (paths, cfg, dominators, loops, patterns, frontiers) load CFGs from database instead of test data
5. `mirage blast-zone --function SYMBOL --block-id N` shows all functions reachable from given block
6. `mirage blast-zone --path-id ID` shows impact scope for specific execution path
7. Database stores block-to-function mappings for impact analysis
8. Charon binary integration works (spawning, parsing ULLBC JSON)

**Plans**: 4 plans in 3 waves

Plans:
- [x] 09-01 — Implement mirage index command (Charon integration, CFG storage)
- [x] 09-02 — Create shared database loading utilities (resolve_function_name, load_cfg_from_db)
- [x] 09-03 — Wire database loading to all CLI commands (replace create_test_cfg)
- [x] 09-04 — Implement mirage blast-zone command (block and path impact analysis)

**Status**: ✓ Complete 2026-02-02

### Phase 10: Magellan v2 Integration & Bugfixes

**Goal**: Integrate Magellan v2.0.0 graph algorithms to enhance inter-procedural analysis capabilities.

**Depends on**: Phase 9, Magellan v2.0.0

**Requirements**: MAG2-01, MAG2-02, MAG2-03, MAG2-04, MAG2-05, MAG2-06, MAG2-07, MAG2-08

**Success Criteria** (what must be TRUE):
1. Magellan v2.0.0 added as dependency and compiles
2. Enhanced reachability combines Magellan's uncalled functions with Mirage's unreachable blocks
3. Blast zone analysis uses call graph reachability + path enumeration
4. Cyclic dependency detection combines SCC detection with intra-function loops
5. Code slicing via Magellan's slice algorithm is available
6. Analysis module with MagellanBridge wrapper provides clean API
7. All commands support combined inter/intra procedural analysis

**Plans**: 5 plans in 4 waves

Plans:
- [x] 10-01 — Add Magellan dependency and create analysis module with MagellanBridge
- [x] 10-02 — Enhanced reachability (uncalled functions + unreachable blocks)
- [x] 10-03 — Improved blast zone with call graph reachability
- [x] 10-04 — Cyclic dependency detection (call graph SCCs + function loops)
- [x] 10-05 — Code slicing command (backward/forward program slicing)

**Status**: ✓ Complete 2026-02-03

### Phase 11: Inter-procedural Dominance, Hotspots & Smart Re-indexing

**Goal**: Complete Magellan v2.0.0 integration with call graph condensation, path-based hotspot analysis, and smart incremental re-indexing.

**Depends on**: Phase 10, Magellan v2.0.0

**Success Criteria** (what must be TRUE):
1. Inter-procedural dominance analysis uses call graph condensation to identify critical path functions
2. Hotspot command combines path counts, call dominance, and complexity for risk scoring
3. Smart re-indexing uses Magellan's graph diff to only re-index affected functions
4. All previously unused Magellan imports are now utilized
5. Project compiles with zero warnings

**Plans**: 6 plans in 4 waves

Plans:
- [x] 11-01 — Wire CondensationGraph to MagellanBridge for inter-procedural dominance (JSON wrappers)
- [ ] 11-02 — Implement path-based hotspot analysis infrastructure (ExecutionPath wrappers)
- [ ] 11-03 — Create hotspots CLI command combining path counts, call dominance, complexity
- [ ] 11-04 — Add --inter-procedural flag to dominators command
- [ ] 11-05 — Implement smart re-indexing with graph diff helpers
- [ ] 11-06 — Clean up - remove all unused imports, verify zero warnings

**Status**: Ready to execute

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9 -> 10 -> 11

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Database Foundation | 3/3 | ✓ Complete | 2026-02-01 |
| 2. CFG Construction | 6/6 | ✓ Complete | 2026-02-01 |
| 3. Reachability & Control Structure | 4/4 | ✓ Complete | 2026-02-01 |
| 4. Dominance Analysis | 3/3 | ✓ Complete | 2026-02-01 |
| 5. Path Enumeration | 6/6 | ✓ Complete | 2026-02-01 |
| 6. CLI Interface | 7/7 | ✓ Complete | 2026-02-01 |
| 7. LLM Integration | 4/4 | ✓ Complete | 2026-02-01 |
| 8. Drift Remediation | 6/6 | ✓ Complete | 2026-02-02 |
| 9. MIR Integration & Database Loading | 4/4 | ✓ Complete | 2026-02-02 |
| 10. Magellan v2 Integration & Bugfixes | 5/5 | ✓ Complete | 2026-02-03 |
| 11. Inter-procedural Dominance, Hotspots & Smart Re-indexing | 1/6 | In progress | 2026-02-03 |

---

**Total Phases:** 11
**Total Requirements:** 68 (68 complete, 0 pending)
**Coverage:** 68/68 requirements complete (100%)
**Total Plans:** 55 (50 complete, 5 planned)
