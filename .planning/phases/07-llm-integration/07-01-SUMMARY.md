---
phase: 07-llm-integration
plan: 01
subsystem: llm-integration
tags: [serde, json, path-serialization, llm-output]

# Dependency graph
requires:
  - phase: 06-cli-interface
    provides: paths() command with PathSummary struct, JsonResponse wrapper
  - phase: 05-path-enumeration
    provides: Path struct with path_id, kind, blocks, length fields
provides:
  - LLM-optimized PathBlock struct with block_id and terminator fields
  - SourceRange struct for future source location integration
  - Enhanced PathSummary with Vec<PathBlock> instead of Vec<usize>
  - Optional summary and source_range fields for future plans
affects: [07-02-source-locations, 07-04-path-summaries]

# Tech tracking
tech-stack:
  added: []
  patterns: [LLM-optimized JSON output, explicit nulls for optional fields, placeholder data for future integration]

key-files:
  modified: [src/cli/mod.rs]

key-decisions:
  - "Use String for terminator field (flexible text representation instead of enum)"
  - "Explicit nulls for optional fields (no skip_serializing_if) - helps LLMs distinguish missing vs null data"
  - "Placeholder 'Unknown' terminator values - full info added in plan 07-02"

patterns-established:
  - "PathBlock struct: LLM-optimized block representation with explicit field names"
  - "SourceRange struct: Defined now, populated later (plan 07-02) for source location data"
  - "PathSummary::from() with placeholder data, from_with_cfg() for enriched data"

# Metrics
duration: 5.5min
completed: 2026-02-01
---

# Phase 7 Plan 1: LLM-Optimized JSON Response Structs Summary

**Path queries now return structured block-level data with PathBlock structs containing block_id and terminator, enabling LLMs to parse execution paths without ambiguity**

## Performance

- **Duration:** 5.5 minutes
- **Started:** 2026-02-01T22:40:00Z
- **Completed:** 2026-02-01T22:45:30Z
- **Tasks:** 3
- **Files modified:** 1 (src/cli/mod.rs)

## Accomplishments

- Added `PathBlock` struct with `block_id` (usize) and `terminator` (String) fields
- Added `SourceRange` struct with `file_path`, `start_line`, `end_line` for future source location integration
- Updated `PathSummary.blocks` from `Vec<usize>` to `Vec<PathBlock>` for LLM-optimized structure
- Added optional `summary` and `source_range` fields to PathSummary (populated in future plans)
- Updated `PathSummary::from()` to create PathBlock structs with placeholder "Unknown" terminator
- Added `Debug`/`PartialEq` derives to PathBlock for test assertions

## Task Commits

1. **Task 1: Add LLM-optimized response structs** - Pre-committed via watcher (c5eabe3)
2. **Task 2: Update PathSummary::from implementation** - Pre-committed via watcher (c5eabe3)
3. **Task 3: Verify JSON output structure** - Verified with manual test

**Note:** Some work was pre-completed by a file watcher before execution. The plan tasks were verified to be complete.

## Files Created/Modified

- `src/cli/mod.rs` - Added PathBlock, SourceRange structs; updated PathSummary; modified From implementation

## Decisions Made

- **String terminator field:** Used flexible String representation instead of enum for maximum compatibility with different CFG terminator types
- **Explicit nulls:** Did NOT add `#[serde(skip_serializing_if = "Option::is_none")]` - explicit nulls help LLMs distinguish between "not provided" and "empty"
- **Placeholder data:** Used "Unknown" for terminator in `From` impl since CFG not available in that context; full terminator info added via `from_with_cfg()` which was pre-completed

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- **Test failure:** Initial `test_path_summary_from_path` test failed because it expected `Vec<usize>` for blocks. Fixed by updating test assertions to check `Vec<PathBlock>` structure with `block_id` and `terminator` fields.
- **Missing derives:** PathBlock needed `Debug`, `Clone`, `PartialEq`, `Eq` derives for test assertions and comparisons. Added these derives.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- PathBlock and SourceRange structs are ready for population with real data
- `from_with_cfg()` method (pre-completed) provides template for source location integration in plan 07-02
- Optional `summary` field ready for population in plan 07-04
- JSON output verified to contain correct structure with explicit nulls

---
*Phase: 07-llm-integration*
*Completed: 2026-02-01*
