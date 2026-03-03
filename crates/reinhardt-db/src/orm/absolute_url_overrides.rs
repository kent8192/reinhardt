//! Absolute URL overrides support
//!
//! This module implements Django's ABSOLUTE_URL_OVERRIDES functionality,
//! which allows overriding model get_absolute_url() methods via configuration.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Type alias for URL override function
type UrlOverrideFn = Arc<dyn Fn(&dyn std::any::Any) -> Option<String> + Send + Sync>;

lazy_static::lazy_static! {
	/// Global registry for absolute URL overrides
	static ref URL_OVERRIDES: RwLock<HashMap<String, UrlOverrideFn>> = {
		RwLock::new(HashMap::new())
	};
}

/// Trait for models that can generate absolute URLs
pub trait HasAbsoluteUrl: std::any::Any {
	/// Get the absolute URL for this model instance
	fn get_absolute_url(&self) -> String
	where
		Self: Sized,
	{
		let model_id = Self::model_identifier();

		// Check if there's an override registered
		if let Some(url) = check_url_override(&model_id, self as &dyn std::any::Any) {
			return url;
		}

		// Fallback to default implementation
		self.default_get_absolute_url()
	}

	/// Default implementation of get_absolute_url
	fn default_get_absolute_url(&self) -> String {
		String::new()
	}

	/// Get the model identifier for URL override lookup (e.g., "app_label.modelname")
	fn model_identifier() -> String;
}
/// Register a URL override for a specific model
///
pub fn register_url_override<F>(model_path: impl Into<String>, generator: F)
where
	F: Fn(&dyn std::any::Any) -> Option<String> + Send + Sync + 'static,
{
	let mut overrides = URL_OVERRIDES.write().unwrap();
	overrides.insert(model_path.into(), Arc::new(generator));
}
/// Clear all URL overrides (useful for testing)
///
pub fn clear_url_overrides() {
	let mut overrides = URL_OVERRIDES.write().unwrap();
	overrides.clear();
}

/// Check if there's a URL override for a model
fn check_url_override(model_path: &str, obj: &dyn std::any::Any) -> Option<String> {
	let overrides = URL_OVERRIDES.read().unwrap();
	if let Some(generator) = overrides.get(model_path) {
		return generator(obj);
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;
	use serial_test::serial;
	use std::any::Any;

	#[derive(Debug)]
	struct TestA {
		pk: i32,
		// Allow dead_code: field used by derive macros for serialization
		#[allow(dead_code)]
		name: String,
	}

	impl TestA {
		fn new(pk: i32, name: impl Into<String>) -> Self {
			Self {
				pk,
				name: name.into(),
			}
		}
	}

	impl HasAbsoluteUrl for TestA {
		fn default_get_absolute_url(&self) -> String {
			format!("/test-a/{}/", self.pk)
		}

		fn model_identifier() -> String {
			"absolute_url_overrides.testa".to_string()
		}
	}

	#[derive(Debug)]
	struct TestB {
		pk: i32,
		// Allow dead_code: field used by derive macros for serialization
		#[allow(dead_code)]
		name: String,
	}

	impl TestB {
		fn new(pk: i32, name: impl Into<String>) -> Self {
			Self {
				pk,
				name: name.into(),
			}
		}
	}

	impl HasAbsoluteUrl for TestB {
		fn default_get_absolute_url(&self) -> String {
			format!("/test-b/{}/", self.pk)
		}

		fn model_identifier() -> String {
			"absolute_url_overrides.testb".to_string()
		}
	}

	#[derive(Debug)]
	struct TestC {
		pk: i32,
		// Allow dead_code: field used by derive macros for serialization
		#[allow(dead_code)]
		name: String,
	}

	impl TestC {
		fn new(pk: i32, name: impl Into<String>) -> Self {
			Self {
				pk,
				name: name.into(),
			}
		}
	}

	impl HasAbsoluteUrl for TestC {
		fn model_identifier() -> String {
			"absolute_url_overrides.testc".to_string()
		}

		// No default implementation - relies entirely on override
	}

	#[test]
	#[serial(url_overrides)]
	fn test_get_absolute_url() {
		clear_url_overrides();

		let obj = TestA::new(1, "Foo");
		assert_eq!("/test-a/1/", obj.get_absolute_url());
	}

	#[test]
	#[serial(url_overrides)]
	fn test_override_get_absolute_url() {
		clear_url_overrides();

		// Register an override for TestB
		register_url_override("absolute_url_overrides.testb", |obj: &dyn Any| {
			obj.downcast_ref::<TestB>()
				.map(|test_b| format!("/overridden-test-b/{}/", test_b.pk))
		});

		let obj = TestB::new(1, "Foo");
		assert_eq!("/overridden-test-b/1/", obj.get_absolute_url());

		clear_url_overrides();
	}

	#[test]
	#[serial(url_overrides)]
	fn test_insert_get_absolute_url() {
		clear_url_overrides();

		// TestC has no default get_absolute_url, but we can add one via override
		register_url_override("absolute_url_overrides.testc", |obj: &dyn Any| {
			obj.downcast_ref::<TestC>()
				.map(|test_c| format!("/test-c/{}/", test_c.pk))
		});

		let obj = TestC::new(1, "Foo");
		assert_eq!("/test-c/1/", obj.get_absolute_url());

		clear_url_overrides();
	}
}
