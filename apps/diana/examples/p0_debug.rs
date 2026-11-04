//! Internal class-fact inspection; not a Diana CLI command.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use diana::analysis::{extract, input::InputOptions, p0, request::RequestScope, selector};
use diana_decomp::RunContext;

#[derive(Parser)]
struct Args {
    input: PathBuf,
    /// Exact class descriptor, dotted name or canonical class-at
    class: String,
    /// Write pretty JSON atomically; otherwise write to stdout
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let selector = selector::class(&args.class)?;
    let scope = RequestScope::open(&args.input, &InputOptions::default())?;
    let mut control = RunContext::new(&|| false);
    let prepared = p0::run(&scope, &selector, &mut control)?;
    let collected = started.elapsed();
    let payload = diana_decomp::p0::render_json(&prepared.facts, &mut control)?;
    let rendered = started.elapsed();
    if let Some(output) = args.output {
        let result = extract::Extracted {
            entry: String::new(),
            bytes: payload.into_bytes(),
            decoded: false,
            warnings: Vec::new(),
        };
        extract::publish(&result, &output, &args.input, false)?;
    } else {
        std::io::stdout().lock().write_all(payload.as_bytes())?;
    }
    eprintln!(
        "P0: selected and collected in {:.3}s; with JSON rendering {:.3}s",
        collected.as_secs_f64(),
        rendered.as_secs_f64(),
    );
    if prepared.facts.is_partial() {
        return Err("P0 snapshot is partial; see its fact states and diagnostics".into());
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run(Args::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("p0_debug: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
