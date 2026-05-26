// Mirage: Path-Aware Code Intelligence Engine
//
// A control-flow and logic graph engine for multi-language codebases.
// Extracts CFGs from Magellan (Rust via MIR, C/C++ via AST), enumerates execution paths,
// and provides graph-based reasoning capabilities.

use anyhow::Result;
use clap::Parser;

use mirage::analysis::telemetry::{is_telemetry_enabled, TelemetryGuard};
use mirage::cli::{Cli, Commands, OutputFormat};
use mirage::storage::BackendFormat;

fn command_name(cmd: &Option<Commands>) -> &'static str {
    match cmd {
        None => "none",
        Some(Commands::Status(_)) => "status",
        Some(Commands::Paths(_)) => "paths",
        Some(Commands::Cfg(_)) => "cfg",
        Some(Commands::Dominators(_)) => "dominators",
        Some(Commands::Loops(_)) => "loops",
        Some(Commands::Unreachable(_)) => "unreachable",
        Some(Commands::Patterns(_)) => "patterns",
        Some(Commands::Frontiers(_)) => "frontiers",
        Some(Commands::Verify(_)) => "verify",
        Some(Commands::BlastZone(_)) => "blast-zone",
        Some(Commands::Cycles(_)) => "cycles",
        Some(Commands::Slice(_)) => "slice",
        Some(Commands::Hotspots(_)) => "hotspots",
        Some(Commands::Hotpaths(_)) => "hotpaths",
        Some(Commands::Diff(_)) => "diff",
        Some(Commands::Icfg(_)) => "icfg",
        Some(Commands::Coverage(_)) => "coverage",
        Some(Commands::Migrate(_)) => "migrate",
        Some(Commands::Docs(_)) => "docs",
        Some(Commands::Risk(_)) => "risk",
        Some(Commands::Suggest(_)) => "suggest",
        Some(Commands::Stats(_)) => "stats",
    }
}

fn main() -> Result<()> {
    // Check platform and warn about limitations
    mirage::platform::check_platform_support();

    let cli = Cli::parse();

    let telemetry_enabled = is_telemetry_enabled(cli.record);
    let _telemetry = TelemetryGuard::new(telemetry_enabled)?;

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    // Run the appropriate command
    let cmd_name = command_name(&cli.command);
    run_command(cli)?;
    _telemetry.record(cmd_name, None);

    Ok(())
}

fn run_command(cli: Cli) -> Result<()> {
    // Handle --detect-backend flag before command dispatch
    if cli.detect_backend {
        let db_str = cli
            .db
            .ok_or_else(|| anyhow::anyhow!("--db required for --detect-backend"))?;
        let db_path = std::path::Path::new(&db_str);

        let format = BackendFormat::detect(db_path)
            .map_err(|e| anyhow::anyhow!("Backend detection failed: {}", e))?;

        let backend_str = match format {
            BackendFormat::SQLite => "sqlite",
            BackendFormat::Geometric => "geometric",
            BackendFormat::Unknown => "unknown",
        };

        if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
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
        None => Err(anyhow::anyhow!(
            "No subcommand provided. Use --help for usage information."
        )),
        Some(ref cmd) => match cmd {
            Commands::Status(args) => mirage::cli::cmds::status(args, &cli),
            Commands::Paths(ref args) => mirage::cli::cmds::paths(args, &cli),
            Commands::Cfg(ref args) => mirage::cli::cmds::cfg(args, &cli),
            Commands::Dominators(ref args) => mirage::cli::cmds::dominators(args, &cli),
            Commands::Loops(ref args) => mirage::cli::cmds::loops(args, &cli),
            Commands::Unreachable(ref args) => mirage::cli::cmds::unreachable(args, &cli),
            Commands::Patterns(ref args) => mirage::cli::cmds::patterns(args, &cli),
            Commands::Frontiers(ref args) => mirage::cli::cmds::frontiers(args, &cli),
            Commands::Verify(ref args) => mirage::cli::cmds::verify(args, &cli),
            Commands::BlastZone(ref args) => mirage::cli::cmds::blast_zone(args, &cli),
            Commands::Cycles(ref args) => mirage::cli::cmds::cycles(args, &cli),
            Commands::Slice(ref args) => mirage::cli::cmds::slice(args, &cli),
            Commands::Hotspots(ref args) => mirage::cli::cmds::hotspots(args, &cli),
            Commands::Hotpaths(ref args) => mirage::cli::cmds::hotpaths(args, &cli),
            Commands::Diff(ref args) => mirage::cli::cmds::diff(args, &cli),
            Commands::Icfg(ref args) => mirage::cli::cmds::icfg(args, &cli),
            Commands::Coverage(ref args) => mirage::cli::cmds::coverage(args, &cli),
            Commands::Migrate(ref args) => mirage::cli::cmds::migrate(args, &cli),
            Commands::Docs(ref args) => mirage::cli::cmds::docs(args, &cli),
            Commands::Risk(ref args) => mirage::cli::cmds::risk(args, &cli),
            Commands::Suggest(ref args) => mirage::cli::cmds::suggest(args, &cli),
            Commands::Stats(ref args) => mirage::cli::cmds::stats(args, &cli),
        },
    }
}
