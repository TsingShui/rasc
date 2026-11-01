pub(crate) mod array_ops;
pub(crate) mod branch;
pub(crate) use branch::verify as verify_method;
pub(crate) mod calls;
pub(crate) mod cast_ops;
pub(crate) mod constructor_entry;
mod declarations;
pub(crate) mod dispatcher;
mod dispatcher_verify;
mod exceptional;
pub(crate) mod field_ops;
pub(crate) mod fresh_fill;
mod method;
pub(crate) mod object_allocation;
pub(crate) mod protected;
mod protected_throws;
#[cfg(test)]
mod protection_tests;
mod receiver_ops;
pub(crate) mod structured;
pub(crate) mod types;
pub(crate) mod values;
pub use calls::{JavaCallSite, JavaInvokeKind};
pub(crate) use method::MethodBody;
pub(crate) use method::source_group_admitted;
pub use method::{CompilationContract, VerifiedJavaMethod};

pub use declarations::JavaMethodDecl;
pub(crate) use declarations::declaration;
