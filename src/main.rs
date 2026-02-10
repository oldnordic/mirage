// Mirage: Path-Aware Code Intelligence Engine
//
// A control-flow and logic graph engine for Rust codebases.
// Extracts MIR from rustc, builds CFGs, enumerates execution paths,
// and provides graph-based reasoning capabilities.

#![allow(dead_code)]

// Compile-time guard: prevent enabling both backends simultaneously
#[cfg(all(feature = "sqlite", feature = "native-v2"))]
compile_error!(
    "Features 'sqlite' and 'native-v2' are mutually exclusive. \
     Enable only one backend feature. Remove one of: \
     --features sqlite \
     --features native-v2 \
     \
     Default is SQLite, so use `cargo build` with no features, or \
     `cargo build --features native-v2` for the native-v2 backend."
);

use clap::Parser;
use anyhow::Result;

mod analysis;
mod cli;
mod cfg;
mod mir;
mod output;
mod platform;
mod storage;

use cli::{Cli, Commands};

fn main() -> Result<()> {
    // Check platform and warn about limitations
    platform::check_platform_support();

    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    // Run the appropriate command
    run_command(cli)?;

    Ok(())
}

fn run_command(cli: Cli) -> Result<()> {
    // Handle --detect-backend flag before command dispatch
    if cli.detect_backend {
        let db_str = cli.db.ok_or_else(|| anyhow::anyhow!("--db required for --detect-backend"))?;
        let db_path = std::path::Path::new(&db_str);

        use magellan::migrate_backend_cmd::{detect_backend_format, BackendFormat};
        let format = detect_backend_format(db_path)
            .map_err(|e| anyhow::anyhow!("Backend detection failed: {}", e))?;

        let backend_str = match format {
            BackendFormat::Sqlite => "sqlite",
            BackendFormat::NativeV2 => "native-v2",
        };

        if matches!(cli.output, cli::OutputFormat::Json | cli::OutputFormat::Pretty) {
            let output = serde_json::json!({
                "backend": backend_str,
                "database": db_str,
            });
            println!("{}", serde_json::to_string(&output)?);
        } else {
            println!("{}", backend_str);
        }
        return Ok(());
    }

    match cli.command {
        None => {
            Err(anyhow::anyhow!("No subcommand provided. Use --help for usage information."))
        }
        Some(ref cmd) => match cmd {
            Commands::Status(args) => cli::cmds::status(args, &cli),
            Commands::Paths(ref args) => cli::cmds::paths(args, &cli),
            Commands::Cfg(ref args) => cli::cmds::cfg(args, &cli),
            Commands::Dominators(ref args) => cli::cmds::dominators(args, &cli),
            Commands::Loops(ref args) => cli::cmds::loops(args, &cli),
            Commands::Unreachable(ref args) => cli::cmds::unreachable(args, &cli),
            Commands::Patterns(ref args) => cli::cmds::patterns(args, &cli),
            Commands::Frontiers(ref args) => cli::cmds::frontiers(args, &cli),
            Commands::Verify(ref args) => cli::cmds::verify(args, &cli),
            Commands::BlastZone(ref args) => cli::cmds::blast_zone(args, &cli),
            Commands::Cycles(ref args) => cli::cmds::cycles(args, &cli),
            Commands::Slice(ref args) => cli::cmds::slice(args, &cli),
            Commands::Hotspots(ref args) => cli::cmds::hotspots(args, &cli),
            Commands::Migrate(ref args) => cli::cmds::migrate(args, &cli),
        },
    }
}
