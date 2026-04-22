# Mirage - Comprehensive Fix Plan

**Created:** 2026-04-13
**Status:** Ready for Implementation
**Goal:** Fix all bugs, remove all deprecated usage, ensure all features work properly
**Principle:** NO placeholders, NO stubs, NO deletions - only proper implementations

---

## Current State Assessment

### ✅ What Works:
- Database schema: coord_x, coord_y, coord_z, coord_t columns exist
- Test suite: 398 passing + 236 CFG tests passing
- JSON export: 4D coordinates appear correctly
- Core commands: cfg, paths, dominators, loops, patterns, frontiers, unreachable, blast-zone, cycles

### ❌ What Needs Fixing:
- **7 warnings**: Using deprecated `storage::DatabaseStatus::cfg_edges` field
- **Missing implementations**: Some features may have placeholders
- **Test reliability**: Ensure tests actually test functionality, not just compile

---

## Phase 1: Fix Deprecated Field Usage

### File 1: `src/cfg/hotpaths.rs`

**Issue:** Using deprecated `status.cfg_edges` field

**Locations:**
- Line 25: `use of deprecated field 'storage::DatabaseStatus::cfg_edges'`
- Line 28: Same issue
- Line 4736: Test assertion `assert_eq!(status.cfg_edges, 1, ...)`
- Line 4752: Test assertion `assert!(status.cfg_edges >= 0, ...)`
- Line 4877: Test assertion `assert_eq!(status.cfg_edges, 0, ...)`

**Root Cause:** `cfg_edges` field removed from `DatabaseStatus` because edges are now computed in memory

**Proper Fix:**
```rust
// Remove cfg_edges assertions and replace with proper edge counting:
// Option 1: Count edges from graph_edges table
// Option 2: Use computed edge count from in-memory graph
// Option 3: Remove edge count assertions if they're testing deprecated functionality

// RECOMMENDED: Count edges from database
fn count_cfg_edges(db_path: &Path) -> Result<i64> {
    let conn = Connection::open(db_path)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM graph_edges WHERE edge_type = 'cfg'",
        []
    )?;
    Ok(count)
}
```

**Implementation:**
1. Remove `cfg_edges` field usage from line 25, 28
2. Update test assertions to use actual edge counting
3. Add helper function to count edges from database

---

### File 2: `src/storage/mod.rs`

**Issue:** Line 1190 - Using deprecated `cfg_edges` field in test

**Location:** Line 1190 in test code

**Current Code:**
```rust
cfg_edges: 0,  // This is deprecated - edges are now computed in memory
```

**Proper Fix:** Remove the field entirely or implement proper edge counting

---

## Phase 2: Verify All Commands Work

### Command Testing Checklist

```bash
# Test each command with real functions:
✅ mirage status --db .magellan/magellan.db
✅ mirage cfg --function "main"
✅ mirage paths --function "main"
✅ mirage dominators --function "main"
✅ mirage frontiers --function "main"
✅ mirage loops --function "main"
✅ mirage unreachable --within-functions
✅ mirage patterns --function "main"
✅ mirage blast-zone --function "main" --block-id 0
✅ mirage cycles
✅ mirage verify --path "src/main.rs"
✅ mirage slice --function "main" --backward
✅ mirage hotspots --db .magellan/magellan.db
✅ mirage hotpaths --db .magellan/magellan.db
✅ mirage diff --db .magellan/magellan.db
✅ mirage icfg --function "main"
✅ mirage migrate --db .magellan/magellan.db
```

**Action:** Test each command, document results, fix any failures

---

## Phase 3: Remove All Placeholders/TODOs/FIXMEs

### Search Pattern:

```bash
# Find all placeholders:
grep -rn "TODO\|FIXME\|placeholder\|stub\|mock\|for now" src/ --include="*.rs"

# Find any unimplemented!() usage:
grep -rn "unimplemented!\(\)" src/ --include="*.rs"

# Find any todo!() usage:
grep -rn "todo!(\)" src/ --include="*.rs"
```

**Files to check:**
- All files in `src/cfg/`
- All files in `src/storage/`
- All files in `src/cli/`
- All files in `src/mir/`

**Requirement:** Replace with proper implementations or remove entirely

---

## Phase 4: Enhance Test Coverage

### Current Test State:
- **398 tests passing** (good!)
- **236 CFG tests passing** (good!)
- **7 warnings** (needs fixing)

### Test Quality Assessment:

**Need to verify:**
1. Tests actually test functionality (not just compilation)
2. Edge cases are covered
3. Error handling is tested
4. Integration tests work end-to-end

**Action:** Run each test category and verify:
```bash
cargo test --lib cfg                    # CFG tests
cargo test --lib storage               # Storage tests
cargo test --lib cli                    # CLI tests
cargo test --lib mirage                # MIR tests
```

---

## Phase 5: Database Schema Verification

### Current Schema Check:
```sql
-- Verify coord columns exist:
PRAGMA table_info(cfg_blocks);

-- Expected result:
coord_x|INTEGER|0|0|0
coord_y|INTEGER|0|0|0
coord_z|INTEGER|0|0|0
coord_t|TEXT|0|NULL|0   -- BONUS: Extra coordinate type!
```

**Issue Found:** `coord_t` column exists but is not being used!

**Question:** Should we:
1. Use `coord_t` for coordinate type metadata?
2. Remove it if it's not needed?
3. Document what it's for?

---

## Phase 6: Missing Feature Implementation

### Feature Gap Analysis:

**Check each documented feature:**

1. **`verify` command** - Does it actually verify paths?
   - Test: `mirage verify --path "src/main.rs"`
   - Expected: Check if path is still valid
   - Actual: Needs investigation

2. **`slice` command** - Does program slicing work?
   - Test: `mirage slice --function "main" --backward`
   - Expected: Show backward slice
   - Actual: Needs testing

3. **`hotspots` command** - High-risk function detection
   - Test: `mirage hotspots --db .magellan/magellan.db`
   - Expected: List high-risk functions
   - Actual: Needs testing

4. **`hotpaths` command** - Most-traversed paths
   - Test: `mirage hotpaths --db .magellan/magellan.db`
   - Expected: Show hot paths
   - Actual: Needs testing

5. **`diff` command** - CFG snapshot comparison
   - Test: `mirage diff --db .magellan/magellan.db`
   - Expected: Compare CFG snapshots
   - Actual: Needs testing

6. **`icfg` command** - Inter-procedural CFG
   - Test: `mirage icfg --function "main"`
   - Expected: Show inter-procedural CFG
   - Actual: Needs testing

---

## Phase 7: File-by-File Implementation Plan

### File 1: `src/cfg/hotpaths.rs` (Priority: HIGH)

**Issues:**
- 7 warnings about deprecated `cfg_edges` field
- Lines 25, 28, 4736, 4752, 4877

**Required Changes:**
1. Remove `cfg_edges` field from DatabaseStatus usage
2. Implement proper edge counting function
3. Update test assertions to use actual edge counts

**No Stubs Allowed:** Must implement actual edge counting from database

---

### File 2: `src/storage/mod.rs` (Priority: HIGH)

**Issue:**
- Line 1190: Test using deprecated `cfg_edges` field

**Required Changes:**
1. Remove `cfg_edges: 0` from test
2. If testing edge count, use proper counting function

---

### File 3: `src/cli/mod.rs` (Priority: MEDIUM)

**Investigation Needed:**
- Are all commands implemented?
- Do any commands have placeholder implementations?

**Action:**
1. Read through command handlers
2. Document any placeholders found
3. Replace with proper implementations

---

### File 4: `src/mir/translator.rs` (Priority: MEDIUM)

**Investigation Needed:**
- Are there MIR translation issues?
- Are all coordinate types properly handled?

**Action:**
1. Review MIR translation code
2. Verify coord_x, coord_y, coord_z are properly populated
3. Verify coord_t is handled if used

---

### File 5: Test Files (Priority: HIGH)

**Files to Review:**
- `src/cfg/hotpaths.rs` (tests section)
- `src/storage/sqlite_backend.rs` (tests)
- `src/storage/paths.rs` (tests)
- `src/cli/mod.rs` (tests)

**Required:**
- Remove all deprecated field usage
- Ensure tests test actual functionality
- Add integration tests for missing features

---

## Phase 8: Integration Testing

### End-to-End Test Scenarios:

1. **Scenario 1: Complex Function Analysis**
   ```bash
   # Find a complex function
   mirage hotspots --db .magellan/magellan.db
   
   # Analyze its CFG
   mirage --output json cfg --function "complex_func"
   
   # Show all execution paths
   mirage paths --function "complex_func"
   
   # Verify paths are valid
   mirage verify --function "complex_func" --path 1
   ```

2. **Scenario 2: Impact Analysis**
   ```bash
   # Check what would break if we modify a block
   mirage blast-zone --function "target" --block-id 5
   
   # Show backward slice
   mirage slice --function "target" --backward
   
   # Show forward slice
   mirage slice --function "target" --forward
   ```

3. **Scenario 3: Dead Code Detection**
   ```bash
   # Find unreachable code
   mirage unreachable --within-functions
   
   # Verify it's actually unreachable
   # (Manually verify that reported blocks are indeed unreachable)
   ```

---

## Phase 9: Documentation Alignment

### Files to Update:
1. **`CLAUDE.md`** - Ensure documentation matches reality
2. **`README.md`** - Update feature list
3. **Help text** - Verify `--help` output is accurate

### Requirement:
- No overclaiming features
- Document actual behavior
- Include known limitations
- Remove outdated information

---

## Phase 10: Final Verification

### Compilation Check:
```bash
# Clean build
cargo clean

# Release build
cargo build --release

# Verify 0 errors
cargo build --release 2>&1 | grep -c "error\["
# Should return: 0
```

### Test Check:
```bash
# Full test suite
cargo test --lib

# Verify:
# - 0 failed (not 0 ignored)
# - 0 errors
# - Minimal warnings
```

### Feature Check:
```bash
# Test ALL documented commands work
for cmd in status cfg paths dominators loops unreachable patterns frontiers verify blast-zone cycles slice hotspots hotpaths diff icfg migrate; do
    echo "Testing: $cmd"
    mirage --db .magellan/magellan.db $cmd --help > /dev/null
done
```

---

## Implementation Order

### Week 1: Critical Fixes
1. ✅ **Day 1-2**: Fix deprecated `cfg_edges` usage (File 1 & 2)
2. **Day 3-4**: Test all commands, document failures
3. **Day 5**: Fix any command failures found

### Week 2: Feature Completeness
4. **Day 1-2**: Implement missing features in commands
5. **Day 3-4**: Remove all placeholders/TODOs
6. **Day 5**: Integration testing

### Week 3: Quality Assurance
7. **Day 1-2**: Fix test issues
8. **Day 3**: Update documentation
9. **Day 4-5**: Final verification and polish

---

## Success Criteria

### Must Have (Non-Negotiable):
- ✅ 0 compilation errors
- ✅ 0 test failures (0 ignored allowed)
- ✅ 0 placeholders/TODOs/stubs in production code
- ✅ All documented features work as advertised
- ✅ 0 deprecated field usage

### Should Have:
- ✅ Warning count < 10
- ✅ All commands have working examples
- ✅ Integration tests pass

### Nice to Have:
- ✅ Performance benchmarks
- ✅ Error messages are helpful
- ✅ Documentation is comprehensive

---

## Next Actions

1. ✅ **START NOW**: Fix deprecated `cfg_edges` usage in `src/cfg/hotpaths.rs`
2. **INVESTIGATE**: Test all 18 commands, document which ones work
3. **IMPLEMENT**: Fix any broken commands found
4. **CLEANUP**: Remove all placeholders/TODOs
5. **VERIFY**: Full integration test suite

**Estimated Timeline:** 3 weeks for proper implementation

---

## Immediate Task (Right Now):

**File:** `src/cfg/hotpaths.rs`

**Specific Issue:** Lines 4736, 4752, 4877 using deprecated `cfg_edges`

**Required Fix:**
```rust
// BEFORE (WRONG - deprecated):
assert_eq!(status.cfg_edges, 1, "Should have 1 cfg_edge");

// AFTER (CORRECT - count from database):
let edge_count = count_graph_edges(&status.db_path)?;
assert_eq!(edge_count, 1, "Should have 1 cfg_edge");
```

**No shortcuts:** Must implement proper edge counting, not just remove tests!

---

**Status:** Ready to begin - No placeholders, only proper implementations allowed
**Next Action:** Fix File 1 (`src/cfg/hotpaths.rs`) - deprecated field usage
