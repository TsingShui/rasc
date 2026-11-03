// Each owning validator has a private constructor; consumers get no mutable dereference.
macro_rules! checked_product {
    ($name:ident, $data:ident) => {
        pub(crate) struct $name<'a>($data<'a>);
        impl<'a> std::ops::Deref for $name<'a> {
            type Target = $data<'a>;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}
pub(crate) use checked_product;
