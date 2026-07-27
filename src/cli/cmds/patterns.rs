use crate::cli::responses::*;
use crate::cli::{resolve_db_path, Cli, OutputFormat, PatternsArgs};
use crate::output;
use anyhow::Result;

pub fn patterns(args: &PatternsArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::load_cfg_from_db;
    use crate::cfg::{detect_if_else_patterns, detect_match_patterns};
    use crate::storage::resolve_function_or_semantic;

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Open database (honest open-error handling via shared helper)
    let db = super::open_db_or_exit(cli, &db_path);

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

    // Detect patterns based on filter flags
    let show_if_else = !args.r#match; // Show if/else unless --match only
    let show_match = !args.if_else; // Show match unless --if-else only

    let if_else_patterns = if show_if_else {
        detect_if_else_patterns(&cfg)
    } else {
        vec![]
    };

    let match_patterns = if show_match {
        detect_match_patterns(&cfg)
    } else {
        vec![]
    };

    // Convert to response format
    let if_else_infos: Vec<IfElseInfo> = if_else_patterns
        .iter()
        .map(|p| IfElseInfo {
            condition_block: cfg[p.condition].id,
            true_branch: cfg[p.true_branch].id,
            false_branch: cfg[p.false_branch].id,
            merge_point: p.merge_point.map(|n| cfg[n].id),
            has_else: p.has_else(),
        })
        .collect();

    let match_infos: Vec<MatchInfo> = match_patterns
        .iter()
        .map(|p| MatchInfo {
            switch_block: cfg[p.switch_node].id,
            branch_count: p.branch_count(),
            targets: p.targets.iter().map(|n| cfg[*n].id).collect(),
            otherwise: cfg[p.otherwise].id,
        })
        .collect();

    // Output based on format
    match cli.output {
        OutputFormat::Human => {
            println!("Function: {}", args.function);
            println!();

            if show_if_else {
                println!("If/Else Patterns: {}", if_else_patterns.len());
                if if_else_patterns.is_empty() {
                    output::info("No if/else patterns detected");
                } else {
                    for (i, info) in if_else_infos.iter().enumerate() {
                        println!("  Pattern {}:", i + 1);
                        println!("    Condition: Block {}", info.condition_block);
                        println!("    True branch: Block {}", info.true_branch);
                        println!("    False branch: Block {}", info.false_branch);
                        if let Some(merge) = info.merge_point {
                            println!("    Merge point: Block {}", merge);
                            println!("    Has else: {}", info.has_else);
                        } else {
                            println!("    Merge point: None (no else)");
                        }
                        println!();
                    }
                }
                println!();
            }

            if show_match {
                println!("Match Patterns: {}", match_patterns.len());
                if match_patterns.is_empty() {
                    output::info("No match patterns detected");
                } else {
                    for (i, info) in match_infos.iter().enumerate() {
                        println!("  Pattern {}:", i + 1);
                        println!("    Switch: Block {}", info.switch_block);
                        println!("    Branch count: {}", info.branch_count);
                        println!("    Targets: {:?}", info.targets);
                        println!("    Otherwise: Block {}", info.otherwise);
                        println!();
                    }
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let response = PatternsResponse {
                function: args.function.clone(),
                if_else_count: if_else_patterns.len(),
                match_count: match_patterns.len(),
                if_else_patterns: if_else_infos,
                match_patterns: match_infos,
            };
            let wrapper = output::JsonResponse::new(response);
            match cli.output {
                OutputFormat::Json => println!("{}", wrapper.to_json()),
                OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}
