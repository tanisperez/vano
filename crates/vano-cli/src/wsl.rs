//! `vano wsl list`: the WSL 2 distributions of this Windows computer and their virtual disks.

use std::io::Write;

use serde::Serialize;
use vano_core::size::format_plain;
use vano_system::wsl::Distro;

use crate::args::WslListArgs;
use crate::exit;
use crate::report::to_json;

#[derive(Debug, Serialize)]
struct DistroJson {
    name: String,
    running: bool,
    default: bool,
    docker: bool,
    vhdx: Option<String>,
    vhdx_bytes: u64,
    /// Only known for a running distribution.
    used_inside: Option<u64>,
}

/// The output of `vano wsl list` for `distros`.
pub fn command_output(args: &WslListArgs, distros: &[Distro]) -> String {
    if args.json {
        return to_json(
            &distros
                .iter()
                .map(|distro| DistroJson {
                    name: distro.name.clone(),
                    running: distro.running,
                    default: distro.default,
                    docker: distro.is_docker(),
                    vhdx: distro.vhdx.as_ref().map(|path| path.display().to_string()),
                    vhdx_bytes: distro.vhdx_size,
                    used_inside: distro.used_inside,
                })
                .collect::<Vec<_>>(),
        );
    }
    if distros.is_empty() {
        return "No WSL 2 distributions.\n".to_string();
    }
    let width = distros
        .iter()
        .map(|distro| distro.name.chars().count() + usize::from(distro.default) * 2)
        .max()
        .unwrap_or(0)
        .max(4);
    let mut out = format!(
        "{:<width$}  {:<7}  {:>9}  INSIDE\n",
        "NAME", "STATE", "DISK"
    );
    for distro in distros {
        let name = if distro.default {
            format!("{} *", distro.name)
        } else {
            distro.name.clone()
        };
        let state = if distro.running { "Running" } else { "Stopped" };
        let inside = distro.used_inside.map_or_else(
            || "-".to_string(),
            |used| format!("{} used", format_plain(used)),
        );
        out.push_str(&format!(
            "{name:<width$}  {state:<7}  {:>9}  {inside}\n",
            format_plain(distro.vhdx_size)
        ));
    }
    out
}

/// `vano wsl list`.
pub fn list_command(args: &WslListArgs) -> u8 {
    if !cfg!(windows) {
        eprintln!("vano wsl: WSL only exists on Windows");
        return exit::USAGE;
    }
    let distros = vano_system::wsl::list_distros();
    let _ = std::io::stdout().write_all(command_output(args, &distros).as_bytes());
    exit::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn distro(name: &str, running: bool, default: bool) -> Distro {
        Distro {
            name: name.to_string(),
            running,
            default,
            vhdx: None,
            vhdx_size: 187_000_000_000,
            used_inside: running.then_some(92_000_000_000),
        }
    }

    #[test]
    fn the_table_marks_the_default_and_what_is_running() {
        let distros = vec![
            distro("Ubuntu-24.04", true, true),
            distro("docker-desktop", false, false),
        ];
        let args = WslListArgs { json: false };
        assert_eq!(
            command_output(&args, &distros),
            "NAME            STATE         DISK  INSIDE\n\
             Ubuntu-24.04 *  Running     187 GB  92.0 GB used\n\
             docker-desktop  Stopped     187 GB  -\n"
        );
    }

    #[test]
    fn json_tells_docker_apart() {
        let args = WslListArgs { json: true };
        let json = command_output(&args, &[distro("docker-desktop", false, false)]);
        assert!(json.contains("\"docker\": true"), "{json}");
    }
}
