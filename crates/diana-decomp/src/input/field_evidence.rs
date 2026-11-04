/// Input-derived field declaration.
/// It does not override a definition in the original logical DEX program.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputFieldEvidence {
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub field_type: Vec<u16>,
    pub access_flags: u32,
}
