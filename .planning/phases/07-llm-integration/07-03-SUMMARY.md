---
phase: 07-llm-integration
plan: 03
subsystem: error-handling
tags: [error-codes, remediation, json-responses, llm-guidance]

# Dependency graph
requires:
  - phase: 07-01
    provides: LLM-optimized JSON response structs
provides:
  - Centralized error code constants (E001-E007)
  - JsonError helper methods for common error patterns
  - CLI commands with JSON/pretty mode remediation support
affects: [llm-clients, error-handling, cli-ux]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Error code constants with E### prefix
    - Remediation hint constants (R_HINT_*)
    - JSON-aware error handling with mode detection
    - Centralized error factory methods

key-files:
  created: []
  modified:
    - src/output/mod.rs (error codes, remediation constants, JsonError helpers)
    - src/cli/mod.rs (JSON-aware error handling in all commands)

key-decisions:
  - Error codes use E### format for consistent parsing
  - Remediation hints stored as constants for maintainability
  - Block not found errors don't include remediation (less actionable)
  - All database errors suggest 'mirage index' command

patterns-established:
  - Pattern: JSON/pretty mode uses JsonError with remediation, human mode uses colored output with hints
  - Pattern: Error factory methods chain .with_remediation() for optional hints
  - Pattern: matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) for JSON mode detection

# Metrics
duration: 3min
completed: 2026-02-01
---

# Phase 7: LLM Integration - Plan 03 Summary

**Error code constants (E001-E007) with remediation hints, centralized JsonError factory methods, and JSON-aware error handling across all CLI commands**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-01T22:49:14Z
- **Completed:** 2026-02-01T22:52:45Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added 7 error code constants (E001-E007) for consistent error categorization
- Added 4 remediation hint constants with actionable command suggestions
- Implemented JsonError factory methods: database_not_found(), function_not_found(), block_not_found(), path_not_found()
- Updated all CLI commands (status, paths, cfg, dominators, unreachable, verify) to use JSON-aware error handling
- Verified remediation appears in JSON/pretty output while human mode remains readable

## Task Commits

Each task was committed atomically:

1. **Task 1: Add error code constants to output module** - `451356a` (feat)
2. **Task 2: Update CLI commands to use JsonError helpers** - `14afaaa` (feat)
3. **Task 3: Verify remediation in JSON output** - (no commit - verification only)

**Plan metadata:** (not yet committed)

## Files Created/Modified

- `src/output/mod.rs` - Added E001-E007 error codes, R_HINT_* constants, JsonError factory methods
- `src/cli/mod.rs` - Updated 6 CLI commands to use JSON-aware error handling with remediation

## Decisions Made

- Error code format uses "E###" prefix for machine parsing (e.g., E001, E002, E003)
- Remediation hints are stored as constants for consistency and easy updates
- Block not found errors don't include remediation (less actionable - block IDs are internal)
- Database errors consistently suggest running 'mirage index' command
- Human mode retains colored output with separate info() calls for hints

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - all tasks completed without issues.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Error response format is LLM-ready with structured error codes and remediation
- Next phase (07-04) can build on these structured error responses
- All CLI commands now provide actionable guidance when commands fail
- LLMs can parse error categories and suggest fixes to users based on remediation field

---
*Phase: 07-llm-integration*
*Completed: 2026-02-01*
