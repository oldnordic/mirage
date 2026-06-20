use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};
use petgraph::graph::{DiGraph, NodeIndex};

pub(super) fn create_linear_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::Fallthrough);

    g
}

pub(super) fn create_diamond_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![1],
            otherwise: 2,
        },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::TrueBranch);
    g.add_edge(b0, b2, EdgeType::FalseBranch);
    g.add_edge(b1, b3, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);

    g
}

pub(super) fn create_loop_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![2],
            otherwise: 3,
        },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);
    g.add_edge(b1, b3, EdgeType::FalseBranch);
    g.add_edge(b2, b1, EdgeType::LoopBack);

    g
}

pub(super) fn create_error_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Abort("panic!".to_string()),
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);

    g
}

pub(super) fn create_unreachable_term_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Unreachable,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);

    g
}

pub(super) fn create_dead_code_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let _b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    let _b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g
}

pub(super) fn create_call_unwind_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Call {
            target: Some(1),
            unwind: Some(2),
        },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    let _b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);

    g
}

pub(super) fn create_self_loop_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let _b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    g.add_edge(b0, NodeIndex::new(1), EdgeType::Fallthrough);

    g
}

pub(super) fn create_nested_loop_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![2],
            otherwise: 4,
        },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![3],
            otherwise: 1,
        },
        source_location: None,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
    });

    let b4 = g.add_node(BasicBlock {
        id: 4,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);
    g.add_edge(b1, b4, EdgeType::FalseBranch);
    g.add_edge(b2, b3, EdgeType::TrueBranch);
    g.add_edge(b2, b1, EdgeType::LoopBack);
    g.add_edge(b3, b2, EdgeType::LoopBack);

    g
}

pub(super) fn create_conflicting_conditions_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![1],
            otherwise: 2,
        },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![3],
            otherwise: 3,
        },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::TrueBranch);
    g.add_edge(b0, b2, EdgeType::FalseBranch);
    g.add_edge(b1, b3, EdgeType::Fallthrough);

    g
}

pub(super) fn create_large_linear_cfg(size: usize) -> Cfg {
    let mut g = DiGraph::new();

    for i in 0..size {
        let kind = if i == 0 {
            BlockKind::Entry
        } else if i == size - 1 {
            BlockKind::Exit
        } else {
            BlockKind::Normal
        };

        let terminator = if i == size - 1 {
            Terminator::Return
        } else {
            Terminator::Goto { target: i + 1 }
        };

        let _node = g.add_node(BasicBlock {
            id: i,
            db_id: None,
            kind,
            statements: vec![],
            terminator,
            source_location: None,
        });
    }

    for i in 0..size - 1 {
        let from = NodeIndex::new(i);
        let to = NodeIndex::new(i + 1);
        g.add_edge(from, to, EdgeType::Fallthrough);
    }

    g
}

pub(super) fn create_large_diamond_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let mut nodes = Vec::new();

    for i in 0..21 {
        let kind = if i == 0 {
            BlockKind::Entry
        } else if i % 2 == 0 && i > 0 {
            BlockKind::Normal
        } else if i == 20 {
            BlockKind::Exit
        } else {
            BlockKind::Normal
        };

        let terminator = if i == 20 {
            Terminator::Return
        } else if i % 2 == 0 {
            let target1 = i + 1;
            let target2 = i + 2;
            Terminator::SwitchInt {
                targets: vec![target1],
                otherwise: target2,
            }
        } else {
            let merge = i + 1;
            Terminator::Goto { target: merge }
        };

        let node = g.add_node(BasicBlock {
            id: i,
            db_id: None,
            kind,
            statements: vec![],
            terminator,
            source_location: None,
        });
        nodes.push(node);
    }

    for i in (0..20).step_by(2) {
        let from = nodes[i];
        let to1 = nodes[i + 1];
        let to2 = nodes[i + 2];
        g.add_edge(from, to1, EdgeType::TrueBranch);
        g.add_edge(from, to2, EdgeType::FalseBranch);
    }

    for i in (1..20).filter(|x| x % 2 == 1) {
        let from = nodes[i];
        let to = nodes[i + 1];
        g.add_edge(from, to, EdgeType::Fallthrough);
    }

    g
}

pub(super) fn create_simple_diamond_cfg() -> Cfg {
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![1],
            otherwise: 2,
        },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::TrueBranch);
    g.add_edge(b0, b2, EdgeType::FalseBranch);
    g.add_edge(b1, b3, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);

    g
}

pub(super) fn setup_test_db() -> rusqlite::Connection {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();

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
         VALUES (1, 4, 3, 0)",
        [],
    )
    .unwrap();

    crate::storage::create_schema(&mut conn, crate::storage::TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
        rusqlite::params!("function", "test_func", "test.rs", "{}"),
    )
    .unwrap();

    conn
}
