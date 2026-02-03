---
phase: 11-inter-procedural-dominance-hotspots-and-smart-reindexing
plan: 05
subsystem: indexing
tags: [incremental-indexing, git-diff, hash-comparison, smart-reindex]

# Dependency graph
requires:
  - phase: 09-mir-integration-database-loading
    provides: function_hash storage, graph_entities table, incremental indexing framework
  - phase: 11-03
    provides: hotspots command, storage module extensions
provides:
  - Smart incremental indexing using git diff for changed file detection
  - hash_changed() helper for consistent hash comparison
  - get_changed_functions() for git-based change detection
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Git-based pre-filtering for incremental operations
    - Dual-strategy change detection (git diff + hash comparison)
    - Non-fatal fallback pattern (git failure -> hash-only)

key-files:
  created: []
  modified:
    - src/storage/mod.rs: Added hash_changed, get_changed_functions, get_function_file
    - src/cli/mod.rs: Enhanced index command with smart re-indexing

key-decisions:
  - "Git diff provides early user feedback but hash comparison remains authoritative"
  - "Non-fatal git detection: graceful fallback to hash comparison on git errors"
  - "Use hash_changed() helper for cleaner, testable incremental logic"

patterns-established:
  - "Pre-filter pattern: git diff for early feedback, hash comparison for accuracy"
  - "Non-blocking optional enhancements: feature failure doesn't prevent core operation"

# Metrics
duration: 4min
completed: 2026-02-03
---

# Phase 11 Plan 05: Smart Re-indexing Summary

**Git diff-based pre-filtering with BLAKE3 hash comparison for incremental re-indexing**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-03T15:20:51Z
- **Completed:** 2026-02-03T15:24:51Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `hash_changed()` helper to storage module for consistent hash comparison
- Added `get_changed_functions()` to detect changed Rust files via git diff
- Added `get_function_file()` to map function names to source files
- Enhanced index command with smart re-indexing using dual-strategy detection
- Updated test fixtures for DominatorsArgs (inter_procedural field)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add graph diff helpers to storage module** - `de245f4` (feat)
2. **Task 2: Enhance index command with smart re-indexing** - `81dcf96` (feat)

**Plan metadata:** N/A (no separate metadata commit)

## Files Created/Modified

- `src/storage/mod.rs` - Added hash_changed(), get_changed_functions(), get_function_file() helpers
- `src/cli/mod.rs` - Enhanced index() with git diff pre-filtering and hash_changed() usage

## Decisions Made

- **Git diff as pre-filter, not authoritative:** Git diff provides early feedback about potentially changed functions, but hash comparison remains the authoritative check for whether a function needs re-indexing. This prevents false negatives from git edge cases.
- **Non-fatal git detection:** If git diff fails (not a git repo, git not available), the indexing continues with hash-only comparison. The warning message clearly indicates the fallback mode.
- **Helper functions for testability:** hash_changed() and get_changed_functions() are public functions in storage module, making them testable and reusable.

## Deviations from Plan

None - plan executed exactly as written. The implementation matches the specified approach of using git diff for detection while maintaining hash comparison as the authoritative check.

## Issues Encountered

None

## Authentication Gates

None

## Next Phase Readiness

- Smart re-indexing is complete and functional
- The `--incremental` flag now provides better feedback via git diff detection
- No blockers for the final plan (11-06)

---
*Phase: 11-inter-procedural-dominance-hotspots-and-smart-reindexing*
*Plan: 05*
*Completed: 2026-02-03*
