use crate::cli::responses::*;
use crate::cli::{resolve_db_path, Cli, LoopsArgs, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn loops(args: &LoopsArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::detect_natural_loops;
    use crate::cfg::{load_cfg_from_db, resolve_function_name_with_file};
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

    // Resolve function name/ID to function_id (with optional file filter)
    let function_id =
        match resolve_function_name_with_file(&db, &args.function, args.file.as_deref()) {
            Ok(id) => id,
            Err(_e) => {
                if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                    let error = output::JsonError::function_not_found(&args.function);
                    let wrapper = output::JsonResponse::new(error);
                    println!("{}", wrapper.to_json());
                    std::process::exit(output::EXIT_DATABASE);
                } else {
                    output::error(&format!(
                        "Function '{}' not found in database",
                        args.function
                    ));
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

    // Detect natural loops
    let natural_loops = detect_natural_loops(&cfg);

    // Compute nesting levels for each loop
    let loop_infos: Vec<LoopInfo> = natural_loops
        .iter()
        .map(|loop_| {
            let nesting_level = loop_.nesting_level(&natural_loops);
            let body_blocks: Vec<usize> = loop_.body.iter().map(|&node| cfg[node].id).collect();
            LoopInfo {
                header: cfg[loop_.header].id,
                back_edge_from: cfg[loop_.back_edge.0].id,
                body_size: loop_.size(),
                nesting_level,
                body_blocks,
            }
        })
        .collect();

    // Output based on format
    match cli.output {
        OutputFormat::Human => {
            println!("Function: {}", args.function);
            println!("Natural Loops: {}", natural_loops.len());
            println!();

            if natural_loops.is_empty() {
                output::info("No natural loops detected in this function");
            } else {
                for (i, loop_info) in loop_infos.iter().enumerate() {
                    println!("Loop {}:", i + 1);
                    println!("  Header: Block {}", loop_info.header);
                    println!("  Back edge from: Block {}", loop_info.back_edge_from);
                    println!("  Body size: {} blocks", loop_info.body_size);
                    println!("  Nesting level: {}", loop_info.nesting_level);

                    if args.verbose {
                        println!("  Body blocks: {:?}", loop_info.body_blocks);
                    }
                    println!();
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let response = LoopsResponse {
                function: args.function.clone(),
                loop_count: natural_loops.len(),
                loops: loop_infos,
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
