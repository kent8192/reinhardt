//! Shared analysis helpers for settings schema macros.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::{Fields, ItemStruct, LitStr, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug)]
struct ParsedSettingAttr {
	requirement: Option<SettingAttr>,
	shape_hint: Option<ShapeHint>,
	secret: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ParsedField {
	pub ident: syn::Ident,
	pub rust_name: String,
	pub key: String,
	pub deserialize_keys: Vec<String>,
	pub ty: syn::Type,
	pub vis: syn::Visibility,
	pub setting_attr: Option<SettingAttr>,
	#[cfg(test)]
	pub shape_hint: Option<ShapeHint>,
	pub has_serde_default: bool,
	pub has_whole_field_deserializer: bool,
	pub has_serde_rename: bool,
	pub skip_deserializing: bool,
	pub cleaned_attrs: Vec<syn::Attribute>,
	pub cfg_attrs: Vec<syn::Attribute>,
	pub shape: TypeShape,
}

#[derive(Clone, Debug)]
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
		key: Box<syn::Type>,
		value: Box<TypeShape>,
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
	validate_struct_serde_attributes(input)?;
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
	let rename_rules = serde_rename_rules(&input.attrs)?;

	named
		.named
		.iter()
		.map(|field| {
			let ident = field
				.ident
				.clone()
				.expect("named settings fields must have identifiers");
			let rust_name = ident.unraw().to_string();
			let setting_attr = parse_setting_attr(field)?;
			let serde_keys = serde_field_keys(field, &rename_rules)?;
			let shape = analyze_type(&field.ty, setting_attr.shape_hint, setting_attr.secret)?;
			if setting_attr.secret && contains_node(&shape) {
				return Err(syn::Error::new(
					setting_attr_span(field),
					"`secret` cannot be applied to a settings node; classify each leaf or add `leaf` when the value is intentionally atomic",
				));
			}
			Ok(ParsedField {
				ident,
				key: serde_keys.key,
				deserialize_keys: serde_keys.deserialize_keys,
				rust_name,
				ty: field.ty.clone(),
				vis: field.vis.clone(),
				setting_attr: setting_attr.requirement,
				#[cfg(test)]
				shape_hint: setting_attr.shape_hint,
				has_serde_default: has_serde_default(field),
				has_whole_field_deserializer: has_whole_field_deserializer(field),
				has_serde_rename: has_serde_rename(field),
				skip_deserializing: serde_skip_deserializing(field),
				cleaned_attrs: strip_setting_attrs(&field.attrs),
				cfg_attrs: cfg_attrs(&field.attrs),
				shape,
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
					type_name: stringify!(#ty),
					secret: #secret,
					check: #conf_crate::settings::schema::settings_value_check::<#ty>,
				}
			}
		}
		TypeShape::Node { ty } => {
			quote! {
				#conf_crate::settings::schema::SettingsValueSchema::Node {
					type_name: stringify!(#ty),
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
		TypeShape::Map { key, value, .. } => {
			let value_tokens = value_schema_tokens(value, conf_crate);
			quote! {
				#conf_crate::settings::schema::SettingsValueSchema::Map {
					key_type: stringify!(#key),
					key_check: #conf_crate::settings::schema::settings_map_key_check::<#key>,
					value: ::std::boxed::Box::new(#value_tokens),
				}
			}
		}
		TypeShape::Transparent { inner } => value_schema_tokens(inner, conf_crate),
	}
}

pub(crate) fn whole_field_check_tokens(
	struct_name: &syn::Ident,
	field: &ParsedField,
	index: usize,
	conf_crate: &TokenStream,
) -> Option<(TokenStream, TokenStream)> {
	if !field.has_whole_field_deserializer {
		return None;
	}
	let check_name = format_ident!(
		"__settings_check_{}_{}",
		camel_to_snake(&struct_name.to_string()),
		index
	);
	let wrapper_name = format_ident!("SettingsCheck{}{}", struct_name, index);
	let attrs = &field.cleaned_attrs;
	let cfg_attrs = &field.cfg_attrs;
	let field_name = &field.ident;
	let field_ty = &field.ty;
	let key = &field.key;
	let rename = (!field.has_serde_rename).then(|| quote! { #[serde(rename = #key)] });
	Some((
		quote! {
			#(#cfg_attrs)*
			fn #check_name(
				value: &#conf_crate::serde_json::Value,
				typed_coercion: bool,
			) -> bool {
				#[derive(::serde::Deserialize)]
				struct #wrapper_name {
					#(#attrs)*
					#rename
					#field_name: #field_ty,
				}
				let mut map = #conf_crate::serde_json::Map::new();
				map.insert(#key.to_string(), value.clone());
				let value = #conf_crate::serde_json::Value::Object(map);
				if typed_coercion {
					match <#wrapper_name as ::serde::Deserialize>::deserialize(
						#conf_crate::settings::typed_deserializer::TypedSettingsDeserializer::new(&value),
					) {
						Ok(parsed) => {
							let _field_value = parsed.#field_name;
							true
						}
						Err(_) => false,
					}
				} else {
					match #conf_crate::serde_json::from_value::<#wrapper_name>(value) {
						Ok(parsed) => {
							let _field_value = parsed.#field_name;
							true
						}
						Err(_) => false,
					}
				}
			}
		},
		quote! { Some(#check_name) },
	))
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

fn serde_skip_deserializing(field: &syn::Field) -> bool {
	field.attrs.iter().any(|attr| {
		if !attr.path().is_ident("serde") {
			return false;
		}
		let mut found = false;
		let _ = attr.parse_nested_meta(|meta| {
			if meta.path.is_ident("skip") || meta.path.is_ident("skip_deserializing") {
				found = true;
			}
			consume_serde_meta(meta)
		});
		found
	})
}

fn has_whole_field_deserializer(field: &syn::Field) -> bool {
	field.attrs.iter().any(|attr| {
		if !attr.path().is_ident("serde") {
			return false;
		}
		let mut found = false;
		let _ = attr.parse_nested_meta(|meta| {
			if meta.path.is_ident("deserialize_with") || meta.path.is_ident("with") {
				found = true;
			}
			consume_serde_meta(meta)
		});
		found
	})
}

fn has_serde_rename(field: &syn::Field) -> bool {
	field.attrs.iter().any(|attr| {
		if !attr.path().is_ident("serde") {
			return false;
		}
		let mut found = false;
		let _ = attr.parse_nested_meta(|meta| {
			if meta.path.is_ident("rename") {
				found = true;
			}
			consume_serde_meta(meta)
		});
		found
	})
}

pub(crate) fn validate_struct_serde_attributes(input: &ItemStruct) -> Result<()> {
	for attr in &input.attrs {
		if !attr.path().is_ident("serde") {
			continue;
		}
		attr.parse_nested_meta(|meta| {
			if [
				"deny_unknown_fields",
				"try_from",
				"from",
				"into",
				"transparent",
			]
			.iter()
			.any(|name| meta.path.is_ident(name))
			{
				return Err(meta.error(
					"this struct-level Serde behavior is not supported by runtime settings verification",
				));
			}
			consume_serde_meta(meta)
		})?;
	}
	Ok(())
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

#[derive(Default)]
struct SerdeRenameRules {
	deserialize: Option<String>,
}

#[derive(Default)]
struct SerdeFieldKeys {
	key: String,
	deserialize_keys: Vec<String>,
}

fn serde_rename_rules(attrs: &[syn::Attribute]) -> Result<SerdeRenameRules> {
	let mut rules = SerdeRenameRules::default();

	for attr in attrs {
		if !attr.path().is_ident("serde") {
			continue;
		}

		attr.parse_nested_meta(|meta| {
			if !meta.path.is_ident("rename_all") {
				return consume_serde_meta(meta);
			}

			if meta.input.peek(syn::Token![=]) {
				let value: LitStr = meta.value()?.parse()?;
				rules.deserialize = Some(value.value());
				return Ok(());
			}

			meta.parse_nested_meta(|nested| {
				if nested.path.is_ident("deserialize") {
					rules.deserialize = Some(nested.value()?.parse::<LitStr>()?.value());
				} else {
					consume_serde_meta(nested)?;
				}
				Ok(())
			})
		})?;
	}

	Ok(rules)
}

pub(crate) fn serde_deserialize_rename_rule(attrs: &[syn::Attribute]) -> Result<Option<String>> {
	Ok(serde_rename_rules(attrs)?.deserialize)
}

fn serde_field_keys(field: &syn::Field, rules: &SerdeRenameRules) -> Result<SerdeFieldKeys> {
	let mut deserialize = None;
	let mut aliases = Vec::new();

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
					deserialize = Some(lit.value());
					return Ok(());
				}

				meta.parse_nested_meta(|nested| {
					if nested.path.is_ident("deserialize") {
						deserialize = Some(nested.value()?.parse::<LitStr>()?.value());
					} else {
						consume_serde_meta(nested)?;
					}
					Ok(())
				})?;
				return Ok(());
			}

			if meta.path.is_ident("alias") {
				aliases.push(meta.value()?.parse::<LitStr>()?.value());
				return Ok(());
			}

			consume_serde_meta(meta)?;
			Ok(())
		})?;
	}

	let rust_name = field
		.ident
		.as_ref()
		.expect("named settings fields must have identifiers")
		.unraw()
		.to_string();
	let deserialize_key = deserialize
		.or_else(|| {
			rules
				.deserialize
				.as_deref()
				.map(|rule| apply_rename_rule(&rust_name, rule))
		})
		.unwrap_or_else(|| rust_name.clone());
	let mut deserialize_keys = vec![deserialize_key.clone()];
	for alias in aliases {
		if !deserialize_keys.contains(&alias) {
			deserialize_keys.push(alias);
		}
	}

	Ok(SerdeFieldKeys {
		key: deserialize_key,
		deserialize_keys,
	})
}

pub(crate) fn apply_rename_rule(name: &str, rule: &str) -> String {
	match rule {
		"lowercase" | "snake_case" => name.to_string(),
		"UPPERCASE" | "SCREAMING_SNAKE_CASE" => name.to_ascii_uppercase(),
		"PascalCase" => pascal_case(name),
		"camelCase" => {
			let mut value = pascal_case(name);
			if let Some(first) = value.get_mut(0..1) {
				first.make_ascii_lowercase();
			}
			value
		}
		"kebab-case" => name.replace('_', "-"),
		"SCREAMING-KEBAB-CASE" | "COBOL-CASE" => name.to_ascii_uppercase().replace('_', "-"),
		_ => name.to_string(),
	}
}

fn pascal_case(name: &str) -> String {
	let mut value = String::new();
	let mut capitalize = true;
	for character in name.chars() {
		if character == '_' {
			capitalize = true;
		} else if capitalize {
			value.push(character.to_ascii_uppercase());
			capitalize = false;
		} else {
			value.push(character);
		}
	}
	value
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

fn analyze_type(ty: &syn::Type, shape_hint: Option<ShapeHint>, secret: bool) -> Result<TypeShape> {
	let Some((last_segment, args)) = type_last_segment(ty) else {
		return Ok(TypeShape::Leaf {
			ty: ty.clone(),
			secret,
			secret_ref: false,
		});
	};

	let segment_name = last_segment.ident.to_string();

	match segment_name.as_str() {
		"Option" => {
			if let Some(inner_ty) = single_type_arg(args) {
				return Ok(TypeShape::Optional {
					original: ty.clone(),
					inner: Box::new(analyze_type(inner_ty, shape_hint, secret)?),
				});
			}
		}
		"Vec" => {
			if let Some(inner_ty) = single_type_arg(args) {
				return Ok(TypeShape::Sequence {
					original: ty.clone(),
					inner: Box::new(analyze_type(inner_ty, shape_hint, secret)?),
				});
			}
		}
		"HashMap" | "BTreeMap" | "IndexMap" => {
			if let (Some(key_ty), Some(value_ty)) = (first_type_arg(args), second_type_arg(args)) {
				return Ok(TypeShape::Map {
					original: ty.clone(),
					key: Box::new(key_ty.clone()),
					value: Box::new(analyze_type(value_ty, shape_hint, secret)?),
				});
			}
		}
		"Box" => {
			if let Some(inner_ty) = single_type_arg(args) {
				return Ok(TypeShape::Transparent {
					inner: Box::new(analyze_type(inner_ty, shape_hint, secret)?),
				});
			}
		}
		_ => {}
	}

	if shape_hint == Some(ShapeHint::Node)
		|| (!secret && shape_hint.is_none() && segment_name.ends_with("Config"))
	{
		Ok(TypeShape::Node { ty: ty.clone() })
	} else if secret || shape_hint == Some(ShapeHint::Leaf) || known_atomic_type(&segment_name) {
		let inherently_secret = segment_name == "SecretString" || segment_name == "SecretValue";
		Ok(TypeShape::Leaf {
			ty: ty.clone(),
			secret: secret || inherently_secret,
			secret_ref: inherently_secret,
		})
	} else {
		Err(syn::Error::new(
			ty.span(),
			"unknown settings type cannot be verified recursively; add `#[setting(leaf)]` for an intentional atomic value or use a concrete container/node type",
		))
	}
}

fn known_atomic_type(name: &str) -> bool {
	matches!(
		name,
		"String"
			| "str" | "bool"
			| "char" | "i8"
			| "i16" | "i32"
			| "i64" | "i128"
			| "isize" | "u8"
			| "u16" | "u32"
			| "u64" | "u128"
			| "usize" | "f32"
			| "f64" | "PathBuf"
			| "Value" | "SecretString"
			| "SecretValue"
	)
}

fn contains_node(shape: &TypeShape) -> bool {
	match shape {
		TypeShape::Node { .. } => true,
		TypeShape::Optional { inner, .. }
		| TypeShape::Sequence { inner, .. }
		| TypeShape::Map { value: inner, .. }
		| TypeShape::Transparent { inner } => contains_node(inner),
		TypeShape::Leaf { .. } => false,
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

fn first_type_arg(args: &syn::PathArguments) -> Option<&syn::Type> {
	let syn::PathArguments::AngleBracketed(args) = args else {
		return None;
	};
	args.args.iter().find_map(|arg| match arg {
		syn::GenericArgument::Type(ty) => Some(ty),
		_ => None,
	})
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
		TypeShape::Map {
			original, value, ..
		} => {
			let inner_ref = schema_ref_type(value, conf_crate);
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
		TypeShape::Map { value, .. } => {
			let inner_init = schema_builder_init(value, conf_crate);
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
	use rstest::rstest;

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
		assert_eq!(field.deserialize_keys, vec!["wire-key"]);
	}

	#[test]
	fn parse_fields_strips_raw_identifier_prefix() {
		let input: ItemStruct = syn::parse_quote! {
			struct TestSettings {
				r#type: String,
			}
		};

		let field = parse_single_field(input);

		assert_eq!(field.rust_name, "type");
		assert_eq!(field.key, "type");
	}

	#[test]
	fn parse_fields_preserves_serde_aliases_and_container_rename_rule() {
		let input: ItemStruct = syn::parse_quote! {
			#[serde(rename_all = "camelCase")]
			struct TestSettings {
				#[serde(alias = "legacy_display_name")]
				display_name: String,
			}
		};

		let field = parse_single_field(input);

		assert_eq!(field.key, "displayName");
		assert_eq!(
			field.deserialize_keys,
			vec!["displayName", "legacy_display_name"]
		);
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

	#[rstest]
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

	#[rstest]
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
	fn parse_fields_classifies_explicit_and_detected_secret_leaves() {
		let plain: ItemStruct = syn::parse_quote! {
			struct TestSettings { value: String }
		};
		let wrapped: ItemStruct = syn::parse_quote! {
			struct TestSettings { value: Option<Vec<String>> }
		};
		let detected: ItemStruct = syn::parse_quote! {
			struct TestSettings { value: SecretString }
		};
		let explicit: ItemStruct = syn::parse_quote! {
			struct TestSettings { #[setting(secret, leaf)] value: NestedSettings }
		};

		assert!(matches!(
			parse_single_field(plain).shape,
			TypeShape::Leaf { secret: false, .. }
		));
		assert!(matches!(
			parse_single_field(wrapped).shape,
			TypeShape::Optional { inner, .. }
				if matches!(inner.as_ref(), TypeShape::Sequence { inner, .. }
					if matches!(inner.as_ref(), TypeShape::Leaf { secret: false, .. }))
		));
		assert!(matches!(
			parse_single_field(detected).shape,
			TypeShape::Leaf { secret: true, .. }
		));
		assert!(matches!(
			parse_single_field(explicit).shape,
			TypeShape::Leaf { secret: true, .. }
		));
	}

	#[test]
	fn analyze_type_treats_config_suffix_as_node() {
		let ty: syn::Type = syn::parse_quote! { DatabaseConfig };

		let shape = analyze_type(&ty, None, false).expect("node type should be analyzed");

		assert!(matches!(shape, TypeShape::Node { .. }));
	}

	#[test]
	fn analyze_type_rejects_unknown_type_without_hint() {
		let ty: syn::Type = syn::parse_quote! { DatabaseSettings };

		let error = analyze_type(&ty, None, false).expect_err("unknown type should fail closed");

		assert_eq!(
			error.to_string(),
			"unknown settings type cannot be verified recursively; add `#[setting(leaf)]` for an intentional atomic value or use a concrete container/node type",
		);
	}
}
