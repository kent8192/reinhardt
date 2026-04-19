//! Implementation of the `#[user(...)]` attribute macro.
//!
//! Generates trait implementations for `BaseUser`, `FullUser`,
//! `PermissionsMixin`, and `AuthIdentity` based on struct fields.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::{Ident, ItemStruct, LitBool, LitStr, Result, Token};

use crate::crate_paths::get_reinhardt_auth_crate;
use crate::user_field_mapping::{
	FieldMapping, FieldRole, resolve_field_mapping, validate_required_fields,
};

struct UserMacroArgs {
	hasher: syn::Path,
	username_field: String,
	full: bool,
}

fn parse_user_args(args: TokenStream) -> Result<UserMacroArgs> {
	let mut hasher: Option<syn::Path> = None;
	let mut username_field: Option<String> = None;
	let mut full = false;

	let parser = syn::meta::parser(|meta| {
		if meta.path.is_ident("hasher") {
			meta.input.parse::<Token![=]>()?;
			hasher = Some(meta.input.parse::<syn::Path>()?);
			Ok(())
		} else if meta.path.is_ident("username_field") {
			let value: LitStr = meta.value()?.parse()?;
			username_field = Some(value.value());
			Ok(())
		} else if meta.path.is_ident("full") {
			let value: LitBool = meta.value()?.parse()?;
			full = value.value();
			Ok(())
		} else {
			Err(meta.error("expected `hasher`, `username_field`, or `full`"))
		}
	});

	parser.parse2(args)?;

	let hasher = hasher.ok_or_else(|| {
		syn::Error::new(
			proc_macro2::Span::call_site(),
			"#[user] requires 'hasher' argument: #[user(hasher = YourHasher)]",
		)
	})?;

	let username_field = username_field.ok_or_else(|| {
		syn::Error::new(
			proc_macro2::Span::call_site(),
			"#[user] requires 'username_field' argument: #[user(username_field = \"email\")]",
		)
	})?;

	Ok(UserMacroArgs {
		hasher,
		username_field,
		full,
	})
}

fn has_model_attribute(input: &ItemStruct) -> bool {
	input.attrs.iter().any(|attr| attr.path().is_ident("model"))
}

fn inject_skip_getter(input: &mut ItemStruct, mapping: &FieldMapping, args: &UserMacroArgs) {
	let mut skip_getter_fields: Vec<String> = Vec::new();

	// Username field (always)
	skip_getter_fields.push(args.username_field.clone());

	// BaseUser fields (always)
	for role in &[
		FieldRole::PasswordHash,
		FieldRole::LastLogin,
		FieldRole::IsActive,
	] {
		if let Some(ident) = mapping.get(*role) {
			skip_getter_fields.push(ident.to_string());
		}
	}

	// FullUser fields (when full = true)
	if args.full {
		for role in &[
			FieldRole::Email,
			FieldRole::FirstName,
			FieldRole::LastName,
			FieldRole::IsStaff,
			FieldRole::DateJoined,
		] {
			if let Some(ident) = mapping.get(*role) {
				skip_getter_fields.push(ident.to_string());
			}
		}
	}

	// IsSuperuser — DB field, skip_getter only
	if let Some(ident) = mapping.get(FieldRole::IsSuperuser) {
		skip_getter_fields.push(ident.to_string());
	}

	// UserPermissions and Groups — non-DB cache fields, fully skipped by model
	let mut skip_fields: Vec<String> = Vec::new();
	for role in &[FieldRole::UserPermissions, FieldRole::Groups] {
		if let Some(ident) = mapping.get(*role) {
			skip_fields.push(ident.to_string());
		}
	}

	if let syn::Fields::Named(ref mut fields) = input.fields {
		for field in &mut fields.named {
			if let Some(ref ident) = field.ident {
				let name = ident.to_string();
				if skip_fields.contains(&name) {
					field.attrs.push(syn::parse_quote!(#[field(skip = true)]));
				} else if skip_getter_fields.contains(&name) {
					field
						.attrs
						.push(syn::parse_quote!(#[field(skip_getter = true)]));
				}
			}
		}
	}
}

/// When `#[model]` is present, inject `ManyToManyField` relationships for
/// `Permission` and `Group` models alongside the existing `Vec<String>` fields.
fn inject_m2m_relationships(input: &mut ItemStruct, mapping: &FieldMapping) {
	let auth_crate = get_reinhardt_auth_crate();
	let db_crate = crate::crate_paths::get_reinhardt_db_crate();

	if let syn::Fields::Named(ref mut fields) = input.fields {
		let mut new_fields: Vec<syn::Field> = Vec::new();

		for field in fields.named.iter_mut() {
			if let Some(ref ident) = field.ident {
				let name = ident.to_string();

				if mapping
					.get(FieldRole::UserPermissions)
					.map(|i| *i == name)
					.unwrap_or(false)
				{
					// Mark original Vec<String> field as serde-skipped (non-serialized cache)
					field.attrs.push(syn::parse_quote!(#[serde(skip)]));

					// Inject ManyToManyField for DB relationship
					let m2m_field: syn::Field = syn::parse_quote! {
						#[serde(skip, default)]
						#[rel(many_to_many, related_name = "users")]
						#[field(skip_getter = true)]
						_permissions_rel: #db_crate::associations::ManyToManyField<Self, #auth_crate::AuthPermission>
					};
					new_fields.push(m2m_field);
				}

				if mapping
					.get(FieldRole::Groups)
					.map(|i| *i == name)
					.unwrap_or(false)
				{
					field.attrs.push(syn::parse_quote!(#[serde(skip)]));

					let m2m_field: syn::Field = syn::parse_quote! {
						#[serde(skip, default)]
						#[rel(many_to_many, related_name = "members")]
						#[field(skip_getter = true)]
						_groups_rel: #db_crate::associations::ManyToManyField<Self, #auth_crate::Group>
					};
					new_fields.push(m2m_field);
				}
			}
		}

		for new_field in new_fields {
			fields.named.push(new_field);
		}
	}
}

fn strip_user_field_attrs(input: &mut ItemStruct) {
	if let syn::Fields::Named(ref mut fields) = input.fields {
		for field in &mut fields.named {
			field
				.attrs
				.retain(|attr| !attr.path().is_ident("user_field"));
		}
	}
}

fn generate_base_user_impl(
	struct_name: &Ident,
	mapping: &FieldMapping,
	args: &UserMacroArgs,
) -> TokenStream {
	let auth_crate = get_reinhardt_auth_crate();
	let hasher = &args.hasher;
	let username_field_str = &args.username_field;
	let username_field_ident = Ident::new(&args.username_field, proc_macro2::Span::call_site());
	let pk_type = mapping.pk_type.as_ref().expect("PK validated");
	let password_hash_field = mapping.get(FieldRole::PasswordHash).expect("validated");
	let last_login_field = mapping.get(FieldRole::LastLogin).expect("validated");
	let is_active_field = mapping.get(FieldRole::IsActive).expect("validated");

	quote! {
		impl #auth_crate::BaseUser for #struct_name {
			type PrimaryKey = #pk_type;
			type Hasher = #hasher;

			fn get_username_field() -> &'static str {
				#username_field_str
			}

			fn get_username(&self) -> &str {
				&self.#username_field_ident
			}

			fn password_hash(&self) -> Option<&str> {
				self.#password_hash_field.as_deref()
			}

			fn set_password_hash(&mut self, hash: String) {
				self.#password_hash_field = Some(hash);
			}

			fn last_login(&self) -> Option<chrono::DateTime<chrono::Utc>> {
				self.#last_login_field
			}

			fn set_last_login(&mut self, time: chrono::DateTime<chrono::Utc>) {
				self.#last_login_field = Some(time);
			}

			fn is_active(&self) -> bool {
				self.#is_active_field
			}
		}
	}
}

fn generate_full_user_impl(
	struct_name: &Ident,
	mapping: &FieldMapping,
	args: &UserMacroArgs,
) -> TokenStream {
	let auth_crate = get_reinhardt_auth_crate();
	let username_field_ident = Ident::new(&args.username_field, proc_macro2::Span::call_site());
	let email_field = mapping.get(FieldRole::Email).expect("validated");
	let first_name_field = mapping.get(FieldRole::FirstName).expect("validated");
	let last_name_field = mapping.get(FieldRole::LastName).expect("validated");
	let is_staff_field = mapping.get(FieldRole::IsStaff).expect("validated");
	let is_superuser_field = mapping.get(FieldRole::IsSuperuser).expect("validated");
	let date_joined_field = mapping.get(FieldRole::DateJoined).expect("validated");

	quote! {
		impl #auth_crate::FullUser for #struct_name {
			fn username(&self) -> &str {
				&self.#username_field_ident
			}

			fn email(&self) -> &str {
				&self.#email_field
			}

			fn first_name(&self) -> &str {
				&self.#first_name_field
			}

			fn last_name(&self) -> &str {
				&self.#last_name_field
			}

			fn is_staff(&self) -> bool {
				self.#is_staff_field
			}

			fn is_superuser(&self) -> bool {
				self.#is_superuser_field
			}

			fn date_joined(&self) -> chrono::DateTime<chrono::Utc> {
				self.#date_joined_field
			}
		}
	}
}

fn generate_permissions_mixin_impl(
	struct_name: &Ident,
	mapping: &FieldMapping,
) -> Option<TokenStream> {
	let user_permissions_field = mapping.get(FieldRole::UserPermissions)?;
	let groups_field = mapping.get(FieldRole::Groups)?;
	let is_superuser_field = mapping.get(FieldRole::IsSuperuser)?;
	let auth_crate = get_reinhardt_auth_crate();

	Some(quote! {
		impl #auth_crate::PermissionsMixin for #struct_name {
			fn is_superuser(&self) -> bool {
				self.#is_superuser_field
			}

			fn user_permissions(&self) -> &[String] {
				&self.#user_permissions_field
			}

			fn groups(&self) -> &[String] {
				&self.#groups_field
			}
		}
	})
}

fn generate_superuser_init_impl(
	struct_name: &Ident,
	mapping: &FieldMapping,
	args: &UserMacroArgs,
) -> TokenStream {
	let auth_crate = get_reinhardt_auth_crate();
	let username_field_ident = Ident::new(&args.username_field, proc_macro2::Span::call_site());

	// email field: use mapping if available, fall back to convention name
	let email_setter = if let Some(email_ident) = mapping.get(FieldRole::Email) {
		quote! { user.#email_ident = email.to_string(); }
	} else {
		quote! {}
	};

	let is_staff_setter = if let Some(ident) = mapping.get(FieldRole::IsStaff) {
		quote! { user.#ident = true; }
	} else {
		quote! {}
	};

	let is_superuser_setter = if let Some(ident) = mapping.get(FieldRole::IsSuperuser) {
		quote! { user.#ident = true; }
	} else {
		// IsSuperuser is required by validate_required_fields, so this is unreachable
		quote! {}
	};

	let is_active_setter = if let Some(ident) = mapping.get(FieldRole::IsActive) {
		quote! { user.#ident = true; }
	} else {
		quote! {}
	};

	let date_joined_setter = if let Some(ident) = mapping.get(FieldRole::DateJoined) {
		quote! { user.#ident = chrono::Utc::now(); }
	} else {
		quote! {}
	};

	quote! {
		impl #auth_crate::SuperuserInit for #struct_name {
			fn init_superuser(username: &str, email: &str) -> Self {
				let mut user = Self::default();
				user.#username_field_ident = username.to_string();
				#email_setter
				#is_staff_setter
				#is_superuser_setter
				#is_active_setter
				#date_joined_setter
				user
			}
		}
	}
}

fn generate_auth_identity_impl(struct_name: &Ident, mapping: &FieldMapping) -> TokenStream {
	let auth_crate = get_reinhardt_auth_crate();
	let pk_field = mapping.pk_field.as_ref().expect("PK validated");
	let is_superuser_field = mapping.get(FieldRole::IsSuperuser).expect("validated");

	quote! {
		impl #auth_crate::AuthIdentity for #struct_name {
			fn id(&self) -> String {
				self.#pk_field.to_string()
			}

			fn is_authenticated(&self) -> bool {
				true
			}

			fn is_admin(&self) -> bool {
				self.#is_superuser_field
			}
		}
	}
}

pub(crate) fn user_attribute_impl(args: TokenStream, mut input: ItemStruct) -> Result<TokenStream> {
	let parsed_args = parse_user_args(args)?;
	let has_model = has_model_attribute(&input);

	let mapping = resolve_field_mapping(&input.fields, has_model)?;
	validate_required_fields(
		&mapping,
		&parsed_args.username_field,
		parsed_args.full,
		&input.fields,
	)?;

	if has_model {
		inject_skip_getter(&mut input, &mapping, &parsed_args);
		inject_m2m_relationships(&mut input, &mapping);
	}

	strip_user_field_attrs(&mut input);

	let struct_name = &input.ident;
	let base_user_impl = generate_base_user_impl(struct_name, &mapping, &parsed_args);
	let full_user_impl = if parsed_args.full {
		generate_full_user_impl(struct_name, &mapping, &parsed_args)
	} else {
		quote! {}
	};
	let permissions_impl =
		generate_permissions_mixin_impl(struct_name, &mapping).unwrap_or_else(|| quote! {});
	let auth_identity_impl = generate_auth_identity_impl(struct_name, &mapping);
	// SuperuserInit is only generated for full user types with #[model].
	// The user type must also implement Default (typically via #[derive(Default)]
	// or a manual impl).
	// When both conditions are met, also auto-register a SuperuserCreator via
	// inventory so that manual register_superuser_creator() calls are not needed.
	let superuser_init_impl = if parsed_args.full && has_model {
		let auth_crate = get_reinhardt_auth_crate();
		let reinhardt_crate = crate::crate_paths::get_reinhardt_crate();
		let superuser_init = generate_superuser_init_impl(struct_name, &mapping, &parsed_args);
		let type_name_str = struct_name.to_string();

		quote! {
			#superuser_init

			#reinhardt_crate::inventory::submit! {
				#auth_crate::SuperuserCreatorRegistration::__macro_new(
					|| #auth_crate::superuser_creator_for::<#struct_name>(),
					#type_name_str,
				)
			}
		}
	} else {
		quote! {}
	};

	Ok(quote! {
		#input

		#base_user_impl
		#full_user_impl
		#permissions_impl
		#auth_identity_impl
		#superuser_init_impl
	})
}
