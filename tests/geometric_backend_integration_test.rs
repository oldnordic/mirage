//! Layer 2: Unit Integration Tests for Geometric Backend
//!
//! Tests verify the GeometricStorage integrates properly with the Backend enum
//! and delegates correctly.

#![cfg(feature = "backend-geometric")]

use mirage::storage::{Backend, GeometricStorage, StorageTrait};
use tempfile::TempDir;

fn create_temp_geo_file() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test.geo");

    // Create valid geometric database using Magellan's backend directly
    let _backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");

    (temp_dir, geo_path)
}

#[test]
fn test_backend_enum_delegates_to_geometric() {
    let (_temp, geo_path) = create_temp_geo_file();
    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // Verify it's detected as Geometric
    match &backend {
        Backend::Geometric(_) => {}
        _ => panic!("Expected Geometric backend, got {:?}", backend),
    }

    // Verify is_geometric() method
    assert!(backend.is_geometric(), "is_geometric() should return true");
}

#[test]
fn test_geometric_storage_trait_methods() {
    let (_temp, geo_path) = create_temp_geo_file();
    let storage = GeometricStorage::open(&geo_path).unwrap();

    // Test get_cfg_blocks (returns empty for now as not fully implemented)
    let blocks = storage.get_cfg_blocks(1);
    assert!(blocks.is_ok(), "get_cfg_blocks should succeed");

    // Test get_entity (returns None for nonexistent entity)
    let entity = storage.get_entity(999);
    assert!(
        entity.is_none(),
        "get_entity should return None for nonexistent entity"
    );

    // Test get_cached_paths
    let paths = storage.get_cached_paths(1);
    assert!(paths.is_ok(), "get_cached_paths should succeed");
    assert!(
        paths.unwrap().is_none(),
        "get_cached_paths should return None for geometric"
    );
}

#[test]
fn test_backend_delegates_get_cfg_blocks() {
    let (_temp, geo_path) = create_temp_geo_file();
    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // This should delegate to GeometricStorage
    let result = backend.get_cfg_blocks(1);
    assert!(result.is_ok(), "Backend::get_cfg_blocks should succeed");
}

#[test]
fn test_backend_delegates_get_entity() {
    let (_temp, geo_path) = create_temp_geo_file();
    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // This should delegate to GeometricStorage
    let result = backend.get_entity(999);
    assert!(
        result.is_none(),
        "Backend::get_entity should return None for nonexistent entity"
    );
}

#[test]
fn test_backend_delegates_get_cached_paths() {
    let (_temp, geo_path) = create_temp_geo_file();
    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // This should delegate to GeometricStorage
    let result = backend.get_cached_paths(1);
    assert!(result.is_ok(), "Backend::get_cached_paths should succeed");
}

#[test]
fn test_geometric_backend_symbol_methods() {
    let (_temp, geo_path) = create_temp_geo_file();
    let storage = GeometricStorage::open(&geo_path).unwrap();

    // Test find_symbols_by_name on empty database
    let symbols = storage.find_symbols_by_name("test");
    assert!(
        symbols.is_empty(),
        "Should return empty for nonexistent symbol"
    );

    // Test find_symbol_by_fqn on empty database
    let symbol = storage.find_symbol_by_fqn("crate::test::function");
    assert!(symbol.is_none(), "Should return None for nonexistent FQN");

    // Test complete_fqn_prefix on empty database
    let completions = storage.complete_fqn_prefix("test", 10);
    assert!(
        completions.is_empty(),
        "Should return empty for empty database"
    );
}

#[test]
fn test_backend_format_detects_geometric() {
    use mirage::storage::BackendFormat;

    let (_temp, geo_path) = create_temp_geo_file();

    let format = BackendFormat::detect(&geo_path).unwrap();
    assert_eq!(
        format,
        BackendFormat::Geometric,
        "Should detect as Geometric"
    );
}

#[test]
fn test_backend_format_detects_sqlite() {
    use mirage::storage::BackendFormat;
    use rusqlite::Connection;

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create a minimal SQLite database
    {
        let mut conn = Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE test (id INTEGER)", []).unwrap();
    }

    let format = BackendFormat::detect(&db_path).unwrap();
    assert_eq!(format, BackendFormat::SQLite, "Should detect as SQLite");
}

#[test]
fn test_backend_format_unknown_for_nonexistent() {
    use mirage::storage::BackendFormat;

    let temp_dir = TempDir::new().unwrap();
    let nonexistent = temp_dir.path().join("nonexistent.geo");

    let format = BackendFormat::detect(&nonexistent).unwrap();
    assert_eq!(
        format,
        BackendFormat::Unknown,
        "Should return Unknown for nonexistent file"
    );
}
