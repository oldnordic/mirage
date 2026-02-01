---
phase: 07-llm-integration
plan: 04
subsystem: llm-integration
tags: [natural-language, path-summary, template-based, cfg-visualization]

# Dependency graph
requires:
  - phase: 07-02
    provides: PathSummary struct with summary and source_range fields
provides:
  - Natural language summaries for CFG paths via summarize_path()
  - Block-level descriptions via describe_block()
  - Function-level overviews via summarize_cfg()
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Template-based NL generation (no external LLM dependency)
    - Path truncation for readability (>5 blocks shows "...")
    - Terminator-to-text mapping for human-readable block descriptions

key-files:
  created:
    - src/cfg/summary.rs
  modified:
    - src/cfg/mod.rs
    - src/cli/mod.rs

key-decisions:
  - Template-based generation chosen over external LLM API calls (zero dependency, always works)
  - Path truncation at 5 blocks to keep summaries concise
  - Terminator descriptions use simple text format (e.g., "goto b1", "if b2|b3")
  - describe_block made public for external testing/tools

patterns-established:
  - Summary pattern: template-based string generation with context-aware formatting
  - Block description: kind + terminator combination for concise labels

# Metrics
duration: 5min
completed: 2026-02-01
---

# Phase 7 Plan 4: Control Flow Natural Language Summaries Summary

**Template-based natural language generation for CFG paths using summarize_path() with terminator-to-text mapping and path truncation**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-01T22:55:04Z
- **Completed:** 2026-02-01T23:00:00Z
- **Tasks:** 4
- **Files modified:** 3

## Accomplishments

- Created `src/cfg/summary.rs` module with `summarize_path()`, `describe_block()`, and `summarize_cfg()` functions
- Integrated natural language summaries into `PathSummary.summary` field (now populated instead of None)
- Template-based generation produces human-readable descriptions like "entry(goto b1) -> b1(if b3|b2) -> exit(return) (3 blocks)"
- Path truncation for long paths (>5 blocks shows "..." with total block count)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create summary module with path description functions** - `b1b68b8` (feat)
2. **Task 2: Export summary functions from cfg module** - `1cab6f0` (feat)
3. **Task 3: Integrate summary into PathSummary** - `2e2f609` (feat)
4. **Task 4: Verify summary in JSON output (with compilation fixes)** - `a9fd195` (fix)

**Plan metadata:** N/A (docs to be committed separately)

## Files Created/Modified

- `src/cfg/summary.rs` - New module with natural language generation functions (273 lines)
  - `summarize_path()` - Generate path descriptions like "entry -> goto b1 -> return (3 blocks)"
  - `describe_block()` - Map blocks to readable text based on kind and terminator
  - `summarize_cfg()` - Function-level overview with block/exit/loop counts
- `src/cfg/mod.rs` - Added `pub mod summary` and exports
- `src/cli/mod.rs` - Updated `PathSummary::from_with_cfg()` to call `summarize_path()`

## Decisions Made

- Template-based generation chosen over external LLM API calls (zero dependency, always works)
- Path truncation at 5 blocks to keep summaries concise for LLM consumption
- Terminator descriptions use simple text format instead of Debug output
- `describe_block()` made public (exported) for potential external tool use

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed compilation errors in summary module**
- **Found during:** Task 4 (Verification)
- **Issue:** Multiple compilation errors:
  - `describe_block` was private but needed to be public (per plan exports)
  - `find_exits()` returns `Vec<NodeIndex>` not `Iterator` (used `.count()` instead of `.len()`)
  - `Terminator::Call` has `unwind` field that wasn't handled in pattern match
  - Tests referenced undefined variables `b1`, `b2`
- **Fix:**
  - Made `describe_block` public
  - Changed `exits.count()` to `exits.len()`
  - Updated pattern to `Terminator::Call { target, unwind: _ }`
  - Added variable bindings `b1`, `b2` in test fixture setup
- **Files modified:** `src/cfg/summary.rs`
- **Verification:** All 10 summary tests pass, JSON output includes summary field
- **Committed in:** `a9fd195`

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Fix necessary for code to compile. No scope creep.

## Issues Encountered

- sccache corruption issue (known from STATE.md) - bypassed with `RUSTC_WRAPPER=""` env var
- Pre-existing test failures in other modules (patterns, post_dominators, reachability) - unrelated to this plan

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- PathSummary.summary field is now populated with natural language descriptions
- LLMs can consume JSON output and understand path semantics without parsing raw block sequences
- Template-based generation ensures summaries are always available (no external API calls)
- Ready for final LLM integration plan (07-05) if needed, or phase completion

## Verification

```bash
# JSON output includes summary field
./target/debug/mirage paths --function test --output json | jq '.data.paths[0].summary'
# Output: "entry(goto b1) -> b1(if b3|b2) -> exit(return) (3 blocks)"

# All summary tests pass
cargo test summary --lib
# 10 passed

# CLI tests pass
cargo test --lib cli
# 76 passed
```

---
*Phase: 07-llm-integration*
*Completed: 2026-02-01*
