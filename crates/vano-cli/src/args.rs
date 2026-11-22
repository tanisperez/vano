//! The arguments of the console commands, parsed with `clap`. Only the subcommands live here:
//! `vano` alone, `--help` and `--version` are handled before `clap` sees them (see
//! [`crate::Command::parse`]).

use std::io::Write;
use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

use crate::exit;

/// The names that make Vano run in console mode when they are the first argument.
pub const SUBCOMMANDS: [&str; 5] = ["drives", "scan", "clean", "wsl", "completions"];

#[derive(Debug, Parser)]
#[command(
    name = "vano",
    disable_version_flag = true,
    about = "Vano, the disk analyzer",
    after_help = "Run vano --help for the overview."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: ConsoleCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConsoleCommand {
    /// List the connected disks, internal and external
    Drives(DrivesArgs),
    /// Scan a folder or a disk and print what fills it
    Scan(ScanArgs),
    /// Remove the Safe recommendations of a folder: caches and build outputs
    Clean(CleanArgs),
    /// The WSL 2 distributions (Windows only)
    #[command(subcommand)]
    Wsl(WslCommand),
    /// Print the completion script of a shell
    Completions(CompletionsArgs),
}

#[derive(Debug, Subcommand)]
pub enum WslCommand {
    /// List the WSL 2 distributions, with the size of their virtual disks
    List(WslListArgs),
}

#[derive(Debug, Args)]
pub struct WslListArgs {
    /// Print JSON instead of a table
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DrivesArgs {
    /// Print JSON instead of a table
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    /// The folder or disk to scan (the home folder when left out)
    pub path: Option<PathBuf>,
    /// How many of the biggest entries to list
    #[arg(long, default_value_t = 10, value_name = "N")]
    pub top: usize,
    /// Print JSON instead of text
    #[arg(long)]
    pub json: bool,
    /// Also enter the other file systems mounted inside the path
    #[arg(long)]
    pub cross_file_systems: bool,
}

#[derive(Debug, Args)]
pub struct CleanArgs {
    /// The folder to clean up (the home folder when left out)
    pub path: Option<PathBuf>,
    /// Only list what would be removed, and never ask
    #[arg(long)]
    pub dry_run: bool,
    /// Delete for good instead of moving to the Trash
    #[arg(long)]
    pub permanent: bool,
    /// Remove without asking
    #[arg(long, short = 'y', conflicts_with = "dry_run")]
    pub yes: bool,
    /// Print JSON instead of text (with --yes, the outcome)
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// bash, zsh, fish, elvish or powershell
    pub shell: Shell,
}

/// `vano completions <SHELL>`: prints the completion script.
pub fn completions_command(args: &CompletionsArgs) -> u8 {
    let mut out = Vec::new();
    clap_complete::generate(args.shell, &mut Cli::command(), "vano", &mut out);
    let _ = std::io::stdout().write_all(&out);
    exit::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_subcommand_name_is_a_clap_subcommand() {
        let command = Cli::command();
        let mut names: Vec<_> = command.get_subcommands().map(|c| c.get_name()).collect();
        let mut expected = SUBCOMMANDS.to_vec();
        names.sort_unstable();
        expected.sort_unstable();
        assert_eq!(names, expected);
    }

    #[test]
    fn clean_cannot_be_a_dry_run_and_confirmed_at_once() {
        let parsed = Cli::try_parse_from(["vano", "clean", "--dry-run", "--yes"]);
        assert!(parsed.is_err());
    }

    #[test]
    fn the_clap_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
