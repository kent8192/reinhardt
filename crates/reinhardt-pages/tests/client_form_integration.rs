#![cfg(not(target_arch = "wasm32"))]

use std::cell::Cell;

use reinhardt_core::reactive::ReactiveScope;
use reinhardt_core::validators::{Validate, ValidationError, ValidationErrors};
use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_pages::server_fn::server_fn;
use reinhardt_pages::{
	ClientForm, ClientFormChoiceSource, ClientFormChoices, FieldError, MutationDispatchOutcome,
	ResetOnDeps, UseFormAsyncSubmitOutcome, use_form,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, ClientFormChoices)]
#[serde(rename_all = "snake_case")]
enum ProviderMode {
	#[default]
	Fake,
	LiveApi,
	HTTPStatus,
	#[serde(skip)]
	Archived,
}

#[derive(Clone, Default, Debug, PartialEq, ClientFormChoices)]
#[serde(
	rename_all = "snake_case",
	crate = "serde",
	bound = "",
	deny_unknown_fields
)]
enum IgnoredContainerProviderMode {
	#[default]
	LiveApi,
	TestHarness,
}

#[derive(Clone, Debug, PartialEq, ClientForm)]
#[client_form(validate)]
struct ProjectRequest {
	name: String,
	title: Option<String>,
	retry_count: i32,
	optional_retry_count: Option<i32>,
	active: bool,
	optional_active: Option<bool>,
	provider_mode: ProviderMode,
	optional_mode: Option<ProviderMode>,
	#[client_form(skip)]
	tenant_id: Option<String>,
	#[client_form(skip)]
	revision: u32,
	#[serde(skip)]
	server_token: String,
}

impl Validate for ProjectRequest {
	fn validate(&self) -> Result<(), ValidationErrors> {
		let mut errors = ValidationErrors::new();
		if self.name.trim().is_empty() {
			errors.add("name", ValidationError::TooShort { length: 0, min: 1 });
		}
		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}

#[test]
fn client_form_defaults_and_request_conversion() {
	ReactiveScope::run(|| {
		let form = ProjectRequestClientForm::new().with_defaults(ProjectRequest {
			name: "demo".to_string(),
			title: Some("Seed".to_string()),
			retry_count: 2,
			optional_retry_count: Some(5),
			active: true,
			optional_active: Some(false),
			provider_mode: ProviderMode::LiveApi,
			optional_mode: Some(ProviderMode::Fake),
			tenant_id: Some("tenant-a".to_string()),
			revision: 7,
			server_token: "token-a".to_string(),
		});
		let runtime = use_form(&form).build();

		assert_eq!(
			runtime.watch_field::<String>(form.name_field()).get(),
			"demo"
		);
		assert_eq!(
			runtime
				.watch_field::<ProviderMode>(form.provider_mode_field())
				.get(),
			ProviderMode::LiveApi
		);

		runtime.set_value(ProjectRequestClientFormField::Title, "   ".to_string());
		let request = ProjectRequestClientForm::to_request(&runtime);

		assert_eq!(request.title, None);
		assert_eq!(request.retry_count, 2);
		assert_eq!(request.optional_retry_count, Some(5));
		assert!(request.active);
		assert_eq!(request.optional_active, Some(false));
		assert_eq!(request.optional_mode, Some(ProviderMode::Fake));
		assert_eq!(request.tenant_id.as_deref(), Some("tenant-a"));
		assert_eq!(request.revision, 7);
		assert_eq!(request.server_token, "token-a");
	});
}

#[test]
fn client_form_enum_choice_metadata_uses_serialized_values() {
	ReactiveScope::run(|| {
		let form = ProjectRequestClientForm::new();
		let choices = form.provider_mode_choices();

		assert_eq!(choices.len(), 3);
		assert_eq!(choices[0].serialized_value, "fake");
		assert_eq!(choices[0].label, "fake");
		assert_eq!(choices[1].serialized_value, "live_api");
		assert_eq!(choices[1].label, "live_api");
		assert_eq!(choices[2].serialized_value, "h_t_t_p_status");
		assert_eq!(choices[2].label, "h_t_t_p_status");
		assert_eq!(ProviderMode::client_form_default(), ProviderMode::Fake);
		assert!(matches!(ProviderMode::Archived, ProviderMode::Archived));
	});
}

#[test]
fn client_form_choices_ignore_non_serialization_container_options() {
	let choices = IgnoredContainerProviderMode::client_form_choices();

	assert_eq!(choices.len(), 2);
	assert_eq!(choices[0].serialized_value, "live_api");
	assert_eq!(choices[1].serialized_value, "test_harness");
}

#[test]
fn client_form_reconcile_refreshes_skipped_defaults() {
	ReactiveScope::run(|| {
		let form = ProjectRequestClientForm::new().with_defaults(ProjectRequest {
			name: "demo".to_string(),
			title: Some("Seed".to_string()),
			retry_count: 2,
			optional_retry_count: Some(5),
			active: true,
			optional_active: Some(false),
			provider_mode: ProviderMode::LiveApi,
			optional_mode: Some(ProviderMode::Fake),
			tenant_id: Some("tenant-a".to_string()),
			revision: 7,
			server_token: "token-a".to_string(),
		});
		let runtime = use_form(&form)
			.deps(0_u8)
			.reset_on_deps(ResetOnDeps::KeepDirtyValues)
			.build();
		runtime.set_value(ProjectRequestClientFormField::Name, "edited".to_string());

		let refreshed = ProjectRequestClientForm::new().with_defaults(ProjectRequest {
			name: "server".to_string(),
			title: Some("Server".to_string()),
			retry_count: 3,
			optional_retry_count: Some(8),
			active: false,
			optional_active: Some(true),
			provider_mode: ProviderMode::Fake,
			optional_mode: None,
			tenant_id: Some("tenant-b".to_string()),
			revision: 8,
			server_token: "token-b".to_string(),
		});
		runtime.reconcile_from(&refreshed, 1_u8);
		let request = ProjectRequestClientForm::to_request(&runtime);

		assert_eq!(request.name, "edited");
		assert_eq!(request.title.as_deref(), Some("Server"));
		assert_eq!(request.tenant_id.as_deref(), Some("tenant-b"));
		assert_eq!(request.revision, 8);
		assert_eq!(request.server_token, "token-b");
	});
}

#[derive(Clone, Debug, PartialEq)]
struct TenantId(&'static str);

fn tenant_default() -> TenantId {
	TenantId("default-tenant")
}

#[derive(Clone, Debug, PartialEq, ClientForm)]
struct CustomTenantDefaultRequest {
	name: String,
	#[serde(skip_serializing, default = "tenant_default")]
	tenant: TenantId,
}

#[test]
fn client_form_preserves_custom_hidden_default_values_from_defaults() {
	ReactiveScope::run(|| {
		let form =
			CustomTenantDefaultRequestClientForm::new().with_defaults(CustomTenantDefaultRequest {
				name: "demo".to_string(),
				tenant: TenantId("custom-tenant"),
			});
		let runtime = use_form(&form).build();
		let request = CustomTenantDefaultRequestClientForm::to_request(&runtime);

		assert_eq!(request.tenant.0, "custom-tenant");
	});
}

#[derive(Clone, Debug, PartialEq, ClientForm)]
struct SelfTenantDefaultRequest {
	name: String,
	#[serde(skip_serializing, default = "Self::tenant_default")]
	tenant: TenantId,
}

impl SelfTenantDefaultRequest {
	fn tenant_default() -> TenantId {
		TenantId("default-tenant")
	}
}

#[test]
fn client_form_resolves_self_hidden_default_against_dto() {
	ReactiveScope::run(|| {
		let form = SelfTenantDefaultRequestClientForm::new();
		let runtime = use_form(&form).build();
		let request = SelfTenantDefaultRequestClientForm::to_request(&runtime);

		assert_eq!(request.tenant.0, "default-tenant");
	});
}

#[test]
fn client_form_validation_maps_dto_field_errors() {
	ReactiveScope::run(|| {
		let form = ProjectRequestClientForm::new();
		let runtime = use_form(&form).build();

		let result = runtime.trigger();

		assert!(result.is_err());
		assert_eq!(
			runtime
				.get_field_state(ProjectRequestClientFormField::Name)
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Length too short: 0 (minimum: 1)")
		);
	});
}

#[derive(Clone, Default, Debug, PartialEq, ClientForm)]
#[client_form(validate)]
struct RawValidationRequest {
	r#type: String,
}

impl Validate for RawValidationRequest {
	fn validate(&self) -> Result<(), ValidationErrors> {
		let mut errors = ValidationErrors::new();
		errors.add(
			"r#type",
			ValidationError::PatternMismatch("expected raw field value".to_string()),
		);
		Err(errors)
	}
}

#[test]
fn client_form_validation_maps_raw_dto_field_errors() {
	ReactiveScope::run(|| {
		let form = RawValidationRequestClientForm::new();
		let runtime = use_form(&form).build();

		let result = runtime.trigger();

		assert!(result.is_err());
		assert_eq!(
			runtime
				.get_field_state(RawValidationRequestClientFormField::Type)
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Pattern mismatch: expected raw field value")
		);
	});
}

#[derive(Clone, Default, Debug, PartialEq, ClientForm)]
struct RenamedServerErrorRequest {
	#[serde(rename = "displayName")]
	display_name: String,
	r#type: String,
}

#[test]
fn client_form_routes_serialized_and_raw_server_field_names() {
	ReactiveScope::run(|| {
		let form =
			RenamedServerErrorRequestClientForm::new().with_defaults(RenamedServerErrorRequest {
				display_name: "Ada".to_string(),
				r#type: "profile".to_string(),
			});
		let runtime = use_form(&form).build();
		let error = ServerFnError::validation_with_message(
			"Please correct the submitted values",
			[
				("displayName", "Display name is already used"),
				("type", "Type is unsupported"),
				("missing_field", "Unknown field"),
			],
		);

		runtime.apply_server_error(&error);

		assert_eq!(
			runtime
				.get_field_state(RenamedServerErrorRequestClientFormField::DisplayName)
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Display name is already used")
		);
		assert_eq!(
			runtime
				.get_field_state(RenamedServerErrorRequestClientFormField::Type)
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Type is unsupported")
		);
		assert_eq!(
			runtime.form_state().form_error.get(),
			Some("Please correct the submitted values\nmissing_field: Unknown field".to_string())
		);
	});
}

#[derive(Clone, Default, Debug, PartialEq, ClientForm)]
struct DirectionalRenameServerErrorRequest {
	#[serde(rename(serialize = "wireDisplayName", deserialize = "display_name"))]
	display_name: String,
	r#type: String,
}

#[test]
fn client_form_routes_directional_serialize_rename_and_raw_field_names() {
	ReactiveScope::run(|| {
		let form = DirectionalRenameServerErrorRequestClientForm::new().with_defaults(
			DirectionalRenameServerErrorRequest {
				display_name: "Ada".to_string(),
				r#type: "profile".to_string(),
			},
		);
		let runtime = use_form(&form).build();
		let error = ServerFnError::validation_with_message(
			"Please correct the submitted values",
			[
				("wireDisplayName", "Display name is already used"),
				("type", "Type is unsupported"),
			],
		);

		runtime.apply_server_error(&error);

		assert_eq!(
			runtime
				.get_field_state(DirectionalRenameServerErrorRequestClientFormField::DisplayName)
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Display name is already used")
		);
		assert_eq!(
			runtime
				.get_field_state(DirectionalRenameServerErrorRequestClientFormField::Type)
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Type is unsupported")
		);
		assert_eq!(runtime.form_state().form_error.get(), None);
	});
}

#[derive(Clone, Default, Debug, PartialEq, ClientForm)]
#[serde(rename_all = "camelCase")]
struct CamelCaseServerErrorRequest {
	display_name: String,
}

#[derive(Clone, Default, Debug, PartialEq, ClientForm)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
struct ScreamingKebabServerErrorRequest {
	display_name: String,
}

#[test]
fn client_form_routes_serde_rename_all_serialized_field_names() {
	ReactiveScope::run(|| {
		let camel_case_form = CamelCaseServerErrorRequestClientForm::new().with_defaults(
			CamelCaseServerErrorRequest {
				display_name: "Ada".to_string(),
			},
		);
		let camel_case_runtime = use_form(&camel_case_form).build();
		let camel_case_error = ServerFnError::validation_with_message(
			"Please correct the submitted values",
			[("displayName", "Display name is already used")],
		);

		camel_case_runtime.apply_server_error(&camel_case_error);

		assert_eq!(
			camel_case_runtime
				.get_field_state(CamelCaseServerErrorRequestClientFormField::DisplayName)
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Display name is already used")
		);

		let screaming_kebab_form = ScreamingKebabServerErrorRequestClientForm::new().with_defaults(
			ScreamingKebabServerErrorRequest {
				display_name: "Grace".to_string(),
			},
		);
		let screaming_kebab_runtime = use_form(&screaming_kebab_form).build();
		let screaming_kebab_error = ServerFnError::validation_with_message(
			"Please correct the submitted values",
			[("DISPLAY-NAME", "Display name is already used")],
		);

		screaming_kebab_runtime.apply_server_error(&screaming_kebab_error);

		assert_eq!(
			screaming_kebab_runtime
				.get_field_state(ScreamingKebabServerErrorRequestClientFormField::DisplayName)
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Display name is already used")
		);
	});
}

thread_local! {
	static SUBMIT_CALL_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ClientForm)]
#[client_form(server_fn = submit_project, validate)]
struct SubmitProjectRequest {
	name: String,
}

impl Validate for SubmitProjectRequest {
	fn validate(&self) -> Result<(), ValidationErrors> {
		let mut errors = ValidationErrors::new();
		if self.name.is_empty() {
			errors.add("name", ValidationError::TooShort { length: 0, min: 1 });
		}
		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmitProjectResponse {
	name: String,
}

fn assert_submit_future<Fut>(value: Fut) -> Fut
where
	Fut: std::future::Future<
			Output = Result<UseFormAsyncSubmitOutcome<SubmitProjectResponse>, ServerFnError>,
		>,
{
	value
}

#[server_fn]
async fn submit_project(
	request: crate::SubmitProjectRequest,
) -> Result<SubmitProjectResponse, ServerFnError> {
	SUBMIT_CALL_COUNT.with(|count| count.set(count.get() + 1));
	Ok(SubmitProjectResponse { name: request.name })
}

#[tokio::test]
async fn client_form_server_submit_blocks_validation_failure() {
	SUBMIT_CALL_COUNT.with(|count| count.set(0));
	let scope = ReactiveScope::new();
	let runtime = scope.enter(|| {
		let form = SubmitProjectRequestClientForm::new();
		use_form(&form).build()
	});

	let outcome = runtime
		.submit_server_fn(|| {
			let request = SubmitProjectRequestClientForm::to_request(&runtime);
			async move { submit_project(request).await }
		})
		.await
		.expect("validation outcome");

	assert_eq!(outcome, UseFormAsyncSubmitOutcome::ValidationFailed);
	assert_eq!(SUBMIT_CALL_COUNT.with(Cell::get), 0);
}

#[test]
fn client_form_server_mutation_is_unsupported_on_native() {
	SUBMIT_CALL_COUNT.with(|count| count.set(0));
	let scope = ReactiveScope::new();
	scope.enter(|| {
		let form = SubmitProjectRequestClientForm::new().with_defaults(SubmitProjectRequest {
			name: "demo".to_string(),
		});
		let runtime = use_form(&form).build();
		let mutation = form.server_mutation(&runtime).build();
		let submit_future = assert_submit_future(form.submit(&runtime));

		assert_eq!(
			mutation.dispatch(),
			MutationDispatchOutcome::UnsupportedTarget
		);
		assert_eq!(mutation.result(), None);
		assert_eq!(mutation.error(), None);
		assert!(!runtime.form_state().is_submitting.get());
		assert!(!runtime.form_state().is_submit_successful.get());
		assert_eq!(runtime.form_state().form_error.get(), None);
		assert_eq!(SUBMIT_CALL_COUNT.with(Cell::get), 0);

		drop(submit_future);
	});
}

#[tokio::test]
async fn client_form_submit_server_fn_calls_server_function_on_success() {
	SUBMIT_CALL_COUNT.with(|count| count.set(0));
	let scope = ReactiveScope::new();
	let runtime = scope.enter(|| {
		let form = SubmitProjectRequestClientForm::new().with_defaults(SubmitProjectRequest {
			name: "demo".to_string(),
		});
		use_form(&form).build()
	});

	let outcome = runtime
		.submit_server_fn(|| {
			let request = SubmitProjectRequestClientForm::to_request(&runtime);
			async move { submit_project(request).await }
		})
		.await
		.expect("submit succeeds");

	assert_eq!(
		outcome,
		UseFormAsyncSubmitOutcome::Submitted(SubmitProjectResponse {
			name: "demo".to_string()
		})
	);
	assert_eq!(SUBMIT_CALL_COUNT.with(Cell::get), 1);
	assert!(runtime.form_state().is_submit_successful.get());
}
