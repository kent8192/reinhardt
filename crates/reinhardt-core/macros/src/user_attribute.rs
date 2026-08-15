//! Implementation of the `#[user(...)]` attribute macro.
//!
//! Generates trait implementations for `BaseUser`, `FullUser`,
//! `PermissionsMixin`, and `AuthIdentity` based on struct fields.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{Ident, ItemStruct, LitBool, LitStr, Result, Token};

use crate::crate_paths::{get_async_trait_crate, get_reinhardt_auth_crate};
use crate::user_field_mapping::{
	FieldMapping, FieldRole, resolve_field_mapping, validate_required_fields,
};

pub(crate) struct UserMacroArgs {
	pub(crate) hasher: syn::Path,
	pub(crate) username_field: String,
	pub(crate) full: bool,
	/// Whether to emit `<Name>Manager` + `BaseUserManager<Name>` impl.
	/// Defaults to `true` (opt-out).
	pub(crate) manager: bool,
	/// Optional rename for the generated manager struct.
	pub(crate) manager_name: Option<Ident>,
}

fn parse_user_args(args: TokenStream) -> Result<UserMacroArgs> {
	let mut hasher: Option<syn::Path> = None;
	let mut username_field: Option<String> = None;
	let mut full = false;
	let mut manager = true;
	let mut manager_name: Option<Ident> = None;

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
		} else if meta.path.is_ident("manager") {
			let value: LitBool = meta.value()?.parse()?;
			manager = value.value();
			Ok(())
		} else if meta.path.is_ident("manager_name") {
			meta.input.parse::<Token![=]>()?;
			manager_name = Some(meta.input.parse::<Ident>()?);
			Ok(())
		} else {
			Err(meta
				.error("expected `hasher`, `username_field`, `full`, `manager`, or `manager_name`"))
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
		manager,
		manager_name,
	})
}

fn has_model_attribute(input: &ItemStruct) -> bool {
	input.attrs.iter().any(|attr| attr.path().is_ident("model"))
}

fn inject_skip_getter(input: &mut ItemStruct, mapping: &FieldMapping, args: &UserMacroArgs) {
	let mut skip_getter_fields: Vec<String> = Vec::new();
	let password_hash_field = mapping.get(FieldRole::PasswordHash).map(Ident::to_string);

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
					if password_hash_field.as_deref() == Some(name.as_str()) {
						field
							.attrs
							.push(syn::parse_quote!(#[field(skip_info = true)]));
					}
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
						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
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
						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
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
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
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
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
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
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
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

	// PK setter: when the primary key is a Uuid (or `Option<Uuid>`), `Self::default()`
	// would leave it as `Uuid::nil()`, which causes a hard PK collision on the
	// second `createsuperuser` run (issue #4237). Reseed the field with a fresh
	// v7 UUID immediately after the `Self::default()` call. Non-Uuid PKs (e.g.
	// integer auto-increment) are left untouched.
	let pk_setter = if let (Some(pk_ident), Some(pk_type)) =
		(mapping.pk_field.as_ref(), mapping.pk_type.as_ref())
	{
		match crate::pk_shape::pk_uuid_shape(pk_type) {
			(true, false) => quote! { user.#pk_ident = ::uuid::Uuid::now_v7(); },
			(true, true) => {
				quote! { user.#pk_ident = ::core::option::Option::Some(::uuid::Uuid::now_v7()); }
			}
			_ => quote! {},
		}
	} else {
		quote! {}
	};

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
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl #auth_crate::SuperuserInit for #struct_name {
			fn init_superuser(username: &str, email: &str) -> Self {
				let mut user = Self::default();
				#pk_setter
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

/// Generate an in-memory `<Name>Manager` struct and its `BaseUserManager<Name>`
/// impl from the resolved field mapping.
///
/// The generated manager is backed by a `Mutex<HashMap<PrimaryKey, User>>` so it
/// satisfies the trait's `Send + Sync` bound and works in async contexts. It is
/// intentionally lightweight (mirrors `DefaultUserManager`): production users
/// that need DB persistence, custom uniqueness rules, or multi-step creation
/// should opt out via `manager = false` and write their own implementation.
///
/// Construction relies on `<User as Default>::default()`. Consumers MUST derive
/// (or hand-implement) `Default` for the user struct.
fn generate_user_manager_impl(
	struct_name: &Ident,
	mapping: &FieldMapping,
	args: &UserMacroArgs,
) -> TokenStream {
	let auth_crate = get_reinhardt_auth_crate();
	let async_trait_crate = get_async_trait_crate();
	let manager_name = args
		.manager_name
		.clone()
		.unwrap_or_else(|| format_ident!("{}Manager", struct_name));

	let pk_field = mapping
		.pk_field
		.as_ref()
		.expect("PK validated by validate_required_fields");
	let pk_type = mapping
		.pk_type
		.as_ref()
		.expect("PK validated by validate_required_fields");
	let username_field_ident = Ident::new(&args.username_field, proc_macro2::Span::call_site());

	// Re-seed Uuid PKs to avoid the nil-collision footgun documented for
	// SuperuserInit (issue #4237). Non-Uuid PKs are left untouched (typically
	// auto-increment integers, which the DB layer assigns).
	let pk_setter = match crate::pk_shape::pk_uuid_shape(pk_type) {
		(true, false) => quote! { user.#pk_field = ::uuid::Uuid::now_v7(); },
		(true, true) => {
			quote! { user.#pk_field = ::core::option::Option::Some(::uuid::Uuid::now_v7()); }
		}
		_ => quote! {},
	};

	// Per-field setters keyed off the resolved FieldMapping. Each block is only
	// emitted when the corresponding role is actually present on the struct;
	// this keeps the generator agnostic about which #[user_field]s exist.
	let email_apply = if let Some(ident) = mapping.get(FieldRole::Email) {
		quote! {
			if let ::core::option::Option::Some(v) = extra.get("email").and_then(|v| v.as_str()) {
				user.#ident = <Self as #auth_crate::BaseUserManager<#struct_name>>::normalize_email(v);
			}
		}
	} else {
		quote! {}
	};

	let first_name_apply = if let Some(ident) = mapping.get(FieldRole::FirstName) {
		quote! {
			if let ::core::option::Option::Some(v) = extra.get("first_name").and_then(|v| v.as_str()) {
				user.#ident = v.to_string();
			}
		}
	} else {
		quote! {}
	};

	let last_name_apply = if let Some(ident) = mapping.get(FieldRole::LastName) {
		quote! {
			if let ::core::option::Option::Some(v) = extra.get("last_name").and_then(|v| v.as_str()) {
				user.#ident = v.to_string();
			}
		}
	} else {
		quote! {}
	};

	let is_active_default = if let Some(ident) = mapping.get(FieldRole::IsActive) {
		quote! { user.#ident = true; }
	} else {
		quote! {}
	};

	let is_active_apply = if let Some(ident) = mapping.get(FieldRole::IsActive) {
		quote! {
			if let ::core::option::Option::Some(v) = extra.get("is_active").and_then(|v| v.as_bool()) {
				user.#ident = v;
			}
		}
	} else {
		quote! {}
	};

	let date_joined_default = if let Some(ident) = mapping.get(FieldRole::DateJoined) {
		quote! { user.#ident = ::chrono::Utc::now(); }
	} else {
		quote! {}
	};

	// Superuser promotion setters: only emitted when the corresponding flag
	// field exists on the user struct. `is_superuser` is required (validated
	// up-front), so it always emits; `is_staff` only fires when `full = true`.
	let is_superuser_setter = if let Some(ident) = mapping.get(FieldRole::IsSuperuser) {
		quote! { user.#ident = true; }
	} else {
		quote! {}
	};

	let is_staff_setter = if let Some(ident) = mapping.get(FieldRole::IsStaff) {
		quote! { user.#ident = true; }
	} else {
		quote! {}
	};

	quote! {
		/// Auto-generated in-memory user manager for this user type.
		///
		/// Backed by `Mutex<HashMap<PrimaryKey, User>>`. The mutex is
		/// `std::sync::Mutex` because the locked critical section
		/// (`HashMap::insert`) contains no `.await`, so the executor cannot
		/// park while the lock is held. For DB-backed persistence or custom
		/// uniqueness rules, opt out via `#[user(..., manager = false)]` and
		/// hand-write a `BaseUserManager` implementation.
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		pub struct #manager_name {
			users: ::std::sync::Arc<::std::sync::Mutex<::std::collections::HashMap<#pk_type, #struct_name>>>,
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl #manager_name {
			/// Creates a new manager with an empty in-memory user store.
			pub fn new() -> Self {
				Self {
					users: ::std::sync::Arc::new(::std::sync::Mutex::new(
						::std::collections::HashMap::new(),
					)),
				}
			}

			/// Build a fresh user from the macro-known field roles without
			/// inserting it into the store. Shared between `create_user` and
			/// `create_superuser` to keep each operation to a single
			/// `HashMap::insert` (one lock acquisition).
			fn build_user_template(
				username: &str,
				password: ::core::option::Option<&str>,
				extra: ::std::collections::HashMap<
					::std::string::String,
					#auth_crate::JsonValue,
				>,
			) -> ::core::result::Result<#struct_name, #auth_crate::BaseUserManagerError> {
				use #auth_crate::BaseUser as _;
				let mut user = <#struct_name as ::core::default::Default>::default();
				#pk_setter
				user.#username_field_ident = username.to_string();
				#is_active_default
				#date_joined_default
				#email_apply
				#first_name_apply
				#last_name_apply
				#is_active_apply
				if let ::core::option::Option::Some(pwd) = password {
					user.set_password(pwd)?;
				}
				::core::result::Result::Ok(user)
			}
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl ::core::default::Default for #manager_name {
			fn default() -> Self {
				Self::new()
			}
		}

		#[#async_trait_crate::async_trait]
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		impl #auth_crate::BaseUserManager<#struct_name> for #manager_name {
			async fn create_user(
				&mut self,
				username: &str,
				password: ::core::option::Option<&str>,
				extra: ::std::collections::HashMap<
					::std::string::String,
					#auth_crate::JsonValue,
				>,
			) -> ::core::result::Result<#struct_name, #auth_crate::BaseUserManagerError> {
				let user = Self::build_user_template(username, password, extra)?;
				let mut guard = self
					.users
					.lock()
					.unwrap_or_else(|e| e.into_inner());
				guard.insert(user.#pk_field.clone(), user.clone());
				::core::result::Result::Ok(user)
			}

			async fn create_superuser(
				&mut self,
				username: &str,
				password: ::core::option::Option<&str>,
				extra: ::std::collections::HashMap<
					::std::string::String,
					#auth_crate::JsonValue,
				>,
			) -> ::core::result::Result<#struct_name, #auth_crate::BaseUserManagerError> {
				let mut user = Self::build_user_template(username, password, extra)?;
				#is_superuser_setter
				#is_staff_setter
				let mut guard = self
					.users
					.lock()
					.unwrap_or_else(|e| e.into_inner());
				guard.insert(user.#pk_field.clone(), user.clone());
				::core::result::Result::Ok(user)
			}
		}
	}
}

fn generate_auth_identity_impl(struct_name: &Ident, mapping: &FieldMapping) -> TokenStream {
	let auth_crate = get_reinhardt_auth_crate();
	let pk_field = mapping.pk_field.as_ref().expect("PK validated");
	let is_superuser_field = mapping.get(FieldRole::IsSuperuser).expect("validated");
	let is_active_field = mapping.get(FieldRole::IsActive).expect("validated");

	quote! {
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
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

			fn is_account_active(&self) -> bool {
				self.#is_active_field
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

	let mut wasm_input = input.clone();

	if has_model {
		inject_skip_getter(&mut input, &mapping, &parsed_args);
		inject_m2m_relationships(&mut input, &mapping);
	}

	strip_user_field_attrs(&mut input);
	strip_user_field_attrs(&mut wasm_input);

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
	// The auto-generated `<Name>Manager` keys its in-memory `HashMap` by the
	// user's PK. For `Uuid` / `Option<Uuid>` we re-seed the PK with
	// `Uuid::now_v7()` on every `create_user`, so collisions are impossible.
	// For any other PK type the value remains at `<User as Default>::default()`,
	// which produces a fixed sentinel (`0_i64`, empty `String`, etc.) and lets
	// repeated `create_user` calls silently overwrite each other in the map.
	// Reject that combination at compile time so users either switch to
	// `Uuid` (or `Option<Uuid>`) or opt out via `manager = false` and hand-write
	// a `BaseUserManager` impl with their own uniqueness story. See issue #4455.
	if parsed_args.manager {
		let pk_type = mapping
			.pk_type
			.as_ref()
			.expect("PK validated by validate_required_fields");
		let (is_uuid, _is_option) = crate::pk_shape::pk_uuid_shape(pk_type);
		if !is_uuid {
			return Err(syn::Error::new_spanned(
				pk_type,
				"#[user(...)] auto-manager only supports Uuid (or Option<Uuid>) primary keys. \
				 Set `manager = false` to provide a custom BaseUserManager implementation, \
				 or change the primary key field to Uuid.",
			));
		}
	}
	let user_manager_impl = if parsed_args.manager {
		generate_user_manager_impl(struct_name, &mapping, &parsed_args)
	} else {
		quote! {}
	};
	// SuperuserInit is generated for any user type that also has #[model].
	// `full = true` is no longer required: generate_superuser_init_impl emits
	// empty token streams for missing optional fields (email, is_staff,
	// date_joined, etc.), so minimal user structs are supported too. The user
	// type must also implement Default (typically via #[derive(Default)] or
	// a manual impl).
	// When #[model] is present, also auto-register a SuperuserCreator via
	// inventory so that manual register_superuser_creator() calls are not needed.
	let superuser_init_impl = if has_model {
		let auth_crate = get_reinhardt_auth_crate();
		let reinhardt_crate = crate::crate_paths::get_reinhardt_crate();
		let superuser_init = generate_superuser_init_impl(struct_name, &mapping, &parsed_args);

		// Build the registration's `type_name` string from `module_path!()`
		// and `stringify!(#struct_name)` so the value is a compile-time
		// `&'static str` literal. `inventory::submit!` expands to a `static`
		// initializer that must be const-evaluable, so using a const macro
		// pair (rather than `std::any::type_name::<T>()`, which only became
		// `const fn` in Rust 1.62 and could surface toolchain edge cases)
		// keeps the call within stable const-context guarantees while still
		// producing the fully-qualified path the tutorial regression test in
		// `examples-tutorial-basis/tests/createsuperuser.rs` compares
		// against (`std::any::type_name::<User>()` for a simple struct
		// returns `concat!(module_path!(), "::", stringify!(User))` —
		// identical formatting). Fixes #4632.
		quote! {
			#superuser_init

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#reinhardt_crate::inventory::submit! {
				#auth_crate::SuperuserCreatorRegistration::__macro_new(
					|| #auth_crate::superuser_creator_for::<#struct_name>(),
					::core::concat!(
						::core::module_path!(),
						"::",
						::core::stringify!(#struct_name),
					),
				)
			}
		}
	} else {
		quote! {}
	};

	Ok(quote! {
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#input

		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		#wasm_input

		#base_user_impl
		#full_user_impl
		#permissions_impl
		#auth_identity_impl
		#superuser_init_impl
		#user_manager_impl
	})
}
