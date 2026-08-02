use reinhardt_pages::server_fn::{
	FieldError, FieldErrors, Page, PageRequest, ServerFnListQuery, ServerFnResource,
	ServerFnSetError, ValidatedPageRequest,
};
#[cfg(all(
	feature = "model-server-fnset",
	not(all(target_family = "wasm", target_os = "unknown"))
))]
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;

#[derive(Clone, Copy)]
struct ArticleListQuery {
	limit: Option<u32>,
	offset: u64,
}

impl ServerFnListQuery for ArticleListQuery {
	fn page_request(&self) -> PageRequest {
		PageRequest {
			limit: self.limit,
			offset: self.offset,
		}
	}
}

struct ArticleResource;

impl ServerFnResource for ArticleResource {
	type Lookup = i64;
	type Read = String;
	type Create = String;
	type Update = String;
	type Patch = String;
	type ListQuery = ArticleListQuery;
}

fn assert_cross_target_resource<R: ServerFnResource>() {}

fn assert_wire_contract<T>()
where
	T: Serialize + DeserializeOwned,
{
}

#[test]
fn server_fn_resource_contract_is_cross_target() {
	assert_cross_target_resource::<ArticleResource>();
	assert_wire_contract::<FieldError>();
	assert_wire_contract::<FieldErrors>();
	assert_wire_contract::<ServerFnSetError>();
	assert_wire_contract::<PageRequest>();
	assert_wire_contract::<ValidatedPageRequest>();
	assert_wire_contract::<Page<String>>();
}

#[test]
fn pagination_wire_types_have_stable_json_shapes() {
	let request = PageRequest {
		limit: None,
		offset: 7,
	};
	let page = Page {
		items: vec!["first".to_owned(), "second".to_owned()],
		total: 9,
		limit: 2,
		offset: 4,
	};

	assert_eq!(
		serde_json::to_value(request).unwrap(),
		json!({ "limit": null, "offset": 7 })
	);
	assert_eq!(
		serde_json::from_value::<PageRequest>(json!({ "limit": 10, "offset": 3 })).unwrap(),
		PageRequest {
			limit: Some(10),
			offset: 3,
		}
	);
	assert_eq!(
		serde_json::to_value(&page).unwrap(),
		json!({
			"items": ["first", "second"],
			"total": 9,
			"limit": 2,
			"offset": 4
		})
	);
	assert_eq!(
		serde_json::from_value::<Page<String>>(serde_json::to_value(&page).unwrap()).unwrap(),
		page
	);
}

#[test]
fn structured_errors_have_stable_json_shapes() {
	let errors = FieldErrors::from([(
		"limit".to_owned(),
		vec![FieldError {
			code: "out_of_range".to_owned(),
			message: "limit must be between 1 and 100".to_owned(),
		}],
	)]);
	let validation = ServerFnSetError::Validation(errors.clone());
	let conflict = ServerFnSetError::Conflict {
		code: "stale_version".to_owned(),
		message: "The resource changed".to_owned(),
	};

	assert_eq!(
		serde_json::to_value(&errors).unwrap(),
		json!({
			"limit": [{
				"code": "out_of_range",
				"message": "limit must be between 1 and 100"
			}]
		})
	);
	assert_eq!(
		serde_json::to_value(validation).unwrap(),
		json!({
			"Validation": {
				"limit": [{
					"code": "out_of_range",
					"message": "limit must be between 1 and 100"
				}]
			}
		})
	);
	assert_eq!(
		serde_json::to_value(ServerFnSetError::Unauthenticated).unwrap(),
		json!("Unauthenticated")
	);
	assert_eq!(
		serde_json::to_value(conflict).unwrap(),
		json!({
			"Conflict": {
				"code": "stale_version",
				"message": "The resource changed"
			}
		})
	);

	let decoded = serde_json::from_value::<ServerFnSetError>(json!({
		"NotFound": { "resource": "article" }
	}))
	.unwrap();
	let ServerFnSetError::NotFound { resource } = decoded else {
		panic!("expected a not-found error");
	};
	assert_eq!(resource, "article");
}

#[test]
fn pagination_uses_default_limit_and_preserves_offset() {
	let query = ArticleListQuery {
		limit: None,
		offset: 13,
	};

	let validated = query.page_request().validate().unwrap();

	assert_eq!(validated.limit, 25);
	assert_eq!(validated.offset, 13);
}

#[test]
fn pagination_accepts_inclusive_limit_bounds() {
	for limit in [1, 100] {
		let validated = PageRequest {
			limit: Some(limit),
			offset: 0,
		}
		.validate()
		.unwrap();

		assert_eq!(validated.limit, limit);
	}
}

#[test]
fn pagination_rejects_limits_outside_bounds() {
	for limit in [0, 101] {
		let error = PageRequest {
			limit: Some(limit),
			offset: 0,
		}
		.validate()
		.unwrap_err();

		let ServerFnSetError::Validation(fields) = error else {
			panic!("expected pagination validation error");
		};
		let limit_errors = fields.get("limit").unwrap();
		assert_eq!(limit_errors.len(), 1);
		assert_eq!(limit_errors[0].code, "out_of_range");
	}
}

#[cfg(all(
	feature = "model-server-fnset",
	not(all(target_family = "wasm", target_os = "unknown"))
))]
#[derive(Clone, Serialize, Deserialize)]
struct ArticleModel {
	id: Option<i64>,
}

#[cfg(all(
	feature = "model-server-fnset",
	not(all(target_family = "wasm", target_os = "unknown"))
))]
#[derive(Clone)]
struct ArticleFields;

#[cfg(all(
	feature = "model-server-fnset",
	not(all(target_family = "wasm", target_os = "unknown"))
))]
impl reinhardt_db::orm::FieldSelector for ArticleFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

#[cfg(all(
	feature = "model-server-fnset",
	not(all(target_family = "wasm", target_os = "unknown"))
))]
impl reinhardt_db::orm::Model for ArticleModel {
	type PrimaryKey = i64;
	type Fields = ArticleFields;
	type Objects = reinhardt_db::orm::Manager<Self>;

	fn table_name() -> &'static str {
		"articles"
	}

	fn new_fields() -> Self::Fields {
		ArticleFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

#[cfg(all(
	feature = "model-server-fnset",
	not(all(target_family = "wasm", target_os = "unknown"))
))]
#[cfg(all(
	feature = "model-server-fnset",
	not(all(target_family = "wasm", target_os = "unknown"))
))]
#[async_trait::async_trait]
impl reinhardt_pages::server_fn::ModelServerFnResource for ArticleResource {
	type Model = ArticleModel;
	type Policy = reinhardt_pages::server_fn::AllowAllPolicy;

	fn lookup_field() -> reinhardt_db::orm::UniqueFieldRef<Self::Model, Self::Lookup> {
		// SAFETY: The handwritten test model declares `id` as its unique primary key.
		unsafe { reinhardt_db::orm::UniqueFieldRef::from_model_field_with_names("id", "id") }
	}

	async fn to_read(
		model: &Self::Model,
		_executor: Option<&mut dyn reinhardt_db::orm::TransactionExecutor>,
	) -> Result<Self::Read, ServerFnSetError> {
		Ok(model.id.unwrap_or_default().to_string())
	}
}

#[cfg(all(
	feature = "model-server-fnset",
	not(all(target_family = "wasm", target_os = "unknown"))
))]
#[test]
fn model_resource_contract_requires_explicit_policy() {
	use reinhardt_pages::server_fn::{ModelServerFnResource, ServerFnSetPolicy};

	fn assert_policy<R>()
	where
		R: ModelServerFnResource,
		R::Policy: ServerFnSetPolicy<R>,
	{
	}

	assert_policy::<ArticleResource>();
}

#[cfg(all(
	feature = "model-server-fnset",
	not(all(target_family = "wasm", target_os = "unknown"))
))]
#[tokio::test(flavor = "current_thread")]
async fn allow_all_principal_uses_standard_request_extraction() {
	use reinhardt_di::params::{FromRequest, ParamContext};
	use reinhardt_pages::server_fn::AllowAllPrincipal;

	let request = reinhardt_http::Request::builder().uri("/").build().unwrap();
	let context = ParamContext::new();

	let principal = AllowAllPrincipal::from_request(&request, &context)
		.await
		.unwrap();

	assert_eq!(principal, AllowAllPrincipal);
}
