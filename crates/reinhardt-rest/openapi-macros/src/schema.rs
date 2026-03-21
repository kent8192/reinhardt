//! Schema attribute parsing and field metadata extraction

use proc_macro2::Span;
use syn::{Attribute, Lit, Meta, MetaNameValue};

/// Container-level schema attributes applied to structs/enums via `#[schema(...)]`
#[derive(Debug, Default, Clone)]
pub(crate) struct ContainerAttributes {
	/// Override the schema title (default: type name)
	pub title: Option<String>,
	/// Schema description (overrides doc comments)
	pub description: Option<String>,
	/// Example value for the entire type (JSON string)
	pub example: Option<String>,
	/// Mark the entire type as deprecated
	pub deprecated: bool,
	/// Allow null values (adds Type::Null to schema_type)
	pub nullable: bool,
}

/// Field-level schema attributes
#[derive(Debug, Default, Clone)]
pub(crate) struct FieldAttributes {
	pub description: Option<String>,
	pub example: Option<String>,
	pub format: Option<String>,
	pub default: bool,
	pub deprecated: bool,
	pub read_only: bool,
	pub write_only: bool,
	pub minimum: Option<i64>,
	pub maximum: Option<i64>,
	pub min_length: Option<usize>,
	pub max_length: Option<usize>,
	pub pattern: Option<String>,
	/// Property name override from `#[serde(rename = "...")]` or `#[schema(rename = "...")]`
	pub rename: Option<String>,
	/// Whether `#[serde(skip)]` is present - excludes field from schema
	/// Fixes #836
	pub skip: bool,
	/// Whether `#[serde(skip_serializing)]` is present - excludes field from schema
	/// Fixes #836
	pub skip_serializing: bool,
	/// Whether `#[serde(skip_deserializing)]` is present - excludes field from schema
	/// Fixes #836
	pub skip_deserializing: bool,
	/// Whether `#[serde(flatten)]` is present - merges field's schema via allOf
	/// Fixes #839
	pub flatten: bool,
	/// Allow null values for this field
	pub nullable: bool,
	/// Exclusive lower bound for numeric values
	pub exclusive_minimum: Option<i64>,
	/// Exclusive upper bound for numeric values
	pub exclusive_maximum: Option<i64>,
	/// Value must be a multiple of this number
	pub multiple_of: Option<f64>,
	/// Minimum number of items in an array
	pub min_items: Option<usize>,
	/// Maximum number of items in an array
	pub max_items: Option<usize>,
	/// Whether array items must be unique
	pub unique_items: bool,
	/// Default value (JSON string representation)
	pub default_value: Option<String>,
	/// Field-level title override
	pub title: Option<String>,
}

impl FieldAttributes {
	pub(crate) fn is_empty(&self) -> bool {
		self.description.is_none()
			&& self.example.is_none()
			&& self.format.is_none()
			&& !self.default
			&& !self.deprecated
			&& !self.read_only
			&& !self.write_only
			&& self.minimum.is_none()
			&& self.maximum.is_none()
			&& self.min_length.is_none()
			&& self.max_length.is_none()
			&& self.pattern.is_none()
			&& self.rename.is_none()
			&& !self.nullable
			&& self.exclusive_minimum.is_none()
			&& self.exclusive_maximum.is_none()
			&& self.multiple_of.is_none()
			&& self.min_items.is_none()
			&& self.max_items.is_none()
			&& !self.unique_items
			&& self.default_value.is_none()
			&& self.title.is_none()
	}
}

/// Extract container-level `#[schema(...)]` attributes from struct/enum attributes.
///
/// Parses doc comments as fallback for description. Explicit `#[schema(description = "...")]`
/// overrides doc comments.
pub(crate) fn extract_container_attributes(
	attrs: &[Attribute],
) -> Result<ContainerAttributes, syn::Error> {
	let mut container_attrs = ContainerAttributes::default();

	// First pass: collect all doc comment lines
	let mut doc_description: Option<String> = None;
	for attr in attrs {
		if attr.path().is_ident("doc")
			&& let Meta::NameValue(MetaNameValue {
				value: syn::Expr::Lit(syn::ExprLit {
					lit: Lit::Str(lit_str),
					..
				}),
				..
			}) = &attr.meta
		{
			let doc = lit_str.value().trim().to_string();
			if !doc.is_empty() {
				if let Some(ref mut desc) = doc_description {
					desc.push(' ');
					desc.push_str(&doc);
				} else {
					doc_description = Some(doc);
				}
			}
		}
	}
	// Use doc comments as fallback description (may be overridden by explicit attribute)
	container_attrs.description = doc_description;

	for attr in attrs {
		if attr.path().is_ident("doc") {
			continue;
		}

		if !attr.path().is_ident("schema") {
			continue;
		}

		if let Ok(meta_list) = attr.meta.require_list() {
			let nested_metas = meta_list.parse_args_with(
				syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
			)?;
			for nested_meta in nested_metas {
				match nested_meta {
					Meta::NameValue(nv) => {
						let Some(ident) = nv.path.get_ident() else {
							continue;
						};
						let ident_str = ident.to_string();

						match ident_str.as_str() {
							"title" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									container_attrs.title = Some(lit_str.value());
								}
							}
							"description" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									// Override doc comments
									container_attrs.description = Some(lit_str.value());
								}
							}
							"example" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									container_attrs.example = Some(lit_str.value());
								}
							}
							_ => {}
						}
					}
					Meta::Path(path) => {
						if let Some(ident) = path.get_ident() {
							match ident.to_string().as_str() {
								"deprecated" => container_attrs.deprecated = true,
								"nullable" => container_attrs.nullable = true,
								_ => {}
							}
						}
					}
					_ => {}
				}
			}
		}
	}

	Ok(container_attrs)
}

/// Extract schema attributes from field attributes.
///
/// Returns `Ok(FieldAttributes)` on success, or `Err(syn::Error)` if attribute
/// syntax is malformed.
///
/// Fixes #842: Propagate parse errors instead of silently ignoring them.
pub(crate) fn extract_field_attributes(attrs: &[Attribute]) -> Result<FieldAttributes, syn::Error> {
	let mut field_attrs = FieldAttributes::default();

	for attr in attrs {
		// Check for doc comments (/// or //!)
		if attr.path().is_ident("doc") {
			if let Meta::NameValue(MetaNameValue {
				value: syn::Expr::Lit(syn::ExprLit {
					lit: Lit::Str(lit_str),
					..
				}),
				..
			}) = &attr.meta
			{
				let doc = lit_str.value().trim().to_string();
				if !doc.is_empty() {
					// Append to description if already exists
					if let Some(ref mut desc) = field_attrs.description {
						desc.push(' ');
						desc.push_str(&doc);
					} else {
						field_attrs.description = Some(doc);
					}
				}
			}
			continue;
		}

		// Check for #[serde(...)] attributes
		// Fixes #836 (skip), #838 (default), #839 (flatten)
		if attr.path().is_ident("serde") {
			if let Ok(meta_list) = attr.meta.require_list() {
				// Fixes #842: Propagate parse errors instead of using unwrap_or_default
				let nested_metas = meta_list.parse_args_with(
					syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
				)?;
				for nested_meta in nested_metas {
					match nested_meta {
						Meta::NameValue(nv) => {
							if nv.path.is_ident("rename") {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									// Only set if not already set by #[schema(rename = "...")]
									if field_attrs.rename.is_none() {
										field_attrs.rename = Some(lit_str.value());
									}
								}
							} else if nv.path.is_ident("default") {
								// #[serde(default = "path")] - field has a default value
								field_attrs.default = true;
							}
						}
						Meta::Path(path) => {
							if path.is_ident("skip") {
								field_attrs.skip = true;
							} else if path.is_ident("skip_serializing") {
								field_attrs.skip_serializing = true;
							} else if path.is_ident("skip_deserializing") {
								field_attrs.skip_deserializing = true;
							} else if path.is_ident("flatten") {
								field_attrs.flatten = true;
							} else if path.is_ident("default") {
								// #[serde(default)] - field has a default value
								field_attrs.default = true;
							}
						}
						_ => {}
					}
				}
			}
			continue;
		}

		// Check for #[schema(...)] attributes
		if !attr.path().is_ident("schema") {
			continue;
		}

		// Parse nested meta items
		if let Ok(meta_list) = attr.meta.require_list() {
			// Fixes #842: Propagate parse errors instead of using unwrap_or_default
			let nested_metas = meta_list.parse_args_with(
				syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
			)?;
			for nested_meta in nested_metas {
				match nested_meta {
					Meta::NameValue(nv) => {
						let Some(ident) = nv.path.get_ident() else {
							// Non-identifier paths are silently skipped
							continue;
						};
						let ident = ident.to_string();

						match ident.as_str() {
							"description" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									// Override doc comments if explicit description is provided
									field_attrs.description = Some(lit_str.value());
								}
							}
							"example" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									field_attrs.example = Some(lit_str.value());
								}
							}
							"format" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									field_attrs.format = Some(lit_str.value());
								}
							}
							"minimum" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Int(lit_int),
									..
								}) = nv.value
								{
									field_attrs.minimum = Some(lit_int.base10_parse()?);
								}
							}
							"maximum" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Int(lit_int),
									..
								}) = nv.value
								{
									field_attrs.maximum = Some(lit_int.base10_parse()?);
								}
							}
							"min_length" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Int(lit_int),
									..
								}) = nv.value
								{
									field_attrs.min_length = Some(lit_int.base10_parse()?);
								}
							}
							"max_length" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Int(lit_int),
									..
								}) = nv.value
								{
									field_attrs.max_length = Some(lit_int.base10_parse()?);
								}
							}
							"pattern" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									field_attrs.pattern = Some(lit_str.value());
								}
							}
							"rename" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									// Override serde rename if explicit schema rename is provided
									field_attrs.rename = Some(lit_str.value());
								}
							}
							"exclusive_minimum" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Int(lit_int),
									..
								}) = nv.value
								{
									field_attrs.exclusive_minimum = Some(lit_int.base10_parse()?);
								}
							}
							"exclusive_maximum" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Int(lit_int),
									..
								}) = nv.value
								{
									field_attrs.exclusive_maximum = Some(lit_int.base10_parse()?);
								}
							}
							"multiple_of" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Float(lit_float),
									..
								}) = &nv.value
								{
									field_attrs.multiple_of = Some(lit_float.base10_parse()?);
								} else if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Int(lit_int),
									..
								}) = nv.value
								{
									field_attrs.multiple_of =
										Some(lit_int.base10_parse::<i64>()? as f64);
								}
							}
							"min_items" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Int(lit_int),
									..
								}) = nv.value
								{
									field_attrs.min_items = Some(lit_int.base10_parse()?);
								}
							}
							"max_items" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Int(lit_int),
									..
								}) = nv.value
								{
									field_attrs.max_items = Some(lit_int.base10_parse()?);
								}
							}
							"default_value" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									field_attrs.default_value = Some(lit_str.value());
								}
							}
							"title" => {
								if let syn::Expr::Lit(syn::ExprLit {
									lit: Lit::Str(lit_str),
									..
								}) = nv.value
								{
									field_attrs.title = Some(lit_str.value());
								}
							}
							_ => {}
						}
					}
					Meta::Path(path) => {
						if let Some(ident) = path.get_ident() {
							match ident.to_string().as_str() {
								"default" => field_attrs.default = true,
								"deprecated" => field_attrs.deprecated = true,
								"read_only" => field_attrs.read_only = true,
								"write_only" => field_attrs.write_only = true,
								"nullable" => field_attrs.nullable = true,
								"unique_items" => field_attrs.unique_items = true,
								_ => {}
							}
						}
					}
					_ => {}
				}
			}
		}
	}

	// Fixes #841: Validate that constraints are not contradictory
	if let (Some(min), Some(max)) = (field_attrs.minimum, field_attrs.maximum)
		&& min > max
	{
		return Err(syn::Error::new(
			Span::call_site(),
			format!(
				"contradictory constraints: minimum ({}) > maximum ({})",
				min, max
			),
		));
	}
	if let (Some(min), Some(max)) = (field_attrs.min_length, field_attrs.max_length)
		&& min > max
	{
		return Err(syn::Error::new(
			Span::call_site(),
			format!(
				"contradictory constraints: min_length ({}) > max_length ({})",
				min, max
			),
		));
	}
	if let (Some(min), Some(max)) = (field_attrs.exclusive_minimum, field_attrs.exclusive_maximum)
		&& min >= max
	{
		return Err(syn::Error::new(
			Span::call_site(),
			format!(
				"contradictory constraints: exclusive_minimum ({}) >= exclusive_maximum ({})",
				min, max
			),
		));
	}
	if let (Some(min), Some(max)) = (field_attrs.min_items, field_attrs.max_items)
		&& min > max
	{
		return Err(syn::Error::new(
			Span::call_site(),
			format!(
				"contradictory constraints: min_items ({}) > max_items ({})",
				min, max
			),
		));
	}

	Ok(field_attrs)
}
