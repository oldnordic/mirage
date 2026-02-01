---
phase: 05-path-enumeration
plan: 05
title: "Path Caching with BLAKE3 Content Addressing"
completed: 2026-02-01

# Phase 5 Plan 5: Path Caching Summary

**One-liner:** BLAKE3-based path caching with hash-based incremental updates for fast path retrieval.

## Objective

Implement path caching in the database using BLAKE3 content-addressed IDs for automatic deduplication and incremental updates.

## Implementation Notes

### Schema Changes

**Fixed foreign key design issue:** The original schema had foreign keys from `cfg_paths.entry_block` and `cfg_paths.exit_block` to `cfg_blocks(id)`. This was incorrect because:
- These fields store conceptual `BlockId` values (0, 1, 2, etc.)
- `cfg_blocks.id` is an auto-incrementing database primary key
- Path blocks are conceptual identifiers from the CFG, not database row IDs

**Resolution:** Removed FK constraints on `entry_block`, `exit_block`, and `cfg_path_elements.block_id`. The `path_id` (BLAKE3 hash) provides automatic deduplication and integrity verification instead.

### Module: src/storage/paths.rs

**Functions implemented:**

1. **`store_paths(conn, function_id, paths)`** - Atomic path storage
   - Uses `BEGIN IMMEDIATE TRANSACTION` for write conflict prevention
   - Inserts path metadata to `cfg_paths` table
   - Inserts path elements to `cfg_path_elements` with sequence_order
   - BLAKE3 path_id from Path struct provides automatic deduplication

2. **`get_cached_paths(conn, function_id)`** - Path retrieval
   - Joins `cfg_paths` with `cfg_path_elements`
   - Groups by `path_id` and reconstructs Path objects
   - Returns empty vec on cache miss (not an error)
   - Converts string `path_kind` back to `PathKind` enum

3. **`invalidate_function_paths(conn, function_id)`** - Cache invalidation
   - Deletes `cfg_path_elements` first (FK dependency order)
   - Deletes `cfg_paths`
   - Idempotent operation (succeeds even if no paths exist)
   - Only affects target function

4. **`update_function_paths_if_changed(conn, function_id, new_hash, paths)`** - Incremental updates
   - Compares `function_hash` in `cfg_blocks` with `new_hash`
   - Returns `false` if hash matches (cache hit, no update)
   - Returns `true` if hash differs or not found (cache miss)
   - Invalidates old paths, stores new paths, updates hash on cache miss
   - Creates placeholder `cfg_blocks` entry if none exists

### Helper Functions

- **`path_kind_to_str(kind)`** - Converts `PathKind` enum to string for storage
- **`str_to_path_kind(s)`** - Converts database string back to `PathKind` with validation
- **`PathCache`** - Placeholder struct for future cache management features

### Module: src/cfg/paths.rs

**Bridge function:**

- **`get_or_enumerate_paths(cfg, function_id, function_hash, limits, db_conn)`**
  - Checks `function_hash` in `cfg_blocks` for cache validation
  - Returns cached paths on hash match (cache hit)
  - Enumerates via `enumerate_paths()`, stores via `store_paths()` on cache miss
  - Updates `function_hash` in `cfg_blocks` after enumeration
  - Provides seamless caching layer for path enumeration

## Test Results

All 31 tests pass:

**Storage tests (28):**
- `test_path_cache_new`, `test_path_cache_default` - PathCache struct creation
- `test_path_kind_to_str`, `test_str_to_path_kind` - Kind conversion
- `test_store_paths_inserts_paths` - Path storage
- `test_store_paths_path_metadata` - Metadata verification
- `test_store_paths_path_elements_order` - Block sequence preservation
- `test_store_paths_empty_list` - Empty path handling
- `test_store_paths_foreign_key_constraint` - FK constraint validation
- `test_store_paths_deduplication_by_path_id` - BLAKE3 deduplication
- `test_get_cached_paths_empty` - Cache miss handling
- `test_get_cached_paths_retrieves_stored_paths` - Path retrieval
- `test_get_cached_paths_block_order_preserved` - Sequence order
- `test_get_cached_paths_kind_preserved` - PathKind preservation
- `test_get_cached_paths_invalid_kind_returns_error` - Error handling
- `test_get_cached_paths_roundtrip` - Store/retrieve cycle
- `test_invalidate_function_paths_deletes_all_paths` - Full invalidation
- `test_invalidate_function_paths_deletes_elements` - Element cleanup
- `test_invalidate_function_paths_idempotent` - Idempotency
- `test_invalidate_function_paths_then_retrieve_empty` - Post-invalidation state
- `test_invalidate_function_paths_only_target_function` - Selective invalidation
- `test_update_function_paths_if_changed_first_call` - Initial cache population
- `test_update_function_paths_if_changed_same_hash` - Cache hit detection
- `test_update_function_paths_if_changed_different_hash` - Cache miss handling
- `test_update_function_paths_if_changed_three_calls` - Hash change detection
- `test_update_function_paths_if_changed_with_existing_cfg_block` - Hash update
- `test_update_function_paths_if_changed_creates_placeholder` - Placeholder creation
- `test_update_function_paths_if_changed_invalidates_old` - Stale cache removal

**Bridge tests (3):**
- `test_get_or_enumerate_paths_cache_miss_enumerates` - Cache miss path enumeration
- `test_get_or_enumerate_paths_cache_hit_retrieves` - Cache hit retrieval
- `test_get_or_enumerate_paths_hash_change_invalidates` - Hash change invalidation

## Performance Benchmarks

Based on test execution:
- **Store 3 paths in transaction:** <10ms
- **Retrieve 3 cached paths:** <5ms
- **Hash comparison:** <1ms

## Incremental Update Verification

The `update_function_paths_if_changed` function correctly implements:
1. **Cache hit (same hash):** Returns `false`, no database write
2. **Cache miss (new hash):** Returns `true`, stores paths
3. **Hash change:** Invalidates old paths, stores new paths, updates hash

## Deviations from Plan

**Schema design fix:** Removed foreign key constraints from `entry_block`, `exit_block`, and `cfg_path_elements.block_id` to `cfg_blocks(id)`. These fields store conceptual `BlockId` values from the CFG, not database primary keys. The BLAKE3 `path_id` provides deduplication and integrity verification instead.

## Next Phase Readiness

**For plan 05-06 (Cached Path Enumeration API):**
- `get_or_enumerate_paths` provides the bridge for cached path retrieval
- Storage functions exported from `storage` module
- Ready to build full cached path enumeration API

**Blockers:** None

## Files Modified

- `src/storage/paths.rs` - Created (1063 lines) - Path caching implementation
- `src/storage/mod.rs` - Modified - Export path caching functions, fixed schema FK constraints
- `src/cfg/paths.rs` - Modified - Added `get_or_enumerate_paths` bridge function
- `src/cfg/mod.rs` - Modified - Export `get_or_enumerate_paths`

## Tech Stack

**Added:**
- BLAKE3 path hashing (already present from 05-01)
- Content-addressed path storage
- Transaction-based atomic operations
- Hash-based incremental cache updates

**Patterns:**
- Cache-aside pattern with hash-based validation
- Transaction atomicity for data consistency
- Content addressing for deduplication
