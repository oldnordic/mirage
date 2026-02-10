---
phase: 14-native-v2-io-integration
plan: 02
subsystem: [database, backend-abstraction]
tags: [native-v2, sqlite, kv-store, magellan, cfg, backend-abstraction]

# Dependency graph
requires:
  - phase: 14-01
    provides: GraphBackend integration with MirageDb wrapper
provides:
  - Feature-gated cfg_blocks loading for both SQLite and native-v2 backends
  - Backend-agnostic load_cfg_from_db() and resolve_function_name() APIs
affects: [14-03, cli-commands, cfg-analysis]

# Tech tracking
tech-stack:
  added: [magellan::graph::get_cfg_blocks_kv, native-v2 kv operations]
  patterns: [feature-gated backend implementations, backend-agnostic API wrappers, shared helper functions]

key-files:
  created: []
  modified:
    - src/storage/mod.rs - Feature-gated load_cfg_from_db and resolve_function_name
    - src/cfg/mod.rs - Re-exports of backend-agnostic and legacy functions
    - src/cli/mod.rs - Updated to use new backend-agnostic API

key-decisions:
  - "Created shared load_cfg_from_rows() helper to avoid code duplication between backends"
  - "Added *_with_conn legacy functions for backward compatibility with existing tests"
  - "Feature-gated test module since all storage tests use SQLite-specific APIs"

patterns-established:
  - "Pattern: Feature-gated helper functions with shared core logic"
  - "Pattern: Backend-agnostic public API delegating to feature-specific implementations"
  - "Pattern: Legacy *_with_conn variants for direct Connection access"

# Metrics
duration: 15min
completed: 2026-02-10
---

# Phase 14: Native V2 I/O Integration - Plan 02 Summary

**Feature-gated cfg_blocks loading using Magellan's KV get_cfg_blocks_kv() for native-v2 backend and SQL queries for SQLite backend**

## Performance

- **Duration:** 15 minutes
- **Started:** 2026-02-10T12:20:00Z
- **Completed:** 2026-02-10T12:34:00Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments

- Implemented `load_cfg_from_sqlite()` helper using SQL query on cfg_blocks table
- Implemented `load_cfg_from_native_v2()` helper using Magellan's `get_cfg_blocks_kv()`
- Created shared `load_cfg_from_rows()` function to avoid code duplication
- Added backend-agnostic `load_cfg_from_db(&MirageDb, function_id)` public API
- Added backend-agnostic `resolve_function_name(&MirageDb, name_or_id)` public API
- Added legacy `*_with_conn` variants for backward compatibility
- Updated all CLI calls to use new backend-agnostic API
- Feature-gated test module since all storage tests use SQLite

## Task Commits

1. **Task 1: Implement feature-gated cfg_blocks loading** - `001d51e` (feat)

**Plan metadata:** N/A (single task commit)

## Files Created/Modified

- `src/storage/mod.rs` - Feature-gated backend implementations for cfg_blocks loading
- `src/cfg/mod.rs` - Re-exports of backend-agnostic and legacy functions
- `src/cli/mod.rs` - Updated to use new backend-agnostic API

## Mapping Details (Magellan CfgBlock to Mirage BasicBlock)

The conversion from Magellan's CfgBlock to Mirage's BasicBlock uses identical mapping to existing SQL code:

| Magellan CfgBlock field | Mirage BasicBlock field | Notes |
|-------------------------|------------------------|-------|
| `kind: String` | `BlockKind` (enum) | entry/return/normal/etc. |
| `terminator: String` | `Terminator` (enum) | fallthrough/conditional/goto/return/etc. |
| `byte_start: u64` | `source_location.byte_start` | Cast to usize |
| `byte_end: u64` | `source_location.byte_end` | Cast to usize |
| `start_line: u64` | `source_location.start_line` | Cast to usize |
| `start_col: u64` | `source_location.start_column` | Cast to usize |
| `end_line: u64` | `source_location.end_line` | Cast to usize |
| `end_col: u64` | `source_location.end_column` | Cast to usize |

## Decisions Made

1. **Shared load_cfg_from_rows() helper** - Both backends use identical block parsing and edge construction logic. Created shared function to avoid duplication.
2. **Legacy *_with_conn variants** - Existing tests and some CLI code need direct Connection access. Created legacy variants for backward compatibility.
3. **Feature-gated test module** - All storage tests use SQLite-specific APIs. Added `#[cfg(all(test, feature = "sqlite"))]` to test module.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

1. **Doctest import errors** - Doctests used `mirage::` crate name which doesn't exist. Fixed by changing to `mirage_analyzer::`.
2. **Test failures with native-v2** - CLI tests for status and verify commands use SQLite-specific queries. These tests need to be feature-gated in future plans.
3. **Hotspots command SQLite fallback** - The intra-procedural fallback uses direct SQL queries. Added `#[cfg(feature = "sqlite")]` gate to this code path.

## Auth Gates

None encountered - no authentication required for this task.

## Next Steps

Plan 14-03 will implement native-v2 schema validation via GraphBackend metadata queries, which is currently a placeholder in the `status()` method.

---
*Phase: 14-native-v2-io-integration*
*Plan: 02*
*Completed: 2026-02-10*
