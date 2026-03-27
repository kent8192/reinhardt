//! Field detection and role resolution for the `#[user]` macro.
//!
//! Implements Convention-over-Configuration: fields are matched by name convention,
//! with explicit `#[user_field(role)]` overrides taking priority.

use proc_macro2::Span;
use syn::{Fields, Ident, Result};

/// Roles that can be assigned to struct fields for auth trait generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldRole {
	PasswordHash,
	LastLogin,
	IsActive,
	IsSuperuser,
	IsStaff,
	Email,
	FirstName,
	LastName,
	DateJoined,
	UserPermissions,
	Groups,
}

impl FieldRole {
	pub(crate) fn convention_name(&self) -> &'static str {
		match self {
			Self::PasswordHash => "password_hash",
			Self::LastLogin => "last_login",
			Self::IsActive => "is_active",
			Self::IsSuperuser => "is_superuser",
			Self::IsStaff => "is_staff",
			Self::Email => "email",
			Self::FirstName => "first_name",
			Self::LastName => "last_name",
			Self::DateJoined => "date_joined",
			Self::UserPermissions => "user_permissions",
			Self::Groups => "groups",
		}
	}

	pub(crate) fn from_str(s: &str) -> Option<Self> {
		match s {
			"password_hash" => Some(Self::PasswordHash),
			"last_login" => Some(Self::LastLogin),
			"is_active" => Some(Self::IsActive),
			"is_superuser" => Some(Self::IsSuperuser),
			"is_staff" => Some(Self::IsStaff),
			"email" => Some(Self::Email),
			"first_name" => Some(Self::FirstName),
			"last_name" => Some(Self::LastName),
			"date_joined" => Some(Self::DateJoined),
			"user_permissions" => Some(Self::UserPermissions),
			"groups" => Some(Self::Groups),
			_ => None,
		}
	}

	pub(crate) fn all_names() -> &'static [&'static str] {
		&[
			"password_hash",
			"last_login",
			"is_active",
			"is_superuser",
			"is_staff",
			"email",
			"first_name",
			"last_name",
			"date_joined",
			"user_permissions",
			"groups",
		]
	}
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedField {
	pub(crate) role: FieldRole,
	pub(crate) field_ident: Ident,
}

#[derive(Debug)]
pub(crate) struct FieldMapping {
	pub(crate) mappings: Vec<ResolvedField>,
	pub(crate) pk_field: Option<Ident>,
	pub(crate) pk_type: Option<syn::Type>,
}

impl FieldMapping {
	pub(crate) fn get(&self, role: FieldRole) -> Option<&Ident> {
		self.mappings
			.iter()
			.find(|m| m.role == role)
			.map(|m| &m.field_ident)
	}

	pub(crate) fn has(&self, role: FieldRole) -> bool {
		self.get(role).is_some()
	}
}

pub(crate) fn resolve_field_mapping(fields: &Fields, has_model_attr: bool) -> Result<FieldMapping> {
	let named_fields = match fields {
		Fields::Named(f) => &f.named,
		_ => {
			return Err(syn::Error::new(
				Span::call_site(),
				"#[user] can only be applied to structs with named fields",
			));
		}
	};

	let mut mappings: Vec<ResolvedField> = Vec::new();
	let mut pk_field: Option<Ident> = None;
	let mut pk_type: Option<syn::Type> = None;

	for field in named_fields {
		let field_ident = field
			.ident
			.as_ref()
			.expect("named fields always have idents");

		// Check for #[user_field(role)]
		for attr in &field.attrs {
			if attr.path().is_ident("user_field") {
				let role_ident: Ident = attr.parse_args()?;
				let role_str = role_ident.to_string();
				let role = FieldRole::from_str(&role_str).ok_or_else(|| {
					syn::Error::new(
						role_ident.span(),
						format!(
							"unknown user_field role '{}'. Valid roles: {}",
							role_str,
							FieldRole::all_names().join(", ")
						),
					)
				})?;

				if let Some(existing) = mappings.iter().find(|m| m.role == role) {
					return Err(syn::Error::new(
						role_ident.span(),
						format!(
							"duplicate user_field mapping: '{}' is mapped to both '{}' and '{}'",
							role.convention_name(),
							existing.field_ident,
							field_ident
						),
					));
				}

				mappings.push(ResolvedField {
					role,
					field_ident: field_ident.clone(),
				});
			}
		}

		// Check for PK: #[field(primary_key = true)]
		if has_model_attr {
			for attr in &field.attrs {
				if attr.path().is_ident("field") {
					let _ = attr.parse_nested_meta(|meta| {
						if meta.path.is_ident("primary_key") {
							let value = meta.value()?.parse::<syn::LitBool>()?;
							if value.value() {
								pk_field = Some(field_ident.clone());
								pk_type = Some(field.ty.clone());
							}
						}
						Ok(())
					});
				}
			}
		}
	}

	// Fallback PK: field named "id"
	if pk_field.is_none() {
		for field in named_fields {
			let field_ident = field.ident.as_ref().unwrap();
			if field_ident == "id" {
				pk_field = Some(field_ident.clone());
				pk_type = Some(field.ty.clone());
				break;
			}
		}
	}

	// Convention-based fallback
	let all_roles = [
		FieldRole::PasswordHash,
		FieldRole::LastLogin,
		FieldRole::IsActive,
		FieldRole::IsSuperuser,
		FieldRole::IsStaff,
		FieldRole::Email,
		FieldRole::FirstName,
		FieldRole::LastName,
		FieldRole::DateJoined,
		FieldRole::UserPermissions,
		FieldRole::Groups,
	];

	for role in &all_roles {
		if mappings.iter().any(|m| m.role == *role) {
			continue;
		}
		let convention_name = role.convention_name();
		for field in named_fields {
			let field_ident = field.ident.as_ref().unwrap();
			if field_ident == convention_name {
				mappings.push(ResolvedField {
					role: *role,
					field_ident: field_ident.clone(),
				});
				break;
			}
		}
	}

	Ok(FieldMapping {
		mappings,
		pk_field,
		pk_type,
	})
}

pub(crate) fn validate_required_fields(
	mapping: &FieldMapping,
	username_field: &str,
	full: bool,
	fields: &Fields,
) -> Result<()> {
	let named_fields = match fields {
		Fields::Named(f) => &f.named,
		_ => unreachable!(),
	};

	let username_exists = named_fields
		.iter()
		.any(|f| f.ident.as_ref().is_some_and(|i| i == username_field));
	if !username_exists {
		return Err(syn::Error::new(
			Span::call_site(),
			format!(
				"#[user] username_field '{}' not found. Add a field named '{}: String'",
				username_field, username_field
			),
		));
	}

	if mapping.pk_field.is_none() {
		return Err(syn::Error::new(
			Span::call_site(),
			"#[user] could not detect primary key field. Add #[field(primary_key = true)] or a field named 'id'",
		));
	}

	let base_required = [
		FieldRole::PasswordHash,
		FieldRole::LastLogin,
		FieldRole::IsActive,
		FieldRole::IsSuperuser,
	];
	for role in &base_required {
		if !mapping.has(*role) {
			return Err(syn::Error::new(
				Span::call_site(),
				format!(
					"#[user] requires a field for '{}'. Add a field named '{}' or annotate a field with #[user_field({})]",
					role.convention_name(),
					role.convention_name(),
					role.convention_name()
				),
			));
		}
	}

	if full {
		let full_required = [
			FieldRole::Email,
			FieldRole::FirstName,
			FieldRole::LastName,
			FieldRole::IsStaff,
			FieldRole::DateJoined,
		];
		for role in &full_required {
			if !mapping.has(*role) {
				return Err(syn::Error::new(
					Span::call_site(),
					format!(
						"#[user(full = true)] requires '{}' field. Add '{}' or annotate a field with #[user_field({})]",
						role.convention_name(),
						role.convention_name(),
						role.convention_name()
					),
				));
			}
		}
	}

	Ok(())
}
