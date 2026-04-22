# Mirage - Comprehensive File-by-File Fix Plan

**Created:** 2026-04-13
**Status:** Ready for implementation
**Current State:** 398/398 tests passing, 4 compilation warnings

---

## Investigation Results

### What's Actually Working:
- ✅ Database schema: coord_x, coord_y, coord_z columns exist and work
- ✅ JSON export: 4D coordinates appear correctly
- ✅ Test suite: 398 tests passing, 0 failures
- ✅ Core features: All CFG algorithms functional

### Actual Issues Found:

#### 1. Deprecated Field Usage (1 warning)
**File:** `src/storage/mod.rs`
**Line:** 1190
**Issue:** Using deprecated `cfg_edges` field in native-v3 backend fallback
**Status:** **This is NOT a bug** - it's intentional backward compatibility

#### 2. Unused Imports (3 warnings)
**File:** `src/cfg/hotpaths.rs`
**Lines:** 25, 28
**Issue:** Unused imports `DiGraph`, `HashMap`, `BasicBlock`, `EdgeType`
**Status:** Trivial to fix

---

## Understanding the cfg_edges Deprecation

### The Commit (20a28d4 from 2026-02-04):

**Intent:**
- Edges are now computed in memory from terminator data
- cfg_edges table kept for backward compatibility
- cfg_edges field in DatabaseStatus deprecated but not removed

**Why Field Still Exists:**
- Backward compatibility with existing databases
- Tests expect the field to exist
- Will always return 0 for new databases

**Conclusion:** This is **working as designed**, not a bug.

---

## File-by-File Fix Plan

### File 1: `src/cfg/hotpaths.rs`

**Issue:** Unused imports (lines 25, 28)

**Fix Required:**
```rust
// Line 25: Remove unused DiGraph import
- use petgraph::graphmap::DiGraph;
+ // Removed: DiGraph not used

// Line 28: Remove unused BasicBlock and EdgeType
- use crate::cfg::{BasicBlock, BlockId, Cfg, EdgeType, Path, Terminator};
+ use crate::cfg::{BlockId, Cfg, Path, Terminator};
```

**Verification:**
```bash
cargo build --lib 2>&1 | grep "warning:" | wc -l
# Should go from 4 to 1 (only cfg_edges deprecation remains)
```

---

### File 2: `src/storage/mod.rs`

**Issue:** Deprecated field usage (line 1190)

**Analysis:**
- This is in native-v3 backend fallback code
- Field is intentionally deprecated but kept for compatibility
- Tests verify this field exists

**Options:**

**Option A: Suppress Warning (Recommended)**
```rust
// Line 1187: Add #[allow(deprecated)] above struct literal
+ #[allow(deprecated)]
  Ok(DatabaseStatus {
      cfg_blocks: 0,
      cfg_edges: 0,  // Kept for backward compatibility
      cfg_paths: 0,
      cfg_dominators: 0,
      mirage_schema_version: MIRAGE_SCHEMA_VERSION,
      magellan_schema_version: MIN_MAGELLAN_SCHEMA_VERSION,
  })
```

**Option B: Remove Field (Breaking Change)**
- Remove cfg_edges from DatabaseStatus struct
- Update all tests that check this field
- Update serialization format
- **NOT RECOMMENDED** - breaking change for backward compatibility

**Decision:** Use Option A - suppress warning with comment explaining why

---

## Feature Verification Plan

### Phase 1: Verify All CLI Commands Work

**Test Database Setup:**
```bash
# Ensure we have a test database
cd /home/feanor/Projects/mirage
cargo build --release
./target/release/mirage --db .magellan/magellan.db status
```

**Command Test Matrix:**

```bash
# 1. Status command
mirage --db .magellan/magellan.db status

# 2. CFG display (find a function first)
mirage --db .magellan/magellan.db cfg --function "main"

# 3. Path enumeration
mirage --db .magellan/magellan.db paths --function "main"

# 4. Dominators
mirage --db .magellan/magellan.db dominators --function "main"

# 5. Dominance frontiers
mirage --db .magellan/magellan.db frontiers --function "main"

# 6. Natural loops
mirage --db .magellan/magellan.db loops --function "main"

# 7. Branching patterns
mirage --db .magellan/magellan.db patterns --function "main"

# 8. Impact analysis
mirage --db .magellan/magellan.db blast-zone --function "main" --block-id 0

# 9. Call graph cycles
mirage --db .magellan/magellan.db cycles

# 10. Dead code detection
mirage --db .magellan/magellan.db unreachable --within-functions

# 11. Path verification (need to research what it does)
mirage --db .magellan/magellan.db verify --help

# 12. Program slicing
mirage --db .magellan/magellan.db slice --help

# 13. Hotspots
mirage --db .magellan/magellan.db hotspots

# 14. Hot paths
mirage --db .magellan/magellan.db hotpaths --help

# 15. CFG diff
mirage --db .magellan/magellan.db diff --help

# 16. ICFG
mirage --db .magellan/magellan.db icfg --help

# 17. Database migration
mirage --db .magellan/magellan.db migrate

# 18. Export (JSON, etc.)
mirage --db .magellan/magellan.db --output json cfg --function "main"
```

**Expected Results:** Each command should work without errors

---

### Phase 2: Check for Placeholders/Stubs

**Search Pattern:**
```bash
# Find all problematic patterns
grep -rn "TODO\|FIXME\|placeholder\|stub\|mock\|for now" src/ --include="*.rs"
grep -rn "unimplemented!\|todo!" src/ --include="*.rs"
```

**For Each Finding:**
- Investigate if it's in production code or test code
- If production: Implement properly OR remove entirely
- If test: Ensure test actually validates functionality
- No exceptions: proper implementation or complete removal

---

### Phase 3: Verify Database Schema

**Schema Verification:**
```sql
-- Connect to database
sqlite3 .magellan/magellan.db

-- Check cfg_blocks schema
PRAGMA table_info(cfg_blocks);

-- Expected columns:
-- coord_x INTEGER NOT NULL DEFAULT 0
-- coord_y INTEGER NOT NULL DEFAULT 0
-- coord_z INTEGER NOT NULL DEFAULT 0
-- coord_t (bonus column discovered)
```

**Expected:** All coord columns exist with proper constraints

---

### Phase 4: Integration Testing

**Real-World Test Scenario:**
```bash
# 1. Find a complex function
mirage --db .magellan/magellan.db unreachable --within-functions | head -20

# 2. Analyze it with all tools
FUNC="complex_function_name"
mirage --db .magellan/magellan.db cfg --function "$FUNC"
mirage --db .magellan/magellan.db paths --function "$FUNC"
mirage --db .magellan/magellan.db dominators --function "$FUNC"
mirage --db .magellan/magellan.db loops --function "$FUNC"

# 3. Verify JSON export includes 4D coordinates
mirage --db .magellan/magellan.db --output json cfg --function "$FUNC" | jq '.data.blocks[0].coord_x'
```

**Expected:** All commands work, coordinates appear in JSON

---

## Implementation Steps

### Step 1: Fix Trivial Warnings (5 minutes)
**File:** `src/cfg/hotpaths.rs`
1. Remove unused DiGraph import (line 25)
2. Remove unused BasicBlock and EdgeType from line 28
3. Verify warnings reduced from 4 to 1

### Step 2: Suppress Deprecation Warning (5 minutes)
**File:** `src/storage/mod.rs`
1. Add `#[allow(deprecated)]` above line 1187
2. Add comment explaining backward compatibility
3. Verify warnings reduced from 1 to 0

### Step 3: Test All Commands (30 minutes)
1. Run command test matrix (all 18 commands)
2. Document any failures
3. Fix any issues found

### Step 4: Search for Placeholders (15 minutes)
1. Grep for all TODO/FIXME/placeholder patterns
2. For each finding: implement or remove
3. Verify no placeholders remain

### Step 5: Integration Testing (20 minutes)
1. Test with real complex function
2. Verify JSON output includes coordinates
3. Test all output formats (human, json, pretty)

### Step 6: Final Verification (10 minutes)
1. Full test suite: `cargo test --lib`
2. Release build: `cargo build --release`
3. Warning count: `cargo build --lib 2>&1 | grep "warning:" | wc -l`
4. Manual command testing

---

## Success Criteria

**Non-Negotiable:**
- ✅ 0 compilation errors
- ✅ 0 test failures (currently 398/398 passing)
- ✅ 0 placeholders/TODOs/stubs in production code
- ✅ Warnings < 5 (currently 4, will be 0 after fixes)
- ✅ All 18 CLI commands work as documented

**Quality Gates:**
- All commands tested with real data
- Documentation matches actual behavior
- 4D coordinates work in all output formats
- No deprecated field usage (or properly suppressed)

---

## Estimated Timeline

- **Step 1-2:** 10 minutes (trivial fixes)
- **Step 3:** 30 minutes (command testing)
- **Step 4:** 15 minutes (placeholder search)
- **Step 5:** 20 minutes (integration testing)
- **Step 6:** 10 minutes (final verification)

**Total:** ~85 minutes (1.5 hours)

---

## Immediate Next Actions

**Right Now:** Fix the 3 unused import warnings in `src/cfg/hotpaths.rs`

**Next:** Suppress the deprecation warning in `src/storage/mod.rs`

**Then:** Test all 18 CLI commands systematically

**Finally:** Search for and eliminate any placeholders

---

## Notes

- The `cfg_edges` deprecation is **intentional** and **working as designed**
- This is backward compatibility, not a bug
- All tests passing (398/398) - excellent code quality
- Only minor cleanup needed (unused imports, warning suppression)
- Focus should be on verifying FEATURES work, not fixing bugs
