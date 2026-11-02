//! Shared application-test fixtures: a private temporary directory and a minimal
//! stored/deflate ZIP writer. These are fixtures, not product code.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use diana::analysis::archive::crc32;

static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "diana-test-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub struct ZipEntry<'a> {
    pub name: &'a str,
    /// Uncompressed payload; it determines the CRC and declared size.
    pub data: &'a [u8],
    /// Bytes actually stored in the entry; defaults to `data` for stored records.
    pub compressed: Option<&'a [u8]>,
    pub method: u16,
    pub flags: u16,
    pub crc: Option<u32>,
}

pub fn stored_entry<'a>(name: &'a str, data: &'a [u8]) -> ZipEntry<'a> {
    ZipEntry {
        name,
        data,
        compressed: None,
        method: 0,
        flags: 0,
        crc: None,
    }
}

pub fn zip(entries: &[ZipEntry<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for entry in entries {
        let offset = u32::try_from(out.len()).unwrap();
        let name = entry.name.as_bytes();
        let crc = entry.crc.unwrap_or_else(|| crc32(entry.data));
        let stored = entry.compressed.unwrap_or(entry.data);
        let size = u32::try_from(stored.len()).unwrap();
        let uncompressed = u32::try_from(entry.data.len()).unwrap();
        let mut local = Vec::new();
        local.extend_from_slice(b"PK\x03\x04");
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&entry.flags.to_le_bytes());
        local.extend_from_slice(&entry.method.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&uncompressed.to_le_bytes());
        local.extend_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name);
        local.extend_from_slice(stored);
        out.extend_from_slice(&local);

        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&entry.flags.to_le_bytes());
        central.extend_from_slice(&entry.method.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&uncompressed.to_le_bytes());
        central.extend_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let directory_offset = u32::try_from(out.len()).unwrap();
    out.extend_from_slice(&central);
    let directory_size = u32::try_from(central.len()).unwrap();
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&directory_size.to_le_bytes());
    out.extend_from_slice(&directory_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// A self-made bare DEX prefix with a distinct payload byte.
pub fn dex(payload: u8) -> Vec<u8> {
    let mut bytes = b"dex\n035\0".to_vec();
    bytes.extend_from_slice(&[payload; 16]);
    bytes
}

/// Minimal UTF-16 AXML `AndroidManifest.xml` with one `manifest` element that
/// carries the `package` attribute. Self-made fixture, not product output.
pub fn axml_manifest(package: &str) -> Vec<u8> {
    fn chunk(kind: u16, header: u16, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&header.to_le_bytes());
        out.extend_from_slice(&u32::try_from(8 + body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(body);
        out
    }
    let strings = ["package", "manifest", package];
    let mut offsets = Vec::new();
    let mut data = Vec::new();
    for entry in &strings {
        offsets.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
        let units: Vec<u16> = entry.encode_utf16().collect();
        data.extend_from_slice(&u16::try_from(units.len()).unwrap().to_le_bytes());
        for unit in units {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        data.extend_from_slice(&[0, 0]);
    }
    let strings_start = 28 + offsets.len();
    let mut pool = Vec::new();
    pool.extend_from_slice(&u32::try_from(strings.len()).unwrap().to_le_bytes());
    pool.extend_from_slice(&0u32.to_le_bytes());
    pool.extend_from_slice(&0u32.to_le_bytes());
    pool.extend_from_slice(&u32::try_from(strings_start).unwrap().to_le_bytes());
    pool.extend_from_slice(&0u32.to_le_bytes());
    pool.extend_from_slice(&offsets);
    pool.extend_from_slice(&data);

    let mut element = Vec::new();
    element.extend_from_slice(&1u32.to_le_bytes());
    element.extend_from_slice(&u32::MAX.to_le_bytes());
    element.extend_from_slice(&u32::MAX.to_le_bytes());
    element.extend_from_slice(&1u32.to_le_bytes());
    element.extend_from_slice(&20u16.to_le_bytes());
    element.extend_from_slice(&20u16.to_le_bytes());
    element.extend_from_slice(&1u16.to_le_bytes());
    element.extend_from_slice(&0u16.to_le_bytes());
    element.extend_from_slice(&0u16.to_le_bytes());
    element.extend_from_slice(&0u16.to_le_bytes());
    element.extend_from_slice(&u32::MAX.to_le_bytes());
    element.extend_from_slice(&0u32.to_le_bytes());
    element.extend_from_slice(&u32::MAX.to_le_bytes());
    element.extend_from_slice(&8u16.to_le_bytes());
    element.push(0);
    element.push(0x03);
    element.extend_from_slice(&2u32.to_le_bytes());

    let mut end = Vec::new();
    end.extend_from_slice(&1u32.to_le_bytes());
    end.extend_from_slice(&u32::MAX.to_le_bytes());
    end.extend_from_slice(&u32::MAX.to_le_bytes());
    end.extend_from_slice(&1u32.to_le_bytes());

    let mut body = chunk(0x0001, 28, &pool);
    body.extend_from_slice(&chunk(0x0102, 16, &element));
    body.extend_from_slice(&chunk(0x0103, 16, &end));
    chunk(0x0003, 8, &body)
}
