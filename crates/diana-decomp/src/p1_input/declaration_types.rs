/// Completeness scope used by declaration consumers.
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclarationCoverage {
    /// Every original DEX declaration was read successfully.
    AllDeclarations,
    /// At least one original declaration could not be read.
    Partial,
}

/// Input-derived method declaration facts. None for throws means Unknown, not
/// an empty checked set. Generic methods require additional source binding facts.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputMethodDeclaration {
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub access_flags: u32,
    pub throws: Option<Vec<Vec<u16>>>,
    pub generic: bool,
    /// Original Signature annotation units, when available.
    pub signature: Option<Vec<u16>>,
}

/// A complete directory in the stated coverage scope. Missing classes remain
/// Unknown. These facts do not override original DEX identities or validate
/// dispatch, access, inheritance cycles or source binding by construction.
pub struct InputClassDeclaration {
    pub descriptor: Vec<u16>,
    pub access_flags: u32,
    pub parent: Option<Vec<u16>>,
    /// Direct interface references. Input readers retain DEX descriptors.
    pub interfaces: Vec<Vec<u16>>,
    pub methods: Vec<InputMethodDeclaration>,
    pub field_names: Vec<Vec<u16>>,
    pub fields: Vec<super::InputFieldEvidence>,
    pub issues: Vec<super::InputError>,
    pub coverage: DeclarationCoverage,
    pub generic: bool,
    pub enclosing_scope: bool,
    /// Original class Signature annotation units, not inferred from the descriptor.
    pub signature: Option<Vec<u16>>,
}
