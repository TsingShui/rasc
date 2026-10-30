#![allow(dead_code)]

#[cfg(feature = "apk")]
use error::DexError;

use crate::dex::file::DexFile;
#[cfg(feature = "apk")]
use crate::dex::reader::DexReader;

pub mod adler32;
pub mod dex;
pub mod error;
mod mutf8;

/// Parse an APK and create a `DexFile` object from the embedded class(es) files
#[cfg(feature = "apk")]
pub fn parse(filepath: &str) -> Result<DexFile, DexError> {
    let readers = DexReader::build_from_file(filepath)?;
    DexFile::merge(readers)
}

/// Return the list of qualified method names from a `DexFile` object
pub fn get_qualified_method_names(dex: &DexFile) -> Vec<String> {
    let mut methods = Vec::new();

    let class_names = dex.get_classes_names();
    for class in &class_names {
        if let Some(class_def) = dex.classes.get_class_def(&dex.types, &dex.strings, class) {
            for method in class_def.get_methods() {
                let name = method.get_method_name();
                methods.push(format!("{class}->{name}"));
            }
        }
    }

    methods
}

/// Get the list of instructions for the given method
pub fn get_bytecode_for_method(
    dex: &DexFile,
    class_name: &str,
    method_name: &str,
) -> Option<Vec<u16>> {
    let class_def = dex.get_class_def(class_name)?;
    let encoded_method = class_def.get_encoded_method(method_name)?;
    encoded_method.code_item.as_ref()?.insns.clone()
}
