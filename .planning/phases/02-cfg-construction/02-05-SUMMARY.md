---
phase: 02-cfg-construction
plan: 05
subsystem: source-tracking
tags: [source-location, charon, tree-sitter, cfg, ullbc, utf-8]

# Dependency graph
requires:
  - phase: 02-cfg-construction
    plan: 03
    provides: BasicBlock, BlockKind, Terminator, EdgeType
  - phase: 02-cfg-construction
    plan: 01
    provides: UllbcBlock, UllbcTerminator, UllbcBody
  - phase: 02-cfg-construction
    plan: 02
    provides: CFGBuilder, ast_to_cfg
provides:
  - SourceLocation type for mapping CFG blocks to source code
  - UllbcSpan integration for Charon span data
  - source_location field on BasicBlock
  - byte_to_line_column() conversion for UTF-8 text
affects: [path-enumeration, visualization]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Optional source_location on BasicBlock (None when unavailable)
    - CharonSpan adapter type for ULLBC span integration
    - UTF-8 aware byte offset to line:column conversion

key-files:
  created:
    - src/cfg/source.rs
  modified:
    - src/cfg/mod.rs
    - src/cfg/mir.rs
    - src/cfg/ast.rs
    - src/cfg/analysis.rs
    - src/mir/charon.rs
    - src/mir/mod.rs

key-decisions:
  - "source_location is Optional on BasicBlock (AST CFG doesn't have spans yet)"
  - "Charon doesn't provide byte offsets, only line:column (file_id mapping deferred)"
  - "UTF-8 multibyte characters handled correctly in byte_to_line_column"

patterns-established:
  - "SourceLocation::from_bytes() for tree-sitter ranges"
  - "SourceLocation::from_charon_span() for ULLBC spans"
  - "ullbc_to_cfg_with_file() for file path parameter"

# Metrics
duration: 8min
completed: 2026-02-01
---

# Phase 2: CFG Construction - Plan 5 Summary

**SourceLocation type with byte/line-column tracking and BasicBlock integration for MIR and AST CFG pipelines**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-01T16:22:02Z
- **Completed:** 2026-02-01T16:30:00Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- Created `SourceLocation` type with file path, byte offsets, and line/column fields
- Implemented `byte_to_line_column()` function correctly handling UTF-8 multibyte characters
- Added `source_location: Option<SourceLocation>` field to `BasicBlock`
- Extended `UllbcBlock` with optional `span` field for Charon source data
- Updated all CFG construction paths to handle source locations
- Added comprehensive tests for source location operations

## Task Commits

Each task was committed atomically:

1. **Task 1: Create SourceLocation type and conversion functions** - `c7eb1ef` (feat)
2. **Task 2: Integrate source location into BasicBlock and database** - `f8b9fa1` (feat)

**Plan metadata:** (created after summary)

## Files Created/Modified

### Created
- `src/cfg/source.rs` - SourceLocation type, byte_to_line_column conversion, CharonSpan adapter, comprehensive tests

### Modified
- `src/cfg/mod.rs` - Added source module export, source_location field to BasicBlock
- `src/cfg/mir.rs` - Updated ullbc_to_cfg to extract source locations, added ullbc_to_cfg_with_file()
- `src/cfg/ast.rs` - Added source_location field (None for now, TODO for tree-sitter ranges)
- `src/cfg/analysis.rs` - Updated all test fixtures with source_location field
- `src/mir/charon.rs` - Added UllbcSpan struct and optional span field to UllbcBlock
- `src/mir/mod.rs` - Exported UllbcSpan

## Decisions Made

1. **source_location is Optional on BasicBlock** - AST-based CFG doesn't have source spans readily available (tree-sitter has byte ranges but not line:column without conversion). Making it optional allows gradual integration.

2. **Charon doesn't provide byte offsets** - ULLBC spans only have line:column, not byte offsets. The CharonSpan adapter type uses 0 for byte_start/byte_end as placeholders. File ID to path mapping is deferred (requires ULLBC file table).

3. **UTF-8 multibyte character handling** - The byte_to_line_column function correctly counts Unicode characters (not bytes) for column numbers, ensuring proper display with international text.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

**sccache corruption issue** - Encountered the known sccache corruption issue noted in STATE.md. Worked around by using `RUSTC_WRAPPER=""` env var for all cargo commands.

**Test fixture updates** - Adding source_location field to BasicBlock required updating all test fixtures across multiple modules (analysis.rs, mir.rs). This was expected mechanical work.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Source location infrastructure is ready for path enumeration
- BasicBlock.source_location available for all CFG blocks (optional)
- byte_to_line_column conversion ready for tree-sitter range integration
- Visualization layer can use SourceLocation::display() for user-friendly output

**Blockers:** None. The AST CFG still needs tree-sitter range to SourceLocation conversion (marked as TODO in ast.rs), but this is not blocking for subsequent plans.

---
*Phase: 02-cfg-construction*
*Completed: 2026-02-01*
