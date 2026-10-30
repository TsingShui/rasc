//! Diana's separate binary target. No analysis commands are implemented yet.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("diana: project scaffold only; CLI and decompiler are not implemented");
    ExitCode::FAILURE
}
