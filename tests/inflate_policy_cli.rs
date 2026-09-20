use flate2::{Compress, Compression, FlushCompress};
use std::process::Command;

fn deflate(payload: &[u8]) -> Vec<u8> {
    let mut compressor = Compress::new(Compression::best(), false);
    let mut compressed = vec![0; payload.len() + 1024];
    compressor
        .compress(payload, &mut compressed, FlushCompress::Finish)
        .expect("deflate fixture");
    compressed.truncate(compressor.total_out() as usize);
    compressed
}

fn one_entry_zip(name: &str, payload: &[u8]) -> Vec<u8> {
    let compressed = deflate(payload);
    let mut local = Vec::new();
    local.extend_from_slice(b"PK\x03\x04");
    local.extend_from_slice(&20u16.to_le_bytes());
    local.extend_from_slice(&0u16.to_le_bytes());
    local.extend_from_slice(&8u16.to_le_bytes());
    local.extend_from_slice(&[0; 4]);
    local.extend_from_slice(&0u32.to_le_bytes());
    local.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    local.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    local.extend_from_slice(&(name.len() as u16).to_le_bytes());
    local.extend_from_slice(&0u16.to_le_bytes());
    local.extend_from_slice(name.as_bytes());
    local.extend_from_slice(&compressed);

    let central_offset = local.len() as u32;
    let mut central = Vec::new();
    central.extend_from_slice(b"PK\x01\x02");
    central.extend_from_slice(&20u16.to_le_bytes());
    central.extend_from_slice(&20u16.to_le_bytes());
    central.extend_from_slice(&0u16.to_le_bytes());
    central.extend_from_slice(&8u16.to_le_bytes());
    central.extend_from_slice(&[0; 4]);
    central.extend_from_slice(&0u32.to_le_bytes());
    central.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    central.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    central.extend_from_slice(&(name.len() as u16).to_le_bytes());
    central.extend_from_slice(&0u16.to_le_bytes());
    central.extend_from_slice(&0u16.to_le_bytes());
    central.extend_from_slice(&0u16.to_le_bytes());
    central.extend_from_slice(&0u16.to_le_bytes());
    central.extend_from_slice(&0u32.to_le_bytes());
    central.extend_from_slice(&0u32.to_le_bytes());
    central.extend_from_slice(name.as_bytes());

    let central_size = central.len() as u32;
    local.extend_from_slice(&central);
    local.extend_from_slice(b"PK\x05\x06");
    local.extend_from_slice(&0u16.to_le_bytes());
    local.extend_from_slice(&0u16.to_le_bytes());
    local.extend_from_slice(&1u16.to_le_bytes());
    local.extend_from_slice(&1u16.to_le_bytes());
    local.extend_from_slice(&central_size.to_le_bytes());
    local.extend_from_slice(&central_offset.to_le_bytes());
    local.extend_from_slice(&0u16.to_le_bytes());
    local
}

fn run_manifest(path: &std::path::Path, limit: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rasc"))
        .args(["manifest", path.to_str().expect("UTF-8 temp path")])
        .env("RASC_MAX_INFLATED_ENTRY", limit)
        .output()
        .expect("run rasc")
}

#[test]
fn cli_inflation_policy_preserves_streams_status_and_environment_semantics() {
    let mut payload = b"<manifest>".to_vec();
    payload.resize(128 * 1024, b' ');
    let path = std::env::temp_dir().join(format!(
        "rasc-inflate-policy-cli-{}.apk",
        std::process::id()
    ));
    std::fs::write(&path, one_entry_zip("AndroidManifest.xml", &payload)).unwrap();

    let limited = run_manifest(&path, "1024");
    assert_eq!(limited.status.code(), Some(1));
    assert!(limited.stdout.is_empty());
    assert_eq!(
        limited.stderr,
        b"Error: AndroidManifest.xml inflates past the 1024 bytes entry limit\n"
    );

    for value in ["0", "invalid"] {
        let output = run_manifest(&path, value);
        assert_eq!(output.status.code(), Some(0), "value={value}");
        assert_eq!(output.stdout, payload, "value={value}");
        assert!(output.stderr.is_empty(), "value={value}");
    }

    std::fs::remove_file(path).unwrap();
}
