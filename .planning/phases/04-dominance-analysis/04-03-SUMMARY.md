---
phase: 04-dominance-analysis
plan: 03
subsystem: cfg-analysis
tags: [dominance-frontiers, cytron-algorithm, ssa-phi-placement, control-dependence]

# Dependency graph
requires:
  - phase: 04-01
    provides: DominatorTree with dominates() and children() methods
provides:
  - DominanceFrontiers struct computing join points in control flow
  - frontier() method returning HashSet of nodes in dominance frontier
  - iterated_frontier() for SSA phi-node placement
  - compute_dominance_frontiers() convenience function
affects: [05-ssa-construction, 06-control-dependence]

# Tech tracking
tech-stack:
  added: []
  patterns: [cytron-iterative-frontier, self-frontier-loop-detection]

key-files:
  created: [src/cfg/dominance_frontiers.rs]
  modified: [src/cfg/mod.rs]

key-decisions:
  - "Returned owned HashSet from frontier() instead of reference to avoid lifetime issues"
  - "Corrected test expectations: nodes in their own frontier occur with back edges (loop headers)"

patterns-established:
  - "Self-frontier pattern: loop header n has n in DF[n] due to back edge from dominated node"
  - "Iterated frontier: closure under dominance relation for phi placement"

# Metrics
duration: 5min
completed: 2026-02-01
---

# Phase 4: Dominance Analysis - Plan 03 Summary

**Dominance frontier computation using Cytron et al. O(|V|²) algorithm for SSA phi-node placement and control dependence analysis**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-01T18:37:02Z
- **Completed:** 2026-02-01T18:42:43Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Implemented Cytron et al. dominance frontier algorithm with two rules: strict dominance boundary + children frontier propagation
- DominanceFrontiers struct providing frontier(), in_frontier(), iterated_frontier(), union_frontier(), nodes_with_frontiers() methods
- Comprehensive test coverage for diamond CFG, loop CFG, linear CFG, and complex join patterns
- Exported public API from crate root for SSA construction and control dependence analysis

## Task Commits

1. **Task 1: Create dominance_frontiers module with DominanceFrontiers struct** - `069aebb` (feat)

**Plan metadata:** N/A (single task plan)

## Files Created/Modified

- `src/cfg/dominance_frontiers.rs` - Dominance frontier computation with Cytron et al. algorithm
- `src/cfg/mod.rs` - Added `pub mod dominance_frontiers` and re-exports

## Decisions Made

### API Design Decision: frontier() returns owned HashSet

**Rationale:** The initial plan specified returning `&HashSet<NodeIndex>`, but this creates lifetime issues with the empty set case. Options considered:
1. Static empty set with LazyLock (complex, adds dependency)
2. Return Option (ergonomics hit)
3. Return owned HashSet (chosen - simple, clean API)

Decision: Return `HashSet<NodeIndex>` (cloned from internal map). Dominance frontiers are typically computed once and queried multiple times, so the clone cost is amortized.

### Test Expectation Corrections

**Issue:** Original plan tests had incorrect expectations for dominance frontiers. The plan expected DF[0] = {3} in diamond CFG, but entry (0) strictly dominates all nodes in a diamond CFG.

**Resolution:** Corrected test expectations based on standard dominance theory:
- Diamond CFG: DF[0] = {}, DF[1] = {3}, DF[2] = {3} (nodes 1 and 2 have frontier at merge point)
- Loop CFG: DF[1] = {1} (self-frontier characterizes loop headers)
- Linear CFG: All frontiers empty (no join points)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed frontier() lifetime issue**
- **Found during:** Task 1 (implementation)
- **Issue:** `frontier()` returning `&HashSet<NodeIndex>` with static empty set failed const eval
- **Fix:** Changed signature to return `HashSet<NodeIndex>` (owned, cloned from internal map)
- **Files modified:** src/cfg/dominance_frontiers.rs
- **Verification:** cargo check passes
- **Committed in:** 069aebb

**2. [Rule 2 - Missing Critical] Updated test expectations to match dominance theory**
- **Found during:** Task 1 (test verification)
- **Issue:** Plan test expected DF[0] = {3} in diamond CFG, but entry strictly dominates all nodes
- **Fix:** Corrected all test expectations based on standard dominance frontier definition
- **Files modified:** src/cfg/dominance_frontiers.rs
- **Verification:** All 9 tests pass
- **Committed in:** 069aebb

---

**Total deviations:** 2 auto-fixed (1 bug, 1 test correction)
**Impact on plan:** Both fixes necessary for correctness. API change is improvement (cleaner semantics). Test corrections align implementation with theory.

## Issues Encountered

**sccache corruption:** Build cache returned stale results, bypassed with `RUSTC_WRAPPER=""` env var. Noted in STATE.md as recurring workaround.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Dominance analysis now complete with:
- DominatorTree (04-01) - immediate dominators and dominance queries
- PostDominatorTree (04-02) - post-dominance via graph reversal
- DominanceFrontiers (04-03) - join points for phi placement

Ready for:
- SSA construction (Phase 5) - using dominance frontiers for phi node placement
- Control dependence analysis (Phase 6) - using dominance relationships

**Key insight for next phase:** Self-frontier pattern (n in DF[n]) identifies loop headers - useful for SSA variable renaming in loop bodies.

---
*Phase: 04-dominance-analysis*
*Completed: 2026-02-01*
