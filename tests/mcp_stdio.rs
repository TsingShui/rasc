mod support;

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn request(
    child: &mut std::process::Child,
    reader: &mut BufReader<std::process::ChildStdout>,
    value: serde_json::Value,
) -> serde_json::Value {
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
    arguments: serde_json::Value,
) -> serde_json::Value {
    request(
        child,
        reader,
        serde_json::json!({
            "jsonrpc":"2.0", "id":id, "method":"tools/call",
            "params":{"name":name,"arguments":arguments}
        }),
    )
}

fn start_with_threads(
    root: &std::path::Path,
    threads: usize,
) -> (std::process::Child, BufReader<std::process::ChildStdout>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rasc"))
        .args([
            "mcp",
            "--root",
            root.to_str().unwrap(),
            "--analysis-threads",
            &threads.to_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let reader = BufReader::new(child.stdout.take().unwrap());
    (child, reader)
}

fn start(root: &std::path::Path) -> (std::process::Child, BufReader<std::process::ChildStdout>) {
    start_with_threads(root, 3)
}

fn initialize(
    child: &mut std::process::Child,
    reader: &mut BufReader<std::process::ChildStdout>,
) -> serde_json::Value {
    let initialized = request(
        child,
        reader,
        serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                       "clientInfo": {"name": "rasc-test", "version": "1"}}
        }),
    );
    writeln!(
        child.stdin.as_mut().unwrap(),
        "{}",
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}})
    )
    .unwrap();
    initialized
}

fn send(child: &mut std::process::Child, value: serde_json::Value) {
    writeln!(child.stdin.as_mut().unwrap(), "{value}").unwrap();
    child.stdin.as_mut().unwrap().flush().unwrap();
}

fn stop(mut child: std::process::Child) {
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdio_parallel_results_match_serial_and_report_configured_threads() {
    let root = tempfile::tempdir().unwrap();
    let first = support::const_string_dex(2);
    let second = support::const_string_dex(3);
    let apk = root.path().join("parallel.apk");
    std::fs::write(
        &apk,
        support::deflated_zip(&[
            ("classes.dex", first.as_slice()),
            ("classes2.dex", second.as_slice()),
        ]),
    )
    .unwrap();

    let mut outputs = Vec::new();
    for threads in [1, 4] {
        let (mut child, mut reader) = start_with_threads(root.path(), threads);
        initialize(&mut child, &mut reader);
        let opened = call(
            &mut child,
            &mut reader,
            2,
            "open",
            serde_json::json!({"path":apk}),
        );
        let target = opened["result"]["structuredContent"]["data"]["target_id"]
            .as_str()
            .unwrap();
        let classes = call(
            &mut child,
            &mut reader,
            3,
            "classes",
            serde_json::json!({"target_id":target}),
        );
        let strings = call(
            &mut child,
            &mut reader,
            4,
            "strings",
            serde_json::json!({"target_id":target}),
        );
        let status = call(&mut child, &mut reader, 5, "status", serde_json::json!({}));
        let status = &status["result"]["structuredContent"]["data"];
        assert_eq!(status["analysis_threads"], threads);
        assert_eq!(status["inflate_cache_loads"], 2);
        assert_eq!(status["inflate_cache_hits"], 2);
        outputs.push((
            classes["result"]["structuredContent"]["data"].clone(),
            strings["result"]["structuredContent"]["data"].clone(),
        ));
        stop(child);
    }
    assert_eq!(outputs[0], outputs[1]);
}

#[test]
fn stdio_server_runs_a_complete_bounded_workflow() {
    let root = tempfile::tempdir().unwrap();
    let dex = support::const_string_dex(2);
    let manifest = b"<?xml version=\"1.0\"?><manifest package=\"example\"/>";
    std::fs::write(
        root.path().join("sample.apk"),
        support::stored_zip(&[
            ("classes.dex", dex.as_slice()),
            ("AndroidManifest.xml", manifest.as_slice()),
        ]),
    )
    .unwrap();
    let (mut child, mut reader) = start(root.path());

    let initialized = initialize(&mut child, &mut reader);
    assert_eq!(initialized["result"]["serverInfo"]["name"], "rasc");
    assert!(
        initialized["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("filter or aggregate them in code mode")
    );

    let tools = request(
        &mut child,
        &mut reader,
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    );
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 9);
    assert!(
        tools.iter().all(|tool| {
            tool.get("inputSchema").is_some() && tool.get("outputSchema").is_some()
        })
    );
    for tool in tools {
        let schema = serde_json::to_string(tool).unwrap();
        for forbidden in [
            "cursor",
            "offset",
            "limit",
            "next_cursor",
            "text_id",
            "read_text",
        ] {
            assert!(
                !schema.contains(forbidden),
                "{} exposes {forbidden}",
                tool["name"]
            );
        }
        if matches!(
            tool["name"].as_str(),
            Some("classes" | "strings" | "entries")
        ) {
            assert!(
                !schema.contains("filter"),
                "{} exposes filter",
                tool["name"]
            );
        }
    }

    let opened = call(
        &mut child,
        &mut reader,
        3,
        "open",
        serde_json::json!({"path":root.path().join("sample.apk")}),
    );
    let body = &opened["result"]["structuredContent"];
    assert_eq!(body["ok"], true);
    let target = body["data"]["target_id"].as_str().unwrap();

    let status = call(&mut child, &mut reader, 30, "status", serde_json::json!({}));
    assert_eq!(
        status["result"]["structuredContent"]["data"]["analysis_threads"],
        3
    );
    assert_eq!(
        status["result"]["structuredContent"]["data"]["max_concurrent_requests"],
        2
    );

    let entries = call(
        &mut child,
        &mut reader,
        4,
        "entries",
        serde_json::json!({"target_id":target}),
    );
    assert_eq!(entries["result"]["structuredContent"]["ok"], true);
    assert_eq!(
        entries["result"]["structuredContent"]["data"]["items"][0]["name"],
        "classes.dex"
    );

    let classes = call(
        &mut child,
        &mut reader,
        5,
        "classes",
        serde_json::json!({"target_id":target}),
    );
    let class_rows = classes["result"]["structuredContent"]["data"]["items"]
        .as_array()
        .unwrap();
    assert_eq!(class_rows.len(), 2);
    let class_id = class_rows[0]["class_id"].as_str().unwrap();

    let strings = call(
        &mut child,
        &mut reader,
        6,
        "strings",
        serde_json::json!({"target_id":target}),
    );
    assert!(
        strings["result"]["structuredContent"]["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["value"] == "Authorization")
    );

    let references = call(
        &mut child,
        &mut reader,
        7,
        "findrefs",
        serde_json::json!({
            "target_id":target, "kind":"string", "value":"Authorization"
        }),
    );
    assert_eq!(
        references["result"]["structuredContent"]["data"]["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let source = call(
        &mut child,
        &mut reader,
        8,
        "getclass",
        serde_json::json!({"target_id":target,"class_id":class_id}),
    );
    assert_eq!(source["result"]["structuredContent"]["ok"], true);
    assert!(
        source["result"]["structuredContent"]["data"]["source"]
            .as_str()
            .unwrap()
            .contains("class Fixture0")
    );

    let decoded_manifest = call(
        &mut child,
        &mut reader,
        9,
        "manifest",
        serde_json::json!({"target_id":target}),
    );
    assert_eq!(
        decoded_manifest["result"]["structuredContent"]["data"]["xml"],
        String::from_utf8_lossy(manifest).as_ref()
    );

    let invalid = call(
        &mut child,
        &mut reader,
        10,
        "getclass",
        serde_json::json!({"target_id":target}),
    );
    assert_eq!(invalid["result"]["isError"], true);
    assert_eq!(invalid["result"]["structuredContent"]["ok"], false);
    assert_eq!(
        invalid["result"]["structuredContent"]["error"]["code"],
        "INVALID_INPUT"
    );

    // Exercise a real protocol cancellation. rmcp suppresses a response for a
    // request after notifications/cancelled, so the following ping must be the
    // next response and proves the server stayed alive.
    send(
        &mut child,
        serde_json::json!({
            "jsonrpc":"2.0", "id":"cancelled-query", "method":"tools/call",
            "params":{"name":"classes","arguments":{"target_id":target}}
        }),
    );
    send(
        &mut child,
        serde_json::json!({
            "jsonrpc":"2.0", "method":"notifications/cancelled",
            "params":{"requestId":"cancelled-query"}
        }),
    );
    let ping = request(
        &mut child,
        &mut reader,
        serde_json::json!({"jsonrpc":"2.0","id":20,"method":"ping"}),
    );
    assert_eq!(ping["id"], 20);

    let closed = call(
        &mut child,
        &mut reader,
        11,
        "close",
        serde_json::json!({"target_id":target}),
    );
    assert_eq!(
        closed["result"]["structuredContent"]["data"]["closed"],
        true
    );

    let expired = call(
        &mut child,
        &mut reader,
        12,
        "status",
        serde_json::json!({"target_id":target}),
    );
    assert_eq!(expired["result"]["isError"], true);
    assert_eq!(
        expired["result"]["structuredContent"]["error"]["code"],
        "TARGET_NOT_FOUND"
    );

    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
