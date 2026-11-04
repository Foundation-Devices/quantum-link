macro_rules! rpc {
    ($(#[$attr:meta])* $vis:vis struct $name:ident;) => {
        compile_error!("rpc! does not support unit structs");
    };
    (#[PartialEq] $(#[$attr:meta])* $vis:vis struct $name:ident $($body:tt)+) => {
        rpc_item!(PartialEq; $(#[$attr])* $vis struct $name $($body)+);
    };
    ($(#[$attr:meta])* $vis:vis struct $name:ident $($body:tt)+) => {
        rpc_item!(Eq; $(#[$attr])* $vis struct $name $($body)+);
    };
    (#[PartialEq] $(#[$attr:meta])* $vis:vis enum $name:ident $($body:tt)+) => {
        rpc_item!(PartialEq; $(#[$attr])* $vis enum $name $($body)+);
    };
    ($(#[$attr:meta])* $vis:vis enum $name:ident $($body:tt)+) => {
        rpc_item!(Eq; $(#[$attr])* $vis enum $name $($body)+);
    };
}

macro_rules! rpc_item {
    (PartialEq; $(#[$attr:meta])* $vis:vis $kind:ident $name:ident $($body:tt)+) => {
        #[derive(Debug, Clone, PartialEq, ql_codec::Codec)]
        $(#[$attr])*
        #[cfg_attr(feature = "frb", flutter_rust_bridge::frb(non_opaque))]
        $vis $kind $name $($body)+
    };
    (Eq; $(#[$attr:meta])* $vis:vis $kind:ident $name:ident $($body:tt)+) => {
        #[derive(Debug, Clone, PartialEq, Eq, ql_codec::Codec)]
        $(#[$attr])*
        #[cfg_attr(feature = "frb", flutter_rust_bridge::frb(non_opaque))]
        $vis $kind $name $($body)+
    };
}
