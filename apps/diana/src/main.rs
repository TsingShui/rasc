//! Diana binary entry point: argument parsing and classified exit codes.

fn main() -> std::process::ExitCode {
    diana::cli::run()
}
