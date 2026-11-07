use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, DexId, DexImage, Engine, ImageOrigin, InputId, Request,
    input::DexContainer, p9::RenderOutcome,
};
use std::{collections::HashMap, io::Read, path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct Class {
    pub header: ByteOffset,
    pub id: ClassDefId,
    pub label: String,
}
pub struct PackageGroup {
    pub name: String,
    pub classes: Vec<usize>,
    pub children: Vec<usize>,
}
pub struct DexGroup {
    pub header: ByteOffset,
    pub packages: Vec<PackageGroup>,
    pub roots: Vec<usize>,
}
pub struct Resource {
    pub name: String,
    pub bytes: Arc<[u8]>,
    pub classes: Vec<Class>,
    pub groups: Vec<DexGroup>,
    class_index: HashMap<(ByteOffset, ClassDefId), usize>,
    pub issues: Vec<String>,
}

impl Resource {
    pub fn class(&self, header: ByteOffset, id: ClassDefId) -> Option<Class> {
        self.classes
            .get(*self.class_index.get(&(header, id))?)
            .cloned()
    }
}

pub fn open(path: PathBuf) -> Result<Resource, String> {
    let mut file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        let count = file.read(&mut chunk).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        bytes.try_reserve(count).map_err(|e| e.to_string())?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    let bytes: Arc<[u8]> = bytes.into();
    let container = DexContainer::open(&bytes).map_err(|e| e.to_string())?;
    let headers = container.logical_headers().map_err(|e| e.to_string())?;
    let mut classes = Vec::new();
    let mut issues = Vec::new();
    for header in headers {
        let program = match container.program(header) {
            Ok(p) => p,
            Err(e) => {
                issues.push(format!("DEX at {}: {e}", header.0));
                continue;
            }
        };
        program
            .visit_classes(|id, result| {
                match result {
                    Ok(class) => match class.descriptor.units() {
                        Ok(units) => {
                            classes.try_reserve(1).map_err(|_| {
                                diana_decomp::input::InputError::Allocation {
                                    offset_byte: header,
                                }
                            })?;
                            classes.push(Class {
                                header,
                                id,
                                label: String::from_utf16_lossy(&units),
                            });
                        }
                        Err(e) => {
                            if matches!(
                                e.kind(),
                                diana_decomp::FailureKind::ResourceExhausted
                                    | diana_decomp::FailureKind::InternalInvariant
                            ) {
                                return Err(e);
                            }
                            issues.push(format!("class {}: {e}", id.0));
                        }
                    },
                    Err(e) => issues.push(format!("class {}: {e}", id.0)),
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
    }
    let mut groups: Vec<DexGroup> = Vec::new();
    let mut header_index = HashMap::new();
    let mut package_index = HashMap::new();
    let mut class_index = HashMap::new();
    for (index, class) in classes.iter().enumerate() {
        class_index.try_reserve(1).map_err(|e| e.to_string())?;
        class_index.insert((class.header, class.id), index);
        let header = if let Some(&header) = header_index.get(&class.header) {
            header
        } else {
            groups.try_reserve(1).map_err(|e| e.to_string())?;
            header_index.try_reserve(1).map_err(|e| e.to_string())?;
            let header = groups.len();
            groups.push(DexGroup {
                header: class.header,
                packages: Vec::new(),
                roots: Vec::new(),
            });
            header_index.insert(class.header, header);
            header
        };
        let descriptor = class
            .label
            .strip_prefix('L')
            .and_then(|v| v.strip_suffix(';'))
            .unwrap_or(&class.label);
        let package = descriptor
            .rsplit_once('/')
            .map_or("", |(package, _)| package);
        let dex = &mut groups[header];
        let mut parent: Option<usize> = None;
        for component in package.split('/') {
            let key = (header, parent, package.is_empty(), component.to_owned());
            let node = if let Some(&node) = package_index.get(&key) {
                node
            } else {
                dex.packages.try_reserve(1).map_err(|e| e.to_string())?;
                package_index.try_reserve(1).map_err(|e| e.to_string())?;
                let node = dex.packages.len();
                let siblings = match parent {
                    Some(parent) => &mut dex.packages[parent].children,
                    None => &mut dex.roots,
                };
                siblings.try_reserve(1).map_err(|e| e.to_string())?;
                siblings.push(node);
                dex.packages.push(PackageGroup {
                    name: if package.is_empty() {
                        "(default package)".to_owned()
                    } else {
                        component.to_owned()
                    },
                    classes: Vec::new(),
                    children: Vec::new(),
                });
                package_index.insert(key, node);
                node
            };
            parent = Some(node);
        }
        let node = parent.ok_or("package has no component")?;
        dex.packages[node]
            .classes
            .try_reserve(1)
            .map_err(|e| e.to_string())?;
        dex.packages[node].classes.push(index);
    }
    Ok(Resource {
        name: path.display().to_string(),
        bytes,
        classes,
        groups,
        class_index,
        issues,
    })
}

pub fn decompile(bytes: Arc<[u8]>, class: Class) -> Result<String, String> {
    let engine = Engine::open(vec![DexImage {
        bytes: &bytes,
        header_offset: class.header,
        origin: ImageOrigin { input: InputId(0) },
    }])
    .map_err(|e| e.to_string())?;
    let result = engine
        .analyze(Request {
            class: ClassKey {
                dex: DexId(0),
                class_def: class.id,
            },
            method: None,
        })
        .map_err(|e| e.to_string())?;
    match result.rendered {
        RenderOutcome::Ready(value) => {
            let mut text = value.text().to_owned();
            for issue in value.issues() {
                text.push_str(&format!(
                    "\n/* {:?}: {} ({:?}) */",
                    issue.kind, issue.detail, issue.cause
                ));
            }
            Ok(text)
        }
        RenderOutcome::Stopped(value) => Ok(format!(
            "{}\n\n/* Rendering stopped: {} */",
            value.text_prefix(),
            value.error()
        )),
    }
}
