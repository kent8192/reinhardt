//! Inner value access trait for extractor wrappers

/// Trait for extractors that wrap a single inner value.
///
/// Enables `Validated<E>` to access the inner value for validation
/// without knowing the concrete extractor type.
///
/// Implemented by `Form<T>`, `Json<T>`, and `Query<T>`.
pub trait HasInner {
	/// The wrapped inner type.
	type Inner;

	/// Borrow the inner value.
	fn inner_ref(&self) -> &Self::Inner;

	/// Consume the extractor and return the inner value.
	fn into_inner(self) -> Self::Inner;
}
