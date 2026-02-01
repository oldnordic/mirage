---
phase: 01-database-foundation
plan: 02
subsystem: cli
tags: [clap, rusqlite, json-output, database-path-resolution]

# Dependency graph
requires:
  - phase: 01-01
    provides: DatabaseStatus struct, MirageDb::status() method
provides:
  - Working `mirage status` command displaying database statistics
  - Database path resolution from CLI arg, env var, or default
  - Three output formats: human, json, pretty
affects: [02-indexing, 03-query-engine]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Magellan-style database path resolution (CLI > env > default)"
    - "Output format enum driving conditional response rendering"
    - "JsonResponse wrapper with metadata for structured output"

key-files:
  created: []
  modified:
    - src/cli/mod.rs: resolve_db_path(), status command with output formats
    - src/main.rs: pass Cli reference to status command

key-decisions:
  - "Added Copy derive to StatusArgs to enable borrowing from Commands enum"
  - "Tests clear env vars to ensure isolation in parallel test execution"
  - "Human format is default, json/pretty available via --output flag"

patterns-established:
  - "CLI commands receive (args, &Cli) when they need global context"
  - "All structured output uses JsonResponse wrapper for consistency"
  - "Database errors include actionable hints (e.g., 'Run mirage index')"

# Metrics
duration: 3min
completed: 2026-02-01
---

# Phase 1 Plan 2: Status Command Summary

**Wired up status command to display database statistics with human, JSON, and pretty output formats**

## Performance

- **Duration:** 3 min
- **Started:** 2025-02-01T14:52:53Z
- **Completed:** 2025-02-01T14:55:23Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added `resolve_db_path()` utility following Magellan's CLI > env > default pattern
- Wired status command to `MirageDb::open()` and `MirageDb::status()` for real database queries
- Implemented three output formats (human, json, pretty) using JsonResponse wrapper
- Added proper error handling with actionable hints for database not found

## Task Commits

Each task was committed atomically:

1. **Task 1: Add database path resolution utility** - `80f04dc` (feat)
2. **Task 2: Wire status command to DatabaseStatus** - `817318b` (feat)
3. **Task 3: Add JSON and pretty output formats** - `39b3a4c` (feat)

**Plan metadata:** To be committed after this summary

## Files Created/Modified

- `src/cli/mod.rs` - Extended with:
  - `resolve_db_path()` function for path resolution with priority ordering
  - Updated `status()` to accept `&Cli` for database/output access
  - Human/JSON/Pretty output format handling
  - Tests for path resolution with env var isolation

- `src/main.rs` - Modified:
  - Pass `&cli` reference to `status()` command handler
  - `StatusArgs` derives `Copy` to enable borrowing

## Output Format Examples

**Human (default):**
```
Mirage Database Status:
  Schema version: 1 (Magellan: 4)
  cfg_blocks: 2
  cfg_edges: 1
  cfg_paths: 1
  cfg_dominators: 2
```

**JSON (`--output json`):**
```json
{"schema_version":"1.0.0","execution_id":"697f694c-1915876","tool":"mirage","timestamp":"2026-02-01T14:55:08.272411560+00:00","data":{"cfg_blocks":2,"cfg_edges":1,"cfg_paths":1,"cfg_dominators":2,"mirage_schema_version":1,"magellan_schema_version":4}}
```

**Pretty (`--output pretty`):**
```json
{
  "schema_version": "1.0.0",
  "execution_id": "697f694c-1915877",
  "tool": "mirage",
  "timestamp": "2026-02-01T14:55:08.274288491+00:00",
  "data": {
    "cfg_blocks": 2,
    "cfg_edges": 1,
    "cfg_paths": 1,
    "cfg_dominators": 2,
    "mirage_schema_version": 1,
    "magellan_schema_version": 4
  }
}
```

## Decisions Made

- Added `Copy` derive to `StatusArgs` to allow pattern matching without moving the value
- Tests explicitly clear `MIRAGE_DB` env var to avoid cross-test pollution
- Error messages include actionable hints ("Run 'mirage index' to create the database")

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

**Test isolation issue**
- **Issue:** Tests failing because MIRAGE_DB env var persisted between tests
- **Resolution:** Added `clear_env()` helper function called at start of each test
- **Impact:** No code changes to production logic, tests now properly isolated

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Status command fully functional with real database queries
- Database path resolution working per Magellan conventions
- All three output formats (human, json, pretty) working correctly
- Ready for Phase 1 Plan 3: Basic indexing pipeline

---
*Phase: 01-database-foundation*
*Completed: 2026-02-01*
