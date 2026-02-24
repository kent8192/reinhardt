//! Implementation of GrpcSubscription derive macro
//!
//! To solve Rust 2024 lifetime capture issues,
//! use Box::pin and explicit lifetime annotations.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, LitStr, Result};

/// Parse result of gRPC subscription attributes
#[derive(Debug, Clone)]
struct GrpcSubscriptionAttr {
	service: Option<String>,
	method: Option<String>,
	filter: Option<String>,
	proto_type: Option<String>,
	graphql_type: Option<String>,
}

impl GrpcSubscriptionAttr {
	/// Parse from meta attributes
	fn from_attrs(attrs: &[syn::Attribute]) -> Result<Self> {
		let mut service = None;
		let mut method = None;
		let mut filter = None;
		let mut proto_type = None;
		let mut graphql_type = None;

		for attr in attrs {
			if attr.path().is_ident("grpc") {
				attr.parse_nested_meta(|meta| {
					if meta.path.is_ident("service") {
						let value = meta.value()?;
						let s: LitStr = value.parse()?;
						service = Some(s.value());
						Ok(())
					} else if meta.path.is_ident("method") {
						let value = meta.value()?;
						let s: LitStr = value.parse()?;
						method = Some(s.value());
						Ok(())
					} else if meta.path.is_ident("proto_type") {
						let value = meta.value()?;
						let s: LitStr = value.parse()?;
						proto_type = Some(s.value());
						Ok(())
					} else {
						Err(meta.error("unsupported grpc attribute"))
					}
				})?;
			} else if attr.path().is_ident("graphql") {
				attr.parse_nested_meta(|meta| {
					if meta.path.is_ident("filter") {
						let value = meta.value()?;
						let s: LitStr = value.parse()?;
						filter = Some(s.value());
						Ok(())
					} else if meta.path.is_ident("type") {
						let value = meta.value()?;
						let s: LitStr = value.parse()?;
						graphql_type = Some(s.value());
						Ok(())
					} else {
						Err(meta.error("unsupported graphql attribute"))
					}
				})?;
			}
		}

		Ok(Self {
			service,
			method,
			filter,
			proto_type,
			graphql_type,
		})
	}
}

pub(crate) fn expand_derive(input: DeriveInput) -> Result<TokenStream> {
	let name = &input.ident;

	// Get service and method from attributes
	let grpc_attr = GrpcSubscriptionAttr::from_attrs(&input.attrs)?;

	// Parse service client type
	let service_client_type: TokenStream = grpc_attr
		.service
		.as_ref()
		.ok_or_else(|| {
			syn::Error::new_spanned(
				&input.ident,
				"#[grpc(service = \"...\")] attribute is required",
			)
		})?
		.parse()
		.map_err(|e| {
			syn::Error::new_spanned(&input.ident, format!("Invalid service type: {}", e))
		})?;

	// Parse method name
	let method_name: TokenStream = grpc_attr
		.method
		.as_ref()
		.ok_or_else(|| {
			syn::Error::new_spanned(
				&input.ident,
				"#[grpc(method = \"...\")] attribute is required",
			)
		})?
		.parse()
		.map_err(|e| {
			syn::Error::new_spanned(&input.ident, format!("Invalid method name: {}", e))
		})?;

	// Parse proto and GraphQL types
	let proto_type: TokenStream = grpc_attr
		.proto_type
		.as_ref()
		.ok_or_else(|| {
			syn::Error::new_spanned(
				&input.ident,
				"#[grpc(proto_type = \"...\")] attribute is required",
			)
		})?
		.parse()
		.map_err(|e| syn::Error::new_spanned(&input.ident, format!("Invalid proto_type: {}", e)))?;

	let graphql_type: TokenStream = grpc_attr
		.graphql_type
		.as_ref()
		.ok_or_else(|| {
			syn::Error::new_spanned(
				&input.ident,
				"#[graphql(type = \"...\")] attribute is required",
			)
		})?
		.parse()
		.map_err(|e| {
			syn::Error::new_spanned(&input.ident, format!("Invalid graphql type: {}", e))
		})?;

	// Generate filter code if filter is specified
	let filter_code = if let Some(filter_expr) = grpc_attr.filter {
		// Parse filter expression as a closure
		let filter_tokens: TokenStream = filter_expr.parse().map_err(|e| {
			syn::Error::new_spanned(&input.ident, format!("Invalid filter expression: {}", e))
		})?;
		quote! {
			.filter(move |item| {
				match item {
					Ok(ref val) => {
						let filter_fn = #filter_tokens;
						filter_fn(val)
					}
					Err(_) => true,
				}
			})
		}
	} else {
		quote! {}
	};

	// Generate GraphQL Subscription implementation
	let expanded = quote! {
		#[async_graphql::Subscription]
		impl #name {
			/// Map gRPC stream to GraphQL Subscription
			async fn subscribe<'ctx>(
				&self,
				ctx: &async_graphql::Context<'ctx>,
			) -> async_graphql::Result<impl futures_util::Stream<Item = Result<#graphql_type, async_graphql::Error>> + 'ctx> {
				use tokio_stream::StreamExt;

				// Get gRPC client from context
				let client = ctx
					.data::<#service_client_type<tonic::transport::Channel>>()
					.map_err(|_| async_graphql::Error::new(format!(
						"Internal server error: {} not found in context",
						stringify!(#service_client_type)
					)))?
					.clone();

				// Call gRPC streaming method
				let stream = match client.#method_name(Default::default()).await {
					Ok(response) => response.into_inner(),
					Err(e) => {
						return Ok(Box::pin(tokio_stream::once(
							Err(async_graphql::Error::new(format!("gRPC call failed: {:?}", e)))
						)));
					}
				};

				// Convert Proto events to GraphQL events
				Ok(Box::pin(stream
					.map(move |result: Result<#proto_type, tonic::Status>| {
						match result {
							Ok(proto_event) => {
								// Convert proto to GraphQL type using Into trait
								let graphql_event: #graphql_type = proto_event.into();
								Ok(graphql_event)
							}
							// Fixes #818
							Err(e) => Err(async_graphql::Error::new(
								format!("Stream error: {:?}", e)
							)),
						}
					})
					#filter_code
				))
			}
		}
	};

	Ok(expanded)
}

#[cfg(test)]
mod tests {
	use super::*;
	use syn::parse_quote;

	#[test]
	fn test_missing_required_attributes() {
		// Missing all required attributes - should fail
		let input: DeriveInput = parse_quote! {
			struct UserCreatedSubscription;
		};

		let result = expand_derive(input);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("service = \"...\"")
		);
	}

	#[test]
	fn test_complete_subscription_with_all_attrs() {
		let input: DeriveInput = parse_quote! {
			#[grpc(service = "proto::UserServiceClient", method = "subscribe_user_events", proto_type = "proto::UserEvent")]
			#[graphql(type = "User")]
			struct UserCreatedSubscription;
		};

		let result = expand_derive(input);
		assert!(result.is_ok());
		let output = result.unwrap();
		let output_str = output.to_string();

		// Check existence of Subscription implementation
		assert!(output_str.contains("# [async_graphql :: Subscription]"));
		assert!(output_str.contains("impl UserCreatedSubscription"));

		// Check gRPC client retrieval
		assert!(output_str.contains("proto :: UserServiceClient"));
		assert!(output_str.contains("tonic :: transport :: Channel"));

		// Check method call
		assert!(output_str.contains("subscribe_user_events"));

		// Check type conversions
		assert!(output_str.contains("proto :: UserEvent"));
		assert!(output_str.contains("User"));

		// Check stream processing
		assert!(output_str.contains(". map"));
		assert!(output_str.contains("into_inner"));
	}

	#[test]
	fn test_subscription_with_filter() {
		let input: DeriveInput = parse_quote! {
			#[grpc(service = "proto::EventServiceClient", method = "subscribe_events", proto_type = "proto::Event")]
			#[graphql(type = "GraphQLEvent", filter = "|event| event.priority > 5")]
			struct ImportantEventsSubscription;
		};

		let result = expand_derive(input);
		assert!(result.is_ok());
		let output = result.unwrap();
		let output_str = output.to_string();

		// Check filter is present
		assert!(output_str.contains(". filter"));
		assert!(output_str.contains("priority"));
	}

	#[test]
	fn test_missing_service_attribute() {
		let input: DeriveInput = parse_quote! {
			#[grpc(method = "subscribe_users", proto_type = "proto::User")]
			#[graphql(type = "User")]
			struct UserSubscription;
		};

		let result = expand_derive(input);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("service = \"...\"")
		);
	}

	#[test]
	fn test_missing_method_attribute() {
		let input: DeriveInput = parse_quote! {
			#[grpc(service = "UserService", proto_type = "proto::User")]
			#[graphql(type = "User")]
			struct UserSubscription;
		};

		let result = expand_derive(input);
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("method = \"...\""));
	}

	#[test]
	fn test_missing_proto_type_attribute() {
		let input: DeriveInput = parse_quote! {
			#[grpc(service = "UserService", method = "subscribe_users")]
			#[graphql(type = "User")]
			struct UserSubscription;
		};

		let result = expand_derive(input);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("proto_type = \"...\"")
		);
	}

	#[test]
	fn test_missing_graphql_type_attribute() {
		let input: DeriveInput = parse_quote! {
			#[grpc(service = "UserService", method = "subscribe_users", proto_type = "proto::User")]
			struct UserSubscription;
		};

		let result = expand_derive(input);
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("type = \"...\""));
	}

	#[test]
	fn test_generated_code_uses_error_handling_not_expect() {
		// Arrange
		let input: DeriveInput = parse_quote! {
			#[grpc(service = "proto::UserServiceClient", method = "subscribe_user_events", proto_type = "proto::UserEvent")]
			#[graphql(type = "User")]
			struct UserCreatedSubscription;
		};

		// Act
		let result = expand_derive(input);
		assert!(result.is_ok());
		let output_str = result.unwrap().to_string();

		// Assert - generated code must NOT contain .expect() (DoS risk via panic)
		assert!(
			!output_str.contains(".expect("),
			"Generated code must not use .expect() - use map_err for graceful error handling"
		);

		// Assert - generated code must use map_err for proper error propagation
		assert!(
			output_str.contains("map_err"),
			"Generated code must use map_err for error handling"
		);

		// Assert - return type must be async_graphql::Result (not bare Stream)
		assert!(
			output_str.contains("async_graphql :: Result"),
			"Return type must be async_graphql::Result for proper error propagation"
		);
	}
}
