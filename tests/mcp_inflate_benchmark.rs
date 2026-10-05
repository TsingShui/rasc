mod support;

use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn request(
    child: &mut std::process::Child,
    reader: &mut BufReader<std::process::ChildStdout>,
    value: Value,
) -> Value {
    writeln!(child.stdin.as_mut().unwrap(), "{value}").unwrap();
    child.stdin.as_mut().unwrap().flush().unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}

fn call(
    child: &mut std::process::Child,
    reader: &mut BufReader<std::process::ChildStdout>,
    id: u64,
    name: &str,
    arguments: Value,
) -> (Value, Duration) {
    let started = Instant::now();
    let value = request(
        child,
        reader,
        serde_json::json!({
            "jsonrpc":"2.0", "id":id, "method":"tools/call",
            "params":{"name":name,"arguments":arguments}
        }),
    );
    (value, started.elapsed())
}

#[test]
#[ignore = "release benchmark: run explicitly; synthetic fixture is not a real-APK result"]
fn synthetic_deflate_cold_and_hot_workflow() {
    let root = tempfile::tempdir().unwrap();
    let dex = support::const_string_dex(20_000);
    let apk = root.path().join("synthetic.apk");
    std::fs::write(
        &apk,
        support::deflated_zip(&[("classes.dex", dex.as_slice())]),
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_rasc"))
        .args(["mcp", "--root", root.path().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    request(
        &mut child,
        &mut reader,
        serde_json::json!({
            "jsonrpc":"2.0", "id":1, "method":"initialize",
            "params":{"protocolVersion":"2025-11-25","capabilities":{},
                      "clientInfo":{"name":"benchmark","version":"1"}}
        }),
    );
    writeln!(
        child.stdin.as_mut().unwrap(),
        "{}",
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}})
    )
    .unwrap();

    let (opened, open_time) = call(
        &mut child,
        &mut reader,
        2,
        "open",
        serde_json::json!({"path":apk}),
    );
    let target = opened["result"]["structuredContent"]["data"]["target_id"]
        .as_str()
        .unwrap();
    let (_, cold_time) = call(
        &mut child,
        &mut reader,
        3,
        "classes",
        serde_json::json!({"target_id":target}),
    );
    let (cold_status, _) = call(&mut child, &mut reader, 4, "status", serde_json::json!({}));
    let cold = &cold_status["result"]["structuredContent"]["data"];
    assert_eq!(cold["inflate_cache_loads"], 1);
    assert_eq!(cold["inflate_cache_hits"], 0);

    let (_, hot_time) = call(
        &mut child,
        &mut reader,
        5,
        "strings",
        serde_json::json!({"target_id":target}),
    );
    let (hot_status, _) = call(&mut child, &mut reader, 6, "status", serde_json::json!({}));
    let hot = &hot_status["result"]["structuredContent"]["data"];
    assert_eq!(hot["inflate_cache_loads"], 1);
    assert!(hot["inflate_cache_hits"].as_u64().unwrap() > 0);

    let (close, _) = call(
        &mut child,
        &mut reader,
        7,
        "close",
        serde_json::json!({"target_id":target}),
    );
    assert_eq!(close["result"]["structuredContent"]["data"]["closed"], true);
    let (closed_status, _) = call(&mut child, &mut reader, 8, "status", serde_json::json!({}));
    let closed = &closed_status["result"]["structuredContent"]["data"];
    assert_eq!(closed["inflate_cache_entries"], 0);
    assert_eq!(closed["inflate_cache_bytes"], 0);

    #[cfg(target_os = "macos")]
    let current_rss_kib = {
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &child.id().to_string()])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .ok()
    };
    #[cfg(not(target_os = "macos"))]
    let current_rss_kib: Option<u64> = None;

    println!(
        "synthetic-only open_ms={:.3} cold_classes_ms={:.3} hot_strings_ms={:.3} loads={} hits={} cache_bytes={} current_rss_kib={}",
        open_time.as_secs_f64() * 1000.0,
        cold_time.as_secs_f64() * 1000.0,
        hot_time.as_secs_f64() * 1000.0,
        hot["inflate_cache_loads"],
        hot["inflate_cache_hits"],
        hot["inflate_cache_bytes"],
        current_rss_kib.map_or_else(|| "not-measured".to_owned(), |v| v.to_string())
    );

    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

fn run_parallel_workflow(
    apk: &std::path::Path,
    root: &std::path::Path,
    threads: usize,
) -> (Duration, Duration) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rasc"))
        .args([
            "mcp",
            "--root",
            root.to_str().unwrap(),
            "--analysis-threads",
            &threads.to_string(),
            "--max-result-items",
            "200000",
            "--max-result-bytes",
            "67108864",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    request(
        &mut child,
        &mut reader,
        serde_json::json!({
            "jsonrpc":"2.0", "id":1, "method":"initialize",
            "params":{"protocolVersion":"2025-11-25","capabilities":{},
                      "clientInfo":{"name":"parallel-benchmark","version":"1"}}
        }),
    );
    writeln!(
        child.stdin.as_mut().unwrap(),
        "{}",
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}})
    )
    .unwrap();
    let (opened, _) = call(
        &mut child,
        &mut reader,
        2,
        "open",
        serde_json::json!({"path":apk}),
    );
    let target = opened["result"]["structuredContent"]["data"]["target_id"]
        .as_str()
        .unwrap();
    let (classes, classes_elapsed) = call(
        &mut child,
        &mut reader,
        3,
        "classes",
        serde_json::json!({"target_id":target}),
    );
    assert_eq!(classes["result"]["structuredContent"]["ok"], true);
    let (strings, strings_elapsed) = call(
        &mut child,
        &mut reader,
        4,
        "strings",
        serde_json::json!({"target_id":target}),
    );
    assert_eq!(strings["result"]["structuredContent"]["ok"], true);
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    (classes_elapsed, strings_elapsed)
}

#[test]
#[ignore = "release benchmark: run explicitly; synthetic fixture is not a real-APK result"]
fn synthetic_multi_dex_analysis_threads() {
    let root = tempfile::tempdir().unwrap();
    let dexes = (0..8)
        .map(|_| support::const_string_dex(8_000))
        .collect::<Vec<_>>();
    let names = (0..dexes.len())
        .map(|index| {
            if index == 0 {
                "classes.dex".to_owned()
            } else {
                format!("classes{}.dex", index + 1)
            }
        })
        .collect::<Vec<_>>();
    let entries = names
        .iter()
        .zip(&dexes)
        .map(|(name, dex)| (name.as_str(), dex.as_slice()))
        .collect::<Vec<_>>();
    let apk = root.path().join("parallel.apk");
    std::fs::write(&apk, support::deflated_zip(&entries)).unwrap();

    let mut classes_serial = Vec::new();
    let mut strings_serial = Vec::new();
    let mut classes_parallel = Vec::new();
    let mut strings_parallel = Vec::new();
    for _ in 0..5 {
        let (classes, strings) = run_parallel_workflow(&apk, root.path(), 1);
        classes_serial.push(classes);
        strings_serial.push(strings);
        let (classes, strings) = run_parallel_workflow(&apk, root.path(), 4);
        classes_parallel.push(classes);
        strings_parallel.push(strings);
    }
    for (tool, serial, parallel) in [
        ("classes", &mut classes_serial, &mut classes_parallel),
        ("strings", &mut strings_serial, &mut strings_parallel),
    ] {
        serial.sort_unstable();
        parallel.sort_unstable();
        let serial = serial[serial.len() / 2];
        let parallel = parallel[parallel.len() / 2];
        let total_uncompressed_bytes = dexes.iter().map(Vec::len).sum::<usize>();
        let largest_entry_bytes = dexes.iter().map(Vec::len).max().unwrap_or(0);
        let parallelizable_tail_bytes = total_uncompressed_bytes - largest_entry_bytes;
        let threshold_bytes = if tool == "strings" { 32 << 20 } else { 4 << 20 };
        let traversal = if parallelizable_tail_bytes >= threshold_bytes {
            "parallel"
        } else {
            "threshold-fallback"
        };
        println!(
            "synthetic-only tool={tool} traversal={traversal} entries={} parallelizable_tail_bytes={} threshold_bytes={} median_threads1_ms={:.3} median_threads4_ms={:.3} ratio={:.3}",
            entries.len(),
            parallelizable_tail_bytes,
            threshold_bytes,
            serial.as_secs_f64() * 1000.0,
            parallel.as_secs_f64() * 1000.0,
            parallel.as_secs_f64() / serial.as_secs_f64()
        );
    }
}
