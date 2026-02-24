//! Schema attribute parsing and field metadata extraction

use proc_macro2::Span;
use syn::{Attribute, Lit, Meta, MetaNameValue};

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
	}
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

	Ok(field_attrs)
}
