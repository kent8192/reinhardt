//! Dependency providers

use crate::DiResult;
use std::any::Any;
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;

/// Type alias for the complex provider future type
type ProviderFutureInner =
	Pin<Box<dyn Future<Output = DiResult<Box<dyn Any + Send + Sync>>> + Send>>;

/// Wrapper type for the future returned by providers
///
/// This newtype wraps a pinned boxed future that resolves to a dependency value.
/// Using a newtype instead of a type alias provides better type safety and allows
/// for trait implementations.
pub struct ProviderFuture(ProviderFutureInner);

impl ProviderFuture {
	/// Create a new ProviderFuture from a pinned boxed future
	pub fn new(future: ProviderFutureInner) -> Self {
		Self(future)
	}

	/// Convert into the inner pinned future
	pub fn into_inner(self) -> ProviderFutureInner {
		self.0
	}
}

impl Deref for ProviderFuture {
	type Target = ProviderFutureInner;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<ProviderFutureInner> for ProviderFuture {
	fn from(future: ProviderFutureInner) -> Self {
		Self::new(future)
	}
}

/// Wrapper type for provider functions
///
/// This newtype wraps an Arc-wrapped function that returns a ProviderFuture.
/// Using a newtype provides better type safety and allows for future extensions.
pub struct ProviderFn(Arc<dyn Fn() -> ProviderFuture + Send + Sync>);

impl ProviderFn {
	/// Create a new ProviderFn from an Arc-wrapped function
	pub fn new(func: Arc<dyn Fn() -> ProviderFuture + Send + Sync>) -> Self {
		Self(func)
	}

	/// Get a reference to the inner function
	pub fn as_fn(&self) -> &Arc<dyn Fn() -> ProviderFuture + Send + Sync> {
		&self.0
	}

	/// Convert into the inner Arc-wrapped function
	pub fn into_inner(self) -> Arc<dyn Fn() -> ProviderFuture + Send + Sync> {
		self.0
	}
}

impl Deref for ProviderFn {
	type Target = Arc<dyn Fn() -> ProviderFuture + Send + Sync>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<Arc<dyn Fn() -> ProviderFuture + Send + Sync>> for ProviderFn {
	fn from(func: Arc<dyn Fn() -> ProviderFuture + Send + Sync>) -> Self {
		Self::new(func)
	}
}

/// A dependency provider that asynchronously creates dependency instances.
pub trait Provider: Send + Sync {
	/// Produces a boxed future that resolves to the provided dependency value.
	fn provide(&self) -> ProviderFuture;
}

impl<F, Fut, T> Provider for F
where
	F: Fn() -> Fut + Send + Sync,
	Fut: Future<Output = DiResult<T>> + Send + 'static,
	T: Any + Send + Sync + 'static,
{
	fn provide(&self) -> ProviderFuture {
		let fut = self();
		ProviderFuture::new(Box::pin(async move {
			let result = fut.await?;
			Ok(Box::new(result) as Box<dyn Any + Send + Sync>)
		}))
	}
}
