pub(crate) mod call_facts;
pub(crate) mod entry;
mod local_ops;
mod normal;
pub(crate) use normal::{
    Arithmetic, ArrayCreation, ArrayKind, Block, BlockId, CallId, CallSite, CallTarget, CastSite,
    DeclarationType, Edge, EdgeId, EmptyFillSite, FilledArraySite, FilledResult, FlowBody,
    FreshFillSite, InvokeKind, InvokeResult, MethodFrame, MethodMonitor, MonitorKind,
    ObjectCreation, Op, OpId, Operand, Operation, ResultType, StringLiteral, Switch, Terminal,
    ValueWidth, decode, destination, field_type, reference_type, terminal_other, terminal_register,
};
pub(crate) mod primitives;
pub(crate) mod protection;
