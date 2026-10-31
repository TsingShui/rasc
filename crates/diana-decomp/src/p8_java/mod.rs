pub(crate) mod branch;
pub(crate) mod calls;
mod declarations;
pub(crate) mod dispatcher;
mod dispatcher_verify;
pub(crate) mod receiver;
mod scalar;
pub use calls::{JavaCallSite, JavaInvokeKind};
pub(crate) use scalar::MethodBody;
pub use scalar::{CompilationContract, VerifiedJavaMethod};
pub(crate) use scalar::{source_group_admitted, verify};

pub use declarations::JavaMethodDecl;
pub(crate) use declarations::declaration;
