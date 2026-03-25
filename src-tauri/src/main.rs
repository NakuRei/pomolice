// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match pomolice_lib::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "Fatal error: {error}");
            ExitCode::FAILURE
        }
    }
}
