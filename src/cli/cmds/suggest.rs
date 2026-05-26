use crate::cli::{resolve_db_path, Cli, OutputFormat, SuggestArgs};
use crate::output;
use anyhow::Result;

pub fn suggest(args: &SuggestArgs, cli: &Cli) -> Result<()> {
    use crate::analysis::suggest;
    use crate::storage::MirageDb;

    let db_path = resolve_db_path(cli.db.clone())?;
    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::database_not_found(&db_path);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to open database: {}", e));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    let function_id = match crate::cfg::resolve_function_name_with_file(
        &db,
        &args.symbol,
        args.file.as_deref(),
    ) {
        Ok(id) => id,
        Err(_) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::function_not_found(&args.symbol);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Symbol '{}' not found", args.symbol));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    let report =
        suggest::compute_suggestions(&db, function_id, &args.symbol, args.file.as_deref())?;

    match cli.output {
        OutputFormat::Human => {
            println!("Suggestions for: {}", report.symbol);
            if let Some(ref fp) = report.file_path {
                println!("  File: {}", fp);
            }
            println!();
            for s in &report.suggestions {
                println!(
                    "  [{}] {} ({})",
                    s.severity.to_uppercase(),
                    s.message,
                    s.kind
                );
                if let Some(ref detail) = s.detail {
                    println!("    {}", detail);
                }
            }
        }
        OutputFormat::Json => {
            let response = output::JsonResponse::new(&report);
            println!("{}", response.to_json());
        }
        OutputFormat::Pretty => {
            let response = output::JsonResponse::new(&report);
            println!("{}", response.to_pretty_json());
        }
    }

    Ok(())
}
