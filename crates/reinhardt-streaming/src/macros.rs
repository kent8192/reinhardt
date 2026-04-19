/// Build a `StreamingRouter` from a list of streaming handler identifiers.
///
/// Each identifier must refer to a function annotated with `#[producer]` or `#[consumer]`.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_streaming::streaming_routes;
///
/// pub fn streaming_routes() -> reinhardt_streaming::StreamingRouter {
///     streaming_routes![create_order, handle_order]
/// }
/// ```
///
/// The returned `StreamingRouter` can be mounted on a `UnifiedRouter`:
///
/// ```rust,ignore
/// UnifiedRouter::new()
///     .mount_unified("/", web_routes())
///     .mount_streaming(streaming_routes())
/// ```
#[macro_export]
macro_rules! streaming_routes {
    [$($handler:ident),* $(,)?] => {{
        let __router = $crate::StreamingRouter::new();
        // Handler functions registered by #[producer]/#[consumer] are available in
        // the current module namespace. This macro simply acknowledges them and
        // builds the router. Metadata is registered via inventory::submit! in each
        // handler's generated code, and is accessible via resolve_streaming_topic().
        $(
            let _ = $handler; // ensure the handler is in scope (compile-time check)
        )*
        __router
    }};
}
