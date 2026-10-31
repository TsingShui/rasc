#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum BuiltinReference {
    Object,
    String,
}
impl BuiltinReference {
    pub(crate) fn descriptor(self) -> &'static [u16] {
        match self {
            Self::Object => &[
                76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 59,
            ],
            Self::String => &[
                76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 83, 116, 114, 105, 110, 103, 59,
            ],
        }
    }
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Object => "java.lang.Object",
            Self::String => "java.lang.String",
        }
    }
}
