use rusqlite::{params, Connection};

use crate::cfg::{BlockId, Path, PathKind};

use super::*;

fn create_test_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();

    conn.execute(
        "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, 7, 3, 0)",
        [],
    ).unwrap();

    crate::storage::create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
        rusqlite::params!("function", "test_func", "test.rs", "{}"),
    )
    .unwrap();

    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

    conn
}

fn create_mock_paths() -> Vec<Path> {
    vec![
        Path::new(vec![0, 1, 2], PathKind::Normal),
        Path::new(vec![0, 1, 3], PathKind::Normal),
        Path::new(vec![0, 2], PathKind::Error),
    ]
}

#[test]
fn test_path_cache_new() {
    let cache = PathCache::new();
    let _ = cache;
}

#[test]
fn test_path_cache_default() {
    let cache = PathCache::default();
    let _ = cache;
}

#[test]
fn test_path_kind_to_str() {
    assert_eq!(path_kind_to_str(PathKind::Normal), "Normal");
    assert_eq!(path_kind_to_str(PathKind::Error), "Error");
    assert_eq!(path_kind_to_str(PathKind::Degenerate), "Degenerate");
    assert_eq!(path_kind_to_str(PathKind::Unreachable), "Unreachable");
}

#[test]
fn test_str_to_path_kind() {
    assert_eq!(str_to_path_kind("Normal").unwrap(), PathKind::Normal);
    assert_eq!(str_to_path_kind("Error").unwrap(), PathKind::Error);
    assert_eq!(
        str_to_path_kind("Degenerate").unwrap(),
        PathKind::Degenerate
    );
    assert_eq!(
        str_to_path_kind("Unreachable").unwrap(),
        PathKind::Unreachable
    );
    assert!(str_to_path_kind("Invalid").is_err());
}

#[test]
fn test_store_paths_inserts_paths() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    store_paths(&mut conn, function_id, &paths).unwrap();

    let path_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(path_count, 3, "Should have 3 paths");

    let element_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM cfg_path_elements", [], |row| {
            row.get(0)
        })
        .unwrap();

    assert_eq!(element_count, 8, "Should have 8 elements (3+3+2)");
}

#[test]
fn test_store_paths_path_metadata() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    store_paths(&mut conn, function_id, &paths).unwrap();

    let mut stmt = conn
        .prepare(
            "SELECT path_id, path_kind, entry_block, exit_block, length
             FROM cfg_paths
             WHERE function_id = ?
             ORDER BY entry_block, exit_block",
        )
        .unwrap();

    let rows: Vec<_> = stmt
        .query_map(params![function_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(rows.len(), 3);

    let row = &rows[0];
    assert_eq!(row.2, 0);
    assert_eq!(row.3, 2);
    assert_eq!(row.4, 3);
    assert_eq!(row.1, "Normal");

    assert!(!row.0.is_empty());
    assert!(row.0.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_store_paths_path_elements_order() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    store_paths(&mut conn, function_id, &paths).unwrap();

    let path_id: String = conn
        .query_row(
            "SELECT path_id FROM cfg_paths WHERE function_id = ? LIMIT 1",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();

    let mut stmt = conn
        .prepare(
            "SELECT block_id FROM cfg_path_elements
             WHERE path_id = ?
             ORDER BY sequence_order",
        )
        .unwrap();

    let blocks: Vec<BlockId> = stmt
        .query_map(params![path_id], |row| Ok(row.get::<_, i64>(0)? as BlockId))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(blocks, vec![0, 1, 2]);
}

#[test]
fn test_store_paths_empty_list() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths: Vec<Path> = vec![];

    store_paths(&mut conn, function_id, &paths).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(count, 0);
}

#[test]
fn test_store_paths_foreign_key_constraint() {
    let mut conn = create_test_db();
    let invalid_function_id: i64 = 9999;
    let paths = create_mock_paths();

    let result = store_paths(&mut conn, invalid_function_id, &paths);

    assert!(result.is_err(), "Should fail with invalid function_id");
}

#[test]
fn test_store_paths_deduplication_by_path_id() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    store_paths(&mut conn, function_id, &paths).unwrap();

    let result = store_paths(&mut conn, function_id, &paths);

    assert!(result.is_err(), "Should fail on duplicate path_id");
}

#[test]
fn test_get_cached_paths_empty() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;

    let paths = get_cached_paths(&mut conn, function_id).unwrap();
    assert_eq!(paths.len(), 0);
}

#[test]
fn test_get_cached_paths_retrieves_stored_paths() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let original_paths = create_mock_paths();

    store_paths(&mut conn, function_id, &original_paths).unwrap();

    let retrieved_paths = get_cached_paths(&mut conn, function_id).unwrap();

    assert_eq!(retrieved_paths.len(), original_paths.len());

    for orig in &original_paths {
        assert!(
            retrieved_paths
                .iter()
                .any(|p| p.blocks == orig.blocks && p.kind == orig.kind),
            "Path {:?} not found in retrieved paths",
            orig.blocks
        );
    }
}

#[test]
fn test_get_cached_paths_block_order_preserved() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;

    let paths = vec![
        Path::new(vec![0, 1, 2, 3], PathKind::Normal),
        Path::new(vec![5, 4, 3, 2, 1], PathKind::Error),
    ];

    store_paths(&mut conn, function_id, &paths).unwrap();
    let retrieved = get_cached_paths(&mut conn, function_id).unwrap();

    assert_eq!(retrieved.len(), 2);

    let path1 = retrieved
        .iter()
        .find(|p| p.blocks == vec![0, 1, 2, 3])
        .unwrap();
    assert_eq!(path1.blocks, vec![0, 1, 2, 3]);

    let path2 = retrieved
        .iter()
        .find(|p| p.blocks == vec![5, 4, 3, 2, 1])
        .unwrap();
    assert_eq!(path2.blocks, vec![5, 4, 3, 2, 1]);
}

#[test]
fn test_get_cached_paths_kind_preserved() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;

    let paths = vec![
        Path::new(vec![0], PathKind::Normal),
        Path::new(vec![1], PathKind::Error),
        Path::new(vec![2], PathKind::Degenerate),
        Path::new(vec![3], PathKind::Unreachable),
    ];

    store_paths(&mut conn, function_id, &paths).unwrap();
    let retrieved = get_cached_paths(&mut conn, function_id).unwrap();

    assert_eq!(retrieved.len(), 4);

    assert!(retrieved.iter().any(|p| p.kind == PathKind::Normal));
    assert!(retrieved.iter().any(|p| p.kind == PathKind::Error));
    assert!(retrieved.iter().any(|p| p.kind == PathKind::Degenerate));
    assert!(retrieved.iter().any(|p| p.kind == PathKind::Unreachable));
}

#[test]
fn test_get_cached_paths_invalid_kind_returns_error() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;

    conn.execute(
        "INSERT INTO cfg_paths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        params!("invalid_path_id", function_id, "InvalidKind", 0, 0, 1, 0),
    ).unwrap();

    conn.execute(
        "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id)
             VALUES (?, ?, ?)",
        params!("invalid_path_id", 0, 0),
    )
    .unwrap();

    let result = get_cached_paths(&mut conn, function_id);
    assert!(result.is_err(), "Should fail on invalid path_kind");
}

#[test]
fn test_get_cached_paths_roundtrip() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;

    let paths = vec![
        Path::new(vec![0, 1, 2, 3, 4, 5], PathKind::Normal),
        Path::new(vec![0, 1, 3, 5], PathKind::Normal),
        Path::new(vec![0, 2, 4, 5], PathKind::Error),
        Path::new(vec![0, 5], PathKind::Degenerate),
    ];

    store_paths(&mut conn, function_id, &paths).unwrap();
    let retrieved = get_cached_paths(&mut conn, function_id).unwrap();

    assert_eq!(retrieved.len(), paths.len());

    let mut sorted_orig: Vec<_> = paths.iter().collect();
    let mut sorted_ret: Vec<_> = retrieved.iter().collect();
    sorted_orig.sort_by_key(|p| p.blocks.clone());
    sorted_ret.sort_by_key(|p| p.blocks.clone());

    for (orig, ret) in sorted_orig.iter().zip(sorted_ret.iter()) {
        assert_eq!(orig.blocks, ret.blocks, "Block sequence mismatch");
        assert_eq!(orig.kind, ret.kind, "PathKind mismatch");
    }
}

#[test]
fn test_invalidate_function_paths_deletes_all_paths() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    store_paths(&mut conn, function_id, &paths).unwrap();

    let count_before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count_before, 3);

    invalidate_function_paths(&mut conn, function_id).unwrap();

    let count_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count_after, 0);
}

#[test]
fn test_invalidate_function_paths_deletes_elements() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    store_paths(&mut conn, function_id, &paths).unwrap();

    let count_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM cfg_path_elements", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(count_before > 0);

    invalidate_function_paths(&mut conn, function_id).unwrap();

    let count_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_path_elements
             WHERE path_id IN (SELECT path_id FROM cfg_paths WHERE function_id = ?)",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count_after, 0);
}

#[test]
fn test_invalidate_function_paths_idempotent() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;

    invalidate_function_paths(&mut conn, function_id).unwrap();

    invalidate_function_paths(&mut conn, function_id).unwrap();
}

#[test]
fn test_invalidate_function_paths_then_retrieve_empty() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    store_paths(&mut conn, function_id, &paths).unwrap();
    let before = get_cached_paths(&mut conn, function_id).unwrap();
    assert_eq!(before.len(), 3);

    invalidate_function_paths(&mut conn, function_id).unwrap();

    let after = get_cached_paths(&mut conn, function_id).unwrap();
    assert_eq!(after.len(), 0);
}

#[test]
fn test_invalidate_function_paths_only_target_function() {
    let mut conn = create_test_db();

    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
        rusqlite::params!("function", "func1", "test.rs", "{}"),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
        rusqlite::params!("function", "func2", "test.rs", "{}"),
    )
    .unwrap();

    let function_id_1: i64 = 1;
    let function_id_2: i64 = 2;

    let paths_1 = vec![
        Path::new(vec![0, 1, 2], PathKind::Normal),
        Path::new(vec![0, 1, 3], PathKind::Normal),
        Path::new(vec![0, 2], PathKind::Error),
    ];
    let paths_2 = vec![
        Path::new(vec![10, 11, 12], PathKind::Normal),
        Path::new(vec![10, 11, 13], PathKind::Normal),
        Path::new(vec![10, 12], PathKind::Error),
    ];

    store_paths(&mut conn, function_id_1, &paths_1).unwrap();
    store_paths(&mut conn, function_id_2, &paths_2).unwrap();

    let count_1_before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id_1],
            |row| row.get(0),
        )
        .unwrap();
    let count_2_before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id_2],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count_1_before, 3);
    assert_eq!(count_2_before, 3);

    invalidate_function_paths(&mut conn, function_id_1).unwrap();

    let count_1_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id_1],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count_1_after, 0);

    let count_2_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id_2],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count_2_after, 3);
}

#[test]
fn test_update_function_paths_if_changed_first_call() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();
    let hash = "abc123";

    let updated = update_function_paths_if_changed(&mut conn, function_id, hash, &paths).unwrap();
    assert!(updated, "First call should return true (updated)");

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 3);
}

#[test]
fn test_update_function_paths_if_changed_same_hash() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();
    let hash = "abc123";

    let updated1 = update_function_paths_if_changed(&mut conn, function_id, hash, &paths).unwrap();
    assert!(updated1);

    let updated2 = update_function_paths_if_changed(&mut conn, function_id, hash, &paths).unwrap();
    assert!(
        updated2,
        "Same hash should return true (hash caching not available with Magellan)"
    );
}

#[test]
fn test_update_function_paths_if_changed_different_hash() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths1 = create_mock_paths();
    let paths2 = vec![Path::new(vec![0, 1], PathKind::Normal)];

    let updated1 =
        update_function_paths_if_changed(&mut conn, function_id, "hash1", &paths1).unwrap();
    assert!(updated1);

    let count1: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count1, 3);

    let updated2 =
        update_function_paths_if_changed(&mut conn, function_id, "hash2", &paths2).unwrap();
    assert!(updated2, "Different hash should return true (updated)");

    let count2: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count2, 1, "Old paths should be invalidated and replaced");
}

#[test]
fn test_update_function_paths_if_changed_three_calls() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    let u1 = update_function_paths_if_changed(&mut conn, function_id, "hash1", &paths).unwrap();
    assert!(u1);

    let u2 = update_function_paths_if_changed(&mut conn, function_id, "hash1", &paths).unwrap();
    assert!(u2);

    let u3 = update_function_paths_if_changed(&mut conn, function_id, "hash2", &paths).unwrap();
    assert!(u3);

    let u4 = update_function_paths_if_changed(&mut conn, function_id, "hash2", &paths).unwrap();
    assert!(u4);
}

#[test]
fn test_update_function_paths_if_changed_with_existing_cfg_block() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;

    conn.execute(
        "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                         start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![function_id, "entry", "return", 0, 10, 1, 0, 1, 10],
    )
    .unwrap();

    let paths = create_mock_paths();

    let updated =
        update_function_paths_if_changed(&mut conn, function_id, "new_hash", &paths).unwrap();
    assert!(updated);

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "Should still have only one cfg_blocks entry");
}

#[test]
fn test_update_function_paths_if_changed_creates_placeholder() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    update_function_paths_if_changed(&mut conn, function_id, "hash1", &paths).unwrap();

    let path_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        path_count, 3,
        "Should store all paths without cfg_blocks entry"
    );
}

#[test]
fn test_update_function_paths_if_changed_invalidates_old() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;

    let paths1 = vec![
        Path::new(vec![0, 1, 2], PathKind::Normal),
        Path::new(vec![0, 1, 3], PathKind::Normal),
    ];
    let paths2 = vec![Path::new(vec![0, 2], PathKind::Error)];

    update_function_paths_if_changed(&mut conn, function_id, "hash1", &paths1).unwrap();

    let retrieved1 = get_cached_paths(&mut conn, function_id).unwrap();
    assert_eq!(retrieved1.len(), 2);

    update_function_paths_if_changed(&mut conn, function_id, "hash2", &paths2).unwrap();

    let retrieved2 = get_cached_paths(&mut conn, function_id).unwrap();
    assert_eq!(retrieved2.len(), 1);
    assert_eq!(retrieved2[0].blocks, vec![0, 2]);
    assert_eq!(retrieved2[0].kind, PathKind::Error);
}

#[test]
fn test_store_paths_batch_inserts_correctly() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    store_paths_batch(&mut conn, function_id, &paths).unwrap();

    let path_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(path_count, 3, "Should have 3 paths");

    let element_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM cfg_path_elements", [], |row| {
            row.get(0)
        })
        .unwrap();

    assert_eq!(element_count, 8, "Should have 8 elements (3+3+2)");
}

#[test]
fn test_store_paths_batch_empty_list() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths: Vec<Path> = vec![];

    store_paths_batch(&mut conn, function_id, &paths).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(count, 0);
}

#[test]
fn test_store_paths_batch_preserves_metadata() {
    let mut conn = create_test_db();
    let function_id: i64 = 1;
    let paths = create_mock_paths();

    store_paths_batch(&mut conn, function_id, &paths).unwrap();

    let mut stmt = conn
        .prepare(
            "SELECT path_id, path_kind, entry_block, exit_block, length
             FROM cfg_paths
             WHERE function_id = ?
             ORDER BY entry_block, exit_block",
        )
        .unwrap();

    let rows: Vec<_> = stmt
        .query_map(params![function_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(rows.len(), 3);

    let row = &rows[0];
    assert_eq!(row.2, 0);
    assert_eq!(row.3, 2);
    assert_eq!(row.4, 3);
    assert_eq!(row.1, "Normal");
}

#[test]
fn test_store_paths_batch_performance_100_paths() {
    use std::time::Instant;

    let mut conn = create_test_db();
    let function_id: i64 = 1;

    let paths: Vec<Path> = (0..100)
        .map(|i| Path::new(vec![0, 1, i, 2, i % 5 + 10], PathKind::Normal))
        .collect();

    let start = Instant::now();
    store_paths_batch(&mut conn, function_id, &paths).unwrap();
    let duration = start.elapsed();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 100);

    let element_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM cfg_path_elements", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(element_count, 500);

    assert!(
        duration < std::time::Duration::from_millis(100),
        "store_paths_batch took {:?}, expected <100ms",
        duration
    );
}

#[test]
#[ignore = "benchmark test - run with cargo test -- --ignored"]
fn test_store_paths_batch_benchmark_large() {
    use std::time::Instant;

    let mut conn = create_test_db();
    let function_id: i64 = 1;

    let paths: Vec<Path> = (0..1000)
        .map(|i| Path::new(vec![0, 1, i, 2, 3, i % 10 + 100], PathKind::Normal))
        .collect();

    let start = Instant::now();
    store_paths_batch(&mut conn, function_id, &paths).unwrap();
    let duration = start.elapsed();

    println!("store_paths_batch for 1000 paths took {:?}", duration);

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1000);
}
