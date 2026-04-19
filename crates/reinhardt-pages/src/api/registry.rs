//! API Model Registry
//!
//! This module provides the `ApiModel` trait that models implement
//! to enable QuerySet-like API access.

use super::queryset::ApiQuerySet;
use serde::{Serialize, de::DeserializeOwned};

/// Trait for models that can be accessed via the API.
///
/// Implement this trait to enable QuerySet-like operations on a model.
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::api::{ApiModel, ApiQuerySet};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize)]
/// struct User {
///     id: i64,
///     username: String,
///     email: String,
/// }
///
/// impl ApiModel for User {
///     fn endpoint() -> &'static str {
///         "/api/users/"
///     }
/// }
///
/// // Now you can use:
/// // let users = User::objects().filter("is_active", true).all().await?;
/// ```
pub trait ApiModel: Serialize + DeserializeOwned + Sized {
	/// Returns the API endpoint for this model.
	///
	/// Should include the trailing slash (e.g., "/api/users/").
	fn endpoint() -> &'static str;

	/// Returns a new QuerySet for this model.
	fn objects() -> ApiQuerySet<Self> {
		ApiQuerySet::new(Self::endpoint())
	}

	/// Returns a QuerySet filtered by primary key.
	fn filter_pk(pk: impl std::fmt::Display) -> ApiQuerySet<Self> {
		ApiQuerySet::new(Self::endpoint()).filter("pk", pk.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, Serialize, Deserialize)]
	struct TestUser {
		id: i64,
		username: String,
	}

	impl ApiModel for TestUser {
		fn endpoint() -> &'static str {
			"/api/users/"
		}
	}

	#[test]
	fn test_api_model_endpoint() {
		assert_eq!(TestUser::endpoint(), "/api/users/");
	}

	#[test]
	fn test_api_model_objects() {
		let qs = TestUser::objects();
		assert_eq!(qs.build_url(), "/api/users/");
	}

	#[test]
	fn test_api_model_filter_pk() {
		let qs = TestUser::filter_pk(42);
		let url = qs.build_url();
		assert!(url.contains("pk=42"));
	}

	#[test]
	fn test_api_model_chaining() {
		let qs = TestUser::objects()
			.filter("is_active", true)
			.order_by(&["-created_at"])
			.limit(10);

		let url = qs.build_url();
		assert!(url.contains("is_active=true"));
		assert!(url.contains("ordering=-created_at"));
		assert!(url.contains("limit=10"));
	}
}
