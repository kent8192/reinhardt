//! Serde attribute parsing for enum tagging strategies
//!
//! This module parses serde's enum representation attributes to determine
//! the appropriate OpenAPI schema tagging strategy.

use syn::{Attribute, Lit, Meta, MetaNameValue};

/// Parsed serde enum attributes
#[derive(Debug, Default, Clone)]
pub(crate) struct SerdeEnumAttrs {
	/// Tag field name from `#[serde(tag = "...")]`
	pub tag: Option<String>,
	/// Content field name from `#[serde(content = "...")]`
	pub content: Option<String>,
	/// Whether `#[serde(untagged)]` is present
	pub untagged: bool,
	/// Container-level rename (from `#[serde(rename = "...")]`)
	pub rename: Option<String>,
	/// Rename all strategy (from `#[serde(rename_all = "...")]`)
	pub rename_all: Option<String>,
}

impl SerdeEnumAttrs {
	/// Determine the tagging strategy based on parsed attributes
	///
	/// - `untagged` = true -> Untagged
	/// - `tag` + `content` -> Adjacent
	/// - `tag` only -> Internal
	/// - None -> External (default)
	pub(crate) fn tagging_strategy(&self) -> TaggingStrategy {
		if self.untagged {
			TaggingStrategy::Untagged
		} else if let Some(ref tag) = self.tag {
			if let Some(ref content) = self.content {
				TaggingStrategy::Adjacent {
					tag: tag.clone(),
					content: content.clone(),
				}
			} else {
				TaggingStrategy::Internal { tag: tag.clone() }
			}
		} else {
			TaggingStrategy::External
		}
	}
}

/// Enum tagging strategy derived from serde attributes
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaggingStrategy {
	/// Externally tagged (default): `{"Variant": {...}}`
	External,
	/// Internally tagged: `{"type": "Variant", ...fields}`
	Internal { tag: String },
	/// Adjacently tagged: `{"tag": "Variant", "content": {...}}`
	Adjacent { tag: String, content: String },
	/// Untagged: no discriminator
	Untagged,
}

/// Extract serde enum attributes from a list of attributes
///
/// Parses `#[serde(...)]` attributes to extract enum tagging configuration:
/// - `tag = "..."` - Internal or adjacent tagging
/// - `content = "..."` - Adjacent tagging content field
/// - `untagged` - Untagged representation
/// - `rename = "..."` - Container rename
/// - `rename_all = "..."` - Variant name transformation
pub(crate) fn extract_serde_enum_attrs(attrs: &[Attribute]) -> SerdeEnumAttrs {
	let mut result = SerdeEnumAttrs::default();

	for attr in attrs {
		if !attr.path().is_ident("serde") {
			continue;
		}

		let Ok(meta_list) = attr.meta.require_list() else {
			continue;
		};

		let Ok(nested_metas) = meta_list
			.parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
		else {
			continue;
		};

		for nested_meta in nested_metas {
			match nested_meta {
				Meta::Path(path) => {
					if path.is_ident("untagged") {
						result.untagged = true;
					}
				}
				Meta::NameValue(MetaNameValue {
					path,
					value:
						syn::Expr::Lit(syn::ExprLit {
							lit: Lit::Str(lit_str),
							..
						}),
					..
				}) => {
					let value = lit_str.value();
					if path.is_ident("tag") {
						result.tag = Some(value);
					} else if path.is_ident("content") {
						result.content = Some(value);
					} else if path.is_ident("rename") {
						result.rename = Some(value);
					} else if path.is_ident("rename_all") {
						result.rename_all = Some(value);
					}
				}
				_ => {}
			}
		}
	}

	result
}

/// Extract container-level `#[serde(rename_all = "...")]` from attributes.
///
/// This is used for struct field name transformations.
/// Fixes #835
pub(crate) fn extract_serde_rename_all(attrs: &[Attribute]) -> Option<String> {
	for attr in attrs {
		if !attr.path().is_ident("serde") {
			continue;
		}

		let Ok(meta_list) = attr.meta.require_list() else {
			continue;
		};

		let Ok(nested_metas) = meta_list
			.parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
		else {
			continue;
		};

		for nested_meta in nested_metas {
			if let Meta::NameValue(MetaNameValue {
				path,
				value: syn::Expr::Lit(syn::ExprLit {
					lit: Lit::Str(lit_str),
					..
				}),
				..
			}) = nested_meta
				&& path.is_ident("rename_all")
			{
				return Some(lit_str.value());
			}
		}
	}

	None
}

/// Parsed serde variant attributes
#[derive(Debug, Default, Clone)]
pub(crate) struct SerdeVariantAttrs {
	/// Variant rename from `#[serde(rename = "...")]`
	pub rename: Option<String>,
	/// Whether `#[serde(skip)]` is present
	pub skip: bool,
	/// Alias names from `#[serde(alias = "...")]`
	pub aliases: Vec<String>,
}

/// Extract serde variant attributes from a list of attributes
pub(crate) fn extract_serde_variant_attrs(attrs: &[Attribute]) -> SerdeVariantAttrs {
	let mut result = SerdeVariantAttrs::default();

	for attr in attrs {
		if !attr.path().is_ident("serde") {
			continue;
		}

		let Ok(meta_list) = attr.meta.require_list() else {
			continue;
		};

		let Ok(nested_metas) = meta_list
			.parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
		else {
			continue;
		};

		for nested_meta in nested_metas {
			match nested_meta {
				Meta::Path(path) => {
					if path.is_ident("skip") {
						result.skip = true;
					}
				}
				Meta::NameValue(MetaNameValue {
					path,
					value:
						syn::Expr::Lit(syn::ExprLit {
							lit: Lit::Str(lit_str),
							..
						}),
					..
				}) => {
					let value = lit_str.value();
					if path.is_ident("rename") {
						result.rename = Some(value);
					} else if path.is_ident("alias") {
						result.aliases.push(value);
					}
				}
				_ => {}
			}
		}
	}

	result
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_is_external() {
		let attrs = SerdeEnumAttrs::default();
		assert_eq!(attrs.tagging_strategy(), TaggingStrategy::External);
	}

	#[test]
	fn test_untagged_strategy() {
		let attrs = SerdeEnumAttrs {
			untagged: true,
			..Default::default()
		};
		assert_eq!(attrs.tagging_strategy(), TaggingStrategy::Untagged);
	}

	#[test]
	fn test_internal_tagging() {
		let attrs = SerdeEnumAttrs {
			tag: Some("type".to_string()),
			..Default::default()
		};
		assert_eq!(
			attrs.tagging_strategy(),
			TaggingStrategy::Internal {
				tag: "type".to_string()
			}
		);
	}

	#[test]
	fn test_adjacent_tagging() {
		let attrs = SerdeEnumAttrs {
			tag: Some("t".to_string()),
			content: Some("c".to_string()),
			..Default::default()
		};
		assert_eq!(
			attrs.tagging_strategy(),
			TaggingStrategy::Adjacent {
				tag: "t".to_string(),
				content: "c".to_string()
			}
		);
	}
}
