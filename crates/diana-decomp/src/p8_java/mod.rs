pub(crate) mod branch;
mod declarations;
mod scalar;
pub(crate) use scalar::MethodBody;
pub use scalar::{CompilationContract, VerifiedJavaMethod};
pub(crate) use scalar::{source_group_admitted, verify};

pub use declarations::JavaMethodDecl;
pub(crate) use declarations::declaration;
