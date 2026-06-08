use crate::cli::{resolve_db_path, Cli, HotpathsArgs, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn hotpaths(args: &HotpathsArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::{
        detect_natural_loops, enumerate_paths, find_entry,
        hotpaths::{compute_hot_paths, HotpathsOptions},
        PathLimits,
    };
    use crate::storage::MirageDb;

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Open database (follows status command pattern for error handling)
    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(_e) => {
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

    // Resolve function name/ID or semantic query to function_id
    let function_id = match crate::storage::resolve_function_or_semantic(
        &db,
        &args.function,
        args.semantic_query.as_deref(),
        None,
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
    let cfg = match db.load_cfg(function_id) {
        Ok(cfg) => cfg,
        Err(_e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "CfgLoadError",
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

    // Find entry block
    let entry = match find_entry(&cfg) {
        Some(entry) => entry,
        None => {
            output::error(&format!(
                "No entry block found for function '{}'",
                args.function
            ));
            std::process::exit(output::EXIT_DATABASE);
        }
    };

    // Detect natural loops
    let natural_loops = detect_natural_loops(&cfg);

    // Enumerate all paths with default limits
    // Note: HotpathsArgs uses 'top' for number of results, not path enumeration limits
    let limits = PathLimits::default();
    let paths = enumerate_paths(&cfg, &limits);

    if paths.is_empty() {
        output::info(&format!("No paths found for function '{}'", args.function));
        return Ok(());
    }

    // Compute hot paths
    let options = HotpathsOptions {
        top_n: args.top,
        include_rationale: args.rationale,
    };

    let mut hot_paths = match compute_hot_paths(&cfg, &paths, entry, &natural_loops, options) {
        Ok(hp) => hp,
        Err(e) => {
            output::error(&format!("Failed to compute hot paths: {}", e));
            std::process::exit(output::EXIT_DATABASE);
        }
    };

    // Apply minimum score filter if specified
    if let Some(min_score) = args.min_score {
        hot_paths.retain(|hp| hp.hotness_score >= min_score);
    }

    // Output based on format
    match cli.output {
        OutputFormat::Human => {
            print_hotpaths_human(&hot_paths, args.rationale);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&hot_paths)?);
        }
        OutputFormat::Pretty => {
            println!("{}", serde_json::to_string_pretty(&hot_paths)?);
        }
    }

    Ok(())
}

fn print_hotpaths_human(hot_paths: &[crate::cfg::hotpaths::HotPath], show_rationale: bool) {
    use crate::output;

    output::header(&format!("Hot Paths (top {})", hot_paths.len()));

    if hot_paths.is_empty() {
        output::info("No hot paths found");
        return;
    }

    for (i, hp) in hot_paths.iter().enumerate() {
        println!(
            "\n{}. Path {} - Score: {:.2}",
            i + 1,
            hp.path_id,
            hp.hotness_score
        );

        if show_rationale && !hp.rationale.is_empty() {
            println!("   Rationale:");
            for r in &hp.rationale {
                println!("     - {}", r);
            }
        }

        println!("   Blocks: {} blocks", hp.blocks.len());
        for (j, block) in hp.blocks.iter().enumerate() {
            if j < 5 || j == hp.blocks.len() - 1 {
                print!("     {}", block);
                if j == 4 && hp.blocks.len() > 6 {
                    println!(" ... (+{} more)", hp.blocks.len() - 6);
                    break;
                } else {
                    println!();
                }
            }
        }
    }
}
