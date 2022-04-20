#[macro_export]
macro_rules! route {
    ($(
        $(#[$($meta:tt)*])*
        $verb:ident: $handler:path
    ),* $(,)?) => {
        $crate::routing::MethodRouter::new()
        $(
            .$verb($crate::handler::DocumentedHandler {
                handler: $handler,
                operation_id: concat!(module_path!(), "::", stringify!($handler)),
                tags: $crate::capture_tags!($(#[$($meta)*])*),
                summary: $crate::capture_summary!($(#[$($meta)*])*),
                description: $crate::capture_description!($(#[$($meta)*])*),
            })
        )*
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! capture_summary {
    () => ("");
    (#[doc = $doc:expr] $(#[$_meta:meta])*) => (
        $doc.trim()
    );
    // Ignore any leading non-doc attributes
    (#[$other:meta] $(#[$($rest:tt)*])*) => {
        $crate::capture_summary!($(#[$($rest)*])*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! capture_tags {
    () => ({
        let tags: &'static [&'static str] = &[];
        tags
    });
    (#[tags($($tag:literal),*)] $(#[$_meta:meta])*) => ({
        let tags: &'static [&'static str] = &[$($tag),*];
        tags
    });
    // Ignore any leading non-tags attributes
    // We can't use `$rest:meta` as recursive macro invocations can't introspect it:
    // https://doc.rust-lang.org/reference/macros-by-example.html#transcribing
    (#[$other:meta] $(#[$($rest:tt)*])*) => {
        $crate::capture_tags!($(#[$($rest)*])*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! capture_description {
    () => ("");
    // Allows mixing other meta like `#[tags()]`
    ($(#[$($meta:tt)*])+) => {
        $crate::capture_description!($(#[$($meta)*])* {})
    };
    (#[doc = $doc:expr] $(#[$($rest:tt)*])* {$($tt:tt)*}) => {
        $crate::capture_description!($(#[$($rest)*])* {$($tt)* $doc,})
    };
    (#[$other:meta] $(#[$($rest:tt)*])* {$($tt:tt)*}) => {
        $crate::capture_description!($(#[$($rest)*])* {$($tt)*})
    };
    ({$($doc:expr,)*}) => {
        concat!($($doc, "\n"),*).trim()
    };
}

macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!(T1);
        $name!(T1, T2);
        $name!(T1, T2, T3);
        $name!(T1, T2, T3, T4);
        $name!(T1, T2, T3, T4, T5);
        $name!(T1, T2, T3, T4, T5, T6);
        $name!(T1, T2, T3, T4, T5, T6, T7);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
    };
}

#[test]
fn test_capture_description() {
    assert_eq!(capture_description!(), "");
    assert_eq!(capture_description!(
        /// Foo
    ), "Foo");

    assert_eq!(capture_description!(
        /// Foo
        ///
        /// Bar
        /// Baz
    ), " Foo\n \n Bar\n Baz")
}