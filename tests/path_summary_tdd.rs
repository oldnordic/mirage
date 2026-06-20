use mirage::cfg::summary::PathSummarizer;
use mirage::cfg::{BasicBlock, BlockKind, Terminator};

#[test]
fn test_noise_reduction() {
    let block = BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![
            "StorageLive(Local(1))".to_string(),
            "Assign(Local(1), Constant(42))".to_string(),
            "StorageDead(Local(1))".to_string(),
            "Nop".to_string(),
        ],
        terminator: Terminator::Return,
        source_location: None,
    };

    let summary = PathSummarizer::summarize_block(&block);
    assert!(summary.contains("State: Local(1) = 42"));
    assert!(!summary.contains("StorageLive"), "Noise should be filtered");
    assert!(!summary.contains("Nop"), "Noise should be filtered");
}

#[test]
fn test_call_extraction() {
    let block = BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["Assign(Local(2), Call(auth::verify, [Local(1)]))".to_string()],
        terminator: Terminator::Return,
        source_location: None,
    };

    let summary = PathSummarizer::summarize_block(&block);
    assert!(
        summary.contains("[Call: auth::verify]"),
        "Should distill calls to a clear action signal"
    );
}

#[test]
fn test_path_linearization() {
    let b1 = BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec!["Assign(Local(0), Constant(true))".to_string()],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    };

    let b2 = BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    };

    let full_summary = PathSummarizer::summarize(&[b1, b2]);
    assert_eq!(
        full_summary,
        "[Entry] -> [State: Local(0) = true] -> [Return]"
    );
}

#[test]
fn test_robustness() {
    let block = BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![
            "  Assign(  Local(1)  ,   Constant(  42  )  )  ".to_string(),
            "Assign(Local(2), Call(auth::verify, [Local(1), Constant(\"token\")]))".to_string(),
        ],
        terminator: Terminator::Return,
        source_location: None,
    };

    let summary = PathSummarizer::summarize_block(&block);
    assert!(summary.contains("[State: Local(1) = 42]"));
    assert!(summary.contains("[Call: auth::verify]"));
}

#[test]
fn test_fakeread_filter() {
    let block = BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["FakeRead(ForLet, Local(1))".to_string()],
        terminator: Terminator::Return,
        source_location: None,
    };

    let summary = PathSummarizer::summarize_block(&block);
    assert!(
        !summary.contains("FakeRead"),
        "FakeRead should be filtered out"
    );
}
