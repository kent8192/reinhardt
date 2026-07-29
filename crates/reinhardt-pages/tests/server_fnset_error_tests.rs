#![cfg(all(feature = "model-server-fnset", not(target_arch = "wasm32")))]

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use hyper::{Method, header};
use reinhardt_db::{
	backends::DatabaseConnection as BackendsConnection,
	orm::{DatabaseConnection, DatabaseConnectionLease},
};
use reinhardt_di::{InjectionContext, SingletonScope};
use reinhardt_http::Request;
use reinhardt_pages::server_fn::{
	ServerFnError, ServerFnErrorKind, ServerFnHandler, ServerFnMetadata, ServerFnRegistration,
	ServerFnSetError,
};

// The shared compile fixture exports its contract types for macro expansion,
// while this integration-test crate only names the generated action markers.
#[allow(dead_code, unreachable_pub)]
mod fixture {
	use reinhardt_db::orm::QuerySet;
	use reinhardt_pages::server_fn::{ServerFnSetAction, ServerFnSetPolicy};

	include!("ui/server_fnset/pass/model_crud_types.inc");

	pub struct RejectingPrincipal;

	#[async_trait::async_trait]
	impl reinhardt_di::params::FromRequest for RejectingPrincipal {
		async fn from_request(
			_request: &reinhardt_http::Request,
			_context: &reinhardt_di::params::ParamContext,
		) -> reinhardt_di::params::ParamResult<Self> {
			Err(reinhardt_di::params::ParamError::Authentication(
				"token=top-secret".to_string(),
			))
		}
	}

	pub struct RejectingPolicy;

	#[async_trait::async_trait]
	impl ServerFnSetPolicy<RejectingResource> for RejectingPolicy {
		type Principal = RejectingPrincipal;

		async fn authorize_action(
			_principal: &Self::Principal,
			_action: ServerFnSetAction,
			_executor: Option<&mut dyn TransactionExecutor>,
		) -> Result<(), ServerFnSetError> {
			Ok(())
		}

		async fn scope_query(
			_principal: &Self::Principal,
			query: QuerySet<Article>,
			_executor: Option<&mut dyn TransactionExecutor>,
		) -> Result<QuerySet<Article>, ServerFnSetError> {
			Ok(query)
		}

		async fn authorize_object(
			_principal: &Self::Principal,
			_action: ServerFnSetAction,
			_object: &Article,
			_executor: Option<&mut dyn TransactionExecutor>,
		) -> Result<(), ServerFnSetError> {
			Ok(())
		}
	}

	pub struct RejectingResource;

	impl ServerFnResource for RejectingResource {
		type Lookup = i64;
		type Read = ArticleDto;
		type Create = CreateArticle;
		type Update = UpdateArticle;
		type Patch = PatchArticle;
		type ListQuery = ListQuery;
	}

	#[async_trait::async_trait]
	impl ModelServerFnResource for RejectingResource {
		type Model = Article;
		type Policy = RejectingPolicy;

		fn lookup_field() -> UniqueFieldRef<Article, i64> {
			// SAFETY: The handwritten test model declares `id` as its unique primary key.
			unsafe { UniqueFieldRef::from_model_field("id", "id") }
		}

		async fn to_read(
			model: &Article,
			_executor: Option<&mut dyn TransactionExecutor>,
		) -> Result<ArticleDto, ServerFnSetError> {
			Ok(ArticleDto {
				id: model.id.unwrap_or_default(),
				title: model.title.clone(),
			})
		}
	}

	pub struct InternalPrincipal;

	#[async_trait::async_trait]
	impl reinhardt_di::params::FromRequest for InternalPrincipal {
		async fn from_request(
			_request: &reinhardt_http::Request,
			_context: &reinhardt_di::params::ParamContext,
		) -> reinhardt_di::params::ParamResult<Self> {
			Err(reinhardt_di::params::ParamError::Internal(
				"database password=top-secret".to_string(),
			))
		}
	}

	pub struct InternalPolicy;

	#[async_trait::async_trait]
	impl ServerFnSetPolicy<InternalResource> for InternalPolicy {
		type Principal = InternalPrincipal;

		async fn authorize_action(
			_principal: &Self::Principal,
			_action: ServerFnSetAction,
			_executor: Option<&mut dyn TransactionExecutor>,
		) -> Result<(), ServerFnSetError> {
			Ok(())
		}

		async fn scope_query(
			_principal: &Self::Principal,
			query: QuerySet<Article>,
			_executor: Option<&mut dyn TransactionExecutor>,
		) -> Result<QuerySet<Article>, ServerFnSetError> {
			Ok(query)
		}

		async fn authorize_object(
			_principal: &Self::Principal,
			_action: ServerFnSetAction,
			_object: &Article,
			_executor: Option<&mut dyn TransactionExecutor>,
		) -> Result<(), ServerFnSetError> {
			Ok(())
		}
	}

	pub struct InternalResource;

	impl ServerFnResource for InternalResource {
		type Lookup = i64;
		type Read = ArticleDto;
		type Create = CreateArticle;
		type Update = UpdateArticle;
		type Patch = PatchArticle;
		type ListQuery = ListQuery;
	}

	#[async_trait::async_trait]
	impl ModelServerFnResource for InternalResource {
		type Model = Article;
		type Policy = InternalPolicy;

		fn lookup_field() -> UniqueFieldRef<Article, i64> {
			// SAFETY: The handwritten test model declares `id` as its unique primary key.
			unsafe { UniqueFieldRef::from_model_field("id", "id") }
		}

		async fn to_read(
			model: &Article,
			_executor: Option<&mut dyn TransactionExecutor>,
		) -> Result<ArticleDto, ServerFnSetError> {
			Ok(ArticleDto {
				id: model.id.unwrap_or_default(),
				title: model.title.clone(),
			})
		}
	}

	#[server_fnset(name = "article-errors")]
	pub fn article_error_fns() -> ModelServerFnSet<ArticleResource> {
		ModelServerFnSet::new()
	}

	#[server_fnset(name = "rejecting-article-errors")]
	pub fn rejecting_article_error_fns() -> ModelServerFnSet<RejectingResource> {
		ModelServerFnSet::new()
	}

	#[server_fnset(name = "internal-article-errors")]
	pub fn internal_article_error_fns() -> ModelServerFnSet<InternalResource> {
		ModelServerFnSet::new()
	}
}

struct LegacyMarker;

impl ServerFnMetadata for LegacyMarker {
	const PATH: &'static str = "/api/server_fn/legacy-error";
	const NAME: &'static str = "legacy-error";
	const CODEC: &'static str = "json";
	const IS_JSON_CODEC: bool = true;
	const INJECTED_PARAMS: &'static [&'static str] = &[];
}

fn unused_handler(_: Request) -> Pin<Box<dyn Future<Output = Result<Bytes, Bytes>> + Send>> {
	Box::pin(async { unreachable!("status tests do not invoke the handler") })
}

impl ServerFnRegistration for LegacyMarker {
	fn handler() -> ServerFnHandler {
		unused_handler
	}
}

fn encoded(error: &impl serde::Serialize) -> Vec<u8> {
	serde_json::to_vec(error).expect("error should serialize")
}

async fn model_request(uri: &str) -> Request {
	let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("SQLite connection should open");
	let lease =
		DatabaseConnectionLease::register(owner).expect("SQLite connection should register");
	let connection = lease.handle();
	let singleton = Arc::new(SingletonScope::new());
	let context = Arc::new(
		InjectionContext::builder(singleton)
			.singleton(lease)
			.singleton(connection)
			.build(),
	);
	let mut request = Request::builder()
		.method(Method::POST)
		.uri(uri)
		.header(header::CONTENT_TYPE, "application/json")
		.body(Bytes::from_static(b"{\"lookup\":1}"))
		.build()
		.expect("request should build");
	request.set_di_context(context);
	request
}

#[tokio::test]
async fn generated_model_principal_returns_sanitized_unauthenticated_error() {
	let body =
		<fixture::rejecting_article_error_fns::retrieve::marker as ServerFnRegistration>::handle(
			model_request("/api/server_fn/rejecting-article-errors/retrieve").await,
		)
		.await
		.expect_err("principal extractor should reject the request");

	assert_eq!(
		serde_json::from_slice::<ServerFnSetError>(&body).expect("error should be structured JSON"),
		ServerFnSetError::Unauthenticated
	);
	assert_eq!(
		<fixture::rejecting_article_error_fns::retrieve::marker as ServerFnRegistration>::error_status(
			&body,
		),
		401
	);
	assert!(
		!String::from_utf8(body.to_vec())
			.unwrap()
			.contains("top-secret")
	);
}

#[tokio::test]
async fn generated_model_principal_returns_sanitized_internal_error() {
	let body =
		<fixture::internal_article_error_fns::retrieve::marker as ServerFnRegistration>::handle(
			model_request("/api/server_fn/internal-article-errors/retrieve").await,
		)
		.await
		.expect_err("principal extractor should reject the request");

	assert_eq!(
		serde_json::from_slice::<ServerFnSetError>(&body).expect("error should be structured JSON"),
		ServerFnSetError::Internal
	);
	assert_eq!(
		<fixture::internal_article_error_fns::retrieve::marker as ServerFnRegistration>::error_status(
			&body,
		),
		500
	);
	assert!(
		!String::from_utf8(body.to_vec())
			.unwrap()
			.contains("top-secret")
	);
}

#[test]
fn legacy_markers_preserve_valid_server_statuses() {
	let body = encoded(&ServerFnError::server(422, "unprocessable"));

	assert_eq!(LegacyMarker::error_status(&body), 422);
}

#[test]
fn legacy_markers_reject_invalid_statuses_and_malformed_bodies() {
	let invalid = encoded(&ServerFnError::server(700, "invalid"));

	assert_eq!(LegacyMarker::error_status(&invalid), 500);
	assert_eq!(LegacyMarker::error_status(b"not json"), 500);
}

#[test]
fn generated_model_markers_map_every_structured_error_deterministically() {
	let cases = [
		(ServerFnSetError::Validation(Default::default()), 400),
		(ServerFnSetError::Unauthenticated, 401),
		(ServerFnSetError::Forbidden, 403),
		(
			ServerFnSetError::NotFound {
				resource: "article".to_string(),
			},
			404,
		),
		(
			ServerFnSetError::Conflict {
				code: "stale".to_string(),
				message: "stale article".to_string(),
			},
			409,
		),
		(
			ServerFnSetError::Application {
				code: "invalid-state".to_string(),
				message: "invalid state".to_string(),
				details: serde_json::json!({"field": "state"}),
			},
			400,
		),
		(ServerFnSetError::Internal, 500),
		(
			ServerFnSetError::Transport(ServerFnError::server(418, "upstream")),
			500,
		),
	];

	for (error, expected) in cases {
		assert_eq!(
			<fixture::article_error_fns::retrieve::marker as ServerFnRegistration>::error_status(
				&encoded(&error),
			),
			expected,
		);
	}
}

#[test]
fn generated_model_markers_fall_back_to_legacy_errors() {
	let body = encoded(&ServerFnError::server(401, "extractor rejected request"));

	assert_eq!(
		<fixture::article_error_fns::retrieve::marker as ServerFnRegistration>::error_status(&body),
		401,
	);
}

#[test]
fn generated_model_markers_reject_invalid_and_malformed_errors() {
	let invalid = encoded(&ServerFnError::server(700, "invalid"));

	assert_eq!(
		<fixture::article_error_fns::retrieve::marker as ServerFnRegistration>::error_status(
			&invalid
		),
		500,
	);
	assert_eq!(
		<fixture::article_error_fns::retrieve::marker as ServerFnRegistration>::error_status(
			b"not json"
		),
		500,
	);
}

#[test]
fn model_transport_errors_are_sanitized_before_serialization() {
	let error =
		ServerFnSetError::Transport(ServerFnError::server(502, "database password=top-secret"));

	let body = encoded(&error.into_server_wire_error());

	assert_eq!(
		serde_json::from_slice::<ServerFnSetError>(&body).expect("wire error should decode"),
		ServerFnSetError::Internal,
	);
	assert!(!String::from_utf8(body).unwrap().contains("top-secret"));
}

#[test]
fn model_client_errors_decode_structured_then_legacy_then_raw() {
	let conflict = ServerFnSetError::Conflict {
		code: "stale".to_string(),
		message: "stale article".to_string(),
	};
	assert_eq!(
		ServerFnSetError::from_http_error(409, &String::from_utf8(encoded(&conflict)).unwrap()),
		conflict,
	);

	let legacy = ServerFnError::server(401, "authentication required");
	assert_eq!(
		ServerFnSetError::from_http_error(401, &String::from_utf8(encoded(&legacy)).unwrap()),
		ServerFnSetError::Transport(legacy),
	);

	assert_eq!(
		ServerFnSetError::from_http_error(502, "upstream unavailable"),
		ServerFnSetError::Transport(ServerFnError::from_http_response(
			502,
			"upstream unavailable",
		)),
	);
}

#[test]
fn model_server_error_bodies_hide_malformed_and_invalid_details() {
	let malformed = ServerFnSetError::sanitize_server_error_body(Bytes::from_static(
		b"database password=top-secret",
	));
	let invalid = ServerFnSetError::sanitize_server_error_body(Bytes::from(encoded(
		&ServerFnError::server(700, "policy token=top-secret"),
	)));

	for body in [malformed, invalid] {
		assert_eq!(
			serde_json::from_slice::<ServerFnSetError>(&body)
				.expect("sanitized body should decode"),
			ServerFnSetError::Internal,
		);
		assert!(
			!String::from_utf8(body.to_vec())
				.unwrap()
				.contains("top-secret")
		);
	}
}

#[test]
fn model_server_error_bodies_preserve_valid_legacy_envelopes() {
	let legacy = encoded(&ServerFnError::server(401, "authentication required"));

	assert_eq!(
		ServerFnSetError::sanitize_server_error_body(Bytes::from(legacy.clone())),
		Bytes::from(legacy),
	);
}

#[tokio::test]
async fn generated_model_handles_sanitize_malformed_request_errors() {
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/article-errors/retrieve")
		.header(header::CONTENT_TYPE, "application/json")
		.body(Bytes::from_static(b"{"))
		.build()
		.expect("request should build");

	let body =
		<fixture::article_error_fns::retrieve::marker as ServerFnRegistration>::handle(request)
			.await
			.expect_err("malformed arguments should fail");

	let error = serde_json::from_slice::<ServerFnError>(&body)
		.expect("error should retain the versioned generic server-function envelope");
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(400));
	assert_eq!(error.user_message(), "Invalid server function request");
	assert_eq!(
		serde_json::from_slice::<serde_json::Value>(&body).expect("error should be versioned JSON"),
		serde_json::json!({
			"version": 1,
			"kind": "server",
			"status": 400,
			"message": "Invalid server function request",
			"field_errors": [],
		}),
	);
	assert_eq!(
		<fixture::article_error_fns::retrieve::marker as ServerFnRegistration>::error_status(&body),
		400,
	);
}
