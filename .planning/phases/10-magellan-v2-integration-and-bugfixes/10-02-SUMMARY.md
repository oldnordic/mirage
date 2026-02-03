---
phase: 10-magellan-v2-integration-and-bugfixes
plan: 02
subsystem: cli-and-analysis
tags: [cli, dead-code-detection, magellan-integration, unreachable-command]

# Phase 10 Plan 02: Enhanced Unreachable Command Summary

## One-Liner
Combined Magellan's inter-procedural uncalled function detection with Mirage's intra-procedural unreachable block detection for complete dead code analysis.

## Objective
Enhance the unreachable command to combine Magellan's uncalled functions detection (inter-procedural) with Mirage's unreachable block detection (intra-procedural).

## Deliverables

### Enhanced Dead Code Types

**File:** `src/analysis/mod.rs`

- **`DeadSymbolJson`**: Serializable wrapper for Magellan's `DeadSymbol`
  - Fields: `fqn`, `file_path`, `kind`, `reason`
  - Implements `From<&DeadSymbol>` for easy conversion

- **`EnhancedDeadCode`**: Combined dead code report
  - `uncalled_functions: Vec<DeadSymbolJson>` - Functions never called from entry
  - `unreachable_blocks: HashMap<String, Vec<usize>>` - Unreachable blocks within functions
  - `total_dead_count: usize` - Total count of dead code items
  - Fully serializable to JSON

### CLI Integration

**File:** `src/cli/mod.rs`

- **`--include-uncalled` flag**: New flag on `mirage unreachable` command
  - Opens Magellan database to detect uncalled functions
  - Gracefully handles missing Magellan database (warns but continues)
  - Uses "main" as default entry point

- **Enhanced `UnreachableResponse`**: Includes optional `uncalled_functions` field
  - Human output: Shows "Uncalled Functions (N)" section followed by "Unreachable Blocks (M)"
  - JSON output: Includes `uncalled_functions` array when `--include-uncalled` is set

### Testing

**File:** `src/analysis/mod.rs`

- `test_dead_symbol_json_from_dead_symbol`: Verifies conversion from `DeadSymbol` to `DeadSymbolJson`
- `test_enhanced_dead_code_serialization`: Verifies `EnhancedDeadCode` JSON serialization

**File:** `src/cli/mod.rs`

- All existing CLI tests updated to include new struct fields
- Tests verify both human and JSON output formats

## Dependency Graph

### Requires
- Phase 10-01 (MagellanBridge wrapper and DeadSymbol re-exports)

### Provides
- Complete dead code detection (inter + intra procedural)
- JSON-serializable dead code reports for LLM consumption
- CLI access to Magellan's dead_symbols() algorithm

### Affects
- Phase 10-03+ (can use enhanced dead code detection in future analysis)

## Tech Stack

### Added
- `serde::Serialize` derives on `DeadSymbolJson` and `EnhancedDeadCode`
- Re-export of `DeadSymbolJson` from `analysis` module to `cli` module

### Patterns
- **Wrapper pattern**: `DeadSymbolJson` wraps external `DeadSymbol` for serialization
- **Optional enhancement**: `uncalled_functions: Option<Vec<>>` allows gradual feature adoption
- **Graceful degradation**: Missing Magellan database warns but continues with intra-procedural analysis

## Decisions Made

1. **Entry point hard-coded to "main"**: For simplicity, the `--include-uncalled` flag defaults to "main" as the entry point. Future enhancement could add `--entry-point` flag.

2. **Graceful degradation for missing Magellan DB**: When Magellan database is unavailable, the command warns but continues with intra-procedural analysis. This allows the feature to work in Mirage-only environments.

3. **Separate DeadSymbolJson wrapper**: Magellan's `DeadSymbol` doesn't implement `Serialize`, so we created a wrapper struct rather than modifying the external type.

4. **None for uncalled_functions by default**: Tests use `uncalled_functions: None` to avoid needing Magellan database for test execution.

## Deviations from Plan

### Auto-fixed Issues

**None** - Plan executed exactly as written. All tasks completed without deviations.

## Authentication Gates

**None** - No authentication required for this plan.

## Files Modified

- `src/analysis/mod.rs`: Added `DeadSymbolJson`, `EnhancedDeadCode`, tests, doctest
- `src/cli/mod.rs`: Added `--include-uncalled` flag, updated `UnreachableResponse`, integrated `MagellanBridge::dead_symbols()`, updated all tests
- `src/main.rs`: Added `analysis` module for binary compilation

## Success Criteria

✅ `--include-uncalled` flag works on unreachable command  
✅ Magellan dead_symbols API integrated  
✅ Output format shows both inter and intra procedural dead code  
✅ Error handling for missing entry point (graceful degradation)  
✅ All tests pass (13 analysis tests + all CLI tests)  
✅ JSON serialization works for new types  

## Metrics

- **Duration**: ~45 minutes
- **Completed**: 2025-02-03
- **Commits**: 3 (feat, feat, test)

## Next Phase Readiness

✅ Ready for Phase 10-03 (Blast Zone Enhancement)  
✅ Enhanced dead code detection available for future commands  
✅ Pattern established for wrapping Magellan types for serialization  

## Verification Commands

```bash
# Test analysis module
cargo test analysis::tests

# Test CLI (requires indexed database)
cargo build
mirage unreachable --help
mirage unreachable --include-uncalled  # Requires Magellan DB
```
