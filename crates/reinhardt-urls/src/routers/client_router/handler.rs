//! Route handler abstractions for typed parameter extraction.
//!
//! This module provides the [`RouteHandler`] trait and implementations
//! for different handler signatures, enabling type-safe path parameter
//! extraction in route definitions.

use super::error::RouterError;
use super::params::{FromPath, ParamContext, Path, SingleFromPath};
use reinhardt_core::types::page::Page;
use std::marker::PhantomData;
use std::sync::Arc;

/// Trait for route handlers that can handle route requests.
///
/// This trait abstracts over different handler signatures:
/// - `Fn() -> Page` - No parameters
/// - `Fn(Path<T>) -> Page` - With typed parameters
/// - `Fn(Path<T>) -> Result<Page, E>` - With error handling
pub trait RouteHandler: Send + Sync {
	/// Handles the route request with the given parameter context.
	///
	/// # Errors
	///
	/// Returns [`RouterError::PathExtraction`] if parameter extraction fails.
	fn handle(&self, ctx: &ParamContext) -> Result<Page, RouterError>;
}

/// Handler for routes without parameters.
///
/// Wraps a `Fn() -> Page` closure.
pub(crate) struct NoParamsHandler<F> {
	handler: F,
}

impl<F> NoParamsHandler<F> {
	/// Creates a new no-params handler.
	pub(crate) fn new(handler: F) -> Self {
		Self { handler }
	}
}

// SAFETY: NoParamsHandler is Send + Sync when F is Send + Sync
unsafe impl<F: Send> Send for NoParamsHandler<F> {}
unsafe impl<F: Sync> Sync for NoParamsHandler<F> {}

impl<F> RouteHandler for NoParamsHandler<F>
where
	F: Fn() -> Page + Send + Sync,
{
	fn handle(&self, _ctx: &ParamContext) -> Result<Page, RouterError> {
		Ok((self.handler)())
	}
}

/// Handler for routes with typed parameters.
///
/// Wraps a `Fn(Path<T>) -> Page` closure.
pub(crate) struct WithParamsHandler<F, T> {
	handler: F,
	_phantom: PhantomData<T>,
}

impl<F, T> WithParamsHandler<F, T> {
	/// Creates a new with-params handler.
	pub(crate) fn new(handler: F) -> Self {
		Self {
			handler,
			_phantom: PhantomData,
		}
	}
}

// SAFETY: WithParamsHandler is Send + Sync when F is Send + Sync
// The PhantomData<T> is only used for type inference and doesn't hold data
unsafe impl<F: Send, T> Send for WithParamsHandler<F, T> {}
unsafe impl<F: Sync, T> Sync for WithParamsHandler<F, T> {}

impl<F, T> RouteHandler for WithParamsHandler<F, T>
where
	F: Fn(Path<T>) -> Page + Send + Sync,
	T: FromPath + Send + Sync,
{
	fn handle(&self, ctx: &ParamContext) -> Result<Page, RouterError> {
		let params = Path::<T>::from_path(ctx).map_err(RouterError::PathExtraction)?;
		Ok((self.handler)(params))
	}
}

/// Handler for routes that return `Result<Page, E>`.
///
/// Wraps a `Fn(Path<T>) -> Result<Page, E>` closure.
pub(crate) struct ResultHandler<F, T, E> {
	handler: F,
	_phantom: PhantomData<(T, E)>,
}

impl<F, T, E> ResultHandler<F, T, E> {
	/// Creates a new result handler.
	pub(crate) fn new(handler: F) -> Self {
		Self {
			handler,
			_phantom: PhantomData,
		}
	}
}

// SAFETY: ResultHandler is Send + Sync when F is Send + Sync
// The PhantomData<(T, E)> is only used for type inference and doesn't hold data
unsafe impl<F: Send, T, E> Send for ResultHandler<F, T, E> {}
unsafe impl<F: Sync, T, E> Sync for ResultHandler<F, T, E> {}

impl<F, T, E> RouteHandler for ResultHandler<F, T, E>
where
	F: Fn(Path<T>) -> Result<Page, E> + Send + Sync,
	T: FromPath + Send + Sync,
	E: Into<RouterError> + Send + Sync,
{
	fn handle(&self, ctx: &ParamContext) -> Result<Page, RouterError> {
		let params = Path::<T>::from_path(ctx).map_err(RouterError::PathExtraction)?;
		(self.handler)(params).map_err(|e| e.into())
	}
}

/// Helper function to create a no-params handler.
pub(crate) fn no_params_handler<F>(handler: F) -> Arc<dyn RouteHandler>
where
	F: Fn() -> Page + Send + Sync + 'static,
{
	Arc::new(NoParamsHandler::new(handler))
}

/// Helper function to create a with-params handler.
pub(crate) fn with_params_handler<F, T>(handler: F) -> Arc<dyn RouteHandler>
where
	F: Fn(Path<T>) -> Page + Send + Sync + 'static,
	T: FromPath + Send + Sync + 'static,
{
	Arc::new(WithParamsHandler::new(handler))
}

/// Helper function to create a result handler.
pub(crate) fn result_handler<F, T, E>(handler: F) -> Arc<dyn RouteHandler>
where
	F: Fn(Path<T>) -> Result<Page, E> + Send + Sync + 'static,
	T: FromPath + Send + Sync + 'static,
	E: Into<RouterError> + Send + Sync + 'static,
{
	Arc::new(ResultHandler::new(handler))
}

// ============================================================================
// Multi-argument Path<T> handlers
// ============================================================================

/// Handler for routes with a single `Path<T>` argument.
///
/// Wraps a `Fn(Path<T>) -> Page` closure.
pub(crate) struct SinglePathHandler<F, T> {
	handler: F,
	_phantom: PhantomData<T>,
}

impl<F, T> SinglePathHandler<F, T> {
	pub(crate) fn new(handler: F) -> Self {
		Self {
			handler,
			_phantom: PhantomData,
		}
	}
}

// SAFETY: SinglePathHandler is Send + Sync when F is Send + Sync
unsafe impl<F: Send, T> Send for SinglePathHandler<F, T> {}
unsafe impl<F: Sync, T> Sync for SinglePathHandler<F, T> {}

impl<F, T> RouteHandler for SinglePathHandler<F, T>
where
	F: Fn(Path<T>) -> Page + Send + Sync,
	T: SingleFromPath + Send + Sync,
{
	fn handle(&self, ctx: &ParamContext) -> Result<Page, RouterError> {
		let value = T::from_path_at(ctx, 0).map_err(RouterError::PathExtraction)?;
		Ok((self.handler)(Path(value)))
	}
}

/// Handler for routes with two `Path<T>` arguments.
///
/// Wraps a `Fn(Path<T1>, Path<T2>) -> Page` closure.
pub(crate) struct TwoPathHandler<F, T1, T2> {
	handler: F,
	_phantom: PhantomData<(T1, T2)>,
}

impl<F, T1, T2> TwoPathHandler<F, T1, T2> {
	pub(crate) fn new(handler: F) -> Self {
		Self {
			handler,
			_phantom: PhantomData,
		}
	}
}

// SAFETY: TwoPathHandler is Send + Sync when F is Send + Sync
unsafe impl<F: Send, T1, T2> Send for TwoPathHandler<F, T1, T2> {}
unsafe impl<F: Sync, T1, T2> Sync for TwoPathHandler<F, T1, T2> {}

impl<F, T1, T2> RouteHandler for TwoPathHandler<F, T1, T2>
where
	F: Fn(Path<T1>, Path<T2>) -> Page + Send + Sync,
	T1: SingleFromPath + Send + Sync,
	T2: SingleFromPath + Send + Sync,
{
	fn handle(&self, ctx: &ParamContext) -> Result<Page, RouterError> {
		let v1 = T1::from_path_at(ctx, 0).map_err(RouterError::PathExtraction)?;
		let v2 = T2::from_path_at(ctx, 1).map_err(RouterError::PathExtraction)?;
		Ok((self.handler)(Path(v1), Path(v2)))
	}
}

/// Handler for routes with three `Path<T>` arguments.
///
/// Wraps a `Fn(Path<T1>, Path<T2>, Path<T3>) -> Page` closure.
pub(crate) struct ThreePathHandler<F, T1, T2, T3> {
	handler: F,
	_phantom: PhantomData<(T1, T2, T3)>,
}

impl<F, T1, T2, T3> ThreePathHandler<F, T1, T2, T3> {
	pub(crate) fn new(handler: F) -> Self {
		Self {
			handler,
			_phantom: PhantomData,
		}
	}
}

// SAFETY: ThreePathHandler is Send + Sync when F is Send + Sync
unsafe impl<F: Send, T1, T2, T3> Send for ThreePathHandler<F, T1, T2, T3> {}
unsafe impl<F: Sync, T1, T2, T3> Sync for ThreePathHandler<F, T1, T2, T3> {}

impl<F, T1, T2, T3> RouteHandler for ThreePathHandler<F, T1, T2, T3>
where
	F: Fn(Path<T1>, Path<T2>, Path<T3>) -> Page + Send + Sync,
	T1: SingleFromPath + Send + Sync,
	T2: SingleFromPath + Send + Sync,
	T3: SingleFromPath + Send + Sync,
{
	fn handle(&self, ctx: &ParamContext) -> Result<Page, RouterError> {
		let v1 = T1::from_path_at(ctx, 0).map_err(RouterError::PathExtraction)?;
		let v2 = T2::from_path_at(ctx, 1).map_err(RouterError::PathExtraction)?;
		let v3 = T3::from_path_at(ctx, 2).map_err(RouterError::PathExtraction)?;
		Ok((self.handler)(Path(v1), Path(v2), Path(v3)))
	}
}

/// Helper function to create a single-path handler.
pub(crate) fn single_path_handler<F, T>(handler: F) -> Arc<dyn RouteHandler>
where
	F: Fn(Path<T>) -> Page + Send + Sync + 'static,
	T: SingleFromPath + Send + Sync + 'static,
{
	Arc::new(SinglePathHandler::new(handler))
}

/// Helper function to create a two-path handler.
pub(crate) fn two_path_handler<F, T1, T2>(handler: F) -> Arc<dyn RouteHandler>
where
	F: Fn(Path<T1>, Path<T2>) -> Page + Send + Sync + 'static,
	T1: SingleFromPath + Send + Sync + 'static,
	T2: SingleFromPath + Send + Sync + 'static,
{
	Arc::new(TwoPathHandler::new(handler))
}

/// Helper function to create a three-path handler.
pub(crate) fn three_path_handler<F, T1, T2, T3>(handler: F) -> Arc<dyn RouteHandler>
where
	F: Fn(Path<T1>, Path<T2>, Path<T3>) -> Page + Send + Sync + 'static,
	T1: SingleFromPath + Send + Sync + 'static,
	T2: SingleFromPath + Send + Sync + 'static,
	T3: SingleFromPath + Send + Sync + 'static,
{
	Arc::new(ThreePathHandler::new(handler))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::routers::client_router::error::PathError;
	use std::collections::HashMap;

	fn test_page() -> Page {
		Page::Empty
	}

	fn page_with_text(s: &str) -> Page {
		Page::Text(s.to_string().into())
	}

	#[test]
	fn test_no_params_handler() {
		let handler = NoParamsHandler::new(test_page);
		let ctx = ParamContext::new(HashMap::new(), Vec::new());

		let result = handler.handle(&ctx);
		assert!(result.is_ok());
	}

	#[test]
	fn test_with_params_handler() {
		let handler =
			WithParamsHandler::new(|Path(id): Path<i32>| page_with_text(&format!("ID: {}", id)));

		let ctx = ParamContext::new(HashMap::new(), vec!["42".to_string()]);

		let result = handler.handle(&ctx);
		assert!(result.is_ok());
	}

	#[test]
	fn test_with_params_handler_error() {
		let handler = WithParamsHandler::new(|Path(_id): Path<i32>| Page::Empty);

		let ctx = ParamContext::new(HashMap::new(), vec!["not_a_number".to_string()]);

		let result = handler.handle(&ctx);
		assert!(result.is_err());

		match result {
			Err(RouterError::PathExtraction(PathError::ParseError { .. })) => {}
			_ => panic!("Expected PathExtraction error"),
		}
	}

	#[test]
	fn test_result_handler_ok() {
		let handler = ResultHandler::new(|Path(id): Path<i32>| {
			if id > 0 {
				Ok(page_with_text(&format!("ID: {}", id)))
			} else {
				Err(RouterError::NotFound("Invalid ID".to_string()))
			}
		});

		let ctx = ParamContext::new(HashMap::new(), vec!["42".to_string()]);

		let result = handler.handle(&ctx);
		assert!(result.is_ok());
	}

	#[test]
	fn test_result_handler_err() {
		let handler = ResultHandler::new(|Path(id): Path<i32>| {
			if id > 0 {
				Ok(Page::Empty)
			} else {
				Err(RouterError::NotFound("Invalid ID".to_string()))
			}
		});

		let ctx = ParamContext::new(HashMap::new(), vec!["-1".to_string()]);

		let result = handler.handle(&ctx);
		assert!(result.is_err());

		match result {
			Err(RouterError::NotFound(_)) => {}
			_ => panic!("Expected NotFound error"),
		}
	}

	#[test]
	fn test_with_params_handler_tuple() {
		let handler = WithParamsHandler::new(|Path((user_id, post_id)): Path<(i64, i64)>| {
			page_with_text(&format!("User: {}, Post: {}", user_id, post_id))
		});

		let ctx = ParamContext::new(HashMap::new(), vec!["123".to_string(), "456".to_string()]);

		let result = handler.handle(&ctx);
		assert!(result.is_ok());
	}

	#[test]
	fn test_helper_no_params_handler() {
		let handler: Arc<dyn RouteHandler> = no_params_handler(test_page);
		let ctx = ParamContext::new(HashMap::new(), Vec::new());

		let result = handler.handle(&ctx);
		assert!(result.is_ok());
	}

	#[test]
	fn test_helper_with_params_handler() {
		let handler: Arc<dyn RouteHandler> =
			with_params_handler(|Path(id): Path<i32>| page_with_text(&format!("ID: {}", id)));

		let ctx = ParamContext::new(HashMap::new(), vec!["42".to_string()]);

		let result = handler.handle(&ctx);
		assert!(result.is_ok());
	}

	#[test]
	fn test_helper_result_handler() {
		let handler: Arc<dyn RouteHandler> = result_handler(|Path(id): Path<i32>| {
			if id > 0 {
				Ok(page_with_text(&format!("ID: {}", id)))
			} else {
				Err(RouterError::NotFound("Invalid ID".to_string()))
			}
		});

		let ctx = ParamContext::new(HashMap::new(), vec!["42".to_string()]);

		let result = handler.handle(&ctx);
		assert!(result.is_ok());
	}
}
