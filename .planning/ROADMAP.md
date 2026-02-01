# Roadmap: Mirage

## Overview

Mirage transforms code into verifiable graph artifacts. Starting from database schema and MIR extraction, we build control flow graphs, analyze dominance relationships, enumerate execution paths, and expose everything through a CLI that produces structured outputs for LLM consumption. Every phase delivers something that can be verified against the graph.

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
- [ ] **Phase 6: CLI Interface** - User-facing commands for all analysis types
- [ ] **Phase 7: LLM Integration** - Structured outputs for agent consumption

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

**Plans**: TBD

Plans:
- [ ] 06-01: Path query commands
- [ ] 06-02: CFG visualization commands
- [ ] 06-03: Dominator commands
- [ ] 06-04: Unreachable code command
- [ ] 06-05: Verification command
- [ ] 06-06: Status command
- [ ] 06-07: Output format standardization

### Phase 7: LLM Integration

**Goal**: Mirage produces structured JSON outputs with path IDs, source locations, and natural language summaries that enable LLMs to reason about control flow without hallucination.

**Depends on**: Phase 5, Phase 6

**Requirements**: LLM-01, LLM-02, LLM-03, LLM-04

**Success Criteria** (what must be TRUE):
1. Path queries return structured JSON with path IDs
2. Path results include block sequence and source locations
3. Error responses include remediation suggestions
4. System provides natural language summaries of control flow

**Plans**: TBD

Plans:
- [ ] 07-01: Structured JSON output format
- [ ] 07-02: Source location inclusion
- [ ] 07-03: Error remediation suggestions
- [ ] 07-04: Control flow natural language summaries

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Database Foundation | 3/3 | ✓ Complete | 2026-02-01 |
| 2. CFG Construction | 6/6 | ✓ Complete | 2026-02-01 |
| 3. Reachability & Control Structure | 4/4 | ✓ Complete | 2026-02-01 |
| 4. Dominance Analysis | 3/3 | ✓ Complete | 2026-02-01 |
| 5. Path Enumeration | 6/6 | ✓ Complete | 2026-02-01 |
| 6. CLI Interface | 0/TBD | Not started | - |
| 7. LLM Integration | 0/TBD | Not started | - |

---

**Total Phases:** 7
**Total Requirements:** 51
**Coverage:** 51/51 requirements mapped
