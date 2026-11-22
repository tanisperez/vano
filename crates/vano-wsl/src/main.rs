//! `vano-wsl`, the program the Windows app runs inside a WSL 2 distribution (see
//! `vano_system::wsl`), where reading through `\\wsl$` would be slow. Not meant for people:
//! `vano scan` and `vano clean` are the command line.
//!
//! - `vano-wsl scan <ROOT>`: scans natively and writes the tree in Vano's binary format to
//!   stdout, with `ScanLine`s of progress on stderr.
//! - `vano-wsl delete`: deletes for good the paths read from stdin, reporting `DeleteLine`s.

mod delete;
mod scan;

use std::process::ExitCode;

/// The command line is wrong.
const USAGE: u8 = 2;
/// The command failed.
const FAILED: u8 = 1;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["scan", root] => scan::command(std::path::Path::new(root)),
        ["delete"] => delete::command(),
        ["--version"] => {
            println!("vano-wsl {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => {
            eprintln!("Usage: vano-wsl scan <ROOT> | vano-wsl delete. Run by Vano for Windows.");
            USAGE
        }
    };
    ExitCode::from(code)
}
