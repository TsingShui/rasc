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
