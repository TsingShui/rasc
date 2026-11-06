pub(crate) fn shared<T: serde::Serialize, S: serde::Serializer>(
    v: &std::sync::Arc<T>,
    s: S,
) -> Result<S::Ok, S::Error> {
    v.as_ref().serialize(s)
}
pub(crate) fn optional_shared<T: serde::Serialize, S: serde::Serializer>(
    v: &Option<std::sync::Arc<T>>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match v {
        Some(v) => s.serialize_some(v.as_ref()),
        None => s.serialize_none(),
    }
}
pub(crate) fn debug<T: std::fmt::Debug, S: serde::Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    struct Display<'a, T>(&'a T);
    impl<T: std::fmt::Debug> std::fmt::Display for Display<'_, T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(self.0, f)
        }
    }
    serializer.collect_str(&Display(value))
}
