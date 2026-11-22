//! Embeds the Windows resources (the executable icon, `assets/vano.rc`) into `vano.exe`, and on
//! Windows the Linux program `vano-wsl` given by `VANO_EMBED_WSL_AGENT` (see
//! `vano_system::wsl`).

use std::path::Path;
use std::{env, fs};

fn main() {
    embed_wsl_agent();
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/vano.rc");
        println!("cargo:rerun-if-changed=assets/vano.ico");
        if let Err(error) =
            embed_resource::compile("assets/vano.rc", embed_resource::NONE).manifest_optional()
        {
            panic!("failed to embed the Windows resources: {error}");
        }
    }
}

/// Writes `$OUT_DIR/wsl_agent.rs` with `AGENT`: the bytes of `VANO_EMBED_WSL_AGENT` when building
/// for Windows, `None` otherwise. The release workflow builds `vano-wsl` first and sets it.
fn embed_wsl_agent() {
    println!("cargo:rerun-if-env-changed=VANO_EMBED_WSL_AGENT");
    let windows = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let source = env::var("VANO_EMBED_WSL_AGENT")
        .ok()
        .filter(|path| windows && !path.is_empty());
    let value = match source {
        Some(path) => {
            let path = match fs::canonicalize(&path) {
                Ok(path) => path,
                Err(error) => panic!("VANO_EMBED_WSL_AGENT={path}: {error}"),
            };
            println!("cargo:rerun-if-changed={}", path.display());
            format!("Some(include_bytes!({:?}))", path.display().to_string())
        }
        None => "None".to_string(),
    };
    let out = Path::new(&env::var("OUT_DIR").unwrap_or_default()).join("wsl_agent.rs");
    let code = format!("pub const AGENT: Option<&[u8]> = {value};\n");
    if let Err(error) = fs::write(&out, code) {
        panic!("failed to write {}: {error}", out.display());
    }
}
