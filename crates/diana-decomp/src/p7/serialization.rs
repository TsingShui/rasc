pub(super) use crate::substrate::serialization::debug;
pub(super) fn metadata<S: serde::Serializer>(
    value: &crate::p0::MethodMetadata,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::{SerializeSeq, SerializeStruct};
    struct Records<'a>(&'a crate::p0::MethodMetadata);
    struct Record<'a>(&'a crate::AnnotationRecord);
    struct References<'a>(&'a crate::p0::MethodMetadata);
    impl serde::Serialize for References<'_> {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_seq(self.0.references().map(Debug))
        }
    }
    struct Debug<T>(T);
    impl<T: std::fmt::Debug> serde::Serialize for Debug<T> {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            debug(&self.0, s)
        }
    }
    impl serde::Serialize for Record<'_> {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let r = self.0;
            let mut m = s.serialize_struct("Annotation", 7)?;
            m.serialize_field("location", &Debug(&r.location()))?;
            m.serialize_field("visibility", &r.visibility())?;
            m.serialize_field("type_id", &r.type_id().0)?;
            m.serialize_field("descriptor", r.descriptor())?;
            m.serialize_field("bytes", r.bytes())?;
            m.serialize_field("tokens", &Debug(&r.tokens()))?;
            m.serialize_field("extent", &Debug(&r.extent()))?;
            m.end()
        }
    }
    impl serde::Serialize for Records<'_> {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut a = s.serialize_seq(None)?;
            for r in self.0.records() {
                a.serialize_element(&Record(r))?;
            }
            a.end()
        }
    }
    let mut m = serializer.serialize_struct("MethodMetadata", 4)?;
    m.serialize_field("extent", &Debug(&value.extent()))?;
    m.serialize_field("issue", &Debug(&value.issue()))?;
    m.serialize_field("records", &Records(value))?;
    m.serialize_field("references", &References(value))?;
    m.end()
}
