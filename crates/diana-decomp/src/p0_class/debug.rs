use serde::Serialize;
use serde::ser::{SerializeSeq, SerializeStruct};

use super::facts::{ClassFacts, DisplayValue, ErrorView};
use crate::substrate::owned::TextBuffer;
use crate::{AnnotationLocation, AnnotationToken, AnnotationValueKind, RequestError, RunContext};

struct JsonWriter<'a, 'c> {
    output: TextBuffer,
    control: &'a mut RunContext<'c>,
    error: Option<RequestError>,
}
impl std::io::Write for JsonWriter<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let text = std::str::from_utf8(bytes).map_err(std::io::Error::other)?;
        match self.output.push(text, self.control) {
            Ok(()) => Ok(bytes.len()),
            Err(error) => {
                self.error = Some(error);
                Err(std::io::Error::other("P0 JSON output stopped"))
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Renders only collected P0 facts as human-readable pretty JSON. Preserves original
/// identities, metadata byte ranges and local errors. Work, allocation and output
/// growth use the caller's request control; no metadata or method code is re-read.
pub fn render_json(
    facts: &ClassFacts,
    control: &mut RunContext<'_>,
) -> Result<String, RequestError> {
    let mut writer = JsonWriter {
        output: TextBuffer::new(),
        control,
        error: None,
    };
    if serde_json::to_writer_pretty(&mut writer, &SnapshotView(facts)).is_err() {
        return Err(writer.error.unwrap_or(RequestError::InternalInvariant(
            "P0 fact serialization failed without a resource error",
        )));
    }
    writer.output.push("\n", writer.control)?;
    Ok(writer.output.finish())
}

struct SnapshotView<'a>(&'a ClassFacts);
impl Serialize for SnapshotView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let facts = self.0;
        let mut record = serializer.serialize_struct("ClassFacts", 9)?;
        record.serialize_field("schema_version", &1)?;
        record.serialize_field("stage", "p0")?;
        record.serialize_field("identity", &IdentityView(facts))?;
        record.serialize_field(
            "status",
            if facts.is_partial() {
                "partial"
            } else {
                "complete"
            },
        )?;
        record.serialize_field("class", &facts.class)?;
        record.serialize_field("class_data", &ShapeView(facts))?;
        record.serialize_field("fields", &FieldsView(facts))?;
        record.serialize_field("methods", &MethodsView(facts))?;
        record.serialize_field("annotations", &AnnotationsView(facts))?;
        record.serialize_field("diagnostics", &DiagnosticsView(facts))?;
        record.end()
    }
}

struct ShapeView<'a>(&'a ClassFacts);
impl Serialize for ShapeView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let shape = self.0.class_data_shape();
        let mut record = serializer.serialize_struct("ClassData", 6)?;
        record.serialize_field("static_fields", &shape.static_fields)?;
        record.serialize_field("instance_fields", &shape.instance_fields)?;
        record.serialize_field("direct_methods", &shape.direct_methods)?;
        record.serialize_field("virtual_methods", &shape.virtual_methods)?;
        record.serialize_field(
            "field_traversal_complete",
            &self.0.field_traversal_complete(),
        )?;
        record.serialize_field(
            "method_traversal_complete",
            &self.0.method_traversal_complete(),
        )?;
        record.end()
    }
}

struct FieldsView<'a>(&'a ClassFacts);
impl Serialize for FieldsView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut sequence = serializer.serialize_seq(Some(self.0.fields.len()))?;
        for field in &self.0.fields {
            sequence.serialize_element(&FieldView(field))?;
        }
        sequence.end()
    }
}

struct FieldView<'a>(&'a super::facts::FieldDirectoryEntry);
impl Serialize for FieldView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let entry = self.0;
        let mut record = serializer.serialize_struct("Field", 7)?;
        record.serialize_field("field_index", &entry.field().0)?;
        record.serialize_field("access_flags", entry.original_flags())?;
        record.serialize_field(
            "storage",
            match entry.storage() {
                crate::FieldStorage::Static(_) => "static",
                crate::FieldStorage::Instance => "instance",
            },
        )?;
        record.serialize_field("group_ordinal", &entry.group_ordinal())?;
        record.serialize_field(
            "static_ordinal",
            &match entry.storage() {
                crate::FieldStorage::Static(crate::StaticFieldOrdinal(ordinal)) => Some(ordinal),
                crate::FieldStorage::Instance => None,
            },
        )?;
        record.serialize_field("declaration", entry.declaration())?;
        record.serialize_field("initial_value", entry.initial_value())?;
        record.serialize_field("scalar", &entry.scalar())?;
        record.end()
    }
}

struct MethodsView<'a>(&'a ClassFacts);
impl Serialize for MethodsView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut sequence = serializer.serialize_seq(Some(self.0.methods.len()))?;
        for method in &self.0.methods {
            sequence.serialize_element(&MethodView(method))?;
        }
        sequence.end()
    }
}

struct MethodView<'a>(&'a super::facts::MethodDirectoryEntry);
impl Serialize for MethodView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let entry = self.0;
        let (group, ordinal) = match entry.group() {
            crate::input::catalog::MethodGroup::Direct { ordinal } => ("direct", ordinal),
            crate::input::catalog::MethodGroup::Virtual { ordinal } => ("virtual", ordinal),
        };
        let mut record = serializer.serialize_struct("Method", 7)?;
        record.serialize_field("method_index", &entry.method().0)?;
        record.serialize_field("access_flags", entry.original_flags())?;
        record.serialize_field("group", group)?;
        record.serialize_field("group_ordinal", &ordinal)?;
        record.serialize_field("code_offset_bytes", &entry.code_offset().map_or(0, |o| o.0))?;
        record.serialize_field("declaration", entry.declaration())?;
        record.serialize_field("code", entry.code())?;
        record.end()
    }
}

struct IdentityView<'a>(&'a ClassFacts);
impl Serialize for IdentityView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut record = serializer.serialize_struct("Identity", 3)?;
        record.serialize_field("dex_index", &self.0.owner().dex.0)?;
        record.serialize_field("header_offset_bytes", &self.0.header_offset().0)?;
        record.serialize_field("class_def_index", &self.0.owner().class_def.0)?;
        record.end()
    }
}

struct DiagnosticsView<'a>(&'a ClassFacts);
impl Serialize for DiagnosticsView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Issue<'a> {
            section: &'static str,
            error: ErrorView<'a>,
        }
        let mut sequence = serializer.serialize_seq(None)?;
        for (section, error) in [
            ("fields", &self.0.fields_issue),
            ("methods", &self.0.methods_issue),
            ("annotations", &self.0.annotations_issue),
        ] {
            if let Some(error) = error {
                sequence.serialize_element(&Issue {
                    section,
                    error: ErrorView(error),
                })?;
            }
        }
        sequence.end()
    }
}

struct AnnotationsView<'a>(&'a ClassFacts);
impl Serialize for AnnotationsView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let facts = self.0.annotations();
        let mut record = serializer.serialize_struct("Annotations", 4)?;
        record.serialize_field("directory_offset_bytes", &facts.directory().0)?;
        record.serialize_field(
            "state",
            if facts.extent() == crate::AnnotationExtent::Prefix {
                "partial"
            } else if facts.directory().0 == 0 {
                "absent"
            } else {
                "present"
            },
        )?;
        record.serialize_field("records", &AnnotationRecords(facts.records()))?;
        #[derive(Serialize)]
        struct Prefix<'a> {
            range_bytes: [usize; 2],
            bytes: &'a [u8],
        }
        let prefix = facts.unavailable_prefix().map(|prefix| Prefix {
            range_bytes: [prefix.range().start.0, prefix.range().end.0],
            bytes: prefix.bytes(),
        });
        record.serialize_field("unavailable_prefix", &prefix)?;
        record.end()
    }
}

struct AnnotationRecords<'a>(&'a [crate::AnnotationRecord]);
impl Serialize for AnnotationRecords<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for record in self.0 {
            sequence.serialize_element(&AnnotationView(record))?;
        }
        sequence.end()
    }
}
struct AnnotationView<'a>(&'a crate::AnnotationRecord);
impl Serialize for AnnotationView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let facts = self.0;
        let mut record = serializer.serialize_struct("Annotation", 8)?;
        record.serialize_field("location", &LocationView(facts.location()))?;
        record.serialize_field("visibility", &facts.visibility())?;
        record.serialize_field("type_index", &facts.type_id().0)?;
        record.serialize_field("descriptor", &TextView(facts.descriptor()))?;
        record.serialize_field("range_bytes", &[facts.range().start.0, facts.range().end.0])?;
        record.serialize_field("bytes", facts.bytes())?;
        record.serialize_field(
            "state",
            if facts.extent() == crate::AnnotationExtent::Complete {
                "present"
            } else {
                "partial"
            },
        )?;
        record.serialize_field("tokens", &TokensView(facts.tokens()))?;
        record.end()
    }
}

struct LocationView(AnnotationLocation);
impl Serialize for LocationView {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut record = serializer.serialize_struct("Location", 3)?;
        match self.0 {
            AnnotationLocation::Class => record.serialize_field("kind", "class")?,
            AnnotationLocation::Field(id) => {
                record.serialize_field("kind", "field")?;
                record.serialize_field("field_index", &id.0)?;
            }
            AnnotationLocation::Method(id) => {
                record.serialize_field("kind", "method")?;
                record.serialize_field("method_index", &id.0)?;
            }
            AnnotationLocation::Parameter { method, parameter } => {
                record.serialize_field("kind", "parameter")?;
                record.serialize_field("method_index", &method.0)?;
                record.serialize_field("parameter_index", &parameter.0)?;
            }
        }
        record.end()
    }
}

struct TextView<'a>(&'a [u16]);
impl std::fmt::Display for TextView<'_> {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for scalar in char::decode_utf16(self.0.iter().copied()) {
            match scalar {
                Ok(character) => write!(output, "{character}")?,
                Err(error) => write!(output, "\\u{:04X}", error.unpaired_surrogate())?,
            }
        }
        Ok(())
    }
}
impl Serialize for TextView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut record = serializer.serialize_struct("OriginalText", 1)?;
        record.serialize_field("text", &DisplayValue(self))?;
        record.end()
    }
}

struct TokensView<'a>(&'a [AnnotationToken]);
impl Serialize for TokensView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for token in self.0 {
            sequence.serialize_element(&TokenView(token))?;
        }
        sequence.end()
    }
}
struct TokenView<'a>(&'a AnnotationToken);
impl Serialize for TokenView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut record = serializer.serialize_struct("AnnotationToken", 10)?;
        let AnnotationToken::Value(value) = self.0 else {
            record.serialize_field("kind", "end_container")?;
            return record.end();
        };
        record.serialize_field("name", &value.name().map(TextView))?;
        record.serialize_field("name_index", &value.name_id().map(|id| id.0))?;
        record.serialize_field("range_bytes", &[value.range().start.0, value.range().end.0])?;
        record.serialize_field("tag", &value.tag())?;
        record.serialize_field("value_arg", &value.value_arg())?;
        match value.kind() {
            AnnotationValueKind::Signed(value) => {
                record.serialize_field("kind", "signed")?;
                record.serialize_field("value", value)?;
            }
            AnnotationValueKind::Unsigned(value) => {
                record.serialize_field("kind", "unsigned")?;
                record.serialize_field("value", value)?;
            }
            AnnotationValueKind::Bits { width, bits } => {
                record.serialize_field("kind", "bits")?;
                record.serialize_field("width_bytes", &width.0)?;
                record.serialize_field("bits", bits)?;
            }
            AnnotationValueKind::String { reference, units } => {
                record.serialize_field("kind", "string")?;
                record.serialize_field("string_index", &reference.0)?;
                record.serialize_field("value", &TextView(units))?;
            }
            AnnotationValueKind::Reference(reference) => {
                record.serialize_field("kind", "reference")?;
                let (kind, id) = match reference {
                    crate::raw::OperandReference::String(id) => ("string", id.0),
                    crate::raw::OperandReference::Type(id) => ("type", id.0),
                    crate::raw::OperandReference::Field(id) => ("field", id.0),
                    crate::raw::OperandReference::Method(id) => ("method", id.0),
                    crate::raw::OperandReference::Proto(id) => ("proto", id.0),
                    crate::raw::OperandReference::CallSite(id) => ("call_site", id.0),
                    crate::raw::OperandReference::MethodHandle(id) => ("method_handle", id.0),
                };
                record.serialize_field("reference_kind", kind)?;
                record.serialize_field("reference_index", &id)?;
            }
            AnnotationValueKind::Null => record.serialize_field("kind", "null")?,
            AnnotationValueKind::Boolean(value) => {
                record.serialize_field("kind", "boolean")?;
                record.serialize_field("value", value)?;
            }
            AnnotationValueKind::Array { count } => {
                record.serialize_field("kind", "array")?;
                record.serialize_field("count", count)?;
            }
            AnnotationValueKind::Annotation { type_id, count } => {
                record.serialize_field("kind", "annotation")?;
                record.serialize_field("type_index", &type_id.0)?;
                record.serialize_field("count", count)?;
            }
        }
        record.end()
    }
}
