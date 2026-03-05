//! Factory derive macro implementation.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, Result};

use crate::faker_attr::{FactoryFieldAttr, FactoryStructAttr, faker_type_ident};

/// Implements the Factory derive macro.
pub(crate) fn derive_factory_impl(input: DeriveInput) -> Result<TokenStream> {
	let struct_name = &input.ident;

	// Parse struct-level attributes
	let struct_attr = FactoryStructAttr::from_attrs(&input.attrs)?;

	// Get the model type
	let model_type = struct_attr.model.ok_or_else(|| {
		Error::new_spanned(
			&input,
			"#[factory(model = ...)] attribute is required on factory struct",
		)
	})?;

	// Get struct fields
	let fields = match &input.data {
		Data::Struct(data) => match &data.fields {
			Fields::Named(fields) => &fields.named,
			_ => {
				return Err(Error::new_spanned(
					&input,
					"Factory derive only supports structs with named fields",
				));
			}
		},
		_ => {
			return Err(Error::new_spanned(
				&input,
				"Factory derive only supports structs",
			));
		}
	};

	// Parse field attributes and generate initializers
	let mut field_inits = Vec::new();
	let mut field_names = Vec::new();
	let mut build_fields = Vec::new();

	for field in fields {
		let field_name = field.ident.as_ref().unwrap();
		let field_type = &field.ty;
		let field_attr = FactoryFieldAttr::from_attrs(&field.attrs)?;

		if field_attr.skip {
			continue;
		}

		field_names.push(field_name);

		// Generate field initialization for new()
		let init_expr = if let Some(faker_str) = &field_attr.faker {
			let faker_type = faker_type_ident(faker_str);
			quote! { #faker_type.generate() }
		} else if let Some(seq_format) = &field_attr.sequence {
			let seq_name = format!("{}_{}", struct_name, field_name);
			quote! {
				reinhardt_seeding::factory::sequence(#seq_name, #seq_format)
			}
		} else if let Some(default_expr) = &field_attr.default {
			quote! { #default_expr.into() }
		} else {
			// Use Default::default() for unconfigured fields
			quote! { <#field_type as std::default::Default>::default() }
		};

		field_inits.push(quote! {
			#field_name: #init_expr
		});

		// Generate field access for build()
		build_fields.push(quote! {
			self.#field_name.clone()
		});
	}

	// Generate the implementation
	let expanded = quote! {
		impl std::default::Default for #struct_name {
			fn default() -> Self {
				Self::new()
			}
		}

		impl #struct_name {
			/// Creates a new factory instance with generated default values.
			pub fn new() -> Self {
				Self {
					#(#field_inits),*
				}
			}
		}

		impl reinhardt_seeding::factory::Factory for #struct_name {
			type Model = #model_type;

			fn build(&self) -> Self::Model {
				#model_type::new(
					#(#build_fields),*
				)
			}

			async fn create(&self) -> reinhardt_seeding::SeedingResult<Self::Model> {
				let model = self.build();
				// TODO: Implement actual persistence through Model trait
				// model.save().await?;
				Ok(model)
			}

			async fn create_batch(&self, count: usize) -> reinhardt_seeding::SeedingResult<Vec<Self::Model>> {
				let mut results = Vec::with_capacity(count);
				for _ in 0..count {
					// Create new factory for each instance to get unique generated values
					let factory = Self::new();
					results.push(factory.create().await?);
				}
				Ok(results)
			}
		}

		impl reinhardt_seeding::factory::FactoryExt for #struct_name {
			fn build_with<F>(&self, customizer: F) -> Self::Model
			where
				F: FnOnce(&mut Self::Model),
			{
				let mut model = self.build();
				customizer(&mut model);
				model
			}

			async fn create_with<F>(&self, customizer: F) -> reinhardt_seeding::SeedingResult<Self::Model>
			where
				F: FnOnce(&mut Self::Model) + Send,
			{
				let mut model = self.build();
				customizer(&mut model);
				// TODO: Implement actual persistence
				// model.save().await?;
				Ok(model)
			}
		}
	};

	Ok(expanded)
}
