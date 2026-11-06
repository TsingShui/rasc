struct Text {
    value: String,
    error: Option<std::collections::TryReserveError>,
}
impl std::fmt::Write for Text {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        if let Err(e) = self.value.try_reserve(s.len()) {
            self.error = Some(e);
            return Err(std::fmt::Error);
        }
        self.value.push_str(s);
        Ok(())
    }
}
pub(crate) fn text(args: std::fmt::Arguments<'_>) -> Result<String, crate::RequestError> {
    use std::fmt::Write;
    let mut out = Text {
        value: String::new(),
        error: None,
    };
    if out.write_fmt(args).is_err() {
        return Err(match out.error {
            Some(e) => crate::RequestError::Allocation(e),
            None => crate::RequestError::InternalInvariant("formatting original fact failed"),
        });
    }
    Ok(out.value)
}
