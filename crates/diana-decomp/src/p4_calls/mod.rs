mod contracts;
mod receivers;
pub(crate) use receivers::{ReceiverContract, ReceiverKind, resolve_receiver};
mod fields;
mod overloads;
pub(crate) use contracts::{CallFormal, StaticContract, StaticResult, resolve_static};
pub(crate) use fields::check_external_static_read;
