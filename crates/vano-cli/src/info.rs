//! `-h`/`--help` and `-V`/`--version`: print something and exit. The output stays in English,
//! like log messages.

use std::io::Write;

use telaio_app::cli;

use crate::IDENTITY;

/// The command line guide `--help` links to.
pub const DOCS_URL: &str = "https://vano.tanis.codes/docs/command-line.html";

const TAGLINE: &str = "A fast, native disk analyzer. See what fills your disk, free it safely.";

/// What an informational option asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Info {
    /// Print the usage summary and exit.
    Help,
    /// Print the version and exit.
    Version,
}

impl Info {
    /// The command asked by the first argument, or `None` when it is not one of ours.
    pub fn parse(first_arg: &str) -> Option<Self> {
        match first_arg {
            "-h" | "--help" => Some(Info::Help),
            "-V" | "--version" => Some(Info::Version),
            _ => None,
        }
    }

    /// The text to print to stdout, ending in a newline.
    pub fn output(self) -> String {
        match self {
            Info::Help => help_text(),
            Info::Version => format!("{}\n", cli::version_line(&IDENTITY)),
        }
    }

    /// Prints the output to stdout and returns the exit code.
    pub fn run(self) -> u8 {
        // A closed stdout (`vano --help | head -1`) is not worth a panic.
        let _ = std::io::stdout().write_all(self.output().as_bytes());
        cli::EXIT_SUCCESS
    }
}

/// The `--help` text.
fn help_text() -> String {
    format!(
        "{header}
Usage: vano
       vano <COMMAND> [ARGS]

Without a command, Vano opens its window.

Commands (run in the terminal, without the window):
  drives            List the connected disks, internal and external
  scan [PATH]       Scan a folder or a disk and print what fills it
  clean [PATH]      Remove the Safe recommendations: caches and build
                    outputs their tools recreate. Asks first, or lists
                    only with --dry-run; --yes removes without asking
  completions <SHELL>
                    Print the completion script of a shell
  Run vano <COMMAND> --help for its options.

Options:
  -h, --help     Print this help and exit
  -V, --version  Print the version and exit

Exit codes:
  0  Done (a dry run included)
  1  Something failed: a path that cannot be read, items not removed
  2  Wrong command line
  3  clean found nothing to clean

Environment variables:
  VANO_LOG  Log level of the window: off, error, warn, info, debug or trace

{footer}",
        header = cli::help_header(&IDENTITY, TAGLINE),
        footer = cli::help_footer(DOCS_URL),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_and_version_flags_are_ours() {
        assert_eq!(Info::parse("-h"), Some(Info::Help));
        assert_eq!(Info::parse("--help"), Some(Info::Help));
        assert_eq!(Info::parse("-V"), Some(Info::Version));
        assert_eq!(Info::parse("--version"), Some(Info::Version));
        assert_eq!(Info::parse("-v"), None);
    }

    #[test]
    fn the_version_line_names_the_app() {
        assert!(Info::Version.output().starts_with("Vano "));
    }

    #[test]
    fn the_help_lists_every_subcommand_and_never_uses_an_em_dash() {
        let help = Info::Help.output();
        for name in crate::args::SUBCOMMANDS {
            assert!(help.contains(&format!("  {name}")), "{name} is missing");
        }
        assert!(!help.contains('\u{2014}'));
    }
}
