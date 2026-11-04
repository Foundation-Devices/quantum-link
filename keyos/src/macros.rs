/// declares public rpc route markers for an app
///
/// each marker gets an [`AppRouteKey`](crate::AppRouteKey) and must implement its named
/// `ql-rpc` trait. route ids must be unique within this invocation
///
/// ```
/// ql_keyos::app_routes! {
///     ql_keyos::AppId::from_hex("0x00000000000000000000000000000001") => {
///         GetStatus: Request = 1,
///     }
/// }
///
/// impl ql_rpc::Request for GetStatus {
///     type Request = ();
///     type Response = bool;
/// }
/// ```
#[macro_export]
macro_rules! app_routes {
    ($app_id:expr => { $($route:ident: $kind:ident = $id:literal,)* }) => {
        $crate::__duplicate_route_check!($($id,)*);

        $(
            #[derive(Debug)]
            pub struct $route;

            impl $crate::__rpc::Route for $route {
                type Key = $crate::AppRouteKey;

                fn key() -> Self::Key {
                    $crate::AppRouteKey {
                        app_id: $app_id,
                        route_id: $crate::RouteId($id),
                    }
                }
            }

            $crate::__assert_route_impl!($route: $kind);
        )*
    };
}

/// declares public rpc route markers for a service
///
/// each marker gets a [`ServiceRouteKey`](crate::ServiceRouteKey) and must implement its named
/// `ql-rpc` trait. route ids must be unique within this invocation
///
/// ```
/// ql_keyos::service_routes! {
///     ql_keyos::ServiceId(1) => {
///         GetStatus: Request = 1,
///     }
/// }
///
/// impl ql_rpc::Request for GetStatus {
///     type Request = ();
///     type Response = bool;
/// }
/// ```
#[macro_export]
macro_rules! service_routes {
    ($service_id:expr => { $($route:ident: $kind:ident = $id:literal,)* }) => {
        $crate::__duplicate_route_check!($($id,)*);

        $(
            #[derive(Debug)]
            pub struct $route;

            impl $crate::__rpc::Route for $route {
                type Key = $crate::ServiceRouteKey;

                fn key() -> Self::Key {
                    $crate::ServiceRouteKey {
                        service_id: $service_id,
                        route_id: $crate::RouteId($id),
                    }
                }
            }

            $crate::__assert_route_impl!($route: $kind);
        )*
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __duplicate_route_check {
    ($($id:literal,)*) => {
        const _: () = {
            const IDS: &[u32] = &[$($id,)*];
            let mut i = 0;
            while i < IDS.len() {
                let mut j = i + 1;
                while j < IDS.len() {
                    assert!(IDS[i] != IDS[j], "duplicate route id");
                    j += 1;
                }
                i += 1;
            }
        };
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __assert_route_impl {
    ($route:ident: $kind:ident) => {
        const _: () = {
            struct Assert<T: $crate::__rpc::$kind>(::core::marker::PhantomData<T>);
            let _ = Assert::<$route>(::core::marker::PhantomData);
        };
    };
}
