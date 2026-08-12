//! Typed settings schema references and recursive settings metadata.

use std::collections::HashSet;
use std::fmt;
use std::marker::PhantomData;

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::builder::BuildError;
use super::policy::{FieldPolicy, FieldRequirement};

/// A segment in a typed settings path.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SettingsPathSegment {
	/// A static serialized settings key.
	Key(&'static str),
	/// A concrete key discovered at runtime.
	DynamicKey(String),
	/// A wildcard key for map-like values.
	AnyKey,
	/// A wildcard index for sequence-like values.
	AnyIndex,
}

/// Owned settings path segments.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SettingsPathBuf {
	segments: Vec<SettingsPathSegment>,
}

impl SettingsPathBuf {
	/// Create an empty settings path.
	pub fn new() -> Self {
		Self::default()
	}

	/// Create a settings path from a single static key.
	pub fn from_key(key: &'static str) -> Self {
		Self {
			segments: vec![SettingsPathSegment::Key(key)],
		}
	}

	/// Create a settings path from owned segments.
	pub fn from_segments(segments: impl IntoIterator<Item = SettingsPathSegment>) -> Self {
		Self {
			segments: segments.into_iter().collect(),
		}
	}

	/// Return a new settings path with a static key appended.
	pub fn with_key(&self, key: &'static str) -> Self {
		let mut path = self.clone();
		path.segments.push(SettingsPathSegment::Key(key));
		path
	}

	/// Return a new settings path with a concrete runtime key appended.
	pub fn with_dynamic_key(&self, key: impl Into<String>) -> Self {
		let mut path = self.clone();
		path.segments
			.push(SettingsPathSegment::DynamicKey(key.into()));
		path
	}

	/// Return a new settings path with a wildcard map key appended.
	pub fn with_any_key(&self) -> Self {
		let mut path = self.clone();
		path.segments.push(SettingsPathSegment::AnyKey);
		path
	}

	/// Return a new settings path with a wildcard sequence index appended.
	pub fn with_any_index(&self) -> Self {
		let mut path = self.clone();
		path.segments.push(SettingsPathSegment::AnyIndex);
		path
	}

	/// Extend this path with another owned path.
	pub fn extend(mut self, other: SettingsPathBuf) -> Self {
		self.segments.extend(other.segments);
		self
	}

	/// Borrow this path's segments.
	pub fn segments(&self) -> &[SettingsPathSegment] {
		&self.segments
	}
}

impl fmt::Display for SettingsPathBuf {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for (index, segment) in self.segments.iter().enumerate() {
			if index > 0 {
				f.write_str(".")?;
			}
			match segment {
				SettingsPathSegment::Key(key) => f.write_str(key)?,
				SettingsPathSegment::DynamicKey(key) => f.write_str(key)?,
				SettingsPathSegment::AnyKey | SettingsPathSegment::AnyIndex => f.write_str("*")?,
			}
		}
		Ok(())
	}
}

/// Typed reference to a settings field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldRef<Root, Value> {
	path: SettingsPathBuf,
	_marker: PhantomData<fn() -> (Root, Value)>,
}

impl<Root, Value> FieldRef<Root, Value> {
	/// Create a typed field reference at the given path.
	pub fn new(path: SettingsPathBuf) -> Self {
		Self {
			path,
			_marker: PhantomData,
		}
	}

	/// Borrow the settings path referenced by this field.
	pub fn path(&self) -> &SettingsPathBuf {
		&self.path
	}
}

/// Typed reference to a secret settings field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretFieldRef<Root, Value> {
	path: SettingsPathBuf,
	_marker: PhantomData<fn() -> (Root, Value)>,
}

impl<Root, Value> SecretFieldRef<Root, Value> {
	/// Create a typed secret field reference at the given path.
	pub fn new(path: SettingsPathBuf) -> Self {
		Self {
			path,
			_marker: PhantomData,
		}
	}

	/// Borrow the settings path referenced by this field.
	pub fn path(&self) -> &SettingsPathBuf {
		&self.path
	}

	/// Erase the referenced value type while preserving root and path.
	pub fn erase_value(&self) -> SecretFieldRef<Root, ()> {
		SecretFieldRef::new(self.path.clone())
	}
}

/// Typed reference to an optional settings value.
#[derive(Clone, Debug)]
pub struct OptionalRef<Root, Value, SomeRef> {
	path: SettingsPathBuf,
	builder: fn(SettingsPathBuf) -> SomeRef,
	_marker: PhantomData<fn() -> (Root, Value)>,
}

impl<Root, Value, SomeRef> OptionalRef<Root, Value, SomeRef> {
	/// Create a typed optional reference.
	pub fn new(path: SettingsPathBuf, builder: fn(SettingsPathBuf) -> SomeRef) -> Self {
		Self {
			path,
			builder,
			_marker: PhantomData,
		}
	}

	/// Build the reference for the contained value.
	pub fn some(&self) -> SomeRef {
		(self.builder)(self.path.clone())
	}

	/// Borrow the settings path referenced by this optional.
	pub fn path(&self) -> &SettingsPathBuf {
		&self.path
	}
}

impl<Root, Value, SomeRef> PartialEq for OptionalRef<Root, Value, SomeRef> {
	fn eq(&self, other: &Self) -> bool {
		self.path == other.path
	}
}

impl<Root, Value, SomeRef> Eq for OptionalRef<Root, Value, SomeRef> {}

/// Typed reference to a sequence settings value.
#[derive(Clone, Debug)]
pub struct SequenceRef<Root, Value, ItemRef> {
	path: SettingsPathBuf,
	builder: fn(SettingsPathBuf) -> ItemRef,
	_marker: PhantomData<fn() -> (Root, Value)>,
}

impl<Root, Value, ItemRef> SequenceRef<Root, Value, ItemRef> {
	/// Create a typed sequence reference.
	pub fn new(path: SettingsPathBuf, builder: fn(SettingsPathBuf) -> ItemRef) -> Self {
		Self {
			path,
			builder,
			_marker: PhantomData,
		}
	}

	/// Build the reference for any item in the sequence.
	pub fn any(&self) -> ItemRef {
		(self.builder)(self.path.with_any_index())
	}

	/// Borrow the settings path referenced by this sequence.
	pub fn path(&self) -> &SettingsPathBuf {
		&self.path
	}
}

impl<Root, Value, ItemRef> PartialEq for SequenceRef<Root, Value, ItemRef> {
	fn eq(&self, other: &Self) -> bool {
		self.path == other.path
	}
}

impl<Root, Value, ItemRef> Eq for SequenceRef<Root, Value, ItemRef> {}

/// Typed reference to a map settings value.
#[derive(Clone, Debug)]
pub struct MapRef<Root, Value, ItemRef> {
	path: SettingsPathBuf,
	builder: fn(SettingsPathBuf) -> ItemRef,
	_marker: PhantomData<fn() -> (Root, Value)>,
}

impl<Root, Value, ItemRef> MapRef<Root, Value, ItemRef> {
	/// Create a typed map reference.
	pub fn new(path: SettingsPathBuf, builder: fn(SettingsPathBuf) -> ItemRef) -> Self {
		Self {
			path,
			builder,
			_marker: PhantomData,
		}
	}

	/// Build the reference for any item in the map.
	pub fn any(&self) -> ItemRef {
		(self.builder)(self.path.with_any_key())
	}

	/// Build the reference for a concrete runtime map entry.
	pub fn entry(&self, key: impl Into<String>) -> ItemRef {
		(self.builder)(self.path.with_dynamic_key(key))
	}

	/// Borrow the settings path referenced by this map.
	pub fn path(&self) -> &SettingsPathBuf {
		&self.path
	}
}

impl<Root, Value, ItemRef> PartialEq for MapRef<Root, Value, ItemRef> {
	fn eq(&self, other: &Self) -> bool {
		self.path == other.path
	}
}

impl<Root, Value, ItemRef> Eq for MapRef<Root, Value, ItemRef> {}

/// Runtime metadata for a single settings field.
#[derive(Clone, Debug)]
pub struct SettingsFieldSchema {
	/// Rust struct field name.
	pub rust_name: &'static str,
	/// Serialized settings key.
	pub key: &'static str,
	/// Keys accepted while deserializing this field.
	pub deserialize_keys: &'static [&'static str],
	/// Required/default policy for this field.
	pub policy: FieldPolicy,
	/// Runtime schema for the field value.
	pub value: SettingsValueSchema,
}

/// Value-free metadata for one resolved settings leaf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSettingsField {
	/// Resolved settings path.
	pub path: SettingsPathBuf,
	/// Rust type name for the leaf.
	pub rust_type: &'static str,
	/// Required/default policy for the leaf.
	pub policy: FieldPolicy,
	/// Whether the leaf contains secret material.
	pub secret: bool,
	/// Whether the merged input contains the leaf.
	pub present: bool,
}

/// Value-free metadata captured while resolving composed settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingsResolutionMetadata {
	fields: Vec<ResolvedSettingsField>,
}

impl SettingsResolutionMetadata {
	/// Borrow the resolved settings fields.
	pub fn fields(&self) -> &[ResolvedSettingsField] {
		&self.fields
	}

	/// Consume the metadata and return its fields.
	pub fn into_fields(self) -> Vec<ResolvedSettingsField> {
		self.fields
	}

	#[doc(hidden)]
	pub fn from_fields(fields: Vec<ResolvedSettingsField>) -> Self {
		Self { fields }
	}
}

/// Runtime metadata for a settings field value.
#[derive(Clone, Debug)]
pub enum SettingsValueSchema {
	/// A leaf value with its Rust type name and secret classification.
	Leaf {
		/// Rust type name.
		type_name: &'static str,
		/// Whether this leaf contains secret material.
		secret: bool,
	},
	/// A nested settings node.
	Node {
		/// Rust type name.
		type_name: &'static str,
		/// Build node metadata for a concrete path.
		node: fn(SettingsPathBuf) -> SettingsNodeSchema,
	},
	/// Optional nested value.
	Optional {
		/// Inner value schema.
		inner: Box<SettingsValueSchema>,
	},
	/// Sequence nested value.
	Sequence {
		/// Inner item schema.
		inner: Box<SettingsValueSchema>,
	},
	/// Map nested value.
	Map {
		/// Inner item schema.
		inner: Box<SettingsValueSchema>,
	},
}

impl SettingsValueSchema {
	fn resolve_fields(
		&self,
		value: Option<&Value>,
		path: SettingsPathBuf,
		policy: FieldPolicy,
		output: &mut Vec<ResolvedSettingsField>,
		visited: &mut HashSet<&'static str>,
	) {
		match self {
			SettingsValueSchema::Leaf { type_name, secret } => output.push(ResolvedSettingsField {
				path,
				rust_type: type_name,
				policy,
				secret: *secret,
				present: value.is_some(),
			}),
			SettingsValueSchema::Node {
				type_name, node, ..
			} => {
				if !visited.insert(type_name) {
					return;
				}
				node(path.clone()).resolve_fields_inner(
					value.and_then(Value::as_object),
					path,
					output,
					visited,
				);
				visited.remove(type_name);
			}
			SettingsValueSchema::Optional { inner } => {
				inner.resolve_fields(value, path, policy, output, visited)
			}
			SettingsValueSchema::Sequence { inner } => {
				inner.resolve_fields(None, path.with_any_index(), policy, output, visited);
				if let Some(items) = value.and_then(Value::as_array) {
					for (index, item) in items.iter().enumerate() {
						inner.resolve_fields(
							Some(item),
							path.with_dynamic_key(index.to_string()),
							policy,
							output,
							visited,
						);
					}
				}
			}
			SettingsValueSchema::Map { inner } => {
				inner.resolve_fields(None, path.with_any_key(), policy, output, visited);
				if let Some(entries) = value.and_then(Value::as_object) {
					for (key, item) in entries {
						inner.resolve_fields(
							Some(item),
							path.with_dynamic_key(key.clone()),
							policy,
							output,
							visited,
						);
					}
				}
			}
		}
	}

	fn validate_required(
		&self,
		value: Option<&Value>,
		path: SettingsPathBuf,
	) -> Result<(), BuildError> {
		match self {
			SettingsValueSchema::Leaf { .. } => Ok(()),
			SettingsValueSchema::Node { node, .. } => {
				if let Some(map) = value.and_then(Value::as_object) {
					node(path.clone()).validate_required_map_at(map, path)?;
				}
				Ok(())
			}
			SettingsValueSchema::Optional { inner } => {
				if let Some(value) = value {
					inner.validate_required(Some(value), path)?;
				}
				Ok(())
			}
			SettingsValueSchema::Sequence { inner } => {
				if let Some(items) = value.and_then(Value::as_array) {
					for (index, item) in items.iter().enumerate() {
						inner.validate_required(
							Some(item),
							path.with_dynamic_key(index.to_string()),
						)?;
					}
				}
				Ok(())
			}
			SettingsValueSchema::Map { inner } => {
				if let Some(entries) = value.and_then(Value::as_object) {
					for (key, item) in entries {
						inner.validate_required(Some(item), path.with_dynamic_key(key.clone()))?;
					}
				}
				Ok(())
			}
		}
	}

	fn collect_secret_paths(&self, path: SettingsPathBuf, output: &mut Vec<SettingsPathBuf>) {
		match self {
			SettingsValueSchema::Leaf { secret, .. } => {
				if *secret {
					output.push(path);
				}
			}
			SettingsValueSchema::Node { node, .. } => {
				node(path.clone()).collect_secret_paths_at(path, output);
			}
			SettingsValueSchema::Optional { inner } => {
				inner.collect_secret_paths(path, output);
			}
			SettingsValueSchema::Sequence { inner } => {
				inner.collect_secret_paths(path.with_any_index(), output);
			}
			SettingsValueSchema::Map { inner } => {
				inner.collect_secret_paths(path.with_any_key(), output);
			}
		}
	}
}

/// Runtime metadata for a settings node.
#[derive(Clone, Debug)]
pub struct SettingsNodeSchema {
	/// Rust type name.
	pub type_name: &'static str,
	/// Field schemas for this node.
	pub fields: Vec<SettingsFieldSchema>,
}

impl SettingsNodeSchema {
	/// Resolve value-free metadata for every leaf below this node.
	pub fn resolve_fields(
		&self,
		map: Option<&serde_json::Map<String, Value>>,
		base_path: SettingsPathBuf,
	) -> Vec<ResolvedSettingsField> {
		let mut fields = Vec::new();
		self.resolve_fields_inner(map, base_path, &mut fields, &mut HashSet::new());
		let present_paths: Vec<_> = fields
			.iter()
			.filter(|field| field.present)
			.map(|field| field.path.clone())
			.collect();
		for field in &mut fields {
			if !field.present && is_wildcard_path(&field.path) {
				field.present = present_paths
					.iter()
					.any(|path| wildcard_matches(&field.path, path));
			}
		}
		fields
	}
	/// Validate required fields in a JSON object map.
	pub fn validate_required_map(
		&self,
		map: &serde_json::Map<String, Value>,
	) -> Result<(), BuildError> {
		self.validate_required_map_at(map, SettingsPathBuf::new())
	}

	/// Validate required fields in a JSON object map rooted at the given path.
	pub fn validate_required_map_at(
		&self,
		map: &serde_json::Map<String, Value>,
		base_path: SettingsPathBuf,
	) -> Result<(), BuildError> {
		self.validate_required_map_inner(map, base_path)
	}

	/// Collect all secret paths reachable from this node.
	pub fn collect_secret_paths(&self, output: &mut Vec<SettingsPathBuf>) {
		self.collect_secret_paths_at(SettingsPathBuf::new(), output);
	}

	fn validate_required_map_inner(
		&self,
		map: &serde_json::Map<String, Value>,
		base_path: SettingsPathBuf,
	) -> Result<(), BuildError> {
		for field in &self.fields {
			let path = base_path.with_key(field.key);
			let value = field.deserialize_keys.iter().find_map(|key| map.get(*key));
			if field.policy.requirement == FieldRequirement::Required && value.is_none() {
				return Err(BuildError::MissingRequiredPath { path });
			}
			field.value.validate_required(value, path)?;
		}
		Ok(())
	}

	fn collect_secret_paths_at(
		&self,
		base_path: SettingsPathBuf,
		output: &mut Vec<SettingsPathBuf>,
	) {
		for field in &self.fields {
			field
				.value
				.collect_secret_paths(base_path.with_key(field.key), output);
		}
	}

	fn resolve_fields_inner(
		&self,
		map: Option<&serde_json::Map<String, Value>>,
		base_path: SettingsPathBuf,
		output: &mut Vec<ResolvedSettingsField>,
		visited: &mut HashSet<&'static str>,
	) {
		for field in &self.fields {
			field.value.resolve_fields(
				map.and_then(|map| field.deserialize_keys.iter().find_map(|key| map.get(*key))),
				base_path.with_key(field.key),
				field.policy,
				output,
				visited,
			);
		}
	}
}

fn is_wildcard_path(path: &SettingsPathBuf) -> bool {
	path.segments().iter().any(|segment| {
		matches!(
			segment,
			SettingsPathSegment::AnyKey | SettingsPathSegment::AnyIndex
		)
	})
}

fn wildcard_matches(pattern: &SettingsPathBuf, concrete: &SettingsPathBuf) -> bool {
	pattern.segments().len() == concrete.segments().len()
		&& pattern
			.segments()
			.iter()
			.zip(concrete.segments())
			.all(|(pattern, concrete)| match pattern {
				SettingsPathSegment::AnyKey | SettingsPathSegment::AnyIndex => {
					matches!(concrete, SettingsPathSegment::DynamicKey(_))
				}
				_ => pattern == concrete,
			})
}

/// Trait for recursive settings nodes that can expose typed schema references.
pub trait SettingsNode:
	Clone + fmt::Debug + serde::Serialize + DeserializeOwned + Send + Sync + 'static
{
	/// Typed reference schema rooted at `Root`.
	type Schema<Root>;

	/// Build typed schema references for this node at the provided path.
	fn schema_at<Root>(path: SettingsPathBuf) -> Self::Schema<Root>;

	/// Build runtime metadata for this node.
	fn node_schema() -> SettingsNodeSchema;
}

/// Trait for root settings values that expose typed schema references.
pub trait HasSettingsSchema {
	/// Typed schema reference type.
	type Schema;

	/// Build root schema references.
	fn schema() -> Self::Schema;

	/// Build root schema references.
	fn settings_schema() -> Self::Schema {
		Self::schema()
	}
}

/// Generated-code support for selecting composed root sections.
#[doc(hidden)]
pub fn root_section<'a>(
	merged: &'a IndexMap<String, Value>,
	primary_key: &'static str,
	fallback_key: &'static str,
) -> Option<&'a serde_json::Map<String, Value>> {
	merged
		.get(primary_key)
		.or_else(|| {
			if primary_key == fallback_key {
				None
			} else {
				merged.get(fallback_key)
			}
		})
		.and_then(Value::as_object)
}

#[cfg(test)]
mod tests {
	use indexmap::IndexMap;
	use serde_json::{Value, json};

	use super::*;

	fn optional_policy(name: &'static str) -> FieldPolicy {
		FieldPolicy {
			name,
			requirement: FieldRequirement::Optional,
			has_default: true,
		}
	}

	fn database_schema(_: SettingsPathBuf) -> SettingsNodeSchema {
		SettingsNodeSchema {
			type_name: "DatabaseSettings",
			fields: vec![SettingsFieldSchema {
				rust_name: "password",
				key: "password",
				deserialize_keys: &["password"],
				policy: optional_policy("password"),
				value: SettingsValueSchema::Leaf {
					type_name: "String",
					secret: true,
				},
			}],
		}
	}

	#[test]
	fn resolve_fields_reports_fixed_optional_and_dynamic_leaves() {
		let schema = SettingsNodeSchema {
			type_name: "CoreSettings",
			fields: vec![
				SettingsFieldSchema {
					rust_name: "databases",
					key: "databases",
					deserialize_keys: &["databases"],
					policy: optional_policy("databases"),
					value: SettingsValueSchema::Map {
						inner: Box::new(SettingsValueSchema::Node {
							type_name: "DatabaseSettings",
							node: database_schema,
						}),
					},
				},
				SettingsFieldSchema {
					rust_name: "fixed",
					key: "fixed",
					deserialize_keys: &["fixed"],
					policy: optional_policy("fixed"),
					value: SettingsValueSchema::Leaf {
						type_name: "String",
						secret: false,
					},
				},
				SettingsFieldSchema {
					rust_name: "optional",
					key: "optional",
					deserialize_keys: &["optional"],
					policy: optional_policy("optional"),
					value: SettingsValueSchema::Optional {
						inner: Box::new(SettingsValueSchema::Leaf {
							type_name: "String",
							secret: false,
						}),
					},
				},
				SettingsFieldSchema {
					rust_name: "ports",
					key: "ports",
					deserialize_keys: &["ports"],
					policy: optional_policy("ports"),
					value: SettingsValueSchema::Sequence {
						inner: Box::new(SettingsValueSchema::Leaf {
							type_name: "u16",
							secret: false,
						}),
					},
				},
			],
		};
		let map = json!({
			"databases": { "default": { "password": "secret" } },
			"fixed": "value",
			"ports": [5432],
		});
		let fields = schema.resolve_fields(
			Some(map.as_object().expect("settings object")),
			SettingsPathBuf::from_key("core"),
		);

		assert_eq!(
			fields[0].path.segments(),
			&[
				SettingsPathSegment::Key("core"),
				SettingsPathSegment::Key("databases"),
				SettingsPathSegment::AnyKey,
				SettingsPathSegment::Key("password"),
			]
		);
		assert!(fields[0].present);
		assert_eq!(
			fields[1].path.segments(),
			&[
				SettingsPathSegment::Key("core"),
				SettingsPathSegment::Key("databases"),
				SettingsPathSegment::DynamicKey("default".to_string()),
				SettingsPathSegment::Key("password"),
			]
		);
		assert!(fields[1].present);
		assert_eq!(fields[2].path.to_string(), "core.fixed");
		assert!(fields[2].present);
		assert_eq!(fields[3].path.to_string(), "core.optional");
		assert!(!fields[3].present);
		assert_eq!(fields[4].path.to_string(), "core.ports.*");
		assert!(fields[4].present);
		assert_eq!(fields[5].path.to_string(), "core.ports.0");
		assert!(fields[5].present);
	}

	#[test]
	fn resolve_fields_stops_absent_recursive_optional_nodes() {
		fn recursive_schema(_: SettingsPathBuf) -> SettingsNodeSchema {
			SettingsNodeSchema {
				type_name: "TreeConfig",
				fields: vec![SettingsFieldSchema {
					rust_name: "child",
					key: "child",
					deserialize_keys: &["child"],
					policy: optional_policy("child"),
					value: SettingsValueSchema::Optional {
						inner: Box::new(SettingsValueSchema::Node {
							type_name: "TreeConfig",
							node: recursive_schema,
						}),
					},
				}],
			}
		}

		let fields = recursive_schema(SettingsPathBuf::from_key("tree")).resolve_fields(
			Some(&serde_json::Map::new()),
			SettingsPathBuf::from_key("tree"),
		);

		assert!(fields.is_empty());
	}

	#[test]
	fn resolve_fields_and_required_validation_accept_deserialize_aliases() {
		let schema = SettingsNodeSchema {
			type_name: "AliasSettings",
			fields: vec![SettingsFieldSchema {
				rust_name: "display_name",
				key: "displayName",
				deserialize_keys: &["displayName", "legacy_name"],
				policy: FieldPolicy {
					name: "display_name",
					requirement: FieldRequirement::Required,
					has_default: false,
				},
				value: SettingsValueSchema::Leaf {
					type_name: "String",
					secret: false,
				},
			}],
		};
		let value = serde_json::json!({"legacy_name": "value"});
		let map = value.as_object().expect("settings object");

		let fields = schema.resolve_fields(Some(map), SettingsPathBuf::from_key("settings"));
		assert_eq!(fields.len(), 1);
		assert_eq!(fields[0].path.to_string(), "settings.displayName");
		assert!(fields[0].present);
		schema
			.validate_required_map(map)
			.expect("deserialize alias should satisfy required field");
	}

	#[test]
	fn root_section_primary_object_wins_over_fallback_object() {
		let mut merged = IndexMap::new();
		merged.insert("primary".to_string(), json!({ "source": "primary" }));
		merged.insert("fallback".to_string(), json!({ "source": "fallback" }));

		let section = root_section(&merged, "primary", "fallback").expect("primary object");

		assert_eq!(
			section.get("source"),
			Some(&Value::String("primary".into()))
		);
	}

	#[test]
	fn root_section_uses_fallback_object_when_primary_absent() {
		let mut merged = IndexMap::new();
		merged.insert("fallback".to_string(), json!({ "source": "fallback" }));

		let section = root_section(&merged, "primary", "fallback").expect("fallback object");

		assert_eq!(
			section.get("source"),
			Some(&Value::String("fallback".into()))
		);
	}

	#[test]
	fn root_section_malformed_primary_scalar_does_not_fall_back() {
		let mut merged = IndexMap::new();
		merged.insert("primary".to_string(), json!("malformed"));
		merged.insert("fallback".to_string(), json!({ "source": "fallback" }));

		let section = root_section(&merged, "primary", "fallback");

		assert!(section.is_none());
	}
}
