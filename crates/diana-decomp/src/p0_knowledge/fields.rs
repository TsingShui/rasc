/// Java compilation treatment of a particular original field read. Unknown is
/// not evidence of NonConstant and never authorizes a class-initialization helper.
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JavaConstantKind {
    Constant,
    NonConstant,
    Unknown,
}

/// Exact caller-supplied field declaration and source constantness evidence.
/// It does not override a definition in the original logical DEX program.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalFieldEvidence {
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub field_type: Vec<u16>,
    pub access_flags: u32,
    pub constant_kind: JavaConstantKind,
}
