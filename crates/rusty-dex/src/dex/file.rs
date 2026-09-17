//! Representation of a DEX file
//!
//! This is the main class of the parser. When creating `DexFile` object, the builder will parse
//! the header and from there decode the contents of the file (strings, methods, etc). Some apps
//! will have multiple DEX files. In this case, the builder will initially create a `DexFile`
//! object per DEX file and then merge them into one. This extra step is needed as the contents of
//! the final `DexFile` object must contain all the contents of the intermediary DEX files but
//! sorted (the actual sorting method depends on the type of content being sorted -- see the
//! classes documentations for details).

use crate::dex::classes::{ClassDecodeLevel, ClassDefItem, ClassNestedMetadata, DexClasses, EncodedMethod};
use crate::dex::code_item::CodeItem;
use crate::dex::fields::DexFields;
use crate::dex::header::DexHeader;
use crate::dex::methods::DexMethods;
use crate::dex::protos::DexProtos;
use crate::dex::reader::DexReader;
use crate::dex::strings::DexStrings;
use crate::dex::types::DexTypes;
use crate::error::DexError;

/// Representation of a DEX file
#[derive(Debug)]
pub struct DexFile {
    source: Option<DexReader>,
    /// Header of the file
    pub header: DexHeader,
    /// List of strings defined in the DEX file
    pub strings: DexStrings,
    /// List of types defined in the DEX file
    pub types: DexTypes,
    /// List of prototypes defined in the DEX file
    pub protos: DexProtos,
    /// List of class fields defined in the DEX file
    pub fields: DexFields,
    /// List of methods defined in the DEX file
    pub methods: DexMethods,
    /// List of classes defined in the DEX file
    pub classes: DexClasses,
}

impl DexFile {
    pub(crate) fn source_reader(&self) -> Result<DexReader, DexError> {
        self.source
            .as_ref()
            .map(DexReader::fork)
            .ok_or(DexError::NoDataLeftError)
    }

    /// Parse a DEX file from the reader and create a `DexFile` object
    pub fn build(dex_reader: DexReader) -> Result<Self, DexError> {
        Self::build_with_level(dex_reader, ClassDecodeLevel::Bytecode)
    }

    /// Parse the class directory while deferring members, bytecode, and debug streams.
    pub fn build_metadata(dex_reader: DexReader) -> Result<Self, DexError> {
        Self::build_with_level(dex_reader, ClassDecodeLevel::Declaration)
    }

    fn build_with_level(
        mut dex_reader: DexReader,
        level: ClassDecodeLevel,
    ) -> Result<Self, DexError> {
        let source = dex_reader.fork();
        let dex_header = DexHeader::new(&mut dex_reader)?;
        let strings_list = DexStrings::build(
            &mut dex_reader,
            dex_header.string_ids_off,
            dex_header.string_ids_size,
        )?;

        let type_ids_list = DexTypes::build(
            &mut dex_reader,
            dex_header.type_ids_off,
            dex_header.type_ids_size,
            &strings_list,
        )?;

        let proto_ids_list = DexProtos::build(
            &mut dex_reader,
            dex_header.proto_ids_off,
            dex_header.proto_ids_size,
            &type_ids_list,
        )?;

        let field_ids_list = DexFields::build(
            &mut dex_reader,
            dex_header.fields_ids_off,
            dex_header.fields_ids_size,
            &type_ids_list,
            &strings_list,
        )?;

        let method_ids_list = DexMethods::build(
            &mut dex_reader,
            dex_header.method_ids_off,
            dex_header.method_ids_size,
            &type_ids_list,
            &proto_ids_list,
            &strings_list,
        )?;

        let class_defs_list = DexClasses::build_with_level(
            &mut dex_reader,
            dex_header.class_defs_off,
            dex_header.class_defs_size,
            &field_ids_list,
            &type_ids_list,
            &proto_ids_list,
            &strings_list,
            &method_ids_list,
            level,
        )?;

        Ok(DexFile {
            source: Some(source),
            header: dex_header,
            strings: strings_list,
            types: type_ids_list,
            protos: proto_ids_list,
            fields: field_ids_list,
            methods: method_ids_list,
            classes: class_defs_list,
        })
    }

    /// Create a `DexFile` from a collection of `DexReader`.
    ///
    /// The id pools are lazy and store file-local string/type indices, so the
    /// pre-lazy merge (concatenate every rendered name, sort, deduplicate) can
    /// no longer be expressed. The only caller is the `apk` entry point; it now
    /// reports the limitation rather than producing a corrupt merged file.
    #[cfg(feature = "apk")]
    pub fn merge(_readers: Vec<DexReader>) -> Result<Self, DexError> {
        Err(DexError::MergeUnsupported)
    }

    pub fn decode_code_item(&self, method: &EncodedMethod) -> Result<Option<CodeItem>, DexError> {
        let Some(offset) = method.code_offset else {
            return Ok(None);
        };
        let source = self.source.as_ref().ok_or(DexError::NoDataLeftError)?;
        let mut reader = source.fork();
        CodeItem::build(&mut reader, offset, &self.types, &self.strings).map(Some)
    }

    /// Decode a class's fields, methods, and member annotations on first use.
    pub fn materialize_class(&mut self, class_index: usize) -> Result<(), DexError> {
        let class = self
            .classes
            .items
            .get(class_index)
            .ok_or(DexError::InvalidClassIdx)?;
        if class.members_loaded() {
            return Ok(());
        }
        let class_index = u32::try_from(class_index).map_err(|_| DexError::InvalidClassIdx)?;
        let offset = class_index
            .checked_mul(32)
            .and_then(|relative| self.header.class_defs_off.checked_add(relative))
            .ok_or(DexError::InvalidClassIdx)?;
        let source = self.source.as_ref().ok_or(DexError::NoDataLeftError)?;
        let mut reader = source.fork();
        let mut decoded = DexClasses::build_with_level(
            &mut reader,
            offset,
            1,
            &self.fields,
            &self.types,
            &self.protos,
            &self.strings,
            &self.methods,
            ClassDecodeLevel::Members,
        )?;
        let class = decoded.items.pop().ok_or(DexError::InvalidClassIdx)?;
        self.classes.items[class_index as usize] = class;
        Ok(())
    }

    /// Resolve a class's lexical / nested-class metadata, decoding only that
    /// class's class-level annotations and caching them on first use.
    ///
    /// `build_metadata` leaves class annotations undecoded, so lexical analysis
    /// can ask about one class without paying for the whole directory.
    pub fn nested_metadata(
        &self,
        class_index: usize,
    ) -> Result<&ClassNestedMetadata, DexError> {
        let class = self
            .classes
            .items
            .get(class_index)
            .ok_or(DexError::InvalidClassIdx)?;
        if let Some(metadata) = class.cached_nested_metadata() {
            return Ok(metadata);
        }
        let metadata = self.read_nested_metadata(class_index)?;
        let _ = class.store_nested_metadata(metadata);
        class
            .cached_nested_metadata()
            .ok_or(DexError::InvalidClassIdx)
    }

    fn read_nested_metadata(&self, class_index: usize) -> Result<ClassNestedMetadata, DexError> {
        let class = self
            .classes
            .items
            .get(class_index)
            .ok_or(DexError::InvalidClassIdx)?;
        let annotations_off = class.annotations_offset();
        if annotations_off == 0 {
            return Ok(ClassNestedMetadata::default());
        }
        let source = self.source.as_ref().ok_or(DexError::NoDataLeftError)?;
        let mut reader = source.fork();
        let directory = crate::dex::classes::read_class_annotations_directory(
            &mut reader,
            annotations_off,
            &self.types,
            &self.strings,
            &self.protos,
            &self.fields,
            &self.methods,
        )?;
        Ok(ClassNestedMetadata {
            signature: directory.class_signature,
            inner_class: directory.inner_class,
            member_classes: directory.member_classes,
            enclosing_class: directory.enclosing_class,
            enclosing_method: directory.enclosing_method,
        })
    }

    /// Returns a vector containing the names of all the classes defined in the DEX file
    pub fn get_classes_names(&self) -> Vec<&str> {
        self.classes
            .items
            .iter()
            .filter_map(|class| class.class_name(&self.types, &self.strings))
            .collect()
    }

    /// Get the `ClassDefItem` object for a given class name
    pub fn get_class_def(&self, class_name: &str) -> Option<&ClassDefItem> {
        self.classes
            .get_class_def(&self.types, &self.strings, class_name)
    }

    /// Get the method of a given class as a vector of `EncodedMethod` objects
    pub fn get_methods_for_class(&self, class_name: &str) -> Vec<&EncodedMethod> {
        if let Some(class_def) = self.get_class_def(class_name) {
            return class_def.get_methods();
        }

        Vec::new()
    }
}
