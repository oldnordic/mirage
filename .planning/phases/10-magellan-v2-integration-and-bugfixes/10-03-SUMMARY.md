---
phase: 10-magellan-v2-integration-and-bugfixes
plan: 03
type: execute
wave: 2
status: partial
subsystem: inter-procedural-analysis
tags: [magellan-v2, call-graph, blast-zone, reachability]

# Phase 10 Plan 03: Enhanced Blast Zone with Call Graph Reachability

**Enhance blast-zone command to use Magellan's call graph reachability for inter-procedural impact analysis, combined with Mirage's path-based impact.**

## Summary

Partially implemented enhanced blast zone with call graph integration. The data structures and CLI flag are in place, but the blast_zone function body integration is incomplete due to file complexity and tooling issues.

## What Was Completed

### 1. Task 1: Call Graph Reachability Wrappers (COMPLETE)

**Files Modified:**
- `src/analysis/mod.rs`

**Changes:**
- Added `EnhancedBlastZone` struct combining call graph and CFG impact data
- Added `PathImpactSummary` struct for CFG-level path impact information
- Added `SymbolInfoJson` wrapper for Magellan `SymbolInfo` serialization
- Existing `MagellanBridge` already has `reachable_symbols()` and `reverse_reachable_symbols()` methods from 10-01

**Commit:** `bf548da`

### 2. Task 2: CLI Integration (PARTIAL)

**Files Modified:**
- `src/cli/mod.rs`

**Changes:**
- Added `--use-call-graph` flag to `BlastZoneArgs`
- Updated `BlockImpactResponse` struct with `forward_impact` and `backward_impact` fields
- Updated `PathImpactResponse` struct with `forward_impact` and `backward_impact` fields

**Missing:**
- `blast_zone()` function body not updated to:
  - Open `MagellanBridge` when `--use-call-graph` is set
  - Compute forward and backward reachability
  - Include call graph data in output

**Commit:** `811b372`

### 3. Task 3: Testing (NOT STARTED)

Tests were not started due to incomplete implementation.

## Deviations from Plan

### Auto-fixed Issues

None - the implementation proceeded according to plan.

### Tooling Issues Encountered

**Issue:** File editing conflicts with rust-analyzer
- **Impact:** Multiple attempts to edit `src/cli/mod.rs` were reset or corrupted
- **Root Cause:** Large file size (~4300 lines) combined with active rust-analyzer process
- **Workaround:** Killed rust-analyzer, but file kept getting modified
- **Resolution:** Partially committed what was stable, documented remaining work

## Implementation Details

### Data Structures

**SymbolInfoJson:**
```rust
#[derive(Debug, Clone, Serialize)]
pub struct SymbolInfoJson {
    pub symbol_id: Option<String>,
    pub fqn: Option<String>,
    pub file_path: String,
    pub kind: String,
}
```

**EnhancedBlastZone:**
```rust
#[derive(Debug, Clone, Serialize)]
pub struct EnhancedBlastZone {
    pub target: String,
    pub forward_reachable: Vec<SymbolInfoJson>,
    pub backward_reachable: Vec<SymbolInfoJson>,
    pub path_impact: Option<PathImpactSummary>,
}
```

### CLI Integration

**BlastZoneArgs:**
```rust
#[derive(Parser, Debug, Clone)]
pub struct BlastZoneArgs {
    // ... existing fields ...
    /// Use call graph for inter-procedural impact analysis
    #[arg(long)]
    pub use_call_graph: bool,
}
```

**Response Updates:**
```rust
struct BlockImpactResponse {
    // ... existing fields ...
    forward_impact: Option<Vec<SymbolInfoJson>>,
    backward_impact: Option<Vec<SymbolInfoJson>>,
}
```

## Remaining Work

### blast_zone Function Integration

The `blast_zone()` function at line 2185 in `src/cli/mod.rs` needs these additions:

1. **After line 2188** (imports):
   ```rust
   use crate::analysis::{MagellanBridge, SymbolInfoJson};
   ```

2. **After line 2191** (db_path resolution):
   ```rust
   let magellan_bridge = if args.use_call_graph {
       match MagellanBridge::open(&db_path) {
           Ok(bridge) => Some(bridge),
           Err(_e) => {
               // ... error handling ...
           }
       }
   } else {
       None
   };
   ```

3. **After getting function_name** (two locations):
   - Path-based: After line ~2300
   - Block-based: After line ~2385
   
   Add call graph reachability computation:
   ```rust
   let (forward_reachable, backward_reachable) = if let Some(ref bridge) = magellan_bridge {
       // ... compute reachability ...
   } else {
       (None, None)
   };
   ```

4. **Update output sections** to include:
   - Forward/backward impact lists in human output
   - forward_impact/backward_impact fields in JSON output

### Testing

Once function integration is complete, add:
1. Unit tests for call graph reachability computation
2. Integration tests for `blast-zone --use-call-graph`
3. Doctest examples

## Verification Status

| Criterion | Status | Notes |
|-----------|--------|-------|
| `mirage blast-zone --function foo --use-call-graph` works | NOT TESTED | Function integration incomplete |
| Forward reachability computed | NOT TESTED | Missing function logic |
| Backward reachability computed | NOT TESTED | Missing function logic |
| Combined with path-based impact | NOT TESTED | Missing function logic |
| Output format separates call graph from CFG | PARTIAL | Response structs updated, output formatting incomplete |

## Success Criteria Status

| Criterion | Status |
|-----------|--------|
| --use-call-graph flag works on blast-zone command | PARTIAL - flag exists, function incomplete |
| Both forward and backward reachability computed | NOT IMPLEMENTED |
| Combined with existing path-based impact | NOT IMPLEMENTED |
| Output format clearly separates call graph from CFG impact | PARTIAL - structs ready, output incomplete |

## Next Steps

1. **Complete blast_zone function integration** (Priority: HIGH)
   - Add MagellanBridge opening logic
   - Add call graph reachability computation
   - Update output formatting

2. **Add comprehensive tests** (Priority: MEDIUM)
   - Unit tests for reachability helpers
   - Integration tests for CLI command
   - Test with real Magellan database

3. **Documentation** (Priority: LOW)
   - Update CLI help text
   - Add examples to documentation

## Technical Notes

### File Size Considerations

The `src/cli/mod.rs` file is ~4300 lines, which caused editing difficulties:
- Large files make targeted edits error-prone
- rust-analyzer modifications interfere with manual edits
- Consider splitting into smaller modules for future changes

### Alternative Implementation Approach

Given the file size issues, consider:
1. Extract blast_zone logic to separate module: `src/cli/blast_zone.rs`
2. Use helper functions to reduce function complexity
3. Add integration tests in separate test file

## Dependencies

- **Requires:** Plan 10-01 (MagellanBridge integration)
- **Blocks:** None
- **Related:** Plan 10-02 (unreachable command with uncalled functions)

## Metrics

- **Duration:** ~14 minutes (partial completion)
- **Tasks Completed:** 1.5 of 3
- **Commits:** 2
- **Files Modified:** 2
- **Lines Added:** ~65
- **Lines Modified:** ~5

## Completion Status

**Status:** PARTIAL - Data structures and CLI flag in place, function integration incomplete

**Recommendation:** Continue with blast_zone function integration in follow-up task or separate session.
