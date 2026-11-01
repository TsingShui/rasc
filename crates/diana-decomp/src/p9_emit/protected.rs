pub(super) fn write(
    body: &crate::p8_java::protected::ProtectedBody,
    names: &crate::p8_java::types::TypeNames,
    output: &mut crate::substrate::owned::TextBuffer,
    budget: &mut crate::MethodBudget<'_, '_>,
) -> Result<(), crate::RequestError> {
    super::source_control::write(&body.storage, &body.source, names, output, budget)
}
