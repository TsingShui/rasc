//! CLI entry point: argument parsing, dispatch, output.
//!
//! stdout carries a command's payload and nothing else: every diagnostic,
//! including `--debug` timing, goes to stderr, so redirecting a result can never
//! capture debug lines. An `-o` file receives exactly the bytes stdout received.

mod apk;
mod bytes;
mod cli;
mod dex;
mod diag;
mod emitter;
mod manifest;
mod query;
mod skill;
mod zip;

use anyhow::{Context, Result, bail};
use clap::Parser;
use cli::{Cli, Command};
use rayon::prelude::*;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

/// Where a command's payload goes: stdout, on both targets that remain.
fn payload_writer() -> impl Write {
    io::BufWriter::new(io::stdout().lock())
}

/// Reads the host's per-entry inflation ceiling from the environment.
///
/// The ceiling is host policy, not command-line policy: a browser tab holding this instance
/// has a memory budget of its own, and the argument line belongs to whatever wrote it. The
/// environment is the one channel a WASI host owns, so the value is read from there.
///
/// It is applied before `dispatch`, because "decide before the first allocation" is the whole
/// point of the limit: a ceiling installed after a decompression has already run was never a
/// ceiling for that entry.
fn apply_inflation_limit_from_env() {
    let Ok(value) = std::env::var("RASC_MAX_INFLATED_ENTRY") else {
        return;
    };
    // `0` restores the built-in default; anything unparseable leaves it in force rather than
    // guessing at an intent.
    match value.trim().parse::<usize>() {
        Ok(0) | Err(_) => {}
        Ok(bytes) => {
            crate::zip::set_max_inflated_entry(bytes);
        }
    }
}
fn main() {
    restore_default_sigpipe();
    apply_inflation_limit_from_env();
    match dispatch(std::env::args().collect()) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            // A consumer that closed the pipe early (`| head -1`) is normal filter behaviour,
            // not a failure. The native path rarely sees this because SIGPIPE kills the
            // process first; a target without signals - the WASI build - does see it as a
            // write error, and has to end the same way the signal would have: quietly, with
            // status 0.
            if is_broken_pipe(&error) {
                std::process::exit(0);
            }
            eprintln!("Error: {error:#}");
            std::process::exit(1);
        }
    }
}

/// clap's own text, on clap's own stream, with clap's own status.
///
/// `--help`, `--version` and a usage error are all *answers*, not failures: they carry a
/// status of their own (0, 0, 2) and text a user asked for. clap's own `exit()` decides that
/// by ending the process, which hides the status from the caller that wants it; returning it
/// keeps the decision where the caller can see it, and `print()` still puts each answer on
/// the stream clap chose for it.
fn cli_message(error: &clap::Error) -> i32 {
    let _ = error.print();
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    error.exit_code()
}

/// Whether an error is only a consumer that closed the pipe.
fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::BrokenPipe)
    })
}

/// Restore the default `SIGPIPE` disposition so that piping into `head`, `grep`,
/// or a closed consumer terminates quietly, the way standard Unix filters do.
///
/// Rust ignores `SIGPIPE` at startup, which turns an early-closed pipe into a
/// `BrokenPipe` write error and then a panic; with `panic = "abort"` that aborts
/// the process and prints a panic message for a completely normal shell idiom.
#[cfg(unix)]
fn restore_default_sigpipe() {
    // SAFETY: setting the disposition of SIGPIPE to SIG_DFL is
    // async-signal-safe and only affects this process.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn restore_default_sigpipe() {}

/// Writes a command's payload to stdout and, when `output` is set, to that file
/// as well.
///
/// Diagnostics deliberately do not travel through here: they go to stderr, so a
/// redirected payload can never pick up `--debug` lines, and the file always
/// receives exactly the bytes stdout received.
fn emit(payload: &str, output: Option<&Path>) -> Result<()> {
    if let Some(path) = output {
        fs::write(path, payload).with_context(|| format!("write {}", path.display()))?;
    }
    let mut stdout = payload_writer();
    stdout.write_all(payload.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

/// Writes payload chunks to stdout and, when `output` is set, to that file as well.
///
/// Produces exactly the bytes [`emit`] would produce for the concatenation, but each
/// chunk is released as it is written. A class index is a ~59 MiB payload, and holding
/// the rendered chunks plus a joined copy of them peak-doubled the command's memory.
/// Returns the number of bytes written.
fn emit_rows(chunks: impl Iterator<Item = String>, output: Option<&Path>) -> Result<usize> {
    let mut file = match output {
        Some(path) => {
            Some(fs::File::create(path).with_context(|| format!("write {}", path.display()))?)
        }
        None => None,
    };
    let mut stdout = payload_writer();
    let mut written = 0usize;
    for chunk in chunks {
        written += chunk.len();
        if let Some(file) = file.as_mut() {
            file.write_all(chunk.as_bytes())?;
        }
        stdout.write_all(chunk.as_bytes())?;
    }
    if let Some(file) = file.as_mut() {
        file.flush()?;
    }
    stdout.flush()?;
    Ok(written)
}

/// This is the only place the CLI reports timing, so `--debug` reads the same
/// for every subcommand.
fn debug_timing(started: Instant) {
    crate::diag::diagnose(format_args!(
        "[DEBUG] Total Execution Time: {:.2} us",
        started.elapsed().as_secs_f64() * 1_000_000.0
    ));
}

/// Parses and runs one command, returning the status the process should exit with.
///
/// The vector is the process's own arguments; there is no second entry point, so a target
/// cannot drift from another in what it accepts or in what it says.
fn dispatch(args: Vec<String>) -> Result<i32> {
    let started = Instant::now();
    // `try_parse_from`, not `parse_from`: clap's own error path ends the process, which
    // makes its status unobservable to the caller that wants it. Returning it keeps the
    // answer - `--help`, `--version`, a usage error - in one place for both targets.
    let args = match Cli::try_parse_from(args) {
        Ok(args) => args,
        Err(error) => return Ok(cli_message(&error)),
    };

    match run_command(args, started) {
        Ok(()) => Ok(0),
        Err(error) => Err(error),
    }
}

/// Runs the parsed command.
fn run_command(args: Cli, started: Instant) -> Result<()> {
    match args.command {
        Command::Findrefs(args) => {
            let query = args.query()?;
            let scan_started = Instant::now();
            let mut hits = apk::find_reference_hits(&args.apk_path, &query, args.threads, args.debug)?;
            let scanned = scan_started.elapsed();
            // The comparator is the string order, so an unstable sort produces exactly the
            // bytes a stable one would: equal lines are identical lines, and there is
            // nothing else to tie-break. Sorting a wide query's ~90k lines in parallel is
            // what keeps it off the main thread. This wasm target has no threads and takes the serial
            // sort; the output is the same either way.
            if cfg!(target_family = "wasm") {
                hits.sort_unstable();
            } else {
                hits.par_sort_unstable();
            }
            let sorted = scan_started.elapsed();
            // Assembling the payload line by line: a pre-sized buffer measured no faster
            // than letting the String grow (macOS's allocator re-extends the large block in
            // place), so the length pass is not worth it.
            let mut payload = String::new();
            for hit in &hits {
                payload.push_str(&hit.render());
                payload.push('\n');
            }
            if args.debug {
                crate::diag::diagnose(format_args!(
                    "[findrefs] rows={} scan={:.2} ms sort={:.2} ms assemble={:.2} ms payload={} MiB",
                    payload.lines().count(),
                    scanned.as_secs_f64() * 1e3,
                    (sorted - scanned).as_secs_f64() * 1e3,
                    (scan_started.elapsed() - sorted).as_secs_f64() * 1e3,
                    payload.len() / (1 << 20)
                ));
            }
            emit(&payload, args.output_path())?;
            if args.debug {
                debug_timing(started);
            }
        }
        Command::FieldsPlan(args) => {
            // Accept what a user types; the DEX carries descriptors.
            let descriptor = crate::query::format_class_name(&args.descriptor)?;
            match apk::field_plan(&args.apk, &descriptor, args.threads)? {
                Some(plan) => emit(&format!("{plan}\n"), args.output.as_deref())?,
                None => bail!("{descriptor} is not defined in the supplied code inputs"),
            }
            if args.debug {
                debug_timing(started);
            }
        }
        Command::Classes(args) => {
            let filter = args.filter.as_deref().map(str::to_lowercase);
            let started = Instant::now();
            let classes = apk::list_classes(&args.apk_path, args.threads, args.debug)?;
            let listed = started.elapsed();
            // Rendering is per-row independent, so the rows are rendered in chunks kept in
            // the order they will be printed, and then written out chunk by chunk. The
            // filter, the row format and the resulting bytes are unchanged. wasm has no
            // threads, so it renders the same chunks serially in the same order.
            const RENDER_CHUNK: usize = 8192;
            let render = |chunk: &[apk::ClassEntry]| -> String {
                let mut payload = String::with_capacity(chunk.len() * 128);
                for class in chunk {
                    // The filter asks about the Java-style (dotted) name, lowercased;
                    // that is the only case where the dotted form has to exist as a
                    // string of its own. The row itself writes the dotted form in place,
                    // so the unfiltered index allocates nothing per class for it.
                    if let Some(pattern) = filter.as_ref()
                        && !class.java_name().replace('/', ".").to_lowercase().contains(pattern)
                    {
                        continue;
                    }
                    payload.push_str(&class.dex_name);
                    payload.push_str(" | ");
                    payload.push_str(&class.descriptor);
                    payload.push_str(" | ");
                    // Char-wise rather than byte-wise: a name is UTF-8, and a byte loop
                    // turns one character of a CJK or emoji name into several.
                    for character in class.java_name().chars() {
                        payload.push(if character == '/' { '.' } else { character });
                    }
                    payload.push_str(" | package=");
                    payload.push_str(class.package());
                    payload.push_str(" | class=");
                    payload.push_str(class.simple_name());
                    payload.push('\n');
                }
                payload
            };
            let payload_len = if cfg!(target_family = "wasm") {
                // wasm renders one chunk at a time, so only one is ever live.
                emit_rows(
                    classes.chunks(RENDER_CHUNK).map(render),
                    args.output.as_deref(),
                )?
            } else {
                // Native renders the chunks in parallel, then streams them out in order.
                let pieces: Vec<String> = classes.par_chunks(RENDER_CHUNK).map(render).collect();
                emit_rows(pieces.into_iter(), args.output.as_deref())?
            };
            if args.debug {
                // The window now covers rendering and writing, not rendering alone.
                crate::diag::diagnose(format_args!(
                    "[classes] render={:.2} ms payload={} MiB (list {:.2} ms)",
                    started.elapsed().as_secs_f64() * 1e3 - listed.as_secs_f64() * 1e3,
                    payload_len / (1 << 20),
                    listed.as_secs_f64() * 1e3
                ));
            }
        }
        Command::Strings(args) => {
            let strings = apk::list_strings(
                &args.apk_path,
                args.threads,
                args.debug,
                args.filter.as_deref(),
                args.limit,
                args.offset,
            )?;
            let mut payload = String::with_capacity(strings.len() * 48);
            for entry in &strings {
                payload.push_str(&entry.dex_name);
                payload.push_str(" | ");
                payload.push_str(&entry.index.to_string());
                payload.push_str(" | ");
                payload.push_str(&entry.value);
                payload.push('\n');
            }
            emit(&payload, args.output.as_deref())?;
        }
        Command::Entries(args) => {
            let entries = apk::list_entries(&args.apk_path)?;
            let mut payload = String::with_capacity(entries.len() * 96);
            for entry in &entries {
                payload.push_str(&entry.name);
                payload.push_str(" | ");
                payload.push_str(match entry.compression {
                    0 => "stored",
                    8 => "deflated",
                    _ => "other",
                });
                payload.push_str(" | ");
                payload.push_str(&entry.compressed_size.to_string());
                payload.push_str(" | ");
                payload.push_str(&entry.uncompressed_size.to_string());
                payload.push('\n');
            }
            emit(&payload, args.output.as_deref())?;
        }
        Command::Manifest(args) => {
            let data = apk::read_entry(&args.apk_path, "AndroidManifest.xml")?;
            let xml = manifest::decode(&data)?;
            emit(&xml, args.output.as_deref())?;
        }
        Command::Skill(args) => {
            if args.print {
                if args.dir.is_some() || !args.agents.is_empty() {
                    bail!("--print writes the skill to stdout; drop AGENT and --dir");
                }
                emit(skill::SKILL_MD, None)?;
            } else {
                // The JS host build has no filesystem of its own. Its payload path is stdout,
                // which is what `--print` is for; only native and WASI can write files.
                if cfg!(all(target_family = "wasm", not(target_os = "wasi"))) {
                    bail!("the wasm host build cannot write files; use `rasc skill --print`");
                }
                for path in skill::targets(&args.agents, args.dir.as_deref())? {
                    let outcome = skill::install(&path)?;
                    crate::diag::diagnose(format_args!(
                        "rasc skill {} at {}",
                        outcome.verb(),
                        path.display()
                    ));
                }
            }
        }
        Command::Getclass(args) => {
            let class_name = query::format_class_name(&args.dalvik_class)?;
            let hit =
                apk::decompile_class(&args.apk_path, &class_name, args.threads, args.debug)?;
            let Some((dex_name, source)) = hit else {
                bail!("Class {class_name} not found in APK.");
            };
            if args.debug {
                crate::diag::diagnose(format_args!("[DEBUG] Hit DEX: {dex_name}"));
                debug_timing(started);
                crate::diag::diagnose(format_args!("{}", "-".repeat(50)));
            }
            let mut payload = String::with_capacity(source.len() + 256);
            payload.push_str(&source);
            payload.push('\n');
            emit(&payload, args.output.as_deref())?;
        }
    }
    Ok(())
}
