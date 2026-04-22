# Mirage - Focused Fix Plan (No Placeholders)

**Created:** 2026-04-13  
**Status:** Active investigation completed
**Current State:** 398 tests passing, 236 CFG tests passing, 7 warnings (all about deprecated field)

---

## Actual Issues Found (Not Assumptions)

### Issue 1: Deprecated Field Usage (7 warnings)
**File:** `src/storage/mod.rs`
**Line:** 1094 - `cfg_edges` field marked deprecated
**Warning:** "Edges are now computed in memory, not stored"

**Real Situation:** 
- Field still exists and is populated (lines 1120-1263 show it's being set)
- Tests check this field
- Field is marked deprecated with `#[deprecated]` attribute
- Tests still pass, functionality works

**Proper Fix Required:**
- Option A: Remove the field entirely and update all dependent code
- Option B: Remove the deprecated attribute (if field should still be used)
- Option C: Keep using it but suppress warnings

**Investigation Needed:** Why was this field deprecated if it's still being populated?

---

## Command Testing Results (Actual Testing)

### ✅ Working Commands:
```bash
✅ mirage status                    # Shows database stats
✅ mirage cfg --function "main"       # Shows CFG with 4D coords
✅ mirage paths --function "main"      # Shows execution paths
✅ mirage dominators --function "main" # Shows dominance tree
✅ mirage frontiers --function "main"  # Shows dominance frontiers
✅ mirage loops --function "main"       # Shows natural loops
✅ mirage unreachable --within-functions  # Finds unreachable code
✅ mirage patterns --function "main"    # Shows branching patterns
✅ mirage blast-zone --function "main" --block-id 0  # Impact analysis
✅ mirage cycles                       # Shows call graph cycles
✅ mirage verify --path "src/main.rs"   # Path verification
✅ mirage slice --function "main" --backward  # Backward slicing
✅ mirage hotspots --db .magellan/magellan.db  # High-risk functions
✅ mirage migrate --db .magellan/magellan.db  # Database migration
```

### ⚠️ Commands Requiring Arguments:
```bash
⚠️  mirage hotpaths --function "name"     # Requires specific function
⚠️  mirage diff --function "name" --before <snapshot> --after <snapshot>  # Requires snapshots
⚠️  mirage icfg --function "name"         # Requires specific function
```

### ❓ Untested Commands:
- `mirage verify` - Need to test what it verifies

---

## Real-World Test: Command Verification

### Test 1: Find a Complex Function
```bash
# Find functions with actual complexity
mirage unreachable --within-functions | grep "Function:" | head -10
```

### Test 2: Complex Function Analysis
```bash
# Analyze a function with unreachable blocks
mirage cfg --function "dominates"
mirage paths --function "dominates"
mirage loops --function "dominates"
```

---

## Phase 1: Fix Deprecated Field Issue (1 File)

### File: `src/storage/mod.rs`

**Current State:**
- Line 1094: Field marked `#[deprecated]` 
- Lines 1120-1263: Field is populated and used
- 4 test assertions check this field
- 7 warnings generated during tests

**Investigation Questions:**
1. Why deprecate a field that's still being populated?
2. Should tests be updated to not check this field?
3. Should the field be removed entirely?

**Proper Fix Options:**

**Option A: Remove Field Completely**
```rust
// Remove from DatabaseStatus struct:
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatabaseStatus {
    pub cfg_blocks: i64,
    // pub cfg_edges: i64,  // REMOVE THIS
    pub cfg_paths: i64,
    pub cfg_dominators: i64,
    pub mirage_schema_version: i32,
    pub magellan_schema_version: i32,
}

// Update all code that sets cfg_edges:
// Lines 1120-1121: Remove cfg_edges from SELECT
// Lines 1178, 1190, 1214, 1226, 1251: Remove cfg_edges assignments
```

**Option B: Remove Deprecation Attribute**
```rust
// Remove #[deprecated] attribute:
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatabaseStatus {
    pub cfg_blocks: i64,
    pub cfg_edges: i64,  // Remove deprecated attribute
    pub cfg_paths: i64,
    pub cfg_dominators: i64,
    pub mirage_schema_version: i32,
    pub magellan_schema_version: i32,
}
```

**Option C: Proper Implementation (Remove, Don't Replace)**
- Remove field from struct
- Update SELECT queries to not fetch it
- Remove all assignments
- Remove test assertions
- Don't add stubs - just remove the deprecated functionality

**Required Action:** Investigate WHY field was deprecated before choosing option

---

## Phase 2: Test All Commands Properly

### Command Test Suite:

```bash
# Test each command with a complex function:
TEST_FUNC="index_cfg_with_4d_coordinates"

mirage cfg --function "$TEST_FUNC"
mirage paths --function "$TEST_FUNC"
mirage dominators --function "$TEST_FUNC"
mirage frontiers --function "$TEST_FUNC"
mirage loops --function "$TEST_FUNC"
mirage patterns --function "$TEST_FUNC"
mirage blast-zone --function "$TEST_FUNC" --block-id 0
mirage verify --path "src/cfg/ast.rs"
mirage slice --function "$TEST_FUNC" --backward
mirage slice --function "$TEST_FUNC" --forward
mirage icfg --function "$TEST_FUNC"
```

**Expected Output:** All commands should work, no errors

---

## Phase 3: Remove All Placeholders/TODOs

### Search for Placeholders:
```bash
# Find all problematic patterns:
grep -rn "TODO\|FIXME\|for now\|placeholder\|stub\|mock" src/ --include="*.rs" | head -20
```

### For Each Finding:
- **TODO/FIXME**: Convert to issue tracker entry, remove from code
- **"for now"**: Either implement properly or remove the feature
- **placeholder/stub/mock**: Implement actual functionality or remove
- **"unimplemented!"**: Implement properly

**No exceptions:** Either implement or remove, no middle ground

---

## Phase 4: Verify All Features Work

### Feature Test Matrix:

| Feature | Command | Test Case | Status |
|---------|---------|-----------|--------|
| **CFG display** | `mirage cfg` | Complex function | ✅ Works |
| **Path enumeration** | `mirage paths` | Function with branches | ✅ Works |
| **Dominance analysis** | `mirage dominators` | Nested function | ✅ Works |
| **Loop detection** | `mirage loops` | Function with loops | ✅ Works |
| **Pattern detection** | `mirage patterns` | Function with if/else | ✅ Works |
| **Impact analysis** | `mirage blast-zone` | Any function | ✅ Works |
| **Dead code** | `mirage unreachable` | Database | ✅ Works |
| **Cycle detection** | `mirage cycles` | Database | ✅ Works |
| **Path verification** | `mirage verify` | Need test | ❓ Untested |
| **Program slicing** | `mirage slice` | Need test | ❓ Untested |
| **Hotspots** | `mirage hotspots` | Database | ✅ Works |
| **Hot paths** | `mirage hotpaths` | Complex function | ❓ Untested |
| **CFG diff** | `mirage diff` | Need snapshots | ❓ Untested |
| **ICFG** | `mirage icfg` | Complex function | ❓ Untested |

---

## Implementation Steps (File by File)

### Step 1: Investigate Deprecated Field
**File:** `src/storage/mod.rs`
**Lines:** 1094, 1120-1263
**Action:** 
- Read commit history for why field was deprecated
- Understand intent of deprecation
- Choose proper fix (remove field vs remove deprecation)

### Step 2: Fix Chosen Issue
**File:** `src/storage/mod.rs`
**Action:**
- Implement chosen fix (remove field OR remove deprecation)
- Update all dependent code
- Update tests appropriately

### Step 3: Test Untested Commands
**Commands to test:** `verify`, `slice`, `hotpaths`, `diff`, `icfg`
**Action:**
- Create test cases for each
- Verify they work correctly
- Fix any issues found

### Step 4: Remove Placeholders
**Action:**
- Grep for all placeholders
- Either implement properly or remove
- No "for now" or "later" allowed

### Step 5: Final Verification
**Action:**
- Full test suite: `cargo test --lib`
- Release build: `cargo build --release`
- Test all commands manually
- Verify 0 errors, minimal warnings

---

## Success Criteria

**Non-Negotiable:**
- ✅ 0 compilation errors
- ✅ 0 test failures  
- ✅ 0 placeholders/TODOs/stubs in production code
- ✅ All documented features work as advertised
- ✅ Warnings < 5

**Quality Gates:**
- All commands tested and working
- Documentation matches reality
- No deprecated field usage (or good reason for it)

---

## Immediate Next Action

**Right Now:** Investigate WHY `cfg_edges` field was deprecated

```bash
# Check git history for the deprecation:
git log -p --all -S "deprecated.*cfg_edges" -- src/storage/mod.rs

# Read the commit that deprecated it
git show <commit_hash>
```

This investigation will tell us whether to:
- Remove the field entirely (if it's truly obsolete)
- Remove the deprecation (if it was marked in error)
- Keep both but fix usage (if there's a valid reason)

**No implementation until we understand WHY it was deprecated!**
