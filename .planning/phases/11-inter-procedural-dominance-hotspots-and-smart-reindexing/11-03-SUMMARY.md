---
phase: 11-inter-procedural-dominance-hotspots-and-smart-reindexing
plan: 03
subsystem: cli-analysis
tags: [hotspots, risk-scoring, inter-procedural, intra-procedural, call-graph, path-enumeration]

# Dependency graph
requires:
  - phase: 10-magellan-v2-integration-and-bugfixes
    provides: MagellanBridge, PathEnumerationJson, CondensationJson
  - phase: 05-path-enumeration
    provides: enumerate_paths_with_context, PathLimits, EnumerationContext
  - phase: 02-cfg-construction
    provides: load_cfg_from_db, CFG node_count
provides:
  - Hotspots CLI command with risk scoring
  - HotspotsArgs and HotspotsEntry response types
  - Inter-procedural and intra-procedural hotspot analysis modes
affects: [11-04-smart-reindexing, future-refactoring-tools]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Risk scoring combining multiple metrics (path_count, dominance, complexity)
    - Graceful degradation from inter-procedural to intra-procedural analysis
    - JSON-serializable response structs with Clone derive for vector operations

key-files:
  created: []
  modified:
    - src/cli/mod.rs - Hotspots command, args, response structs
    - src/main.rs - Command dispatch wiring

key-decisions:
  - "Risk score formula: path_count * 1.0 + dominance_factor * 2.0 (inter-procedural) or path_count * 0.5 + complexity * 0.1 (intra-procedural)"
  - "Graceful fallback: When Magellan DB unavailable, automatically use intra-procedural analysis instead of failing"
  - "Dominance factor: SCC size from call graph condensation indicates mutual recursion coupling risk"

patterns-established:
  - "CLI response struct pattern: serde::Serialize + Clone for JSON output and vector cloning"
  - "Dual-mode analysis: inter-procedural (via Magellan) with intra-procedural fallback"

# Metrics
duration: 8min 13s
completed: 2026-02-03
---

# Phase 11 Plan 03: Hotspots Command Summary

**Hotspots CLI command combining path counts, call graph dominance, and complexity metrics to identify high-risk functions**

## Performance

- **Duration:** 8 min 13s
- **Started:** 2026-02-03T15:09:48Z
- **Completed:** 2026-02-03T15:18:01Z
- **Tasks:** 3 (plus 1 auto-fix)
- **Files modified:** 2

## Accomplishments
- Hotspots CLI command with `mirage hotspots` entry point
- Risk scoring algorithm combining path counts, dominance factor, and complexity
- Support for both inter-procedural (Magellan) and intra-procedural (CFG) analysis modes
- Graceful fallback when Magellan database unavailable
- All three output formats (human, json, pretty) working

## Task Commits

Each task was committed atomically:

1. **Task 1: Add HotspotsArgs and response structs to CLI** - `a09163b` (feat)
2. **Task 2: Implement hotspots command in cmds module** - `f311827` (feat)
3. **Task 3: Wire hotspots command in main.rs dispatch** - `98dfd5a` (feat)
4. **Auto-fix: Fix SQL query for hotspots function lookup** - `d4c84c4` (fix)

**Plan metadata:** N/A (no separate metadata commit)

## Files Created/Modified
- `src/cli/mod.rs` - Added HotspotsArgs, HotspotsResponse, HotspotEntry, hotspots() function
- `src/main.rs` - Added Commands::Hotspots match arm in run_command()

## Decisions Made

1. **Risk score formula differs between modes**: Inter-procedural uses path_count + dominance*2 (dominance weighted higher for SCC coupling), intra-procedural uses path_count*0.5 + complexity*0.1 (more balanced)
2. **Graceful degradation pattern**: When `--inter-procedural` is set but Magellan DB is unavailable, log a warning and fall back to intra-procedural rather than failing
3. **Database query joins graph_entities**: Function names come from graph_entities table (not cfg_blocks), requiring JOIN for proper lookup

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed SQL query for function name lookup**
- **Found during:** Task 3 verification (testing `mirage hotspots` command)
- **Issue:** Query referenced non-existent `function_name` column in cfg_blocks table. Function names are stored in graph_entities table.
- **Fix:** Changed query to JOIN cfg_blocks with graph_entities: `SELECT DISTINCT cb.function_id, ge.name, ge.file_path FROM cfg_blocks cb JOIN graph_entities ge ON cb.function_id = ge.id`
- **Files modified:** src/cli/mod.rs
- **Verification:** `mirage hotspots --output json` now returns valid JSON response instead of SQL error
- **Committed in:** `d4c84c4`

---

**Total deviations:** 1 auto-fixed (1 blocking issue)
**Impact on plan:** SQL query fix was necessary for the command to work at all. No scope creep.

## Issues Encountered
- MirageDb.conn is private field, needed to use MirageDb.conn_mut() method instead
- HotspotEntry needed Clone derive for vector cloning in response construction

## User Setup Required

None - no external service configuration required. Users must have a Mirage database (created via `mirage index`) to see results. Magellan database is optional for inter-procedural mode.

## Next Phase Readiness
- Hotspots command fully functional and ready for use
- Risk scoring algorithm provides actionable metrics for prioritizing refactoring/testing
- Pattern established for combining multiple analysis metrics into single score

**Blockers/Concerns:** None

---
*Phase: 11-inter-procedural-dominance-hotspots-and-smart-reindexing*
*Plan: 03*
*Completed: 2026-02-03*
