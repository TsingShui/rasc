pub(crate) enum Outcome<T> {
    Ready(T),
    Declined(crate::OutputReason),
}
