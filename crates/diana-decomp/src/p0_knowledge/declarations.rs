/// Which source-visible declarations the caller has supplied for a class.
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclarationCoverage {
    /// The complete original classfile declaration directory.
    AllDeclarations,
    /// An API signature directory; package-private/private absence is unproven.
    ExportedApi,
}

/// Caller-owned method declaration facts. None for throws means Unknown, not
/// an empty checked set. Generic methods require additional source binding facts.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalMethodDeclaration {
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub access_flags: u32,
    pub throws: Option<Vec<Vec<u16>>>,
    pub generic: bool,
}

/// A complete directory in the stated coverage scope. Missing classes remain
/// Unknown. These facts do not override original DEX identities or validate
/// dispatch, access, inheritance cycles or source binding by construction.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalClassDeclaration {
    pub descriptor: Vec<u16>,
    pub access_flags: u32,
    pub parent: Option<Vec<u16>>,
    /// Direct interface references as DEX descriptors or JVM internal names.
    pub interfaces: Vec<Vec<u16>>,
    pub methods: Vec<ExternalMethodDeclaration>,
    pub field_names: Vec<Vec<u16>>,
    pub coverage: DeclarationCoverage,
    pub generic: bool,
    pub enclosing_scope: bool,
}
