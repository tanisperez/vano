//! Embeds the Windows resources (the executable icon, `assets/vano.rc`) into `vano.exe`.

fn main() {
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
