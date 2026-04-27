use mirage_analyzer::cfg::summary::{summarize_path, PathSummarizer};
use mirage_analyzer::cfg::{BasicBlock, BlockKind, Cfg, Path, PathKind, Terminator};

#[test]
fn test_summary_empty_statements() {
    let blocks = vec![
        BasicBlock {
            id: 0,
            db_id: None,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        },
        BasicBlock {
            id: 1,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        },
    ];

    let summary = PathSummarizer::summarize(&blocks);
    // Current behavior: Entry block adds "[Entry]", Return block adds "[Return]"
    assert_eq!(summary, "[Entry] -> [Return]");
}

#[test]
fn test_summary_all_empty() {
    let blocks = vec![
        BasicBlock {
            id: 0,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        },
        BasicBlock {
            id: 1,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        },
    ];

    let summary = PathSummarizer::summarize(&blocks);
    // New behavior: returns "no logical effects"
    println!("Summary for all empty: '{}'", summary);
    assert_eq!(summary, "no logical effects");
}

#[test]
fn test_summarize_path_empty_summary() {
    let mut cfg = Cfg::new();
    let _b0 = cfg.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });
    let _b1 = cfg.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let path = Path::new(vec![0, 1], PathKind::Normal);

    let summary = summarize_path(&cfg, &path);
    println!("Path summary: '{}'", summary);
    assert_eq!(summary, "no logical effects (2 blocks)");
}
