#![allow(deprecated)]

use mirage::mir::charon_llbc::Crate;
use mirage::mir::translator::translate_function;
use mirage::storage::{store_cfg, MirageDb};
use rusqlite::Connection;
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_full_mir_to_db_flow() {
    // 1. Load and parse LLBC
    let json_content = fs::read_to_string("tests/data/add.llbc").expect("Failed to read test data");
    let llbc_crate: Crate =
        serde_json::from_str(&json_content).expect("Failed to deserialize LLBC JSON");

    let fun_decl = &llbc_crate.fun_decls[0];
    assert_eq!(fun_decl.name, vec!["test_crate", "add"]);

    // 2. Translate to CFG
    let cfg = translate_function(fun_decl, &llbc_crate.files);
    assert_eq!(cfg.node_count(), 1, "add.llbc should have 1 basic block");

    // 3. Setup temporary database
    let temp_db = NamedTempFile::new().expect("Failed to create temp db");
    let db_path = temp_db.path();

    {
        let mut conn = Connection::open(db_path).expect("Failed to open connection");

        // Initialize Magellan-like schema
        conn.execute_batch(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT
            );
            CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL
            );
            INSERT INTO magellan_meta (id, magellan_schema_version) VALUES (1, 7);",
        )
        .expect("Failed to initialize Magellan schema");

        // Initialize Mirage schema
        mirage::storage::create_schema(&mut conn, 7)
            .expect("Failed to create Mirage schema");

        // Add function entity
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES ('Symbol', 'add', 'tests/data/add.rs', '{\"kind\": \"Function\"}')",
            []
        ).expect("Failed to insert function entity");

        let function_id = conn.last_insert_rowid();

        // 4. Store CFG
        #[allow(deprecated)]
        store_cfg(&mut conn, function_id, "dummy_hash", &cfg).expect("Failed to store CFG");
    }

    // 5. Load and Verify using MirageDb
    let db = MirageDb::open(db_path).expect("Failed to open MirageDb");
    let function_id = db
        .resolve_function_name("add")
        .expect("Failed to resolve function");

    let loaded_cfg = db.load_cfg(function_id).expect("Failed to load CFG");

    assert_eq!(loaded_cfg.node_count(), cfg.node_count());

    // Verify block details
    for (idx, node_idx) in loaded_cfg.node_indices().enumerate() {
        let loaded_block = loaded_cfg.node_weight(node_idx).unwrap();
        let original_block = cfg
            .node_weight(cfg.node_indices().nth(idx).unwrap())
            .unwrap();

        assert_eq!(loaded_block.kind, original_block.kind);
        // Terminator might be simplified in database
        // Original add.llbc terminator is Return, which is preserved.
    }
}
