use crate::cli::{resolve_db_path, CfgArgs, CfgFormat, Cli, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn cfg(args: &CfgArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::load_cfg_from_db;
    use crate::cfg::{export_dot, export_json, CFGExport};
    use crate::storage::resolve_function_or_semantic;
    use crate::storage::MirageDb;

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Open database (follows status command pattern for error handling)
    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(_e) => {
            // JSON-aware error handling with remediation
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::database_not_found(&db_path);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to open database: {}", db_path));
                output::info("Hint: Run 'magellan watch' to create the database");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Resolve function name/ID or semantic query to function_id (with optional file filter)
    let function_id = match resolve_function_or_semantic(
        &db,
        &args.function,
        args.semantic_query.as_deref(),
        args.file.as_deref(),
    ) {
        Ok(id) => id,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "FunctionNotFound",
                    &format!("{}", e),
                    output::E_CFG_ERROR,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("{}", e));
                output::info(&format!("Hint: {}", output::R_HINT_LIST_FUNCTIONS));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Load CFG from database
    let cfg = match load_cfg_from_db(&db, function_id) {
        Ok(cfg) => cfg,
        Err(_e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "CgfLoadError",
                    &format!("Failed to load CFG for function '{}'", args.function),
                    output::E_CFG_ERROR,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!(
                    "Failed to load CFG for function '{}'",
                    args.function
                ));
                output::info("The function may be corrupted. Try re-running 'magellan watch'");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Query coverage data (Magellan v3.1.6+ tables; graceful fallback if absent)
    let coverage: Option<std::collections::HashMap<i64, i64>> = db.conn().ok().and_then(|conn| {
        let sql = "SELECT block_id, hit_count FROM cfg_block_coverage \
                   WHERE block_id IN (SELECT id FROM cfg_blocks WHERE function_id = ?1)";
        let mut stmt = conn.prepare(sql).ok()?;
        let rows = stmt.query_map([function_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        });
        let mut map = std::collections::HashMap::new();
        if let Ok(iter) = rows {
            for (block_id, hit_count) in iter.flatten() {
                map.insert(block_id, hit_count);
            }
        }
        if map.is_empty() {
            None
        } else {
            Some(map)
        }
    });

    // Determine output format (args.format overrides cli.output)
    let format = args.format.unwrap_or(match cli.output {
        OutputFormat::Human => CfgFormat::Human,
        OutputFormat::Json => CfgFormat::Json,
        OutputFormat::Pretty => CfgFormat::Json,
    });

    match format {
        CfgFormat::Human | CfgFormat::Dot => {
            // Both Human and Dot use DOT format
            let dot = export_dot(&cfg);
            println!("{}", dot);
        }
        CfgFormat::Json => {
            // Export to JSON and wrap in JsonResponse for consistency
            let export: CFGExport = export_json(&cfg, &args.function, coverage.as_ref());
            let response = output::JsonResponse::new(export);

            match cli.output {
                OutputFormat::Json => println!("{}", response.to_json()),
                OutputFormat::Pretty => println!("{}", response.to_pretty_json()),
                OutputFormat::Human => println!("{}", response.to_pretty_json()),
            }
        }
    }

    Ok(())
}

#[cfg(test)]
pub(crate) fn create_test_cfg() -> crate::cfg::Cfg {
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec!["let x = 1".to_string()],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["if x > 0".to_string()],
        terminator: Terminator::SwitchInt {
            targets: vec![2],
            otherwise: 3,
        },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["return true".to_string()],
        terminator: Terminator::Return,
        source_location: None,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["return false".to_string()],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);
    g.add_edge(b1, b3, EdgeType::FalseBranch);

    g
}
