//! Optional Java-bytecode input preparation for the one-shot CLI.
//!
//! APK/DEX inputs remain on the native path. A conventional JAR, or the JARs held
//! by an AAR, are passed to a system-installed d8 and presented to the existing
//! analyzer as a temporary DEX ZIP. This module deliberately is not used by MCP.

use crate::analysis::apk::{self, ArchivePolicy};
use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::{NamedTempFile, TempDir};

const MAX_D8_DIAGNOSTIC_BYTES: usize = 64 << 10;
const DEFAULT_D8_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, Default)]
pub(crate) struct InputPolicy {
    /// `None` means automatic discovery. An explicitly configured path is never
    /// silently replaced by another installation.
    pub(crate) d8: Option<PathBuf>,
    pub(crate) disable_d8: bool,
}

/// A native input, or a temporary DEX ZIP produced from Java bytecode.
///
/// Holding this value keeps every temporary input alive for the entire analysis.
pub(crate) struct PreparedInput {
    analysis_path: PathBuf,
    _temporary: Option<TempDir>,
}

impl PreparedInput {
    pub(crate) fn path(&self) -> &Path {
        &self.analysis_path
    }
}

pub(crate) fn prepare(
    path: &Path,
    archive_policy: ArchivePolicy,
    policy: &InputPolicy,
) -> Result<PreparedInput> {
    let entries = match apk::list_entries(path) {
        Ok(entries) => entries,
        // Bare DEX is represented by list_entries as classes.dex. A malformed or
        // unsupported input keeps the native parser's existing diagnostic.
        Err(_) => {
            return Ok(PreparedInput {
                analysis_path: path.to_owned(),
                _temporary: None,
            });
        }
    };
    if entries
        .iter()
        .any(|entry| !entry.name.contains('/') && entry.name.ends_with(".dex"))
        || entries.len() == 1 && entries[0].bare
    {
        return Ok(PreparedInput {
            analysis_path: path.to_owned(),
            _temporary: None,
        });
    }

    let is_aar = entries
        .iter()
        .any(|entry| entry.name == "AndroidManifest.xml")
        && entries.iter().any(|entry| entry.name == "classes.jar");
    let is_jar = entries.iter().any(|entry| entry.name.ends_with(".class"));
    if !is_aar && !is_jar {
        bail!(
            "archive contains no root DEX or Java bytecode; rasc code analysis supports APK, DEX, JAR, and AAR inputs"
        );
    }
    if policy.disable_d8 {
        bail!("Java bytecode input requires d8, but d8 conversion is disabled by --no-d8");
    }

    let d8 = discover_d8(policy.d8.as_deref())?;
    let android_jar = discover_android_jar(&d8);
    let temporary = tempfile::tempdir().context("create temporary d8 workspace")?;
    let program_inputs = if is_aar {
        extract_aar_jars(path, &entries, archive_policy, temporary.path())?
    } else {
        vec![path.to_owned()]
    };
    if program_inputs.is_empty() {
        bail!("AAR contains no Java bytecode in classes.jar or libs/*.jar");
    }

    let output = temporary.path().join("classes.zip");
    run_d8(&d8, android_jar.as_deref(), &program_inputs, &output)?;
    validate_output(&output)?;
    Ok(PreparedInput {
        analysis_path: output,
        _temporary: Some(temporary),
    })
}

fn extract_aar_jars(
    aar: &Path,
    entries: &[crate::analysis::archive::ZipEntry],
    archive_policy: ArchivePolicy,
    directory: &Path,
) -> Result<Vec<PathBuf>> {
    let names = entries
        .iter()
        .filter(|entry| {
            entry.name == "classes.jar"
                || entry
                    .name
                    .strip_prefix("libs/")
                    .is_some_and(|name| !name.contains('/') && name.ends_with(".jar"))
        })
        .map(|entry| entry.name.clone())
        .collect::<Vec<_>>();
    let mut paths = Vec::with_capacity(names.len());
    for (index, name) in names.iter().enumerate() {
        let data = apk::read_archive_entry_with_policy(aar, archive_policy, name)
            .with_context(|| format!("read {name} from {}", aar.display()))?;
        if !archive_contains_class(&data)? {
            continue;
        }
        let path = directory.join(format!("program-{index:04}.jar"));
        fs::write(&path, data).with_context(|| format!("write temporary {name}"))?;
        paths.push(path);
    }
    Ok(paths)
}

fn archive_contains_class(bytes: &[u8]) -> Result<bool> {
    let entries = crate::analysis::archive::parse_zip_entries(bytes, |name| {
        name.ends_with(b".class") && !name.ends_with(b"module-info.class")
    })
    .context("parse embedded JAR")?;
    Ok(!entries.is_empty())
}

fn discover_d8(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return executable(path).with_context(|| {
            format!(
                "configured d8 is not an executable file: {}",
                path.display()
            )
        });
    }
    if let Some(path) = env::var_os("RASC_D8") {
        let path = PathBuf::from(path);
        return executable(&path).with_context(|| {
            format!(
                "RASC_D8 does not name an executable file: {}",
                path.display()
            )
        });
    }

    let mut sdk_roots = Vec::new();
    for name in ["ANDROID_SDK_ROOT", "ANDROID_HOME"] {
        if let Some(root) = env::var_os(name) {
            push_unique(&mut sdk_roots, PathBuf::from(root));
        }
    }
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        push_unique(&mut sdk_roots, home.join("Library/Android/sdk"));
        push_unique(&mut sdk_roots, home.join("Android/Sdk"));
    }
    if cfg!(windows)
        && let Some(local) = env::var_os("LOCALAPPDATA")
    {
        push_unique(&mut sdk_roots, PathBuf::from(local).join("Android/Sdk"));
    }
    for root in sdk_roots {
        if let Some(path) = newest_sdk_d8(&root) {
            return Ok(path);
        }
    }
    if let Some(path) = find_on_path("d8") {
        return Ok(path);
    }
    bail!(
        "Java bytecode input requires d8; install Android SDK Build Tools, set RASC_D8, or pass --d8 PATH"
    )
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}

fn newest_sdk_d8(root: &Path) -> Option<PathBuf> {
    let build_tools = root.join("build-tools");
    let mut candidates = fs::read_dir(build_tools)
        .ok()?
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| version_cmp(&left.file_name(), &right.file_name()));
    candidates.into_iter().rev().find_map(|entry| {
        let path = entry.path().join(d8_filename());
        executable(&path)
    })
}

fn version_cmp(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> std::cmp::Ordering {
    let left = left.to_string_lossy();
    let right = right.to_string_lossy();
    let mut left_parts = left.split(['.', '-']);
    let mut right_parts = right.split(['.', '-']);
    loop {
        match (left_parts.next(), right_parts.next()) {
            (Some(left), Some(right)) => {
                let order = match (left.parse::<u64>(), right.parse::<u64>()) {
                    (Ok(left), Ok(right)) => left.cmp(&right),
                    _ => left.cmp(right),
                };
                if order != std::cmp::Ordering::Equal {
                    return order;
                }
            }
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (None, None) => return std::cmp::Ordering::Equal,
        }
    }
}

fn d8_filename() -> &'static str {
    if cfg!(windows) { "d8.bat" } else { "d8" }
}

fn executable(path: &Path) -> Option<PathBuf> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    Some(path.to_owned())
}

fn find_on_path(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for directory in env::split_paths(&path) {
        let candidate = directory.join(if cfg!(windows) {
            format!("{command}.bat")
        } else {
            command.to_owned()
        });
        if let Some(candidate) = executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn discover_android_jar(d8: &Path) -> Option<PathBuf> {
    if let Some(path) = env::var_os("RASC_ANDROID_JAR") {
        let path = PathBuf::from(path);
        return fs::metadata(&path)
            .is_ok_and(|metadata| metadata.is_file())
            .then_some(path);
    }
    // SDK d8 lives at <sdk>/build-tools/<version>/d8. An explicitly supplied
    // standalone d8 simply runs without --lib.
    let sdk = d8.parent()?.parent()?.parent()?;
    let platforms = sdk.join("platforms");
    let mut candidates = fs::read_dir(platforms)
        .ok()?
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| version_cmp(&left.file_name(), &right.file_name()));
    candidates.into_iter().rev().find_map(|entry| {
        let path = entry.path().join("android.jar");
        fs::metadata(&path)
            .is_ok_and(|metadata| metadata.is_file())
            .then_some(path)
    })
}

fn run_d8(d8: &Path, android_jar: Option<&Path>, inputs: &[PathBuf], output: &Path) -> Result<()> {
    // Write stdout/stderr straight to files so an arbitrarily chatty converter
    // cannot fill a pipe or retain unbounded diagnostics in memory.
    let stdout = NamedTempFile::new().context("create d8 stdout capture")?;
    let stderr = NamedTempFile::new().context("create d8 stderr capture")?;
    let mut command = Command::new(d8);
    command
        .arg("-JXmx1024M")
        .arg("--debug")
        .arg("--min-api")
        .arg("1");
    if let Some(android_jar) = android_jar {
        command.arg("--lib").arg(android_jar);
    }
    command
        .arg("--output")
        .arg(output)
        .args(inputs)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen()?))
        .stderr(Stdio::from(stderr.reopen()?));
    let mut child = command
        .spawn()
        .with_context(|| format!("start d8 at {}", d8.display()))?;
    let timeout = d8_timeout();
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error).context("wait for d8");
            }
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            emit_diagnostics(&stdout, &stderr)?;
            bail!(
                "d8 conversion timed out after {} seconds",
                timeout.as_secs()
            );
        }
        thread::sleep(Duration::from_millis(20));
    };
    emit_diagnostics(&stdout, &stderr)?;
    if !status.success() {
        bail!("d8 conversion failed with {status}");
    }
    Ok(())
}

fn d8_timeout() -> Duration {
    env::var("RASC_D8_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds != 0)
        .map_or(DEFAULT_D8_TIMEOUT, Duration::from_secs)
}

fn emit_diagnostics(stdout: &NamedTempFile, stderr: &NamedTempFile) -> Result<()> {
    let stdout_text = read_diagnostic(stdout)?;
    let stderr_text = read_diagnostic(stderr)?;
    for text in [&stdout_text, &stderr_text] {
        if !text.trim().is_empty() {
            eprint!("{text}");
            if !text.ends_with('\n') {
                eprintln!();
            }
        }
    }
    Ok(())
}

fn read_diagnostic(file: &NamedTempFile) -> Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = file.reopen()?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(MAX_D8_DIAGNOSTIC_BYTES as u64);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(if start == 0 {
        text.into_owned()
    } else {
        format!("[d8 diagnostics truncated]\n{text}")
    })
}

fn validate_output(path: &Path) -> Result<()> {
    let entries = apk::list_entries(path).context("read d8 output")?;
    if entries.iter().any(|entry| {
        !entry.name.contains('/')
            && entry.name.starts_with("classes")
            && entry.name.ends_with(".dex")
    }) {
        Ok(())
    } else {
        bail!("d8 completed without producing a root classes*.dex entry")
    }
}
