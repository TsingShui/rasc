pub(super) fn write(
    body: &crate::p7_java::protected::ProtectedBody,
    names: &crate::p7_java::types::TypeNames,
    output: &mut crate::substrate::owned::TextBuffer,
    budget: &mut crate::RunContext<'_>,
) -> Result<(), crate::RequestError> {
    super::source_control::write(&body.storage, &body.source, names, output, budget)
}
