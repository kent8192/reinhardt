use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::{Client, Method};
use rstest::rstest;
use url::Url;

use super::*;

#[rstest]
#[tokio::test]
async fn presigned_url_uses_the_fixed_signing_time() {
	let client = S3Client::with_test_dependencies(test_config(None), Client::new(), fixed_now());

	let url = client
		.presigned_get_url("test.txt", Duration::from_secs(86_400))
		.await
		.expect("presigning succeeds");
	let parsed = Url::parse(&url).expect("presigned URL is valid");
	let query = parsed.query_pairs().collect::<BTreeMap<_, _>>();

	assert_eq!(query["X-Amz-Date"], "20130524T000000Z");
	assert_eq!(query["X-Amz-Expires"], "86400");
}

#[rstest]
fn signing_key_matches_fixed_s3_vector() {
	assert_eq!(
		hex_lower(&signing_key(
			"wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
			"20130524",
			"us-east-1",
		)),
		"f117494eff5d09da21cbf7f0339559ea04fc9582d31299cb992be70a6b27c97a"
	);
}

#[rstest]
fn authorization_headers_match_fixed_get_vector() {
	let client = S3Client::with_test_dependencies(test_config(None), Client::new(), fixed_now());
	let headers = client
		.authorization_headers(SigningRequest {
			method: &Method::GET,
			canonical_uri: "/test.txt",
			canonical_query: "",
			host: "examplebucket.s3.amazonaws.com",
			date: "20130524",
			amz_date: "20130524T000000Z",
			payload_hash: EMPTY_SHA256,
			credentials: static_credentials(&client),
			region: "us-east-1",
			additional_headers: &HeaderMap::new(),
		})
		.expect("valid signing headers");

	assert_eq!(headers["x-amz-date"], "20130524T000000Z");
	assert_eq!(headers["x-amz-content-sha256"], EMPTY_SHA256);
	assert_eq!(
		headers["authorization"],
		"AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature=14f6a0997b2b70a86f4726658a6575b5109092ccb5fd328f51b369c44b4ac958"
	);
}

#[rstest]
fn authorization_headers_include_session_token_in_the_signature() {
	let client = session_client();
	let headers = client
		.authorization_headers(SigningRequest {
			method: &Method::GET,
			canonical_uri: "/test.txt",
			canonical_query: "",
			host: "examplebucket.s3.amazonaws.com",
			date: "20130524",
			amz_date: "20130524T000000Z",
			payload_hash: EMPTY_SHA256,
			credentials: static_credentials(&client),
			region: "us-east-1",
			additional_headers: &HeaderMap::new(),
		})
		.expect("valid signing headers");

	assert_eq!(headers["x-amz-security-token"], "session-token");
	let authorization = headers["authorization"]
		.to_str()
		.expect("valid authorization");
	let signed_headers = authorization
		.split("SignedHeaders=")
		.nth(1)
		.expect("authorization includes signed headers")
		.split(", Signature=")
		.next()
		.expect("signed headers precede the signature");
	assert_eq!(
		signed_headers,
		"host;x-amz-content-sha256;x-amz-date;x-amz-security-token"
	);
}

#[rstest]
#[case("path/to/file name.txt", false, "path/to/file%20name.txt")]
#[case("path/to/file name.txt", true, "path%2Fto%2Ffile%20name.txt")]
#[case("ファイル.txt", false, "%E3%83%95%E3%82%A1%E3%82%A4%E3%83%AB.txt")]
#[case("a&b=c", false, "a%26b%3Dc")]
fn uri_encoding_is_aws_compatible(
	#[case] input: &str,
	#[case] encode_slash: bool,
	#[case] expected: &str,
) {
	assert_eq!(uri_encode(input, encode_slash), expected);
}

#[rstest]
#[case(
	None,
	false,
	"https://examplebucket.s3.us-east-1.amazonaws.com/test.txt"
)]
#[case(None, true, "https://s3.amazonaws.com/examplebucket/test.txt")]
#[case(
	Some("http://127.0.0.1:9000/base/"),
	false,
	"http://127.0.0.1:9000/base/examplebucket/test.txt"
)]
fn object_url_uses_expected_addressing(
	#[case] endpoint: Option<&str>,
	#[case] force_path_style: bool,
	#[case] expected: &str,
) {
	let mut config = test_config(endpoint.map(str::to_owned));
	config.force_path_style = force_path_style;
	let client = S3Client::new(config);

	let (url, _) = client
		.object_url("test.txt", "us-east-1")
		.expect("object URL is valid");

	assert_eq!(url.as_str(), expected);
}

#[rstest]
#[case("https://s3.amazonaws.com/base", "s3.amazonaws.com")]
#[case("http://127.0.0.1:9000/base", "127.0.0.1:9000")]
fn canonical_host_uses_the_expected_port(#[case] input: &str, #[case] expected: &str) {
	let url = Url::parse(input).expect("URL is valid");
	assert_eq!(canonical_host(&url).expect("host is present"), expected);
}

#[rstest]
fn canonical_host_rejects_urls_without_a_host() {
	// Arrange
	let url = Url::parse("file:///tmp/object").expect("URL is valid");

	// Act
	let error = canonical_host(&url).expect_err("file URL has no host");

	// Assert
	match error {
		ProviderError::Config(message) => assert_eq!(message, "S3 URL is missing a host"),
		other => panic!("unexpected host validation error: {other:?}"),
	}
}

#[rstest]
fn canonical_query_sorts_keys_and_encodes_values() {
	let query = BTreeMap::from([
		("z key".to_string(), "last/value".to_string()),
		("a".to_string(), "first".to_string()),
	]);
	assert_eq!(canonical_query(&query), "a=first&z%20key=last%2Fvalue");
}

#[rstest]
#[case(&[][..], EMPTY_SHA256)]
#[case(
	b"hello",
	"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
)]
fn sha256_hashes_fixed_vectors(#[case] input: &[u8], #[case] expected: &str) {
	assert_eq!(sha256_hex(input), expected);
}

#[rstest]
fn insert_header_reports_invalid_header_values() {
	// Arrange
	let mut headers = reqwest::header::HeaderMap::new();

	// Act
	let error = insert_header(&mut headers, "x-amz-meta-test", "line\nbreak")
		.expect_err("newline is not a valid HTTP header value");

	// Assert
	match error {
		ProviderError::Header(message) => assert_eq!(message, "failed to parse header value"),
		other => panic!("unexpected header validation error: {other:?}"),
	}
}

#[rstest]
fn invalid_endpoint_returns_a_url_error() {
	// Arrange
	let client = S3Client::new(test_config(Some("://not-a-url".to_string())));

	// Act
	let error = client
		.object_url("test.txt", "us-east-1")
		.expect_err("invalid endpoint must not build an object URL");

	// Assert
	match error {
		ProviderError::Url(error) => assert_eq!(error, url::ParseError::RelativeUrlWithoutBase),
		other => panic!("unexpected endpoint validation error: {other:?}"),
	}
}

#[rstest]
#[tokio::test]
async fn presigning_accepts_the_seven_day_expiry_limit() {
	// Arrange
	let client = S3Client::with_test_dependencies(test_config(None), Client::new(), fixed_now());

	// Act
	let url = client
		.presigned_get_url("test.txt", Duration::from_secs(604_800))
		.await;

	// Assert
	let parsed =
		Url::parse(&url.expect("seven-day expiry is accepted")).expect("presigned URL is valid");
	let query = parsed.query_pairs().collect::<BTreeMap<_, _>>();
	assert_eq!(query["X-Amz-Expires"], "604800");
}

#[rstest]
#[tokio::test]
async fn presigning_rejects_expiries_longer_than_seven_days() {
	// Arrange
	let client = S3Client::with_test_dependencies(test_config(None), Client::new(), fixed_now());

	// Act
	let result = client
		.presigned_get_url("test.txt", Duration::from_secs(604_801))
		.await;

	// Assert
	match result.expect_err("expiry must be rejected") {
		ProviderError::Config(message) => assert_eq!(
			message,
			"S3 presigned URLs cannot expire after more than seven days"
		),
		other => panic!("unexpected expiry validation error: {other:?}"),
	}
}

#[rstest]
#[tokio::test]
async fn presigned_url_matches_fixed_s3_vector() {
	let client = S3Client::with_test_dependencies(test_config(None), Client::new(), fixed_now());
	let url = client
		.presigned_get_url("test.txt", Duration::from_secs(86_400))
		.await
		.expect("presigning succeeds");

	assert_eq!(
		url,
		"https://examplebucket.s3.us-east-1.amazonaws.com/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20130524%2Fus-east-1%2Fs3%2Faws4_request&X-Amz-Date=20130524T000000Z&X-Amz-Expires=86400&X-Amz-Signature=e88bfc86a6838bda6e6b842bfd69edd8741f6aedb0459e1275be0713ad3e2633&X-Amz-SignedHeaders=host"
	);
}

#[rstest]
#[tokio::test]
async fn session_presigned_url_contains_the_fixed_token_vector() {
	let url = session_client()
		.presigned_get_url("test.txt", Duration::from_secs(86_400))
		.await
		.expect("presigning succeeds");
	let parsed = Url::parse(&url).expect("URL is valid");
	let query = parsed
		.query_pairs()
		.map(|(key, value)| (key.into_owned(), value.into_owned()))
		.collect::<BTreeMap<_, _>>();

	assert_eq!(
		query,
		BTreeMap::from([
			(
				"X-Amz-Algorithm".to_string(),
				"AWS4-HMAC-SHA256".to_string()
			),
			(
				"X-Amz-Credential".to_string(),
				"AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request".to_string(),
			),
			("X-Amz-Date".to_string(), "20130524T000000Z".to_string()),
			("X-Amz-Expires".to_string(), "86400".to_string()),
			(
				"X-Amz-Security-Token".to_string(),
				"session-token".to_string()
			),
			(
				"X-Amz-Signature".to_string(),
				"64693fa10671fce2bf33b18c6639147a5a4f36f6577f8d41b5683a9249fddccf".to_string(),
			),
			("X-Amz-SignedHeaders".to_string(), "host".to_string()),
		])
	);
}

#[rstest]
fn object_url_includes_endpoint_base_path_in_canonical_uri() {
	let client = S3Client::new(S3ClientConfig {
		bucket: "test-bucket".to_string(),
		region: Some("us-east-1".to_string()),
		endpoint: Some("http://127.0.0.1:9000/base/".to_string()),
		credentials: AwsCredentialsSource::Static(AwsCredentials::new("test", "test")),
		force_path_style: true,
	});

	let (url, canonical_uri) = client
		.object_url("path/to/file name.txt", "us-east-1")
		.expect("object URL should be built");

	assert_eq!(canonical_uri, "/base/test-bucket/path/to/file%20name.txt");
	assert_eq!(
		url.as_str(),
		"http://127.0.0.1:9000/base/test-bucket/path/to/file%20name.txt"
	);
}

#[rstest]
#[tokio::test]
async fn explicit_region_wins_for_static_credentials() {
	let client = S3Client::with_test_dependencies(test_config(None), Client::new(), fixed_now());
	let resolved = client
		.resolve_signing_config()
		.await
		.expect("signing config resolves");
	assert_eq!(resolved.region, "us-east-1");
}

#[rstest]
#[tokio::test]
async fn static_credentials_default_to_us_east_1_without_a_region() {
	let mut config = test_config(None);
	config.region = None;
	let client = S3Client::with_test_dependencies(config, Client::new(), fixed_now());
	let resolved = client
		.resolve_signing_config()
		.await
		.expect("signing config resolves");
	assert_eq!(resolved.region, "us-east-1");
}

fn static_credentials(client: &S3Client) -> &AwsCredentials {
	match &client.config.credentials {
		AwsCredentialsSource::Static(credentials) => credentials,
		AwsCredentialsSource::DefaultChain { .. } => unreachable!("static test config"),
	}
}

fn session_client() -> S3Client {
	let mut config = test_config(None);
	config.credentials = AwsCredentialsSource::Static(
		AwsCredentials::new(
			"AKIAIOSFODNN7EXAMPLE",
			"wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
		)
		.with_session_token("session-token"),
	);
	S3Client::with_test_dependencies(config, Client::new(), fixed_now())
}
