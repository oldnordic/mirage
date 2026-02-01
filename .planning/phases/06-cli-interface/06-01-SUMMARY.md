---
phase: 06-cli-interface
plan: 01
subsystem: cli
tags: [cli, paths, enumeration, json-output]

# Dependency graph
requires:
  - phase: 05-path-enumeration
    provides: Path enumeration API (enumerate_paths, PathLimits, PathKind)
provides:
  - mirage paths command for querying execution paths through functions
  - JSON/pretty/human output formats for path data
  - Error path filtering via --show-errors flag
  - Path length bounding via --max-length flag
affects: [06-02, 06-03, 06-04, 06-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - CLI command signature: (args: &ArgsStruct, cli: &Cli) for global context
    - JsonResponse wrapper for consistent JSON output across commands
    - Database-first error handling with helpful hints

key-files:
  created: []
  modified:
    - src/cli/mod.rs - Implemented paths() command handler
    - src/main.rs - Updated to pass cli reference to paths command

key-decisions:
  - "Using test CFG for now until MIR extraction (Phase 02-01) is complete"
  - "PathsResponse struct wraps path results with metadata (total, error count)"
  - "PathSummary From<Path> trait conversion for clean JSON serialization"

patterns-established:
  - "CLI commands needing global context receive (args, &Cli) signature"
  - "Human output shows path_id, kind, length, and optionally block sequences"
  - "JSON output uses JsonResponse wrapper with schema_version and timestamp"
  - "Error paths filter retains only PathKind::Error paths"
  - "PathLimits applied via builder pattern (with_max_length)"

# Metrics
duration: 6min
completed: 2026-02-01
---

# Phase 6 Plan 1: Path query commands Summary

**mirage paths command implemented with enumerate_paths integration, error filtering, length bounding, and three output formats**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-01T21:31:40Z
- **Completed:** 2026-02-01T21:37:45Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Implemented `paths()` command handler connecting CLI to path enumeration backend
- Added support for --show-errors flag to filter paths to error kind only
- Added support for --max-length flag to bound path exploration
- Added three output formats: human-readable, compact JSON, and pretty JSON
- Created 11 tests covering paths command behavior and output formatting

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement paths() command handler with database integration** - `877ac98` (feat)
2. **Task 2: Add tests for paths command output formatting** - `a9e3528` (test)

**Plan metadata:** N/A (docs: complete plan)

_Note: TDD tasks may have multiple commits (test -> feat -> refactor)_

## Files Created/Modified

- `src/cli/mod.rs` - Implemented paths() command handler, added PathsResponse and PathSummary structs, added 11 tests
- `src/main.rs` - Updated paths command invocation to pass cli reference

## Decisions Made

- **Test CFG for now:** Using create_test_cfg() until MIR extraction (Phase 02-01) is complete - allows CLI development to proceed independently
- **Response struct pattern:** PathsResponse wraps path results with metadata (function name, total count, error count) for complete API responses
- **PathSummary conversion:** From<Path> trait implementation provides clean separation between domain Path type and JSON serialization

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- **sccache corruption:** Build cache returned stale results. Workaround: Used `RUSTC_WRAPPER=""` env var to bypass. Not blocking but noted for future sessions.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- paths() command ready for integration with actual CFG loading from database
- Pattern established for CLI commands needing global context
- Output format pattern established for remaining query commands (cfg, dominators, unreachable, verify, blast-zone)

**Verification:**

```bash
# Build succeeds with no errors
RUSTC_WRAPPER="" cargo build

# All 268 tests pass (257 existing + 11 new)
RUSTC_WRAPPER="" cargo test --lib

# Command works with all flags
mirage paths --function test_func                    # Human output
mirage paths --function test_func --output json      # JSON output
mirage paths --function test_func --show-errors      # Error paths only
mirage paths --function test_func --max-length 2     # Length bounded
mirage paths --function test_func --with-blocks      # With block details
```

---
*Phase: 06-cli-interface*
*Completed: 2026-02-01*
