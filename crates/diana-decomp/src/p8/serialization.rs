use crate::substrate::serialization::debug;
use serde::{Serialize, Serializer, ser::SerializeStruct};
struct Debug<'a, T>(&'a T);
impl<T: std::fmt::Debug> Serialize for Debug<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        debug(self.0, s)
    }
}
pub(super) fn available<T: Serialize, S: Serializer>(
    r: &Result<T, crate::RequestError>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match r {
        Ok(v) => v.serialize(s),
        Err(e) => Debug(e).serialize(s),
    }
}
pub(super) fn delivery<S: Serializer>(
    r: &Result<super::MethodDelivery, crate::RequestError>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match r {
        Ok(r) => r.serialize(s),
        Err(e) => Debug(e).serialize(s),
    }
}
pub(super) fn facts<S: Serializer>(
    f: &std::sync::Arc<crate::p0::ClassFacts>,
    s: S,
) -> Result<S::Ok, S::Error> {
    struct Field<'a>(&'a crate::p0::FieldDirectoryEntry);
    impl Serialize for Field<'_> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let f = self.0;
            let mut out = s.serialize_struct("OriginalField", 7)?;
            out.serialize_field("id", &f.field().0)?;
            out.serialize_field("flags", f.original_flags())?;
            out.serialize_field("storage", &Debug(&f.storage()))?;
            out.serialize_field("ordinal", &f.group_ordinal())?;
            out.serialize_field("declaration", f.declaration())?;
            out.serialize_field("initial", f.initial_value())?;
            out.serialize_field("scalar", &f.scalar())?;
            out.end()
        }
    }
    struct Method<'a>(&'a crate::p0::MethodDirectoryEntry);
    impl Serialize for Method<'_> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let m = self.0;
            let mut out = s.serialize_struct("OriginalMethod", 5)?;
            out.serialize_field("id", &m.method().0)?;
            out.serialize_field("flags", m.original_flags())?;
            out.serialize_field("group", &Debug(&m.group()))?;
            out.serialize_field("code_offset", &Debug(&m.code_offset()))?;
            out.serialize_field("declaration", m.declaration())?;
            out.end()
        }
    }
    struct Fields<'a>(&'a crate::p0::ClassFacts);
    struct Methods<'a>(&'a crate::p0::ClassFacts);
    impl Serialize for Fields<'_> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_seq(self.0.fields().iter().map(Field))
        }
    }
    impl Serialize for Methods<'_> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_seq(self.0.methods().iter().map(Method))
        }
    }
    let mut out = s.serialize_struct("OriginalClass", 16)?;
    out.serialize_field("key", &Debug(&f.owner()))?;
    out.serialize_field("header_offset", &Debug(&f.header_offset()))?;
    out.serialize_field("type_id", &f.type_index())?;
    out.serialize_field("descriptor", f.descriptor())?;
    out.serialize_field("flags", &f.access_flags().raw())?;
    out.serialize_field("superclass", f.superclass())?;
    out.serialize_field("interfaces", f.interfaces())?;
    out.serialize_field("source_file", f.source_file())?;
    out.serialize_field("shape", &Debug(&f.class_data_shape()))?;
    out.serialize_field("delivered", &Debug(&f.delivered_shape()))?;
    out.serialize_field("terminated_at", &Debug(&f.coverage_terminated_at()))?;
    out.serialize_field("field_issue", &Debug(&f.fields_issue()))?;
    out.serialize_field("method_issue", &Debug(&f.methods_issue()))?;
    out.serialize_field("annotations", &Debug(f.annotations()))?;
    out.serialize_field("annotation_issue", &Debug(&f.annotations_issue()))?;
    out.serialize_field("fields", &Fields(f))?;
    out.serialize_field("methods", &Methods(f))?;
    out.end()
}
