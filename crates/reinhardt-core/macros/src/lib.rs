//! # Reinhardt Procedural Macros
//!
//! Provides Django-style decorators as Rust procedural macros.
//!
//! ## Macros
//!
//! - `#[routes]` - Register URL pattern function for automatic discovery
//! - `#[api_view]` - Convert function to API view
//! - `#[action]` - Define custom ViewSet action
//! - `#[get]`, `#[post]`, etc. - HTTP method decorators
//! - `#[permission_required]` - Permission decorator
//!

use proc_macro::TokenStream;
use syn::{ItemFn, ItemStruct, parse_macro_input};

mod action;
mod admin;
mod api_view;
mod app_config_attribute;
mod app_config_derive;
mod collect_migrations;
mod crate_paths;
mod injectable_common;
mod injectable_fn;
mod injectable_struct;
mod installed_apps;
mod model_attribute;
mod model_derive;
mod orm_reflectable_derive;
mod path_macro;
mod permission_macro;
mod permissions;
mod query_fields;
mod receiver;
mod rel;
mod routes;
mod routes_registration;
mod schema;
mod use_inject;

use action::action_impl;
use admin::admin_impl;
use api_view::api_view_impl;
use app_config_attribute::app_config_attribute_impl;
use injectable_fn::injectable_fn_impl;
use injectable_struct::injectable_struct_impl;
use installed_apps::installed_apps_impl;
use model_attribute::model_attribute_impl;
use model_derive::model_derive_impl;
use orm_reflectable_derive::orm_reflectable_derive_impl;
use path_macro::path_impl;
use permissions::permission_required_impl;
use query_fields::derive_query_fields_impl;
use receiver::receiver_impl;
use routes::{delete_impl, get_impl, patch_impl, post_impl, put_impl};
use routes_registration::routes_impl;
use schema::derive_schema_impl;
use use_inject::use_inject_impl;

/// Decorator for function-based API views
#[proc_macro_attribute]
pub fn api_view(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	api_view_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Decorator for ViewSet custom actions
#[proc_macro_attribute]
pub fn action(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	action_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// GET method decorator
#[proc_macro_attribute]
pub fn get(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	get_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// POST method decorator
#[proc_macro_attribute]
pub fn post(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	post_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// PUT method decorator
#[proc_macro_attribute]
pub fn put(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	put_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// PATCH method decorator
#[proc_macro_attribute]
pub fn patch(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	patch_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// DELETE method decorator
#[proc_macro_attribute]
pub fn delete(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	delete_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Permission required decorator
#[proc_macro_attribute]
pub fn permission_required(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	permission_required_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Defines installed applications with compile-time validation.
///
/// Generates an `InstalledApp` enum with variants for each application,
/// along with `Display`, `FromStr` traits and helper methods.
///
/// **Important**: This macro is for **user applications only**. Built-in framework features
/// (auth, sessions, admin, etc.) are enabled via Cargo feature flags, not through `installed_apps!`.
///
/// # Generated Code
///
/// The macro generates:
///
/// - `enum InstalledApp { ... }` - Type-safe app references with variants for each app
/// - `impl Display` - Convert enum variants to path strings
/// - `impl FromStr` - Parse path strings to enum variants
/// - `fn all_apps() -> Vec<String>` - List all app paths as strings
/// - `fn path(&self) -> &'static str` - Get app path without allocation
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt::installed_apps;
///
/// installed_apps! {
///     users: "users",
///     posts: "posts",
/// }
///
/// // Use generated enum
/// let app = InstalledApp::users;
/// println!("{}", app);  // Output: "users"
///
/// // Get all apps
/// let all = InstalledApp::all_apps();
/// assert_eq!(all, vec!["users".to_string(), "posts".to_string()]);
///
/// // Parse from string
/// use std::str::FromStr;
/// let app = InstalledApp::from_str("users")?;
/// assert_eq!(app, InstalledApp::users);
///
/// // Get path without allocation
/// assert_eq!(app.path(), "users");
/// ```
///
/// # Compile-time Validation
///
/// Framework modules (starting with `reinhardt.`) are validated at compile time.
/// Non-existent modules will cause compilation errors:
///
/// ```rust,ignore
/// installed_apps! {
///     nonexistent: "reinhardt.contrib.nonexistent",
/// }
/// // Compile error: cannot find module `nonexistent` in `contrib`
/// ```
///
/// User apps (not starting with `reinhardt.`) skip compile-time validation,
/// allowing flexible user-defined application names.
///
/// # Framework Features
///
/// **Do NOT use this macro for built-in framework features.** Instead, enable them
/// via Cargo feature flags:
///
/// ```toml
/// [dependencies]
/// reinhardt = { version = "0.1.0-alpha.1", features = ["auth", "sessions", "admin"] }
/// ```
///
/// Then import them directly:
///
/// ```rust,ignore
/// use reinhardt::auth::*;
/// use reinhardt::auth::sessions::*;
/// use reinhardt::admin::*;
/// ```
///
/// # See Also
///
/// - Module documentation in `installed_apps.rs` for detailed information about
///   generated code structure, trait implementations, and advanced usage
/// - `crates/reinhardt-apps/README.md` for comprehensive usage guide
/// - Tutorial: `docs/tutorials/en/basis/1-project-setup.md`
///
#[proc_macro]
pub fn installed_apps(input: TokenStream) -> TokenStream {
	installed_apps_impl(input.into())
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Register URL patterns for automatic discovery by the framework
///
/// This attribute macro automatically registers a function as the URL pattern
/// provider for the framework. The function will be discovered and used when
/// running management commands like `runserver`.
///
/// # Important: Single Usage Only
///
/// **Only one function per project can be annotated with `#[routes]`.**
/// If multiple `#[routes]` attributes are used, the linker will fail with a
/// "duplicate symbol" error for `__reinhardt_routes_registration_marker`.
///
/// To organize routes across multiple files, use the `.mount()` method:
///
/// ```rust,ignore
/// // In src/config/urls.rs - Only ONE #[routes] in the entire project
/// #[routes]
/// pub fn routes() -> UnifiedRouter {
///     UnifiedRouter::new()
///         .mount("/api/", api::routes())   // api::routes() returns UnifiedRouter
///         .mount("/admin/", admin::routes())  // WITHOUT #[routes] attribute
/// }
///
/// // In src/apps/api/urls.rs - NO #[routes] attribute
/// pub fn routes() -> UnifiedRouter {
///     UnifiedRouter::new()
///         .endpoint(views::list)
///         .endpoint(views::create)
/// }
/// ```
///
/// # Usage
///
/// In your `src/config/urls.rs`:
///
/// ```rust,ignore
/// use reinhardt::prelude::*;
/// use reinhardt::routes;
///
/// #[routes]
/// pub fn routes() -> UnifiedRouter {
///     UnifiedRouter::new()
///         .endpoint(views::index)
///         .mount("/api/", api::routes())
/// }
/// ```
///
/// # Notes
///
/// - The function can have any name (e.g., `routes`, `app_routes`, `url_patterns`)
/// - The return type must be `UnifiedRouter` (not `Arc<UnifiedRouter>`)
/// - The framework automatically wraps the router in `Arc`
#[proc_macro_attribute]
pub fn routes(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	routes_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Validate URL patterns at compile time
///
/// This macro validates URL pattern syntax at compile time, catching common errors
/// before they reach runtime. It supports both simple parameters and Django-style
/// typed parameters.
///
/// # Compile-time Validation
///
/// The macro will fail to compile if:
/// - Braces are not properly matched (e.g., `{id` or `id}`)
/// - Parameter names are empty (e.g., `{}`)
/// - Parameter names contain invalid characters
/// - Type specifiers are invalid (valid: `int`, `str`, `uuid`, `slug`, `path`)
/// - Django-style parameters are used outside braces (e.g., `<int:id>` instead of `{<int:id>}`)
///
/// # Supported Type Specifiers
///
/// - `int` - Integer values
/// - `str` - String values
/// - `uuid` - UUID values
/// - `slug` - Slug strings (alphanumeric, hyphens, underscores)
/// - `path` - Path segments (can include slashes)
///
#[proc_macro]
pub fn path(input: TokenStream) -> TokenStream {
	path_impl(input.into())
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Connect a receiver function to a signal automatically
///
/// This macro provides Django-style `@receiver` decorator functionality for Rust.
/// It automatically registers the function as a signal receiver at startup.
///
#[proc_macro_attribute]
pub fn receiver(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	receiver_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Automatic dependency injection macro
///
/// This macro enables FastAPI-style dependency injection using parameter attributes.
/// Parameters marked with `#[inject]` will be automatically resolved from the
/// `InjectionContext`. Can be used with any function, not just endpoints.
///
/// # Generated Code
///
/// The macro transforms the function by:
/// 1. Removing `#[inject]` parameters from the signature
/// 2. Adding an `InjectionContext` parameter
/// 3. Injecting dependencies at the start of the function
///
#[proc_macro_attribute]
pub fn use_inject(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	use_inject_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Derive macro for type-safe field lookups
///
/// Automatically generates field accessor methods for models, enabling
/// compile-time validated field lookups.
///
/// # Generated Methods
///
/// For each field in the struct, the macro generates a static method that
/// returns a `Field<Model, FieldType>`. The field type determines which
/// lookup methods are available:
///
/// - String fields: `lower()`, `upper()`, `trim()`, `contains()`, etc.
/// - Numeric fields: `abs()`, `ceil()`, `floor()`, `round()`
/// - DateTime fields: `year()`, `month()`, `day()`, `hour()`, etc.
/// - All fields: `eq()`, `ne()`, `gt()`, `gte()`, `lt()`, `lte()`
///
#[proc_macro_derive(QueryFields)]
pub fn derive_query_fields(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as syn::DeriveInput);

	derive_query_fields_impl(input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Derive macro for automatic OpenAPI schema generation
///
/// Automatically implements the `ToSchema` trait for structs and enums,
/// generating OpenAPI 3.0 schemas from Rust type definitions.
///
/// # Supported Types
///
/// - Primitives: `String`, `i32`, `i64`, `f32`, `f64`, `bool`
/// - `Option<T>`: Makes fields optional in the schema
/// - `Vec<T>`: Generates array schemas
/// - Custom types implementing `ToSchema`
///
/// # Features
///
/// - Automatic field metadata extraction
/// - Documentation comments become field descriptions
/// - Required/optional field detection
/// - Nested schema support
/// - Enum variant handling
///
#[proc_macro_derive(Schema)]
pub fn derive_schema(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as syn::DeriveInput);

	derive_schema_impl(input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Attribute macro for injectable factory/provider functions and structs
///
/// This macro can be applied to both functions and structs to enable dependency injection.
///
/// # Field Attributes (Struct Only)
///
/// All struct fields must have either `#[inject]` or `#[no_inject]` attribute:
///
/// - **`#[inject]`**: Inject this field from the DI container
/// - **`#[inject(cache = false)]`**: Inject without caching
/// - **`#[inject(scope = Singleton)]`**: Use singleton scope
/// - **`#[no_inject(default = Default)]`**: Initialize with `Default::default()`
/// - **`#[no_inject(default = value)]`**: Initialize with specific value
/// - **`#[no_inject]`**: Initialize with `None` (field must be `Option<T>`)
///
/// # Restrictions
///
/// **For functions:**
/// - Function must have an explicit return type
/// - All parameters must be marked with `#[inject]`
///
/// **For structs:**
/// - Struct must have named fields
/// - All fields must have either `#[inject]` or `#[no_inject]` attribute
/// - `#[no_inject]` without default value requires field type to be `Option<T>`
/// - Struct must be `Clone` (required by `Injectable` trait)
/// - All `#[inject]` field types must implement `Injectable`
///
#[proc_macro_attribute]
pub fn injectable(_args: TokenStream, input: TokenStream) -> TokenStream {
	// Try to parse as ItemFn first
	if let Ok(item_fn) = syn::parse::<ItemFn>(input.clone()) {
		return injectable_fn_impl(proc_macro2::TokenStream::new(), item_fn)
			.unwrap_or_else(|e| e.to_compile_error())
			.into();
	}

	// Try to parse as ItemStruct
	if let Ok(item_struct) = syn::parse::<ItemStruct>(input.clone()) {
		// Convert ItemStruct to DeriveInput for compatibility
		let derive_input = syn::DeriveInput {
			attrs: item_struct.attrs,
			vis: item_struct.vis,
			ident: item_struct.ident,
			generics: item_struct.generics,
			data: syn::Data::Struct(syn::DataStruct {
				struct_token: item_struct.struct_token,
				fields: item_struct.fields,
				semi_token: item_struct.semi_token,
			}),
		};

		return injectable_struct_impl(derive_input)
			.unwrap_or_else(|e| e.to_compile_error())
			.into();
	}

	// Neither ItemFn nor ItemStruct
	syn::Error::new(
		proc_macro2::Span::call_site(),
		"#[injectable] can only be applied to functions or structs",
	)
	.to_compile_error()
	.into()
}

/// Attribute macro for Django-style model definition with automatic derive
///
/// Automatically adds `#[derive(Model)]` and keeps the `#[model(...)]` attribute.
/// This provides a cleaner syntax by eliminating the need to explicitly write
/// `#[derive(Model)]` on every model struct.
///
/// # Model Attributes
///
/// Same as `#[derive(Model)]`. See [`derive_model`] for details.
///
#[proc_macro_attribute]
pub fn model(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemStruct);

	model_attribute_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Derive macro for automatic Model implementation and migration registration
///
/// Automatically implements the `Model` trait and registers the model with the global
/// ModelRegistry for automatic migration generation.
///
/// # Model Attributes
///
/// - `app_label`: Application label (default: "default")
/// - `table_name`: Database table name (default: struct name in snake_case)
/// - `constraints`: List of unique constraints (e.g., `unique(fields = ["field1", "field2"], name = "name")`)
///
/// # Field Attributes
///
/// - `primary_key`: Mark field as primary key (required for exactly one field)
/// - `max_length`: Maximum length for String fields (required for String)
/// - `null`: Allow NULL values (default: inferred from `Option<T>`)
/// - `blank`: Allow blank values in forms
/// - `unique`: Enforce uniqueness constraint
/// - `default`: Default value
/// - `db_column`: Custom database column name
/// - `editable`: Whether field is editable (default: true)
///
/// # Supported Types
///
/// - `i32` → IntegerField
/// - `i64` → BigIntegerField
/// - `String` → CharField (requires max_length)
/// - `bool` → BooleanField
/// - `DateTime<Utc>` → DateTimeField
/// - `Date` → DateField
/// - `Time` → TimeField
/// - `f32`, `f64` → FloatField
/// - `Option<T>` → Sets null=true automatically
///
/// # Requirements
///
/// - Struct must have named fields
/// - Struct must implement `Serialize` and `Deserialize`
/// - Exactly one field must be marked with `primary_key = true`
/// - String fields must specify `max_length`
///
#[proc_macro_derive(Model, attributes(model, model_config, field, rel, fk_id_field))]
pub fn derive_model(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as syn::DeriveInput);

	model_derive_impl(input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Derive macro for automatic OrmReflectable implementation
///
/// Automatically implements the `OrmReflectable` trait for structs,
/// enabling reflection-based field and relationship access for association proxies.
///
/// ## Type Inference
///
/// Fields are automatically classified based on their types:
/// - `Vec<T>` → Collection relationship
/// - `Option<T>` (where T is non-primitive) → Scalar relationship
/// - Primitive types (i32, String, etc.) → Regular fields
///
/// ## Attributes
///
/// Override automatic inference with explicit attributes:
///
/// - `#[orm_field(type = "Integer")]` - Mark as regular field with specific type
/// - `#[orm_relationship(type = "collection")]` - Mark as collection relationship
/// - `#[orm_relationship(type = "scalar")]` - Mark as scalar relationship
/// - `#[orm_ignore]` - Exclude field from reflection
///
/// ## Supported Field Types
///
/// - **Integer**: i8, i16, i32, i64, i128, u8, u16, u32, u64, u128
/// - **Float**: f32, f64
/// - **Boolean**: bool
/// - **String**: String, str
///
#[proc_macro_derive(OrmReflectable, attributes(orm_field, orm_relationship, orm_ignore))]
pub fn derive_orm_reflectable(input: TokenStream) -> TokenStream {
	orm_reflectable_derive_impl(input)
}

/// Attribute macro for Django-style AppConfig definition with automatic derive
///
/// Automatically adds `#[derive(AppConfig)]` and keeps the `#[app_config(...)]` attribute.
/// This provides a cleaner syntax by eliminating the need to explicitly write
/// `#[derive(AppConfig)]` on every app config struct.
///
/// # Example
///
/// ```rust,ignore
/// #[app_config(name = "hello", label = "hello")]
/// pub struct HelloConfig;
///
/// // Generates a config() method:
/// let config = HelloConfig::config();
/// assert_eq!(config.name, "hello");
/// assert_eq!(config.label, "hello");
/// ```
///
/// # Attributes
///
/// - `name`: Application name (required, string literal)
/// - `label`: Application label (required, string literal)
/// - `verbose_name`: Verbose name (optional, string literal)
///
/// # Note
///
/// Direct use of `#[derive(AppConfig)]` is not allowed. Always use
/// `#[app_config(...)]` attribute macro instead.
///
#[proc_macro_attribute]
pub fn app_config(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemStruct);

	app_config_attribute_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Derive macro for automatic AppConfig factory method generation
///
/// **Note**: Do not use this derive macro directly. Use `#[app_config(...)]`
/// attribute macro instead.
///
/// This derive macro is invoked automatically by the `#[app_config(...)]` attribute.
/// Direct use will result in a compile error.
///
#[proc_macro_derive(AppConfig, attributes(app_config, app_config_internal))]
pub fn derive_app_config(input: TokenStream) -> TokenStream {
	app_config_derive::derive(input)
}

/// Collect migrations and register them with the global registry
///
/// This macro generates a `MigrationProvider` implementation and automatically
/// registers it with the global migration registry using `linkme::distributed_slice`.
///
/// # Requirements
///
/// - Each migration module must export a `migration()` function returning `Migration`
/// - The crate must have `reinhardt-migrations` and `linkme` as dependencies
///
#[proc_macro]
pub fn collect_migrations(input: TokenStream) -> TokenStream {
	collect_migrations::collect_migrations_impl(input.into())
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Attribute macro for ModelAdmin configuration
///
/// Automatically implements the `ModelAdmin` trait for a struct with compile-time
/// field validation against the specified model type.
///
/// # Attributes
///
/// ## Required
///
/// - `for = ModelType` - The model type to validate fields against
/// - `name = "ModelName"` - The display name for the model
///
/// ## Optional
///
/// - `list_display = [field1, field2, ...]` - Fields to display in list view (default: `[id]`)
/// - `list_filter = [field1, field2, ...]` - Fields for filtering (default: `[]`)
/// - `search_fields = [field1, field2, ...]` - Fields for search (default: `[]`)
/// - `fields = [field1, field2, ...]` - Fields to display in forms (default: all)
/// - `readonly_fields = [field1, field2, ...]` - Read-only fields (default: `[]`)
/// - `ordering = [(field1, asc/desc), ...]` - Default ordering (default: `[(id, desc)]`)
/// - `list_per_page = N` - Items per page (default: site default)
///
/// # Compile-time Field Validation
///
/// All field names are validated at compile time against the model's `field_xxx()` methods.
/// If a field doesn't exist, compilation will fail with an error.
///
/// # Generated Code
///
/// The macro generates:
/// 1. The struct definition
/// 2. Compile-time field validation code
/// 3. `ModelAdmin` trait implementation with `#[async_trait]`
///
#[proc_macro_attribute]
pub fn admin(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemStruct);

	admin_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}
