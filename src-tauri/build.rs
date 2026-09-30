use std::error::Error;
use std::io::Write;

use tauri_build::{AppManifest, Attributes};

#[path = "src/ipc/registration.rs"]
mod registration;

fn main() -> Result<(), Box<dyn Error>> {
    macro_rules! command_names {
        ($($module:ident::$command:ident),* $(,)?) => { &[$(stringify!($command)),*] };
    }
    tauri_build::try_build(
        Attributes::new()
            .app_manifest(AppManifest::new().commands(registration::with_commands!(command_names))),
    )?;

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let mut output = std::io::stdout().lock();
        let output_dir = std::env::var("OUT_DIR")?;
        writeln!(output, "cargo:rustc-link-search=native={output_dir}")?;
    }

    Ok(())
}
