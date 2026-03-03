//! Props system for component properties.

use std::collections::HashMap;

/// Trait for component properties.
///
/// Props are the input data for components. They can be constructed
/// from HTML attributes (for hydration) or directly in code.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::component::Props;
///
/// #[derive(Default)]
/// struct ButtonProps {
///     variant: String,
///     disabled: bool,
///     label: String,
/// }
///
/// impl Props for ButtonProps {
///     fn from_attrs(attrs: &HashMap<String, String>) -> Self {
///         Self {
///             variant: attrs.get("variant").cloned().unwrap_or_else(|| "primary".to_string()),
///             disabled: attrs.get("disabled").map(|v| v == "true").unwrap_or(false),
///             label: attrs.get("label").cloned().unwrap_or_default(),
///         }
///     }
/// }
/// ```
pub trait Props: Default {
	/// Constructs props from HTML attributes.
	///
	/// This is used during hydration to reconstruct component props
	/// from the serialized data in the DOM.
	fn from_attrs(attrs: &HashMap<String, String>) -> Self;
}

/// Empty props for components that don't need any.
#[derive(Debug, Clone, Default)]
// Allow dead_code: placeholder type for components without props
#[allow(dead_code)]
pub(super) struct EmptyProps;

impl Props for EmptyProps {
	fn from_attrs(_attrs: &HashMap<String, String>) -> Self {
		Self
	}
}

/// Builder for constructing props dynamically.
#[derive(Debug, Clone, Default)]
// Allow dead_code: builder pattern for constructing typed props
#[allow(dead_code)]
pub(super) struct PropsBuilder {
	attrs: HashMap<String, String>,
}

// Allow dead_code: impl block for PropsBuilder reserved for future use
#[allow(dead_code)]
impl PropsBuilder {
	/// Creates a new props builder.
	pub(super) fn new() -> Self {
		Self::default()
	}

	/// Sets an attribute value.
	pub(super) fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
		self.attrs.insert(name.into(), value.into());
		self
	}

	/// Sets a boolean attribute.
	pub(super) fn bool_attr(mut self, name: impl Into<String>, value: bool) -> Self {
		self.attrs.insert(name.into(), value.to_string());
		self
	}

	/// Sets an optional attribute.
	pub(super) fn optional_attr(
		self,
		name: impl Into<String>,
		value: Option<impl Into<String>>,
	) -> Self {
		match value {
			Some(v) => self.attr(name, v),
			None => self,
		}
	}

	/// Builds the props.
	pub(super) fn build<P: Props>(self) -> P {
		P::from_attrs(&self.attrs)
	}

	/// Returns the raw attributes.
	pub(super) fn into_attrs(self) -> HashMap<String, String> {
		self.attrs
	}
}

/// Serializes props to HTML attributes for SSR.
#[allow(dead_code)]
pub(super) fn serialize_props<P: serde::Serialize>(
	props: &P,
) -> Result<HashMap<String, String>, serde_json::Error> {
	let json = serde_json::to_value(props)?;

	let mut attrs = HashMap::new();
	if let serde_json::Value::Object(map) = json {
		for (key, value) in map {
			let str_value = match value {
				serde_json::Value::String(s) => s,
				serde_json::Value::Bool(b) => b.to_string(),
				serde_json::Value::Number(n) => n.to_string(),
				serde_json::Value::Null => continue,
				other => other.to_string(),
			};
			attrs.insert(key, str_value);
		}
	}

	Ok(attrs)
}

/// Deserializes props from HTML attributes for hydration.
#[allow(dead_code)]
pub(super) fn deserialize_props<P: serde::de::DeserializeOwned>(
	attrs: &HashMap<String, String>,
) -> Result<P, serde_json::Error> {
	let json = serde_json::to_value(attrs)?;
	serde_json::from_value(json)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[derive(Debug, Default, PartialEq)]
	struct TestProps {
		name: String,
		count: i32,
		enabled: bool,
	}

	impl Props for TestProps {
		fn from_attrs(attrs: &HashMap<String, String>) -> Self {
			Self {
				name: attrs.get("name").cloned().unwrap_or_default(),
				count: attrs.get("count").and_then(|v| v.parse().ok()).unwrap_or(0),
				enabled: attrs.get("enabled").map(|v| v == "true").unwrap_or(false),
			}
		}
	}

	#[test]
	fn test_props_from_attrs() {
		let mut attrs = HashMap::new();
		attrs.insert("name".to_string(), "Test".to_string());
		attrs.insert("count".to_string(), "42".to_string());
		attrs.insert("enabled".to_string(), "true".to_string());

		let props = TestProps::from_attrs(&attrs);
		assert_eq!(props.name, "Test");
		assert_eq!(props.count, 42);
		assert!(props.enabled);
	}

	#[test]
	fn test_props_default_values() {
		let attrs = HashMap::new();
		let props = TestProps::from_attrs(&attrs);
		assert_eq!(props, TestProps::default());
	}

	#[test]
	fn test_empty_props() {
		let attrs = HashMap::new();
		let props = EmptyProps::from_attrs(&attrs);
		assert_eq!(std::mem::size_of_val(&props), 0);
	}

	#[test]
	fn test_props_builder() {
		let props: TestProps = PropsBuilder::new()
			.attr("name", "Builder")
			.attr("count", "100")
			.bool_attr("enabled", true)
			.build();

		assert_eq!(props.name, "Builder");
		assert_eq!(props.count, 100);
		assert!(props.enabled);
	}

	#[test]
	fn test_props_builder_optional() {
		let props: TestProps = PropsBuilder::new()
			.attr("name", "Test")
			.optional_attr("count", Some("50"))
			.optional_attr("missing", None::<String>)
			.build();

		assert_eq!(props.name, "Test");
		assert_eq!(props.count, 50);
	}

	#[test]
	fn test_serialize_props() {
		use serde::Serialize;

		#[derive(Serialize)]
		struct SerProps {
			name: String,
			count: i32,
		}

		let props = SerProps {
			name: "Test".to_string(),
			count: 42,
		};

		let attrs = serialize_props(&props).unwrap();
		assert_eq!(attrs.get("name"), Some(&"Test".to_string()));
		assert_eq!(attrs.get("count"), Some(&"42".to_string()));
	}

	#[test]
	fn test_deserialize_props() {
		use serde::Deserialize;

		#[derive(Deserialize, Debug, PartialEq)]
		struct DeProps {
			name: String,
			count: String, // Note: comes as string from attrs
		}

		let mut attrs = HashMap::new();
		attrs.insert("name".to_string(), "Test".to_string());
		attrs.insert("count".to_string(), "42".to_string());

		let props: DeProps = deserialize_props(&attrs).unwrap();
		assert_eq!(props.name, "Test");
		assert_eq!(props.count, "42");
	}
}
