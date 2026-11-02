#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Arithmetic {
    Add,
    Sub,
    Mul,
    And,
    Div,
    Rem,
    Or,
    Xor,
    Shl,
    Shr,
    Ushr,
}
