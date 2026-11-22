//! The exit codes of the console commands, distinct so scripts can tell the outcomes apart.

use telaio_app::cli;

/// The command did what it was asked (a dry run counts).
pub const SUCCESS: u8 = cli::EXIT_SUCCESS;
/// Something failed: a path that cannot be read, items that could not be removed.
pub const FAILED: u8 = 1;
/// The command line is wrong.
pub const USAGE: u8 = cli::EXIT_USAGE;
/// `vano clean` found nothing to clean.
pub const NOTHING_TO_CLEAN: u8 = 3;
