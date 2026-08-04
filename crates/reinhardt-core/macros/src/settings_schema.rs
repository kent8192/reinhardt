//! Shared analysis helpers for settings schema macros.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Fields, ItemStruct, LitStr, Result};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SettingAttr {
	Required,
	Optional,
	Default(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShapeHint {
	Node,
	Leaf,
}

#[derive(Debug)]
struct ParsedSettingAttr {
	requirement: Option<SettingAttr>,
	shape_hint: Option<ShapeHint>,
	secret: bool,
}

#[derive(Debug)]
pub(crate) struct ParsedField {
	pub ident: syn::Ident,
	pub rust_name: String,
	pub key: String,
	pub ty: syn::Type,
	pub vis: syn::Visibility,
	pub setting_attr: Option<SettingAttr>,
	#[cfg(test)]
	pub shape_hint: Option<ShapeHint>,
	pub has_serde_default: bool,
	pub cleaned_attrs: Vec<syn::Attribute>,
	pub cfg_attrs: Vec<syn::Attribute>,
	pub shape: TypeShape,
}

#[derive(Debug)]
pub(crate) enum TypeShape {
	Leaf {
		ty: syn::Type,
		secret: bool,
		secret_ref: bool,
	},
	Node {
		ty: syn::Type,
	},
	Optional {
		original: syn::Type,
		inner: Box<TypeShape>,
	},
	Sequence {
		original: syn::Type,
		inner: Box<TypeShape>,
	},
	Map {
		original: syn::Type,
		inner: Box<TypeShape>,
	},
	Transparent {
		inner: Box<TypeShape>,
	},
}

const RUST_KEYWORDS: &[&str] = &[
	"as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
	"false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
	"ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
	"unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "macro",
	"override", "priv", "try", "typeof", "unsized", "virtual", "yield", "union",
];

pub(crate) fn infer_type_key(type_name: &str) -> std::result::Result<String, String> {
	let prefix = type_name.strip_suffix("Settings").ok_or_else(|| {
		format!(
			"Type `{}` does not end with `Settings`. Use explicit syntax: `field_name: {}`",
			type_name, type_name
		)
	})?;

	if prefix.is_empty() {
		return Err(
			"Type `Settings` has an empty prefix after stripping `Settings` suffix.".to_string(),
		);
	}

	let field_name = camel_to_snake(prefix);

	if RUST_KEYWORDS.contains(&field_name.as_str()) {
		return Err(format!(
			"Type `{}` infers field name `{}`, which is a Rust keyword. Use explicit syntax: `{}_field: {}`",
			type_name, field_name, field_name, type_name
		));
	}

	Ok(field_name)
}

pub(crate) fn camel_to_snake(s: &str) -> String {
	let mut result = String::with_capacity(s.len() + 4);
	let chars: Vec<char> = s.chars().collect();

	for (i, &ch) in chars.iter().enumerate() {
		if ch.is_uppercase() {
			if i > 0 {
				let prev = chars[i - 1];
				let needs_separator = prev.is_lowercase()
					|| prev.is_ascii_digit()
					|| (prev.is_uppercase()
						&& chars.get(i + 1).is_some_and(|next| next.is_lowercase()));
				if needs_separator {
					result.push('_');
				}
			}
			result.push(ch.to_lowercase().next().unwrap());
		} else {
			result.push(ch);
		}
	}

	result
}

pub(crate) fn parse_fields(input: &ItemStruct) -> Result<Vec<ParsedField>> {
	match &input.fields {
		Fields::Unnamed(unnamed) => {
			return Err(syn::Error::new(
				unnamed.paren_token.span.join(),
				"tuple structs are not supported for `#[settings(fragment = true)]`. \
				 Use a named-field struct instead.",
			));
		}
		Fields::Unit => {
			return Err(syn::Error::new(
				input.ident.span(),
				"unit structs are not supported for `#[settings(fragment = true)]`. \
				 Use a named-field struct instead.",
			));
		}
		Fields::Named(_) => {}
	}

	let Fields::Named(named) = &input.fields else {
		unreachable!("settings schema fields were validated as named");
	};

	named
		.named
		.iter()
		.map(|field| {
			let ident = field
				.ident
				.clone()
				.expect("named settings fields must have identifiers");
			let rust_name = ident.to_string();
			let setting_attr = parse_setting_attr(field)?;
			Ok(ParsedField {
				ident,
				key: serde_key(field)?.unwrap_or_else(|| rust_name.clone()),
				rust_name,
				ty: field.ty.clone(),
				vis: field.vis.clone(),
				setting_attr: setting_attr.requirement,
				#[cfg(test)]
				shape_hint: setting_attr.shape_hint,
				has_serde_default: has_serde_default(field),
				cleaned_attrs: strip_setting_attrs(&field.attrs),
				cfg_attrs: cfg_attrs(&field.attrs),
				shape: analyze_type(&field.ty, setting_attr.shape_hint, setting_attr.secret),
			})
		})
		.collect()
}

pub(crate) fn schema_type_name(struct_name: &syn::Ident) -> syn::Ident {
	format_ident!("{}Schema", struct_name)
}

pub(crate) fn value_schema_tokens(shape: &TypeShape, conf_crate: &TokenStream) -> TokenStream {
	match shape {
		TypeShape::Leaf { ty, secret, .. } => {
			quote! {
				#conf_crate::settings::schema::SettingsValueSchema::Leaf {
					type_name: ::std::any::type_name::<#ty>(),
					secret: #secret,
				}
			}
		}
		TypeShape::Node { ty } => {
			quote! {
				#conf_crate::settings::schema::SettingsValueSchema::Node {
					type_name: ::std::any::type_name::<#ty>(),
					node: |_path| <#ty as #conf_crate::settings::schema::SettingsNode>::node_schema(),
				}
			}
		}
		TypeShape::Optional { inner, .. } => {
			let inner_tokens = value_schema_tokens(inner, conf_crate);
			quote! {
				#conf_crate::settings::schema::SettingsValueSchema::Optional {
					inner: ::std::boxed::Box::new(#inner_tokens),
				}
			}
		}
		TypeShape::Sequence { inner, .. } => {
			let inner_tokens = value_schema_tokens(inner, conf_crate);
			quote! {
				#conf_crate::settings::schema::SettingsValueSchema::Sequence {
					inner: ::std::boxed::Box::new(#inner_tokens),
				}
			}
		}
		TypeShape::Map { inner, .. } => {
			let inner_tokens = value_schema_tokens(inner, conf_crate);
			quote! {
				#conf_crate::settings::schema::SettingsValueSchema::Map {
					inner: ::std::boxed::Box::new(#inner_tokens),
				}
			}
		}
		TypeShape::Transparent { inner } => value_schema_tokens(inner, conf_crate),
	}
}

pub(crate) fn schema_struct_fields(
	fields: &[ParsedField],
	conf_crate: &TokenStream,
) -> Vec<TokenStream> {
	fields
		.iter()
		.map(|field| {
			let ident = &field.ident;
			let vis = &field.vis;
			let ty = schema_ref_type(&field.shape, conf_crate);
			let cfg_attrs = &field.cfg_attrs;
			quote! {
				#[doc = "Typed schema reference for this settings field."]
				#(#cfg_attrs)*
				#vis #ident: #ty
			}
		})
		.collect()
}

pub(crate) fn schema_struct_inits(
	fields: &[ParsedField],
	conf_crate: &TokenStream,
) -> Vec<TokenStream> {
	fields
		.iter()
		.map(|field| {
			let ident = &field.ident;
			let key = &field.key;
			let init = schema_ref_init(&field.shape, quote! { path.with_key(#key) }, conf_crate);
			let cfg_attrs = &field.cfg_attrs;
			quote! {
				#(#cfg_attrs)*
				#ident: #init
			}
		})
		.collect()
}

fn parse_setting_attr(field: &syn::Field) -> Result<ParsedSettingAttr> {
	let mut has_required = false;
	let mut has_optional = false;
	let mut has_default = false;
	let mut has_node = false;
	let mut has_leaf = false;
	let mut has_secret = false;
	let mut default_expr: Option<String> = None;

	for attr in &field.attrs {
		if !attr.path().is_ident("setting") {
			continue;
		}

		attr.parse_nested_meta(|meta| {
			if meta.path.is_ident("required") {
				has_required = true;
				Ok(())
			} else if meta.path.is_ident("optional") {
				has_optional = true;
				Ok(())
			} else if meta.path.is_ident("node") {
				has_node = true;
				Ok(())
			} else if meta.path.is_ident("leaf") {
				has_leaf = true;
				Ok(())
			} else if meta.path.is_ident("secret") {
				has_secret = true;
				Ok(())
			} else if meta.path.is_ident("default") {
				has_default = true;
				let lit: LitStr = meta.value()?.parse()?;
				default_expr = Some(lit.value());
				Ok(())
			} else {
				Err(meta.error(
					"unknown setting attribute, expected one of: `required`, `optional`, `default`, `node`, `leaf`, `secret`",
				))
			}
		})?;
	}

	if has_required && has_default {
		return Err(syn::Error::new(
			setting_attr_span(field),
			"`required` and `default` are mutually exclusive in `#[setting(...)]`",
		));
	}

	if has_required && has_optional {
		return Err(syn::Error::new(
			setting_attr_span(field),
			"`required` and `optional` are mutually exclusive in `#[setting(...)]`",
		));
	}

	if has_node && has_leaf {
		return Err(syn::Error::new(
			setting_attr_span(field),
			"`node` and `leaf` are mutually exclusive in `#[setting(...)]`",
		));
	}

	if has_node && has_secret {
		return Err(syn::Error::new(
			setting_attr_span(field),
			"`node` and `secret` are mutually exclusive in `#[setting(...)]`",
		));
	}

	let requirement = if has_required {
		Some(SettingAttr::Required)
	} else if has_default {
		Some(SettingAttr::Default(default_expr.unwrap()))
	} else if has_optional {
		Some(SettingAttr::Optional)
	} else {
		None
	};

	let shape_hint = if has_node {
		Some(ShapeHint::Node)
	} else if has_leaf {
		Some(ShapeHint::Leaf)
	} else {
		None
	};

	Ok(ParsedSettingAttr {
		requirement,
		shape_hint,
		secret: has_secret,
	})
}

fn setting_attr_span(field: &syn::Field) -> proc_macro2::Span {
	field
		.attrs
		.iter()
		.find(|a| a.path().is_ident("setting"))
		.map(|a| a.path().span())
		.unwrap_or_else(proc_macro2::Span::call_site)
}

fn has_serde_default(field: &syn::Field) -> bool {
	field.attrs.iter().any(|attr| {
		if !attr.path().is_ident("serde") {
			return false;
		}
		let mut found = false;
		let _ = attr.parse_nested_meta(|meta| {
			if meta.path.is_ident("default") {
				found = true;
				if meta.input.peek(syn::Token![=]) {
					consume_serde_meta(meta)?;
				}
			} else {
				consume_serde_meta(meta)?;
			}
			Ok(())
		});
		found
	})
}

fn strip_setting_attrs(attrs: &[syn::Attribute]) -> Vec<syn::Attribute> {
	attrs
		.iter()
		.filter(|attr| !attr.path().is_ident("setting"))
		.cloned()
		.collect()
}

fn cfg_attrs(attrs: &[syn::Attribute]) -> Vec<syn::Attribute> {
	attrs
		.iter()
		.filter(|attr| attr.path().is_ident("cfg"))
		.cloned()
		.collect()
}

fn serde_key(field: &syn::Field) -> Result<Option<String>> {
	let mut key = None;

	for attr in &field.attrs {
		if !attr.path().is_ident("serde") {
			continue;
		}

		attr.parse_nested_meta(|meta| {
			if meta.path.is_ident("flatten") {
				return Err(meta.error("`serde(flatten)` is not supported inside settings nodes"));
			}

			if meta.path.is_ident("rename") {
				if meta.input.peek(syn::Token![=]) {
					let value = meta.value()?;
					let lit: LitStr = value.parse()?;
					key = Some(lit.value());
					return Ok(());
				}

				meta.parse_nested_meta(|nested| {
					if nested.path.is_ident("deserialize") {
						let lit: LitStr = nested.value()?.parse()?;
						key = Some(lit.value());
					} else {
						consume_serde_meta(nested)?;
					}
					Ok(())
				})?;
				return Ok(());
			}

			consume_serde_meta(meta)?;
			Ok(())
		})?;
	}

	Ok(key)
}

fn consume_serde_meta(meta: syn::meta::ParseNestedMeta<'_>) -> Result<()> {
	if meta.input.peek(syn::Token![=]) {
		let value = meta.value()?;
		let _: syn::Expr = value.parse()?;
	} else if meta.input.peek(syn::token::Paren) {
		meta.parse_nested_meta(consume_serde_meta)?;
	}
	Ok(())
}

fn analyze_type(ty: &syn::Type, shape_hint: Option<ShapeHint>, secret: bool) -> TypeShape {
	let Some((last_segment, args)) = type_last_segment(ty) else {
		return TypeShape::Leaf {
			ty: ty.clone(),
			secret,
			secret_ref: false,
		};
	};

	let segment_name = last_segment.ident.to_string();

	match segment_name.as_str() {
		"Option" => {
			if let Some(inner_ty) = single_type_arg(args) {
				return TypeShape::Optional {
					original: ty.clone(),
					inner: Box::new(analyze_type(inner_ty, shape_hint, secret)),
				};
			}
		}
		"Vec" => {
			if let Some(inner_ty) = single_type_arg(args) {
				return TypeShape::Sequence {
					original: ty.clone(),
					inner: Box::new(analyze_type(inner_ty, shape_hint, secret)),
				};
			}
		}
		"HashMap" | "BTreeMap" | "IndexMap" => {
			if let Some(inner_ty) = second_type_arg(args) {
				return TypeShape::Map {
					original: ty.clone(),
					inner: Box::new(analyze_type(inner_ty, shape_hint, secret)),
				};
			}
		}
		"Box" => {
			if let Some(inner_ty) = single_type_arg(args) {
				return TypeShape::Transparent {
					inner: Box::new(analyze_type(inner_ty, shape_hint, secret)),
				};
			}
		}
		_ => {}
	}

	if shape_hint == Some(ShapeHint::Node)
		|| (!secret && shape_hint.is_none() && segment_name.ends_with("Config"))
	{
		TypeShape::Node { ty: ty.clone() }
	} else {
		let inherently_secret = segment_name == "SecretString" || segment_name == "SecretValue";
		TypeShape::Leaf {
			ty: ty.clone(),
			secret: secret || inherently_secret,
			secret_ref: inherently_secret,
		}
	}
}

fn type_last_segment(ty: &syn::Type) -> Option<(&syn::PathSegment, &syn::PathArguments)> {
	match ty {
		syn::Type::Path(type_path) if type_path.qself.is_none() => type_path
			.path
			.segments
			.last()
			.map(|segment| (segment, &segment.arguments)),
		_ => None,
	}
}

fn single_type_arg(args: &syn::PathArguments) -> Option<&syn::Type> {
	let syn::PathArguments::AngleBracketed(args) = args else {
		return None;
	};
	let mut types = args.args.iter().filter_map(|arg| match arg {
		syn::GenericArgument::Type(ty) => Some(ty),
		_ => None,
	});
	let first = types.next()?;
	if types.next().is_some() {
		None
	} else {
		Some(first)
	}
}

fn second_type_arg(args: &syn::PathArguments) -> Option<&syn::Type> {
	let syn::PathArguments::AngleBracketed(args) = args else {
		return None;
	};
	args.args
		.iter()
		.filter_map(|arg| match arg {
			syn::GenericArgument::Type(ty) => Some(ty),
			_ => None,
		})
		.nth(1)
}

fn schema_ref_type(shape: &TypeShape, conf_crate: &TokenStream) -> TokenStream {
	match shape {
		TypeShape::Leaf { ty, secret_ref, .. } => {
			if *secret_ref {
				quote! { #conf_crate::settings::schema::SecretFieldRef<Root, #ty> }
			} else {
				quote! { #conf_crate::settings::schema::FieldRef<Root, #ty> }
			}
		}
		TypeShape::Node { ty } => {
			quote! { <#ty as #conf_crate::settings::schema::SettingsNode>::Schema<Root> }
		}
		TypeShape::Optional { original, inner } => {
			let inner_ref = schema_ref_type(inner, conf_crate);
			quote! { #conf_crate::settings::schema::OptionalRef<Root, #original, #inner_ref> }
		}
		TypeShape::Sequence { original, inner } => {
			let inner_ref = schema_ref_type(inner, conf_crate);
			quote! { #conf_crate::settings::schema::SequenceRef<Root, #original, #inner_ref> }
		}
		TypeShape::Map { original, inner } => {
			let inner_ref = schema_ref_type(inner, conf_crate);
			quote! { #conf_crate::settings::schema::MapRef<Root, #original, #inner_ref> }
		}
		TypeShape::Transparent { inner } => schema_ref_type(inner, conf_crate),
	}
}

fn schema_ref_init(
	shape: &TypeShape,
	path_tokens: TokenStream,
	conf_crate: &TokenStream,
) -> TokenStream {
	match shape {
		TypeShape::Leaf { secret_ref, .. } => {
			if *secret_ref {
				quote! { #conf_crate::settings::schema::SecretFieldRef::new(#path_tokens) }
			} else {
				quote! { #conf_crate::settings::schema::FieldRef::new(#path_tokens) }
			}
		}
		TypeShape::Node { ty } => {
			quote! { <#ty as #conf_crate::settings::schema::SettingsNode>::schema_at(#path_tokens) }
		}
		TypeShape::Optional { inner, .. } => {
			let inner_init = schema_builder_init(inner, conf_crate);
			quote! { #conf_crate::settings::schema::OptionalRef::new(#path_tokens, #inner_init) }
		}
		TypeShape::Sequence { inner, .. } => {
			let inner_init = schema_builder_init(inner, conf_crate);
			quote! { #conf_crate::settings::schema::SequenceRef::new(#path_tokens, #inner_init) }
		}
		TypeShape::Map { inner, .. } => {
			let inner_init = schema_builder_init(inner, conf_crate);
			quote! { #conf_crate::settings::schema::MapRef::new(#path_tokens, #inner_init) }
		}
		TypeShape::Transparent { inner } => schema_ref_init(inner, path_tokens, conf_crate),
	}
}

fn schema_builder_init(shape: &TypeShape, conf_crate: &TokenStream) -> TokenStream {
	let init = schema_ref_init(shape, quote! { path }, conf_crate);
	quote! { |path| #init }
}

#[cfg(test)]
mod tests {
	use super::*;

	fn parse_single_field(input: ItemStruct) -> ParsedField {
		parse_fields(&input)
			.expect("settings fields should parse")
			.into_iter()
			.next()
			.expect("test struct should have one field")
	}

	#[test]
	fn parse_fields_accepts_serde_default_value() {
		let input: ItemStruct = syn::parse_quote! {
			struct TestSettings {
				#[serde(default = "default_value")]
				value: String,
			}
		};

		let field = parse_single_field(input);

		assert_eq!(field.key, "value");
		assert!(field.has_serde_default);
	}

	#[test]
	fn parse_fields_uses_deserialize_rename_key() {
		let input: ItemStruct = syn::parse_quote! {
			struct TestSettings {
				#[serde(rename(deserialize = "wire-key", serialize = "wireKey"))]
				value: String,
			}
		};

		let field = parse_single_field(input);

		assert_eq!(field.key, "wire-key");
	}

	#[test]
	fn parse_fields_detects_default_after_nested_serde_meta() {
		let input: ItemStruct = syn::parse_quote! {
			struct TestSettings {
				#[serde(rename(deserialize = "wire-key"), default = "default_value")]
				value: String,
			}
		};

		let field = parse_single_field(input);

		assert_eq!(field.key, "wire-key");
		assert!(field.has_serde_default);
	}

	#[test]
	fn parse_fields_rejects_serde_flatten() {
		let input: ItemStruct = syn::parse_quote! {
			struct TestSettings {
				#[serde(flatten)]
				value: NestedSettings,
			}
		};

		let err = parse_fields(&input).expect_err("serde flatten should be rejected");

		assert_eq!(
			err.to_string(),
			"`serde(flatten)` is not supported inside settings nodes"
		);
	}

	#[test]
	fn parse_fields_accepts_optional_node_hint() {
		let input: ItemStruct = syn::parse_quote! {
			struct TestSettings {
				#[setting(optional, node)]
				value: Option<NestedSettings>,
			}
		};

		let field = parse_single_field(input);

		assert_eq!(field.setting_attr, Some(SettingAttr::Optional));
		assert_eq!(field.shape_hint, Some(ShapeHint::Node));
		assert!(matches!(
			field.shape,
			TypeShape::Optional { ref inner, .. }
				if matches!(inner.as_ref(), TypeShape::Node { .. })
		));
	}

	#[test]
	fn parse_fields_marks_plain_string_as_secret_with_explicit_hint() {
		let input: ItemStruct = syn::parse_quote! {
			struct TestSettings {
				#[setting(secret)]
				value: String,
			}
		};

		let field = parse_single_field(input);

		assert!(matches!(
			field.shape,
			TypeShape::Leaf {
				secret: true,
				secret_ref: false,
				..
			}
		));
		assert_eq!(
			schema_ref_type(&field.shape, &quote! { reinhardt_conf }).to_string(),
			"reinhardt_conf :: settings :: schema :: FieldRef < Root , String >"
		);
	}

	#[test]
	fn parse_fields_treats_inferred_node_as_leaf_with_secret_hint() {
		let input: ItemStruct = syn::parse_quote! {
			struct TestSettings {
				#[setting(secret)]
				value: Option<Vec<NestedConfig>>,
			}
		};

		let field = parse_single_field(input);

		assert!(matches!(
			field.shape,
			TypeShape::Optional { ref inner, .. }
				if matches!(
					inner.as_ref(),
					TypeShape::Sequence { inner, .. }
						if matches!(
							inner.as_ref(),
							TypeShape::Leaf {
								secret: true,
								secret_ref: false,
								..
							}
						)
				)
		));
	}

	#[test]
	fn parse_fields_rejects_node_and_leaf_hints() {
		let input: ItemStruct = syn::parse_quote! {
			struct TestSettings {
				#[setting(node, leaf)]
				value: NestedSettings,
			}
		};

		let err = parse_fields(&input).expect_err("node and leaf should conflict");

		assert_eq!(
			err.to_string(),
			"`node` and `leaf` are mutually exclusive in `#[setting(...)]`"
		);
	}

	#[test]
	fn analyze_type_treats_config_suffix_as_node() {
		let ty: syn::Type = syn::parse_quote! { DatabaseConfig };

		let shape = analyze_type(&ty, None, false);

		assert!(matches!(shape, TypeShape::Node { .. }));
	}

	#[test]
	fn analyze_type_treats_settings_suffix_as_leaf_without_hint() {
		let ty: syn::Type = syn::parse_quote! { DatabaseSettings };

		let shape = analyze_type(&ty, None, false);

		assert!(matches!(shape, TypeShape::Leaf { .. }));
	}
}
