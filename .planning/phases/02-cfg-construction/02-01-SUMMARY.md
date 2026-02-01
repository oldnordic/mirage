---
phase: 02-cfg-construction
plan: 01
subsystem: mir-extraction
tags: [charon, ullbc, cfg, rust-mir, serde, json]

# Dependency graph
requires:
  - phase: 02-cfg-construction
    plan: 03
    provides: Core CFG data structures (BasicBlock, Terminator, EdgeType)
provides:
  - MIR extraction module with Charon ULLBC parsing
  - ULLBC to CFG conversion function with proper edge classification
affects: [02-02, 02-04, 02-05]

# Tech tracking
tech-stack:
  added: [serde (JSON parsing), anyhow (error handling)]
  patterns: [External binary spawning, Enum-based terminator classification]

key-files:
  created: [src/mir/mod.rs, src/mir/charon.rs, src/cfg/mir.rs]
  modified: [src/lib.rs, src/cfg/mod.rs, src/cfg/ast.rs]

key-decisions:
  - "Use Charon as external binary (not linked) to avoid nightly Rust requirement"
  - "ULLBC structures simplified for CFG needs - full Charon types are much larger"
  - "EdgeType classification matches MIR terminator semantics (Call/Exception for unwind)"

patterns-established:
  - "External tool integration pattern: spawn binary, capture stdout, parse JSON"
  - "Terminator-to-edge mapping: each terminator variant produces specific edge types"
  - "BlockKind inference: Entry (id=0), Exit (Return/Unreachable), Normal (others)"

# Metrics
duration: 8min
completed: 2026-02-01
---

# Phase 2 Plan 1: MIR Extraction via Charon Summary

**Charon ULLBC parsing with JSON deserialization, terminator classification, and CFG construction with proper edge type mapping**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-01T16:08:12Z
- **Completed:** 2026-02-01T16:16:15Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- MIR module (`src/mir/`) with Charon ULLBC data structures and JSON parsing
- ULLBC to CFG conversion (`ullbc_to_cfg`) with proper edge classification (Fallthrough, TrueBranch, FalseBranch, Call, Exception)
- Comprehensive test coverage for JSON parsing and CFG construction (9 tests passing)
- Fixed blocking compilation errors in AST CFG builder (unrelated but blocking)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create MIR module with Charon ULLBC data structures** - `e6aaeef` (feat)
2. **Task 2: Implement ULLBC to Cfg conversion** - `6627125` (feat)
3. **Blocking fix: AST CFG builder compilation errors** - `fffd95b` (fix)

**Plan metadata:** (pending final docs commit)

_Note: All tests passing, no TDD tasks in this plan_

## Files Created/Modified

- `src/mir/mod.rs` - MIR module entry point, re-exports Charon types
- `src/mir/charon.rs` - Charon binary spawning, ULLBC JSON parsing, data structures
- `src/cfg/mir.rs` - ULLBC to CFG conversion with edge classification
- `src/lib.rs` - Added `pub mod mir`
- `src/cfg/mod.rs` - Added `pub mod mir`, exported `ullbc_to_cfg`, added `Eq` to `Terminator`
- `src/cfg/ast.rs` - Fixed lifetime and borrow checker issues (blocking fix)

## Decisions Made

- **Charon as external binary:** Using Charon as an external process rather than linking directly avoids the nightly Rust compiler requirement and build complexity. The `run_charon()` function spawns the binary and captures JSON stdout.
- **Simplified ULLBC structures:** Full Charon types are extensive. We only parse what's needed for CFG construction: blocks, terminators, and basic statement representations.
- **BlockKind inference logic:** Entry blocks are explicitly id=0, Exit blocks identified by Return/Unreachable terminators, all others are Normal. This matches standard CFG conventions.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed AST CFG builder compilation errors**
- **Found during:** Task 1 (cargo check after creating mir module)
- **Issue:** `src/cfg/ast.rs` had multiple compilation errors preventing any work in the crate:
  - Missing lifetime annotations on helper methods
  - Cursor lifetime issue in `get_function_body`
  - Variable naming conflicts with reserved keywords (`true`, `false`)
  - Borrow checker errors in `handle_if` and `handle_loop`
  - Extra semicolon in `classify_block` causing type mismatch
- **Fix:**
  - Added `Node<'a>` lifetime annotations to all helper methods
  - Split cursor usage to avoid lifetime issues
  - Renamed `true_block`/`false_block` to `then_block`/`else_block`
  - Used `.copied()` to extract values before mutable borrows
  - Removed extra semicolon in `classify_block`
- **Files modified:** `src/cfg/ast.rs`
- **Verification:** `cargo check` passes, all 46 tests pass
- **Committed in:** `fffd95b`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** AST CFG builder errors were completely blocking compilation. Fix was necessary to proceed with MIR extraction work. No scope creep - only fixed existing broken code.

## Issues Encountered

- **sccache corruption:** Build cache occasionally returns stale results. Workaround used: `RUSTC_WRAPPER=""` env var to bypass sccache. Noted in STATE.md but not blocking.

## User Setup Required

**Charon binary installation required.** To use the MIR extraction pipeline:

1. Install Charon from https://github.com/AeneasVerif/charon
2. Ensure `charon` binary is in PATH
3. Verify: `charon --version`

The `run_charon()` function will fail with a helpful error message if Charon is not installed.

## Next Phase Readiness

- MIR extraction pipeline complete and tested
- ULLBC to CFG conversion handles all terminator variants
- Edge classification (Call/Exception) properly represents unwind paths
- Ready for plan 02-02 (AST-based CFG construction) or 02-04 (Integration testing)

**Blockers/concerns:** None. Charon external binary dependency is documented.

---
*Phase: 02-cfg-construction*
*Completed: 2026-02-01*
