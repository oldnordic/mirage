// Mirage: Path-Aware Code Intelligence Engine
//
// A control-flow and logic graph engine for Rust codebases.
// Extracts MIR from rustc, builds CFGs, enumerates execution paths,
// and provides graph-based reasoning capabilities.

#![allow(dead_code)]

use clap::Parser;
use anyhow::Result;

mod analysis;
mod cli;
mod cfg;
mod mir;
mod output;
mod storage;

use cli::{Cli, Commands};

fn main() -> Result<()> {
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
    match cli.command {
        Commands::Index(ref args) => cli::cmds::index(args, &cli)?,
        Commands::Status(args) => cli::cmds::status(args, &cli)?,
        Commands::Paths(ref args) => cli::cmds::paths(args, &cli)?,
        Commands::Cfg(ref args) => cli::cmds::cfg(args, &cli)?,
        Commands::Dominators(ref args) => cli::cmds::dominators(args, &cli)?,
        Commands::Loops(ref args) => cli::cmds::loops(args, &cli)?,
        Commands::Unreachable(ref args) => cli::cmds::unreachable(args, &cli)?,
        Commands::Patterns(ref args) => cli::cmds::patterns(args, &cli)?,
        Commands::Frontiers(ref args) => cli::cmds::frontiers(args, &cli)?,
        Commands::Verify(ref args) => cli::cmds::verify(args, &cli)?,
        Commands::BlastZone(ref args) => cli::cmds::blast_zone(args, &cli)?,
        Commands::Cycles(ref args) => cli::cmds::cycles(args, &cli)?,
        _ => {
            // Placeholder for unimplemented commands
            eprintln!("This command is not yet implemented");
            std::process::exit(1);
        }
    }
    Ok(())
}
