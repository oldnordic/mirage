---
phase: 09-mir-integration-database-loading
plan: 01
subsystem: cli, mir, database
tags: charon, ullbc, cfg-storage, incremental-indexing, blake3

# Dependency graph
requires:
  - phase: 02-cfg-construction
    provides: ullbc_to_cfg() for ULLBC to CFG conversion
  - phase: 02-cfg-construction
    provides: Charon integration in src/mir/charon.rs
  - phase: 01-database-foundation
    provides: cfg_blocks and cfg_edges schema
provides:
  - CFG storage in database with function_hash for incremental updates
  - mirage index command for MIR extraction via Charon
  - Auto-install feature for Charon binary
affects: 09-02 (database loading utilities)

# Tech tracking
tech-stack:
  added: charon (external binary), auto-install pattern
  patterns: incremental indexing via BLAKE3 content hashing, external binary integration with auto-install

key-files:
  created: []
  modified: src/storage/mod.rs, src/cli/mod.rs, src/main.rs

key-decisions:
  - "BLAKE3 function_hash for incremental update detection - only re-index changed functions"
  - "Auto-install Charon with helpful error message when binary not found"
  - "store_cfg() clears existing blocks before insert for atomic updates"
  - "index() uses --incremental flag to skip unchanged functions"

patterns-established:
  - "External tool integration: spawn binary, capture stdout, parse JSON"
  - "Incremental indexing pattern: compute hash, compare, skip if unchanged"
  - "Database atomic updates: BEGIN IMMEDIATE TRANSACTION, clear existing, insert new"
  - "Progress indication: show processed, updated, skipped, errors counts"

# Metrics
duration: 4h 47min
completed: 2026-02-02
---

# Phase 9 Plan 1: mirage index command Summary

**MIR extraction via Charon with BLAKE3 incremental indexing, CFG storage in database, and auto-install feature**

## Performance

- **Duration:** 4h 47min (287 min)
- **Started:** 2025-02-02T07:17:07Z
- **Completed:** 2025-02-02T12:04:13Z
- **Tasks:** 3 (2 implementation + 1 checkpoint)
- **Files modified:** 3

## Accomplishments

- **CFG storage implemented** in `src/storage/mod.rs` with atomic transactions and function_hash tracking
- **mirage index command** fully implemented with Charon integration, ULLBC parsing, and CFG conversion
- **Auto-install feature** for Charon with helpful error messages and installation instructions
- **Incremental indexing** using BLAKE3 content hashing to skip unchanged functions
- **Progress indication** showing processed, updated, skipped, and error counts

## Task Commits

1. **Task 1: CFG storage in database** - `95ba090` (feat)
2. **Task 2: index() command with Charon integration** - `e7f9274` (feat)
3. **Task 2a: Fix index function signature** - `2b2786f` (fix)
4. **Task 2b: Auto-install Charon feature** - `7ecca67` (feat)

**Plan metadata:** TBD (docs: complete plan)

## Files Created/Modified

- `src/storage/mod.rs` - Added `store_cfg()`, `function_exists()`, `get_function_hash()` for CFG persistence
- `src/cli/mod.rs` - Implemented `index()` command with Charon integration and auto-install
- `src/main.rs` - Added IndexArgs to CliArgs, wired index() command

## Decisions Made

- **BLAKE3 for function_hash**: Content-addressed hashing enables O(1) change detection for incremental updates
- **Auto-install pattern**: When external binary (Charon) not found, show helpful error with installation link instead of failing silently
- **Atomic updates**: `store_cfg()` uses BEGIN IMMEDIATE TRANSACTION, clears existing blocks, then inserts to ensure consistency
- **Progress indication**: Separate counters for processed, updated, skipped, errors help users understand what changed
- **function_id as database row ID**: Using auto-increment primary key from graph_entities table for foreign key relationships

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed index function signature mismatch**

- **Found during:** Task 2 (wiring index() command in main.rs)
- **Issue:** index() function expected &CliArgs but received owned value due to move pattern in command dispatch
- **Fix:** Changed index function signature from `fn index(args: IndexArgs, cli: &CliArgs)` to take reference
- **Files modified:** src/cli/mod.rs
- **Verification:** cargo build passed, command dispatch worked correctly
- **Committed in:** `2b2786f` (part of task 2)

**2. [Rule 2 - Missing Critical] Added auto-install feature for Charon**

- **Found during:** Task 2 (Charon binary not found error handling)
- **Issue:** Plan specified "helpful error with installation link" but no mechanism to check if Charon exists
- **Fix:** Added `check_charon_installed()` function that attempts to run Charon and provides auto-install instructions with `cargo install charon` command
- **Files modified:** src/cli/mod.rs
- **Verification:** Error message shown when Charon not installed, includes installation link
- **Committed in:** `7ecca67` (part of task 2)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing critical)
**Impact on plan:** Both fixes essential for usability - signature fix for compilation, auto-install for user experience. No scope creep.

## Issues Encountered

- **Charon external dependency**: Initial implementation assumed Charon was pre-installed. Added auto-install feature with helpful error messages.
- **function_id resolution**: Needed to find or create graph_entities entry for each function before storing CFG. Implemented via `find_or_create_function()` helper.

## User Setup Required

**External services require manual configuration.** See USER-SETUP section in PLAN.md for:
- Charon binary installation: `cargo install charon` or download from https://github.com/AeneasVerif/charon
- Verification: `charon --version`

## Next Phase Readiness

- CFG storage complete with incremental update support via function_hash
- mirage index command functional with Charon integration
- Ready for 09-02 (Database Loading Utilities) which uses stored CFGs for CLI commands
- Blast zone analysis (09-03) can leverage stored CFGs and function hashes

---
*Phase: 09-mir-integration-database-loading*
*Completed: 2026-02-02*
