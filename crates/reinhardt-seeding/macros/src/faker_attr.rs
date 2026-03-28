//! Factory attribute parsing.
//!
//! This module handles parsing of `#[factory(...)]` attributes.

use proc_macro2::Span;
use syn::{Expr, LitStr, Result, Token};

/// Parsed factory attribute for a struct.
#[derive(Debug, Default)]
pub(crate) struct FactoryStructAttr {
	/// The model type this factory creates.
	pub model: Option<syn::Type>,
}

impl FactoryStructAttr {
	/// Parses factory attributes from struct attributes.
	pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> Result<Self> {
		let mut result = Self::default();

		for attr in attrs {
			if !attr.path().is_ident("factory") {
				continue;
			}

			attr.parse_nested_meta(|meta| {
				if meta.path.is_ident("model") {
					let _: Token![=] = meta.input.parse()?;
					result.model = Some(meta.input.parse()?);
					Ok(())
				} else {
					Err(meta.error("unknown factory attribute"))
				}
			})?;
		}

		Ok(result)
	}
}

/// Parsed factory attribute for a field.
#[derive(Debug, Default, Clone)]
pub(crate) struct FactoryFieldAttr {
	/// Faker type to use for generation.
	pub faker: Option<String>,

	/// Sequence format string.
	pub sequence: Option<String>,

	/// Default value expression.
	pub default: Option<Expr>,

	/// Whether to skip this field.
	pub skip: bool,

	/// Subfactory type (reserved for future use).
	#[allow(dead_code)]
	pub subfactory: Option<syn::Type>,

	/// Lazy evaluation (reserved for future use).
	#[allow(dead_code)]
	pub lazy: bool,
}

impl FactoryFieldAttr {
	/// Parses factory attributes from field attributes.
	pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> Result<Self> {
		let mut result = Self::default();

		for attr in attrs {
			if !attr.path().is_ident("factory") {
				continue;
			}

			attr.parse_nested_meta(|meta| {
				if meta.path.is_ident("faker") {
					let _: Token![=] = meta.input.parse()?;
					let lit: LitStr = meta.input.parse()?;
					result.faker = Some(lit.value());
					Ok(())
				} else if meta.path.is_ident("sequence") {
					let _: Token![=] = meta.input.parse()?;
					let lit: LitStr = meta.input.parse()?;
					result.sequence = Some(lit.value());
					Ok(())
				} else if meta.path.is_ident("default") {
					let _: Token![=] = meta.input.parse()?;
					result.default = Some(meta.input.parse()?);
					Ok(())
				} else if meta.path.is_ident("skip") {
					result.skip = true;
					Ok(())
				} else if meta.path.is_ident("subfactory") {
					let _: Token![=] = meta.input.parse()?;
					result.subfactory = Some(meta.input.parse()?);
					Ok(())
				} else if meta.path.is_ident("lazy") {
					result.lazy = true;
					Ok(())
				} else {
					Err(meta.error("unknown factory field attribute"))
				}
			})?;
		}

		Ok(result)
	}
}

/// Converts a faker type string to a FakerType identifier.
pub(crate) fn faker_type_ident(faker_str: &str) -> proc_macro2::TokenStream {
	let variant = match faker_str.to_lowercase().as_str() {
		"name" => quote::quote!(Name),
		"first_name" | "firstname" => quote::quote!(FirstName),
		"last_name" | "lastname" => quote::quote!(LastName),
		"email" => quote::quote!(Email),
		"safe_email" | "safeemail" => quote::quote!(SafeEmail),
		"username" => quote::quote!(Username),
		"password" => quote::quote!(Password),
		"domain_name" | "domainname" | "domain" => quote::quote!(DomainName),
		"url" => quote::quote!(Url),
		"word" => quote::quote!(Word),
		"words" => quote::quote!(Words),
		"sentence" => quote::quote!(Sentence),
		"paragraph" => quote::quote!(Paragraph),
		"street_name" | "streetname" | "street" => quote::quote!(StreetName),
		"city" => quote::quote!(City),
		"state" => quote::quote!(State),
		"zip_code" | "zipcode" | "zip" | "postal_code" | "postalcode" => quote::quote!(ZipCode),
		"country" => quote::quote!(Country),
		"phone_number" | "phonenumber" | "phone" => quote::quote!(PhoneNumber),
		"cell_number" | "cellnumber" | "cell" | "mobile" => quote::quote!(CellNumber),
		"company_name" | "companyname" | "company" => quote::quote!(CompanyName),
		"integer" | "int" | "number" => quote::quote!(Integer),
		"float" | "decimal" | "double" => quote::quote!(Float),
		"boolean" | "bool" => quote::quote!(Boolean),
		"date" => quote::quote!(Date),
		"datetime" | "date_time" => quote::quote!(DateTime),
		"time" => quote::quote!(Time),
		"uuid" => quote::quote!(Uuid),
		_ => {
			// Unknown type - will cause a compile error
			let ident = syn::Ident::new(&faker_str.to_uppercase(), Span::call_site());
			quote::quote!(#ident)
		}
	};

	quote::quote!(reinhardt_seeding::factory::FakerType::#variant)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_faker_type_ident() {
		let tokens = faker_type_ident("email");
		assert!(tokens.to_string().contains("Email"));

		let tokens = faker_type_ident("first_name");
		assert!(tokens.to_string().contains("FirstName"));
	}
}
