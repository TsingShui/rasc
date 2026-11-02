use super::archive::{EntryPayload, EntryRecord, read_entry};
use super::input::{InputError, InputKind, InputLease};

/// One root DEX image borrowed from a lease. `entry` is `None` only for a bare
/// DEX input; `label` is display metadata and never decides identity.
#[derive(Debug)]
pub struct DexRoot<'a> {
    label: String,
    entry: Option<&'a EntryRecord>,
    payload: EntryPayload<'a>,
}

impl<'a> DexRoot<'a> {
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn entry(&self) -> Option<&'a EntryRecord> {
        self.entry
    }
    pub fn bytes(&self) -> &[u8] {
        self.payload.bytes()
    }
}

/// Selects the root DEX images in physical directory order.
///
/// A bare DEX is one root labelled with the input file name. An archive
/// contributes `classes.dex` and `classes<index>.dex` records; a duplicate root
/// name still yields one root per physical record, in directory order, rather than
/// being deduplicated by name.
pub fn roots(lease: &InputLease) -> Result<Vec<DexRoot<'_>>, InputError> {
    match lease.kind() {
        InputKind::BareDex => {
            let label = lease.identity().path().file_name().map_or_else(
                || "input.dex".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            );
            Ok(vec![DexRoot {
                label,
                entry: None,
                payload: EntryPayload::Borrowed(lease.bytes()),
            }])
        }
        InputKind::Archive => {
            let directory = lease.archive().ok_or(InputError::MalformedArchive(
                super::input::ArchiveProblem::InconsistentDirectory,
            ))?;
            let mut roots = Vec::new();
            for entry in directory.entries() {
                if !is_root_name(entry.name()) {
                    continue;
                }
                let payload = read_entry(lease.bytes(), entry, lease.limits())
                    .map_err(InputError::MalformedArchive)?;
                roots.try_reserve(1).map_err(InputError::Allocation)?;
                roots.push(DexRoot {
                    label: entry.name().to_owned(),
                    entry: Some(entry),
                    payload,
                });
            }
            roots.try_reserve(1).map_err(InputError::Allocation)?;
            Ok(roots)
        }
        InputKind::Opaque => Err(InputError::NotDex),
    }
}

/// `classes.dex` or `classes<N>.dex` with `N >= 2` and no leading zero, matching
/// the root naming rule. A bare other name is not a root.
pub fn is_root_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("classes") else {
        return false;
    };
    let Some(index) = rest.strip_suffix(".dex") else {
        return false;
    };
    if index.is_empty() {
        return true;
    }
    // The first root is `classes.dex`; split archives continue at `classes2.dex`.
    if index == "1" {
        return false;
    }
    match index.as_bytes() {
        [b'0', ..] => false,
        _ => index.bytes().all(|byte| byte.is_ascii_digit()),
    }
}
