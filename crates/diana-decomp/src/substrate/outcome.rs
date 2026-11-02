pub(crate) enum Outcome<T> {
    Ready(T),
    Declined(crate::OutputReason),
}
#[cfg(test)]
impl<T> Outcome<T> {
    pub(crate) fn unwrap(self) -> T {
        match self {
            Self::Ready(value) => value,
            Self::Declined(reason) => panic!("analysis declined: {reason:?}"),
        }
    }
}
