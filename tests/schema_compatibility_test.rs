use mirage::MirageDb;
use rusqlite::Connection;

fn create_test_magellan_db(schema_version: i32) -> tempfile::NamedTempFile {
    let db = tempfile::NamedTempFile::new().unwrap();
    let conn = Connection::open(db.path()).unwrap();

    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
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
        "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
         VALUES (1, ?1, 3, 0)",
        [schema_version],
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
        "CREATE TABLE cfg_blocks (
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
            cfg_hash TEXT,
            statements TEXT,
            cfg_condition TEXT,
            FOREIGN KEY (function_id) REFERENCES graph_entities(id)
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            edge_type TEXT NOT NULL,
            data TEXT
        )",
        [],
    )
    .unwrap();

    db
}

#[test]
fn test_mirage_opens_magellan_schema_v18_database() {
    let db = create_test_magellan_db(18);

    let mirage_db = MirageDb::open(db.path()).expect("schema v18 database should open");
    let status = mirage_db.status().expect("status should load");

    assert_eq!(status.magellan_schema_version, 18);
    assert_eq!(status.mirage_schema_version, 1);
}
