//! CLI entry point: argument parsing, dispatch, output.
//!
//! stdout carries a command's payload and nothing else: every diagnostic,
//! including `--debug` timing, goes to stderr, so redirecting a result can never
//! capture debug lines. An `-o` file receives exactly the bytes stdout received.

mod apk;
mod bytes;
mod cli;
mod clock;
mod dex;
mod diag;
mod emitter;
mod json;
mod outline;
mod progress;
mod manifest;
mod query;
mod skill;
mod zip;

use crate::clock::Instant;
use anyhow::{Context, Result, bail};
use clap::Parser;
use cli::{Cli, Command};
use rayon::prelude::*;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Where a command's payload goes.
///
/// Native and WASI write to stdout. Under a JS host there is no stdout, so the bytes go
/// back to the host through an imported function instead.
#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
fn payload_writer() -> impl Write {
    io::BufWriter::new(io::stdout().lock())
}

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
fn payload_writer() -> HostWriter {
    HostWriter
}

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
struct HostWriter;

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
impl Write for HostWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        unsafe { rasc_host_write(buffer.as_ptr() as u32, buffer.len() as u32) };
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// What a JS host has to provide.
//
// `wasm_import_module` is what makes rustc emit these as imports instead of asking the
// linker to find definitions.
#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    // Hands `len` bytes of payload back to the host.
    fn rasc_host_write(ptr: u32, len: u32) -> u32;
    // Writes the `index`-th argument into wasm memory at `ptr`, up to `capacity` bytes and
    // unterminated, returning its full length so the caller can grow and retry.
    fn rasc_host_arg(index: u32, ptr: u32, capacity: u32) -> u32;
    // How far a walk over the archive's entries has got. A host that wants a fraction
    // installs a handler; one that does not ignores the calls.
    fn rasc_host_progress(done: u32, total: u32);
}

/// Sets the per-entry inflation ceiling before a command runs, returning the previous value.
///
/// A host with a tighter memory budget than the default (a browser holding this instance)
/// uses this; passing 0 restores the built-in default. The CLI and the WASI build leave it
/// alone.
#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
#[unsafe(no_mangle)]
pub extern "C" fn rasc_set_max_inflated_entry(bytes: u32) -> u32 {
    let requested = if bytes == 0 {
        crate::zip::DEFAULT_MAX_INFLATED_ENTRY
    } else {
        bytes as usize
    };
    crate::zip::set_max_inflated_entry(requested) as u32
}

/// C-ABI entry point for a JS host.
///
/// The host announces the archive length, implements the `rasc_host_*` imports, and calls
/// this with the argument count (`argv[0]` included, as `Cli` expects). The return value is
/// the exit code the CLI would have used.
#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
#[unsafe(no_mangle)]
pub extern "C" fn rasc_run(argc: u32) -> i32 {
    // A trap in wasm says nothing about why, so a panic has to be reported through the
    // host before `panic = "abort"` turns it into `unreachable`.
    std::panic::set_hook(Box::new(|info| {
        let _ = writeln!(payload_writer(), "PANIC: {info}");
    }));
    let mut args = Vec::with_capacity(argc as usize);
    for index in 0..argc {
        let mut buffer = vec![0u8; 256];
        loop {
            let length = unsafe {
                rasc_host_arg(index, buffer.as_mut_ptr() as u32, buffer.len() as u32) as usize
            };
            if length <= buffer.len() {
                buffer.truncate(length);
                break;
            }
            buffer = vec![0u8; length];
        }
        args.push(String::from_utf8_lossy(&buffer).into_owned());
    }
    match dispatch(args) {
        Ok(code) => code,
        Err(error) => {
            if is_broken_pipe(&error) {
                return 0;
            }
            let _ = writeln!(payload_writer(), "Error: {error:#}");
            1
        }
    }
}

fn main() {
    restore_default_sigpipe();
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
/// status of their own (0, 0, 2) and text a user asked for. Letting clap's `exit()` decide
/// froze them into whatever the platform does with a process exit, and on
/// `wasm32-unknown-unknown` that is `unreachable` - so a host that passed one bad argument
/// got a trap and an instance it could no longer use, with no message at all. Returning the
/// status and writing the text is the same behaviour on every target.
fn cli_message(error: &clap::Error) -> i32 {
    #[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
    {
        // No stderr to write to here, so the message goes where the host reads: the payload.
        // The text is still clap's, which is what keeps the two hosts saying one thing.
        let mut out = payload_writer();
        let _ = out.write_all(error.render().to_string().as_bytes());
        let _ = out.flush();
    }
    #[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
    {
        let _ = error.print();
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
    }
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
/// A JS host supplies the vector through [`rasc_run`] instead of the process environment;
/// both entry points reach the same dispatch, so a target can never drift from another in
/// what it accepts or what it says.
fn dispatch(args: Vec<String>) -> Result<i32> {
    let started = Instant::now();
    // `try_parse_from`, not `parse_from`: clap's error path calls `exit()`, and `exit()` on
    // a target without processes is a trap that takes the host's instance with it.
    let args = match Cli::try_parse_from(args) {
        Ok(args) => args,
        Err(error) => return Ok(cli_message(&error)),
    };

    // What a failure looks like depends on what the caller asked for: a record in JSON mode,
    // a sentence otherwise. The message itself is the same either way.
    let json = args.command.is_json();
    match run_command(args, started) {
        Ok(()) => Ok(0),
        Err(error) if is_broken_pipe(&error) => Err(error),
        Err(error) if json => {
            let mut payload = String::with_capacity(64);
            json::error(&format!("{error:#}"), &mut payload);
            emit(&payload, None)?;
            Ok(1)
        }
        Err(error) => Err(error),
    }
}

/// Runs the parsed command.
fn run_command(args: Cli, started: Instant) -> Result<()> {
    match args.command {
        Command::Findrefs(args) => {
            let query = args.query()?;
            let json = args.json;
            let scan_started = Instant::now();
            let mut hits = apk::find_reference_hits(&args.apk_path, &query, args.threads, args.debug)?;
            let scanned = scan_started.elapsed();
            // The comparator is the string order, so an unstable sort produces exactly the
            // bytes a stable one would: equal lines are identical lines, and there is
            // nothing else to tie-break. Sorting a wide query's ~90k lines in parallel is
            // what keeps it off the main thread. wasm has no threads and takes the serial
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
                if json {
                    payload.push_str("{\"dex\":");
                    json::escape(&hit.dex_name, &mut payload);
                    payload.push_str(",\"member\":");
                    json::escape(&hit.member, &mut payload);
                    payload.push_str(",\"matched\":");
                    json::escape(&hit.matched, &mut payload);
                    payload.push_str("}\n");
                    continue;
                }
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
            let json = args.json;
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
                    if json {
                        // Written straight into the payload: the dotted name and the line
                        // were two allocations per class, and on a 76,982-class index that
                        // was the whole of the render's 47 ms in the host build.
                        payload.push_str("{\"dex\":");
                        json::escape(&class.dex_name, &mut payload);
                        payload.push_str(",\"descriptor\":");
                        json::escape(&class.descriptor, &mut payload);
                        payload.push_str(",\"name\":");
                        json::escape_dotted(class.java_name(), &mut payload);
                        payload.push_str("}\n");
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
            if args.count {
                // A header field per DEX entry: no string data is read, which is the
                // difference between a label and a table.
                let count = apk::count_strings(&args.apk_path, args.threads, args.debug)?;
                emit(&format!("{{\"count\":{count}}}\n"), args.output.as_deref())?;
                return Ok(());
            }
            if args.xrefs {
                let counts = apk::list_string_reference_counts(&args.apk_path, args.threads, args.debug)?;
                let mut payload = String::with_capacity(counts.len() * 48);
                for entry in &counts {
                    if args.json {
                        payload.push_str("{\"dex\":");
                        crate::json::escape(&entry.dex_name, &mut payload);
                        payload.push_str(",\"index\":");
                        payload.push_str(&entry.index.to_string());
                        payload.push_str(",\"count\":");
                        payload.push_str(&entry.count.to_string());
                        payload.push_str("}\n");
                        continue;
                    }
                    payload.push_str(&entry.dex_name);
                    payload.push_str(" | ");
                    payload.push_str(&entry.index.to_string());
                    payload.push_str(" | ");
                    payload.push_str(&entry.count.to_string());
                    payload.push('\n');
                }
                emit(&payload, args.output.as_deref())?;
                return Ok(());
            }
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
                if args.json {
                    payload.push_str("{\"dex\":");
                    crate::json::escape(&entry.dex_name, &mut payload);
                    payload.push_str(",\"index\":");
                    payload.push_str(&entry.index.to_string());
                    payload.push_str(",\"value\":");
                    crate::json::escape(&entry.value, &mut payload);
                    payload.push_str("}\n");
                    continue;
                }
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
                if args.json {
                    payload.push_str("{\"name\":");
                    crate::json::escape(&entry.name, &mut payload);
                    payload.push_str(",\"method\":");
                    payload.push_str(&entry.compression.to_string());
                    payload.push_str(",\"compressed\":");
                    payload.push_str(&entry.compressed_size.to_string());
                    payload.push_str(",\"uncompressed\":");
                    payload.push_str(&entry.uncompressed_size.to_string());
                    payload.push_str(",\"offset\":");
                    payload.push_str(&entry.local_header_offset.to_string());
                    payload.push_str("}\n");
                    continue;
                }
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
            if args.outline {
                // The record first, then the document exactly as `getclass` prints it: a
                // host that wants only the source reads the rest of the payload, and a
                // host that wants the boundaries does not run a scanner of its own.
                outline::render(&source, &mut payload);
                payload.push('\n');
            }
            payload.push_str(&source);
            payload.push('\n');
            emit(&payload, args.output.as_deref())?;
        }
    }
    Ok(())
}
