// Mirage: Path-Aware Code Intelligence Engine
//
// A control-flow and logic graph engine for Rust codebases.
// Extracts MIR from rustc, builds CFGs, enumerates execution paths,
// and provides graph-based reasoning capabilities.

#![allow(dead_code)]

use clap::Parser;
use anyhow::Result;

mod cli;
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
        Commands::Index(args) => cli::cmds::index(args)?,
        Commands::Status(args) => cli::cmds::status(args, &cli)?,
        Commands::Paths(args) => cli::cmds::paths(args)?,
        Commands::Cfg(args) => cli::cmds::cfg(args)?,
        Commands::Dominators(args) => cli::cmds::dominators(args)?,
        Commands::Unreachable(args) => cli::cmds::unreachable(args)?,
        Commands::Verify(args) => cli::cmds::verify(args)?,
        Commands::BlastZone(args) => cli::cmds::blast_zone(args)?,
    }
    Ok(())
}
