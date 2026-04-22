# 4D Coordinate Integration - Comprehensive Fix Plan

**Created:** 2026-04-13
**Status:** 🔄 In Progress
**Goal:** Fix broken 4D coordinate integration and verify all tests pass

---

## Executive Summary

The previous 4D coordinate integration is **BROKEN** with:
- ❌ Database schema missing coordinate columns
- ❌ 15 compilation errors in test fixtures
- ❌ Tests cannot compile or run

This plan fixes all issues systematically.

---

## Phase 1: Database Schema Fixes

### ✅ Task 1: Fix `src/storage/sqlite_backend.rs`

**Location:** Lines 286-298
**Issue:** `CREATE TABLE cfg_blocks` missing `coord_x`, `coord_y`, `coord_z` columns

**Current Code:**
```sql
CREATE TABLE cfg_blocks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    function_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    terminator TEXT NOT NULL,
    byte_start INTEGER,
    byte_end INTEGER,
    start_line INTEGER,
    start_col INTEGER,
    end_line INTEGER,
    end_col INTEGER,
    FOREIGN KEY (function_id) REFERENCES graph_entities(id)
)
```

**Fix Required:**
```sql
CREATE TABLE cfg_blocks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    function_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    terminator TEXT NOT NULL,
    byte_start INTEGER,
    byte_end INTEGER,
    start_line INTEGER,
    start_col INTEGER,
    end_line INTEGER,
    end_col INTEGER,
    coord_x INTEGER DEFAULT 0,
    coord_y INTEGER DEFAULT 0,
    coord_z INTEGER DEFAULT 0,
    FOREIGN KEY (function_id) REFERENCES graph_entities(id)
)
```

**Action:** Add 3 lines before FOREIGN KEY:
```rust
coord_x INTEGER DEFAULT 0,
coord_y INTEGER DEFAULT 0,
coord_z INTEGER DEFAULT 0,
```

---

### ✅ Task 2: Fix `src/storage/mod.rs`

**Location:** Lines 3262-3274
**Issue:** Same as above - `CREATE TABLE cfg_blocks` missing coordinate columns

**Current Code:**
```sql
CREATE TABLE cfg_blocks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    function_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    terminator TEXT NOT NULL,
    byte_start INTEGER NOT NULL,
    byte_end INTEGER NOT NULL,
    start_line INTEGER NOT NULL,
    start_col INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    end_col INTEGER NOT NULL,
    FOREIGN KEY (function_id) REFERENCES graph_entities(id)
)
```

**Fix Required:** Add same 3 lines as Task 1 before FOREIGN KEY

**Note:** This uses `NOT NULL` constraints, so use `coord_x INTEGER NOT NULL DEFAULT 0`

---

## Phase 2: Test Fixture Fixes

### ✅ Task 3: Fix `src/cli/mod.rs` (4 errors)

**Error Locations:** Lines 5166, 5539, 5728, 5847

**Pattern to Find:**
```rust
let bX = g.add_node(BasicBlock {
    id: X,
    kind: BlockKind::Entry,  // or other kind
    statements: vec![...],
    terminator: Terminator::Goto { target: Y },
    source_location: None,
    // MISSING: coord_x, coord_y, coord_z
});
```

**Fix Pattern:**
```rust
let bX = g.add_node(BasicBlock {
    id: X,
    kind: BlockKind::Entry,
    statements: vec![...],
    terminator: Terminator::Goto { target: Y },
    source_location: None,
    coord_x: 0,  // ADD THIS
    coord_y: 0,  // ADD THIS
    coord_z: 0,  // ADD THIS
});
```

**Specific Lines:**
- Line 5166: Add `coord_x: 0, coord_y: 0, coord_z: 0,` after `source_location: None,`
- Line 5539: Add `coord_x: 0, coord_y: 0, coord_z: 0,` after `source_location: None,`
- Line 5728: Add `coord_x: 0, coord_y: 0, coord_z: 0,` after `source_location: None,`
- Line 5847: Add `coord_x: 0, coord_y: 0, coord_z: 0,` after `source_location: None,`

---

### ✅ Task 4: Fix `src/cfg/analysis.rs` (3 errors)

**Error Locations:** Lines 91, 203, 263

**Same Fix Pattern:** Add `coord_x: 0, coord_y: 0, coord_z: 0,` after `source_location: None,` for each

**Specific Lines:**
- Line 91: Add coordinate fields to BasicBlock
- Line 203: Add coordinate fields to BasicBlock
- Line 263: Add coordinate fields to BasicBlock

---

### ✅ Task 5: Fix `src/cfg/paths.rs` (8 errors)

**Error Locations:** Lines 1819, 1886, 2177, 3276, 3321, 3590, 4912, 5053

**Same Fix Pattern:** Add `coord_x: 0, coord_y: 0, coord_z: 0,` after `source_location: None,`

**Specific Lines:**
- Line 1819: Add coordinate fields to BasicBlock
- Line 1886: Add coordinate fields to BasicBlock
- Line 2177: Add coordinate fields to BasicBlock
- Line 3276: Add coordinate fields to BasicBlock
- Line 3321: Add coordinate fields to BasicBlock
- Line 3590: Add coordinate fields to BasicBlock
- Line 4912: Add coordinate fields to BasicBlock
- Line 5053: Add coordinate fields to BasicBlock

---

## Phase 3: Verification

### ✅ Task 6: Verify Compilation

**Command:**
```bash
cd /home/feanor/Projects/mirage
cargo build --release
```

**Expected Output:**
```
    Finished `release` profile [optimized] target(s) in X.XXs
```

**Success Criteria:** No compilation errors, 0 warnings about coord fields

---

### ✅ Task 7: Run Test Suite

**Command:**
```bash
cd /home/feanor/Projects/mirage
cargo test --lib cfg
```

**Expected Output:**
```
test result: ok. X passed in Y.YYs
```

**Success Criteria:** All CFG tests pass, no failures

---

### ✅ Task 8: Test Runtime Functionality

**Command:**
```bash
mirage --db .magellan/magellan.db --output json cfg --function "main" | jq '.data.blocks[0]'
```

**Expected Output:**
```json
{
  "id": 0,
  "kind": "ENTRY",
  "coord_x": 0,
  "coord_y": 0,
  "coord_z": 0,
  ...
}
```

**Success Criteria:** JSON output includes coord_x, coord_y, coord_z fields

---

## Summary Matrix

| File | Type | Errors | Status |
|------|------|--------|--------|
| src/storage/sqlite_backend.rs | Schema | 3 columns | ⏳ Pending |
| src/storage/mod.rs | Schema | 3 columns | ⏳ Pending |
| src/cli/mod.rs | Test fixtures | 4 literals | ⏳ Pending |
| src/cfg/analysis.rs | Test fixtures | 3 literals | ⏳ Pending |
| src/cfg/paths.rs | Test fixtures | 8 literals | ⏳ Pending |
| Compilation | Verification | Build | ⏳ Pending |
| Tests | Verification | cargo test | ⏳ Pending |
| Runtime | Verification | mirage CLI | ⏳ Pending |

**Total:** 8 tasks, 0 completed, 8 pending

---

## Execution Order

**Phase 1 (Schema):** Task 1 → Task 2
**Phase 2 (Fixtures):** Task 3 → Task 4 → Task 5
**Phase 3 (Verify):** Task 6 → Task 7 → Task 8

**Estimated Time:** 15-20 minutes

---

## Rollback Plan

If anything breaks:
```bash
cd /home/feanor/Projects/mirage
git checkout src/storage/sqlite_backend.rs
git checkout src/storage/mod.rs
git checkout src/cli/mod.rs
git checkout src/cfg/analysis.rs
git checkout src/cfg/paths.rs
```

---

## Notes

- All coordinate fields default to 0 for test fixtures
- Schema uses `DEFAULT 0` for backward compatibility
- No migration script needed - fresh database creation
- Tests use hardcoded 0 values (no coordinate calculation in tests)

---

## Next Steps

1. ✅ Execute Task 1: Fix sqlite_backend.rs schema
2. ✅ Execute Task 2: Fix storage/mod.rs schema
3. ✅ Execute Tasks 3-5: Fix all test fixtures
4. ✅ Execute Tasks 6-8: Verify everything works

**Status:** Ready to begin fixes
