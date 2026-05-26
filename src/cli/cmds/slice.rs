use crate::cli::{resolve_db_path, Cli, OutputFormat, SliceArgs, SliceDirectionArg};
use crate::output;
use anyhow::Result;

pub fn slice(args: &SliceArgs, cli: &Cli) -> Result<()> {
    use crate::analysis::{MagellanBridge, SliceWrapper};

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Open Magellan database
    let bridge = match MagellanBridge::open(&db_path) {
        Ok(bridge) => bridge,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "DatabaseError",
                    &format!("Failed to open Magellan database: {}", e),
                    output::E_DATABASE_NOT_FOUND,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to open Magellan database: {}", e));
                output::info("Note: Program slicing requires a Magellan code graph database");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Perform the slice based on direction
    let slice_result: SliceWrapper = match args.direction {
        SliceDirectionArg::Backward => bridge.backward_slice(&args.symbol)?,
        SliceDirectionArg::Forward => bridge.forward_slice(&args.symbol)?,
    };

    // Output based on format
    match cli.output {
        OutputFormat::Human => {
            println!("Program Slice: {}", slice_result.direction);
            println!();

            // Target symbol
            println!("Target:");
            println!(
                "  Symbol: {}",
                slice_result.target.fqn.as_deref().unwrap_or(&args.symbol)
            );
            println!("  Kind: {}", slice_result.target.kind);
            println!("  File: {}", slice_result.target.file_path);
            println!();

            // Statistics
            println!("Statistics:");
            println!("  Total symbols in slice: {}", slice_result.symbol_count);
            println!(
                "  Data dependencies: {}",
                slice_result.statistics.data_dependencies
            );
            println!(
                "  Control dependencies: {}",
                slice_result.statistics.control_dependencies
            );
            println!();

            // Included symbols (verbose only)
            if args.verbose {
                println!(
                    "Included symbols ({}):",
                    slice_result.included_symbols.len()
                );
                for (i, symbol) in slice_result.included_symbols.iter().enumerate() {
                    println!(
                        "  {}. {}",
                        i + 1,
                        symbol.fqn.as_deref().unwrap_or("<unknown>")
                    );
                    println!("     Kind: {}, File: {}", symbol.kind, symbol.file_path);
                }
            } else {
                println!("Use --verbose to see all included symbols");
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let wrapper = output::JsonResponse::new(slice_result);
            match cli.output {
                OutputFormat::Json => println!("{}", wrapper.to_json()),
                OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}
