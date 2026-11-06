use super::{Origin, RenderError, RenderIssue, SourceSpan, TextOffset};
use std::fmt::Write;

pub(super) struct Buffer {
    pub text: String,
    pub spans: Vec<SourceSpan>,
    pub issues: Vec<RenderIssue>,
    pub depth: usize,
    allocation: Option<std::collections::TryReserveError>,
}
impl Buffer {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            spans: Vec::new(),
            issues: Vec::new(),
            depth: 0,
            allocation: None,
        }
    }
    pub fn put(&mut self, text: &str) -> Result<(), RenderError> {
        self.text
            .try_reserve(text.len())
            .map_err(RenderError::Allocation)?;
        self.text.push_str(text);
        Ok(())
    }
    pub fn fmt(&mut self, args: std::fmt::Arguments<'_>) -> Result<(), RenderError> {
        self.write_fmt(args)
            .map_err(|_| match self.allocation.take() {
                Some(e) => RenderError::Allocation(e),
                None => RenderError::OutputInvariant("formatting an existing display fact failed"),
            })
    }
    pub fn position(&self) -> TextOffset {
        TextOffset(self.text.len())
    }
    pub fn span(&mut self, start: TextOffset, origin: Origin) -> Result<(), RenderError> {
        self.spans.try_reserve(1).map_err(RenderError::Allocation)?;
        self.spans.push(SourceSpan {
            start,
            end: self.position(),
            origin,
        });
        Ok(())
    }
    pub fn indent(&mut self) -> Result<(), RenderError> {
        let n = self.depth.checked_mul(4).ok_or(RenderError::Capacity)?;
        self.text.try_reserve(n).map_err(RenderError::Allocation)?;
        for _ in 0..n {
            self.text.push(' ');
        }
        Ok(())
    }
    pub fn line(&mut self, s: &str) -> Result<(), RenderError> {
        self.indent()?;
        self.put(s)?;
        self.put("\n")
    }
    pub fn open(&mut self) -> Result<(), RenderError> {
        self.put(" {\n")?;
        self.depth = self.depth.checked_add(1).ok_or(RenderError::Capacity)?;
        Ok(())
    }
    pub fn close(&mut self) -> Result<(), RenderError> {
        self.depth = self
            .depth
            .checked_sub(1)
            .ok_or(RenderError::OutputInvariant("unbalanced output scope"))?;
        self.line("}")
    }
    /// Exact UTF-16 identity is retained through escaping; comments and names
    /// cannot inject new source structure. Only valid scalars are encoded as UTF-8.
    pub fn quote(&mut self, s: &str) -> Result<(), RenderError> {
        self.encode(s.encode_utf16(), Some("\""))
    }
    pub fn units(&mut self, units: &[u16], quoted: bool) -> Result<(), RenderError> {
        self.encode(units.iter().copied(), quoted.then_some("\""))
    }
    fn encode(
        &mut self,
        units: impl IntoIterator<Item = u16>,
        delimiter: Option<&str>,
    ) -> Result<(), RenderError> {
        if let Some(delimiter) = delimiter {
            self.put(delimiter)?;
        }
        for c in char::decode_utf16(units) {
            match c {
                Err(e) => self.fmt(format_args!("\\u{:04x}", e.unpaired_surrogate()))?,
                Ok('"') if delimiter == Some("\"") => self.put("\\\"")?,
                Ok('`') if delimiter == Some("`") => self.put("\\`")?,
                Ok('\\') => self.put("\\\\")?,
                Ok('\n') if delimiter != Some("`") => self.put("\\n")?,
                Ok('\r') if delimiter != Some("`") => self.put("\\r")?,
                Ok('\t') if delimiter != Some("`") => self.put("\\t")?,
                Ok(c)
                    if c.is_control()
                        || matches!(c, '\u{2028}' | '\u{2029}'|'\u{061c}'|'\u{200e}'..='\u{200f}'|'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') =>
                {
                    self.fmt(format_args!("\\u{:04x}", c as u32))?
                }
                Ok(c) => {
                    let mut bytes = [0u8; 4];
                    self.put(c.encode_utf8(&mut bytes))?;
                }
            }
        }
        if let Some(delimiter) = delimiter {
            self.put(delimiter)?;
        }
        Ok(())
    }
    pub fn name(&mut self, units: &[u16], form: crate::p7::NameForm) -> Result<(), RenderError> {
        if form == crate::p7::NameForm::Original {
            self.encode(units.iter().copied(), Some("`"))
        } else {
            self.units(units, false)
        }
    }
    pub fn path(&mut self, units: &[u16], separator: u16) -> Result<(), RenderError> {
        for (index, segment) in units.split(|&unit| unit == separator).enumerate() {
            if index > 0 {
                self.put(".")?;
            }
            self.name(segment, crate::substrate::names::name_form(segment))?;
        }
        Ok(())
    }
    pub fn problem(
        &mut self,
        method: crate::MethodId,
        cause: super::LocalRenderError,
    ) -> Result<(), RenderError> {
        let message = match &cause {
            super::LocalRenderError::Invariant(s) => *s,
            super::LocalRenderError::Scratch(_) => "method rendering scratch allocation failed",
        };
        let kind = cause.kind();
        if let Err(allocation) = self.issues.try_reserve(1) {
            return Err(RenderError::DiagnosticRetention {
                method,
                cause,
                allocation,
            });
        }
        self.issues.push(RenderIssue {
            method: Some(method),
            detail: message,
            kind,
            cause,
        });
        self.indent()?;
        self.put("unrecovered_rendering ")?;
        self.quote(message)?;
        self.put(";\n")
    }
}
impl Write for Buffer {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        if let Err(e) = self.text.try_reserve(s.len()) {
            self.allocation = Some(e);
            return Err(std::fmt::Error);
        }
        self.text.push_str(s);
        Ok(())
    }
}
