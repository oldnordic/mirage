# Code Drift Analysis Report

**Date:** 2026-02-02
**Analyzer:** Claude (mirage codebase scan)
**Trigger:** Post Phase 8 completion scan

---

## Executive Summary

Drift analysis identified **2 blocking stub commands**, **7 database loading TODOs**, and **17 unused import warnings**. The core issue is that MIR extraction (`mirage index`) was never implemented, blocking all database-dependent features.

### Priority Breakdown

| Priority | Count | Items |
|----------|-------|-------|
| 🔴 Critical | 2 | Stub commands: `index`, `blast-zone` |
| 🟠 High | 7 | Database loading TODOs in CLI commands |
| 🟡 Low | 17 | Unused import warnings |

---

## 1. STUB COMMANDS (Critical)

### 1.1 `mirage index`

**Location:** `src/cli/mod.rs:536-538`

```rust
pub fn index(_args: IndexArgs) Result<()> {
    // TODO: Implement M1 (MIR Extraction)
    output::error("Indexing not yet implemented - requires MIR extraction (Milestone 1)");
    std::process::exit(1);
}
```

**Impact:** Blocks ALL database-dependent features. Without MIR extraction, no CFGs are stored, so all analysis commands fall back to test data.

**Dependencies:**
- Charon binary installation
- `src/mir/charon.rs` (already exists - `run_charon`, `parse_ullbc`)
- `src/cfg/mir.rs` (already exists - `ullbc_to_cfg`)
- Database schema (Phase 1 complete)

### 1.2 `mirage blast-zone`

**Location:** `src/cli/mod.rs:1528-1529`

```rust
pub fn blast_zone(_args: BlastZoneArgs) -> Result<()> {
    // TODO: Implement path-based impact analysis
    output::error("Blast zone analysis not yet implemented");
    std::process::exit(1);
}
```

**Impact:** No impact analysis feature. Users cannot determine "what breaks if I change this code?"

**Dependencies:**
- Path enumeration (Phase 5 complete)
- Call graph from Magellan (already available)
- CFG data in database (requires `mirage index`)

---

## 2. DATABASE LOADING TODOs (High)

All analysis commands currently use `create_test_cfg()` placeholder instead of loading from database:

| Command | Line | TODO |
|---------|------|------|
| `paths()` | 622 | "Load CFG from database using args.function" |
| `cfg()` | 736 | "Load CFG from database for the specified function" |
| `dominators()` | 846 | "Load CFG from database for the specified function" |
| `loops()` | 1172 | "Load CFG from database for the specified function" |
| `unreachable()` | 1264 | "Load CFG from database using function filter" |
| `patterns()` | 1558 | "Load CFG from database for the specified function" |
| `frontiers()` | 1688 | "Load CFG from database for the specified function" |

**Current Pattern:**
```rust
// TODO: Load CFG from database for the specified function.
let cfg = create_test_cfg();
```

**Required Pattern:**
```rust
// Resolve function name to function_id
let function_id = resolve_function(&mut db, &args.function)?;

// Load CFG from database
let cfg = load_cfg_from_db(&mut db, function_id)?;
```

---

## 3. UNUSED EXPORTS (Low - Code Hygiene)

### 3.1 Path Enumeration Functions (7 unused imports)

```
EnumerationContext
check_path_explosion
enumerate_paths_cached_with_context
enumerate_paths_cached
enumerate_paths_with_context
estimate_path_count
hash_path
```

**Note:** `get_or_enumerate_paths` is used and calls these internally. They're exported for API users but not directly imported in CLI.

### 3.2 Pattern Detection Functions (5 unused imports)

```
BranchType, IfElsePattern, MatchPattern
classify_branch, detect_all_patterns
```

**Note:** `detect_if_else_patterns` and `detect_match_patterns` are used. The unused ones are alternative/helper APIs.

### 3.3 Reachability Functions (6 unused imports)

```
ReachabilityCache, can_reach_cached, can_reach
find_reachable, unreachable_block_ids
```

**Note:** `find_unreachable` is used. Others are alternative APIs for different use cases.

### 3.4 MIR/Charon Functions (4 unused imports)

```
UllbcBlock, UllbcData, parse_ullbc, run_charon
ullbc_to_cfg (from mir module)
```

**Note:** These will be used once `mirage index` is implemented.

### 3.5 Path Caching Functions (5 not imported)

```
PathCache, get_cached_paths
invalidate_function_paths, store_paths
update_function_paths_if_changed
```

**Note:** These are internal storage APIs used by `get_or_enumerate_paths`.

### 3.6 Summary Functions (2 not imported)

```
describe_block, summarize_cfg
```

**Note:** `summarize_path` is used. Others are additional utility APIs.

---

## 4. API DRIFT

✅ **No API drift detected**

All exported types maintain consistent signatures. The warnings are purely about unused imports, not breaking changes.

---

## 5. DATABASE STATUS

**Magellan Database:** `.codemcp/mirage.db`
- Files: 17
- Symbols: 237
- References: 413
- Calls: 62

**Mirage Schema Extensions:**
- `cfg_blocks` - Exists (Phase 1)
- `cfg_edges` - Not implemented (edges can be derived from blocks)
- `cfg_paths` - Exists (Phase 5)
- `cfg_path_elements` - Not implemented (paths stored as JSON)
- `cfg_dominators` - Not implemented (can be computed on-demand)

---

## 6. RECOMMENDED PHASE 9 SCOPE

Based on this analysis, Phase 9 should implement:

### 6.1 MIR Extraction (mirage index)

1. **Plan 09-01:** Charon binary integration
   - Find or spawn `charon` binary
   - Capture ULLBC JSON output
   - Error handling for missing Charon

2. **Plan 09-02:** ULLBC to CFG conversion
   - Use existing `ullbc_to_cfg()` from `src/cfg/mir.rs`
   - Store CFGs in database (`cfg_blocks` table)
   - Compute and store `function_hash` for incremental updates

3. **Plan 09-03:** Index command CLI
   - Implement `index()` function
   - Support `--project PATH`, `--crate NAME`, `--incremental`
   - Progress reporting for large projects

### 6.2 Database Loading (shared utility)

4. **Plan 09-04:** CFG loading from database
   - `resolve_function_name()` - name/ID lookup
   - `load_cfg_from_db()` - reconstruct Cfg from cfg_blocks
   - Update all 7 commands to use database loading

### 6.3 Impact Analysis (mirage blast-zone)

5. **Plan 09-05:** Blast zone command - block-based
   - `mirage blast-zone --function F --block-id N`
   - Show all blocks/functions reachable from given block
   - Use path enumeration + call graph

6. **Plan 09-06:** Blast zone command - path-based
   - `mirage blast-zone --path-id ID`
   - Show impact scope for specific execution path
   - Aggregate results across path blocks

---

## 7. SUCCESS METRICS

Phase 9 is complete when:

- [ ] `mirage index --project .` successfully indexes a Rust project
- [ ] `mirage paths --function main` works on indexed code (not test CFG)
- [ ] `mirage blast-zone --function main --block-id 0` shows reachable functions
- [ ] All 7 commands load from database (no more `create_test_cfg()`)
- [ ] Compilation has 0 warnings (unused imports resolved)

---

_This report was generated using magellan/llmgrep for symbol discovery and cargo build for warning detection._
