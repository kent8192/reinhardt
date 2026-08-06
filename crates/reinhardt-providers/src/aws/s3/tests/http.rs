use bytes::Bytes;
use chrono::{DateTime, Utc};
use reqwest::Client;
use rstest::rstest;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use super::super::*;
use super::{fixed_now, test_config};

struct TestS3Server {
	server: MockServer,
}

struct RawHttpServer {
	endpoint: String,
	task: JoinHandle<()>,
}

impl RawHttpServer {
	async fn truncated_error_body() -> Self {
		let listener = TcpListener::bind("127.0.0.1:0")
			.await
			.expect("bind raw HTTP fixture");
		let address = listener.local_addr().expect("fixture address");
		let task = tokio::spawn(async move {
			let (mut socket, _) = listener.accept().await.expect("accept one request");
			socket
				.write_all(
					b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 100\r\nConnection: close\r\n\r\nshort",
				)
				.await
				.expect("write truncated response");
		});
		Self {
			endpoint: format!("http://{address}"),
			task,
		}
	}
}

impl Drop for RawHttpServer {
	fn drop(&mut self) {
		self.task.abort();
	}
}

impl TestS3Server {
	async fn start() -> Self {
		Self {
			server: MockServer::start().await,
		}
	}

	fn client(&self) -> S3Client {
		let mut config = test_config(Some(self.server.uri()));
		config.force_path_style = true;
		S3Client::with_test_dependencies(config, Client::new(), fixed_now())
	}

	fn session_client(&self) -> S3Client {
		let mut config = test_config(Some(self.server.uri()));
		config.force_path_style = true;
		config.credentials = AwsCredentialsSource::Static(
			AwsCredentials::new(
				"AKIAIOSFODNN7EXAMPLE",
				"wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
			)
			.with_session_token("session-token"),
		);
		S3Client::with_test_dependencies(config, Client::new(), fixed_now())
	}

	async fn respond(&self, response: ResponseTemplate) {
		Mock::given(any())
			.respond_with(response)
			.mount(&self.server)
			.await;
	}

	async fn single_request(&self) -> Request {
		let requests = self
			.server
			.received_requests()
			.await
			.expect("wiremock records requests");
		assert_eq!(requests.len(), 1);
		requests.into_iter().next().expect("one recorded request")
	}
}

fn assert_signature(request: &Request, expected_prefix: &str) {
	let authorization = request.headers["authorization"]
		.to_str()
		.expect("ASCII authorization header");
	let (prefix, signature) = authorization
		.split_once(", Signature=")
		.expect("authorization includes a signature");
	assert_eq!(prefix, expected_prefix);
	// The random loopback port is part of the signed host, so this signature varies per run.
	// Task 2 verifies exact signature correctness with a fixed-host vector.
	assert_eq!(signature.len(), 64);
	assert!(
		signature
			.bytes()
			.all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
	);
}

#[rstest]
#[tokio::test]
async fn put_object_sends_a_signed_binary_request() {
	// Arrange
	let server = TestS3Server::start().await;
	server.respond(ResponseTemplate::new(200)).await;
	let client = server.client();

	// Act
	client
		.put_object("path/to/object.bin", Bytes::from_static(b"\x00\x01payload"))
		.await
		.expect("PUT succeeds");

	// Assert
	let request = server.single_request().await;
	assert_eq!(request.method.as_str(), "PUT");
	assert_eq!(request.url.path(), "/examplebucket/path/to/object.bin");
	assert_eq!(request.body, b"\x00\x01payload");
	assert_eq!(
		request.headers["x-amz-content-sha256"]
			.to_str()
			.expect("ASCII hash"),
		sha256_hex(b"\x00\x01payload")
	);
	assert_eq!(
		request.headers["x-amz-date"]
			.to_str()
			.expect("ASCII timestamp"),
		"20130524T000000Z"
	);
	assert_signature(
		&request,
		"AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date",
	);
}

#[rstest]
#[tokio::test]
async fn get_object_returns_exact_binary_bytes() {
	// Arrange
	let server = TestS3Server::start().await;
	server
		.respond(ResponseTemplate::new(200).set_body_bytes(b"\x00\xffpayload"))
		.await;

	// Act
	let object = server
		.client()
		.get_object("path/to/object.bin")
		.await
		.expect("GET succeeds");

	// Assert
	assert_eq!(object, Bytes::from_static(b"\x00\xffpayload"));
	let request = server.single_request().await;
	assert_eq!(request.method.as_str(), "GET");
	assert_eq!(request.url.path(), "/examplebucket/path/to/object.bin");
}

#[rstest]
#[tokio::test]
async fn get_object_returns_empty_bytes_for_an_empty_response() {
	// Arrange
	let server = TestS3Server::start().await;
	server.respond(ResponseTemplate::new(200)).await;

	// Act
	let object = server
		.client()
		.get_object("empty.bin")
		.await
		.expect("GET succeeds");

	// Assert
	assert_eq!(object, Bytes::new());
}

#[rstest]
#[tokio::test]
async fn delete_object_sends_the_expected_path() {
	// Arrange
	let server = TestS3Server::start().await;
	server.respond(ResponseTemplate::new(204)).await;

	// Act
	server
		.client()
		.delete_object("path/to/object.bin")
		.await
		.expect("DELETE succeeds");

	// Assert
	let request = server.single_request().await;
	assert_eq!(request.method.as_str(), "DELETE");
	assert_eq!(request.url.path(), "/examplebucket/path/to/object.bin");
}

#[rstest]
#[tokio::test]
async fn head_object_parses_complete_metadata() {
	// Arrange
	let server = TestS3Server::start().await;
	server
		.respond(
			ResponseTemplate::new(200)
				.insert_header("Content-Length", "12")
				.insert_header("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT")
				.insert_header("ETag", "\"abc123\""),
		)
		.await;

	// Act
	let metadata = server
		.client()
		.head_object("metadata.txt")
		.await
		.expect("HEAD succeeds")
		.expect("object exists");

	// Assert
	assert_eq!(
		metadata,
		ObjectMetadata {
			size: Some(12),
			last_modified: Some(
				DateTime::parse_from_rfc2822("Wed, 21 Oct 2015 07:28:00 GMT")
					.expect("valid fixture date")
					.with_timezone(&Utc),
			),
			etag: Some("\"abc123\"".to_string()),
		}
	);
}

#[rstest]
#[tokio::test]
async fn head_object_uses_none_for_absent_metadata_headers() {
	// Arrange
	let server = TestS3Server::start().await;
	server.respond(ResponseTemplate::new(200)).await;

	// Act
	let metadata = server
		.client()
		.head_object("metadata.txt")
		.await
		.expect("HEAD succeeds")
		.expect("object exists");

	// Assert
	assert_eq!(
		metadata,
		ObjectMetadata {
			size: None,
			last_modified: None,
			etag: None,
		}
	);
}

#[rstest]
#[tokio::test]
async fn head_object_ignores_invalid_individual_metadata_headers() {
	// Arrange
	let server = TestS3Server::start().await;
	server
		.respond(
			ResponseTemplate::new(200)
				.insert_header("Content-Length", "not-a-number")
				.insert_header("Last-Modified", "not-a-date")
				.insert_header("ETag", "\"still-present\""),
		)
		.await;

	// Act
	let metadata = server
		.client()
		.head_object("metadata.txt")
		.await
		.expect("HEAD succeeds")
		.expect("object exists");

	// Assert
	assert_eq!(
		metadata,
		ObjectMetadata {
			size: None,
			last_modified: None,
			etag: Some("\"still-present\"".to_string()),
		}
	);
}

#[rstest]
#[tokio::test]
async fn get_object_percent_encodes_unicode_keys() {
	// Arrange
	let server = TestS3Server::start().await;
	server.respond(ResponseTemplate::new(200)).await;

	// Act
	server
		.client()
		.get_object("folder/ファイル & data.txt")
		.await
		.expect("GET succeeds");

	// Assert
	let request = server.single_request().await;
	assert_eq!(
		request.url.path(),
		"/examplebucket/folder/%E3%83%95%E3%82%A1%E3%82%A4%E3%83%AB%20%26%20data.txt"
	);
}

#[rstest]
#[tokio::test]
async fn get_object_signs_and_sends_the_session_token() {
	// Arrange
	let server = TestS3Server::start().await;
	server.respond(ResponseTemplate::new(200)).await;

	// Act
	server
		.session_client()
		.get_object("session.txt")
		.await
		.expect("GET succeeds");

	// Assert
	let request = server.single_request().await;
	assert_eq!(
		request.headers["x-amz-security-token"]
			.to_str()
			.expect("ASCII session token"),
		"session-token"
	);
	assert_signature(
		&request,
		"AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date;x-amz-security-token",
	);
}

#[rstest]
#[tokio::test]
async fn get_object_maps_a_missing_object_to_not_found() {
	// Arrange
	let server = TestS3Server::start().await;
	server.respond(ResponseTemplate::new(404)).await;

	// Act
	let result = server.client().get_object("missing.txt").await;

	// Assert
	match result {
		Err(ProviderError::NotFound(key)) => assert_eq!(key, "missing.txt"),
		other => panic!("unexpected GET result: {other:?}"),
	}
	let request = server.single_request().await;
	assert_eq!(request.method.as_str(), "GET");
}

#[rstest]
#[tokio::test]
async fn put_object_maps_a_missing_object_to_not_found() {
	// Arrange
	let server = TestS3Server::start().await;
	server.respond(ResponseTemplate::new(404)).await;

	// Act
	let result = server
		.client()
		.put_object("missing.txt", Bytes::from_static(b"body"))
		.await;

	// Assert
	match result {
		Err(ProviderError::NotFound(key)) => assert_eq!(key, "missing.txt"),
		other => panic!("unexpected PUT result: {other:?}"),
	}
	let request = server.single_request().await;
	assert_eq!(request.method.as_str(), "PUT");
}

#[rstest]
#[tokio::test]
async fn delete_object_maps_a_missing_object_to_not_found() {
	// Arrange
	let server = TestS3Server::start().await;
	server.respond(ResponseTemplate::new(404)).await;

	// Act
	let result = server.client().delete_object("missing.txt").await;

	// Assert
	match result {
		Err(ProviderError::NotFound(key)) => assert_eq!(key, "missing.txt"),
		other => panic!("unexpected DELETE result: {other:?}"),
	}
	let request = server.single_request().await;
	assert_eq!(request.method.as_str(), "DELETE");
}

#[rstest]
#[tokio::test]
async fn head_object_maps_a_missing_object_to_none() {
	// Arrange
	let server = TestS3Server::start().await;
	server.respond(ResponseTemplate::new(404)).await;

	// Act
	let metadata = server
		.client()
		.head_object("missing.txt")
		.await
		.expect("HEAD maps 404");

	// Assert
	assert_eq!(metadata, None);
	let request = server.single_request().await;
	assert_eq!(request.method.as_str(), "HEAD");
}

#[rstest]
#[tokio::test]
async fn get_object_maps_forbidden_responses_to_permission_denied() {
	// Arrange
	let server = TestS3Server::start().await;
	server
		.respond(ResponseTemplate::new(403).set_body_string("access denied"))
		.await;

	// Act
	let result = server.client().get_object("secret.txt").await;

	// Assert
	match result {
		Err(ProviderError::PermissionDenied(message)) => assert_eq!(message, "access denied"),
		other => panic!("unexpected 403 result: {other:?}"),
	}
	let request = server.single_request().await;
	assert_eq!(request.method.as_str(), "GET");
}

#[rstest]
#[tokio::test]
async fn put_object_maps_general_service_errors() {
	// Arrange
	let server = TestS3Server::start().await;
	server
		.respond(ResponseTemplate::new(500).set_body_string("temporary failure"))
		.await;

	// Act
	let result = server
		.client()
		.put_object("unavailable.txt", Bytes::new())
		.await;

	// Assert
	match result {
		Err(ProviderError::Service { status, message }) => {
			assert_eq!(status, 500);
			assert_eq!(message, "temporary failure");
		}
		other => panic!("unexpected service error: {other:?}"),
	}
	let request = server.single_request().await;
	assert_eq!(request.method.as_str(), "PUT");
}

#[rstest]
#[tokio::test]
async fn get_object_maps_connection_refusals_to_http_errors() {
	// Arrange
	let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback port");
	let address = listener.local_addr().expect("loopback address");
	drop(listener);
	let mut config = test_config(Some(format!("http://{address}")));
	config.force_path_style = true;
	let http = Client::builder()
		.connect_timeout(std::time::Duration::from_millis(200))
		.timeout(std::time::Duration::from_secs(1))
		.build()
		.expect("test HTTP client");
	let client = S3Client::with_test_dependencies(config, http, fixed_now());

	// Act
	let result = client.get_object("unreachable.txt").await;

	// Assert
	// The OS connection diagnostic differs by platform, so only the stable Reinhardt variant is asserted.
	assert!(matches!(result, Err(ProviderError::Http(_))));
}

#[rstest]
#[tokio::test]
async fn get_object_preserves_the_service_variant_when_an_error_body_is_truncated() {
	// Arrange
	let server = RawHttpServer::truncated_error_body().await;
	let mut config = test_config(Some(server.endpoint.clone()));
	config.force_path_style = true;
	let client = S3Client::with_test_dependencies(config, Client::new(), fixed_now());

	// Act
	let result = client.get_object("truncated.txt").await;

	// Assert
	match result {
		Err(ProviderError::Service { status, message }) => {
			assert_eq!(status, 500);
			// The nested reqwest/hyper diagnostic varies by platform and version, so its stable prefix is asserted.
			assert!(message.starts_with("failed to read provider error body:"));
		}
		other => panic!("unexpected truncated-body result: {other:?}"),
	}
}
