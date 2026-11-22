//! The command line of Vano: the commands that run in the terminal and exit instead of opening
//! the window. No `gpui`, so everything is unit tested without a window. English only, like the
//! log messages.
//!
//! The first argument decides: `-h`/`--help` and `-V`/`--version` print and exit ([`info`]), a
//! subcommand name ([`args::SUBCOMMANDS`]) runs in console mode, and anything else (no argument,
//! an unknown flag) is left to the app.
//!
//! To add a subcommand:
//! 1. Add its arguments to [`args`]: a variant of [`args::ConsoleCommand`] and its name in
//!    [`args::SUBCOMMANDS`].
//! 2. Create a module with a `command` function returning the exit code ([`exit`]), and call it
//!    from [`run_console`].
//! 3. List it in the help of [`info`] and add its tests next to it.

pub mod args;
mod clean;
mod drives;
pub mod exit;
pub mod info;
mod report;
mod scan;
mod wsl;

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use clap::Parser;
use telaio_app::AppIdentity;

use args::{Cli, ConsoleCommand};
pub use info::Info;

rust_i18n::i18n!("../vano-rules/locales", fallback = "en");

/// Who Vano is, for every piece of `telaio-app` the app and the command line use: the log file
/// and folder, the `VANO_*` environment variables, the window's app id (the Linux
/// `codes.tanis.vano.desktop` file uses the same value), the version and the About header.
pub const IDENTITY: AppIdentity = AppIdentity {
    name: "Vano",
    slug: "vano",
    app_id: "codes.tanis.vano",
    version: env!("CARGO_PKG_VERSION"),
    env_prefix: "VANO",
    repository: "https://github.com/tanisperez/vano",
};

/// A command line command that runs without opening the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `-h`/`--help` and `-V`/`--version`.
    Info(Info),
    /// A subcommand, with every argument after the program name.
    Console(Vec<OsString>),
}

impl Command {
    /// The command the arguments (without the program name) ask for, or `None` when Vano should
    /// open its window.
    pub fn parse(args: &[OsString]) -> Option<Self> {
        let first = args.first()?.to_str()?;
        if let Some(info) = Info::parse(first) {
            return Some(Command::Info(info));
        }
        args::SUBCOMMANDS
            .contains(&first)
            .then(|| Command::Console(args.to_vec()))
    }

    /// Runs the command and returns the process exit code.
    pub fn run(self) -> ExitCode {
        let code = match self {
            Command::Info(info) => info.run(),
            Command::Console(args) => run_console(&args),
        };
        ExitCode::from(code)
    }
}

/// Parses and runs a subcommand, returning its exit code.
fn run_console(args: &[OsString]) -> u8 {
    let all = std::iter::once(OsStr::new("vano").to_os_string()).chain(args.iter().cloned());
    let cli = match Cli::try_parse_from(all) {
        Ok(cli) => cli,
        Err(error) => {
            // `--help` of a subcommand is an "error" that prints to stdout.
            let _ = error.print();
            return if error.use_stderr() {
                exit::USAGE
            } else {
                exit::SUCCESS
            };
        }
    };
    match cli.command {
        ConsoleCommand::Drives(args) => drives::command(&args),
        ConsoleCommand::Scan(args) => scan::command(&args),
        ConsoleCommand::Clean(args) => clean::command(&args),
        ConsoleCommand::Wsl(args::WslCommand::List(args)) => wsl::list_command(&args),
        ConsoleCommand::Completions(args) => args::completions_command(&args),
    }
}

/// Handles the command line before anything else is set up. Returns `None` when the arguments
/// (without the program name) are not a command (the app opens its window), otherwise runs it
/// and returns the exit code.
///
/// On Windows `vano.exe` is a console subsystem binary, so the output goes to the terminal, file
/// or pipe the shell gave it, and the shell waits for the exit code.
pub fn run_if_cli(args: &[OsString]) -> Option<ExitCode> {
    let command = Command::parse(args)?;
    Some(command.run())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn no_argument_opens_the_window() {
        assert_eq!(Command::parse(&[]), None);
    }

    #[test]
    fn a_subcommand_runs_in_the_terminal() {
        assert_eq!(
            Command::parse(&os(&["drives", "--json"])),
            Some(Command::Console(os(&["drives", "--json"])))
        );
    }

    #[test]
    fn help_and_version_are_info() {
        assert_eq!(
            Command::parse(&os(&["--version"])),
            Some(Command::Info(Info::Version))
        );
    }

    #[test]
    fn anything_else_is_left_to_the_app() {
        assert_eq!(Command::parse(&os(&["/home/ana"])), None);
        assert_eq!(Command::parse(&os(&["--unknown"])), None);
    }
}
