//! ORM-integrated shortcut functions for database queries with 404 error handling
//!
//! These functions provide direct integration with `reinhardt-orm` for database
//! operations that return 404 errors when objects are not found.
//!
//! This module is only available with the `database` feature enabled.

#[cfg(feature = "database")]
use reinhardt_db::prelude::Model;
#[cfg(feature = "database")]
use reinhardt_http::Response;

/// Get a single object from the database or return a 404 response
///
/// This function directly integrates with the ORM to query the database and
/// returns a 404 HTTP response if the object is not found.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_shortcuts::get_object_or_404;
/// use reinhardt_db::orm::Model;
/// use reinhardt_http::Response;
///
/// // In an async view handler:
/// async fn user_detail(user_id: i64) -> Result<Response, Response> {
///     let user = get_object_or_404::<User>(user_id).await?;
///     // user is guaranteed to exist here
///     Ok(render_json(&user))
/// }
/// ```
///
/// # Arguments
///
/// * `pk` - The primary key of the object to retrieve
///
/// # Returns
///
/// Either the queried object or a 404 Response
///
/// # Errors
///
/// Returns `Err(Response)` with HTTP 404 if the object is not found,
/// or HTTP 500 if a database error occurs.
#[cfg(feature = "database")]
pub async fn get_object_or_404<M>(pk: M::PrimaryKey) -> Result<M, Response>
where
	M: Model + serde::de::DeserializeOwned + 'static,
	M::PrimaryKey: ToString,
{
	// Get the manager for this model
	let manager = M::objects();

	// Query by primary key
	let queryset = manager.get(pk);

	// Execute the query - await the async result
	let results = queryset.all().await.map_err(|e| {
		// Log the full error server-side only; never expose it in the HTTP response
		tracing::error!("Database query error in get_object_or_404: {:?}", e);
		Response::internal_server_error()
	})?;

	match results.into_iter().next() {
		Some(obj) => Ok(obj),
		None => Err(Response::not_found()),
	}
}

/// Get a list of objects from the database or return a 404 response if empty
///
/// This function queries the database using the provided `QuerySet` and returns
/// a 404 HTTP response if the result list is empty.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_shortcuts::get_list_or_404;
/// use reinhardt_db::orm::{Model, QuerySet};
/// use reinhardt_db::prelude::{FilterOperator, FilterValue};
/// use reinhardt_http::Response;
///
/// // In an async view handler:
/// async fn user_list(status: &str) -> Result<Response, Response> {
///     let queryset = User::objects()
///         .filter("status", FilterOperator::Eq, FilterValue::String(status.to_string()));
///
///     let users = get_list_or_404(queryset).await?;
///     // users is guaranteed to be non-empty here
///     Ok(render_json(&users))
/// }
/// ```
///
/// # Arguments
///
/// * `queryset` - A QuerySet to execute
///
/// # Returns
///
/// Either a non-empty list of objects or a 404 Response
///
/// # Errors
///
/// Returns `Err(Response)` with HTTP 404 if the result list is empty,
/// or HTTP 500 if a database error occurs.
#[cfg(feature = "database")]
pub async fn get_list_or_404<M>(
	queryset: reinhardt_db::prelude::QuerySet<M>,
) -> Result<Vec<M>, Response>
where
	M: Model + 'static,
{
	// Execute the query - await the async result
	let results = queryset.all().await.map_err(|e| {
		// Log the full error server-side only; never expose it in the HTTP response
		tracing::error!("Database query error in get_list_or_404: {:?}", e);
		Response::internal_server_error()
	})?;

	if results.is_empty() {
		Err(Response::not_found())
	} else {
		Ok(results)
	}
}
