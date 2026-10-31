pub(crate) mod branch;
mod declarations;
pub(crate) mod dispatcher;
mod dispatcher_verify;
mod scalar;
pub(crate) use scalar::MethodBody;
pub use scalar::{CompilationContract, VerifiedJavaMethod};
pub(crate) use scalar::{source_group_admitted, verify};

pub use declarations::JavaMethodDecl;
pub(crate) use declarations::declaration;
