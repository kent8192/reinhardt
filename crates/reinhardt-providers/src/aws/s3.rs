//! Minimal Amazon S3 client.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, IF_NONE_MATCH};
use reqwest::{Client, Method, Response, StatusCode};
use sha2::{Digest, Sha256};
use url::Url;

use crate::aws::{AwsCredentials, AwsCredentialsSource};
use crate::{ProviderError, Result};

type HmacSha256 = Hmac<Sha256>;

const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const S3_SERVICE: &str = "s3";
const AWS4_REQUEST: &str = "aws4_request";
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

struct SigningRequest<'a> {
	method: &'a Method,
	canonical_uri: &'a str,
	canonical_query: &'a str,
	host: &'a str,
	date: &'a str,
	amz_date: &'a str,
	payload_hash: &'a str,
	credentials: &'a AwsCredentials,
	region: &'a str,
	additional_headers: &'a HeaderMap,
}

struct ResolvedS3SigningConfig {
	credentials: AwsCredentials,
	region: String,
}

/// Metadata returned by S3 `HEAD Object`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMetadata {
	/// Object size in bytes, when S3 returned a `Content-Length` header.
	pub size: Option<u64>,
	/// Object modification timestamp, when S3 returned a parseable
	/// `Last-Modified` header.
	pub last_modified: Option<DateTime<Utc>>,
	/// Object entity tag, when S3 returned an `ETag` header.
	pub etag: Option<String>,
}

/// Configuration for [`S3Client`].
#[derive(Debug, Clone)]
pub struct S3ClientConfig {
	/// S3 bucket name.
	pub bucket: String,
	/// AWS region used for SigV4 signing.
	pub region: Option<String>,
	/// Custom S3-compatible endpoint URL.
	pub endpoint: Option<String>,
	/// Credentials used for request signing.
	pub credentials: AwsCredentialsSource,
	/// Use path-style addressing (`/{bucket}/{key}`).
	pub force_path_style: bool,
}

impl S3ClientConfig {
	/// Build S3 client config for a bucket and region.
	#[must_use]
	pub fn new(bucket: impl Into<String>, region: impl Into<String>) -> Self {
		let region = region.into();
		Self {
			bucket: bucket.into(),
			region: Some(region.clone()),
			endpoint: None,
			credentials: AwsCredentialsSource::default_chain(Some(region)),
			force_path_style: false,
		}
	}
}

/// Minimal S3 client for the object operations Reinhardt needs.
#[derive(Debug, Clone)]
pub struct S3Client {
	http: Client,
	config: S3ClientConfig,
}

impl S3Client {
	/// Create a new S3 client.
	#[must_use]
	pub fn new(config: S3ClientConfig) -> Self {
		Self {
			http: Client::builder()
				.connect_timeout(DEFAULT_CONNECT_TIMEOUT)
				.timeout(DEFAULT_REQUEST_TIMEOUT)
				.build()
				.expect("static reqwest client configuration should be valid"),
			config,
		}
	}

	/// Store an object.
	///
	/// # Errors
	///
	/// Returns an error when request signing, HTTP transport, or S3 fails.
	pub async fn put_object(&self, key: &str, body: impl Into<Bytes>) -> Result<()> {
		let response = self
			.signed_request(Method::PUT, key, body.into(), HeaderMap::new())
			.await?;
		self.expect_success(response, key).await
	}

	/// Store an object only if it does not already exist.
	///
	/// # Errors
	///
	/// Returns an error when request signing, HTTP transport, or S3 fails.
	pub async fn put_object_if_absent(&self, key: &str, body: impl Into<Bytes>) -> Result<()> {
		let mut headers = HeaderMap::new();
		headers.insert(IF_NONE_MATCH, HeaderValue::from_static("*"));
		let response = self
			.signed_request(Method::PUT, key, body.into(), headers)
			.await?;
		self.expect_success(response, key).await
	}

	/// Load an object into memory.
	///
	/// # Errors
	///
	/// Returns [`ProviderError::NotFound`] for missing objects and transport or
	/// service errors for other failures.
	pub async fn get_object(&self, key: &str) -> Result<Bytes> {
		let response = self
			.signed_request(Method::GET, key, Bytes::new(), HeaderMap::new())
			.await?;

		if response.status() == StatusCode::NOT_FOUND {
			return Err(ProviderError::NotFound(key.to_string()));
		}
		if !response.status().is_success() {
			return Err(service_error(response).await);
		}

		Ok(response.bytes().await?)
	}

	/// Delete an object.
	///
	/// # Errors
	///
	/// Returns an error when request signing, HTTP transport, or S3 fails.
	pub async fn delete_object(&self, key: &str) -> Result<()> {
		let response = self
			.signed_request(Method::DELETE, key, Bytes::new(), HeaderMap::new())
			.await?;
		self.expect_success(response, key).await
	}

	/// Fetch object metadata.
	///
	/// Returns `Ok(None)` for missing objects.
	///
	/// # Errors
	///
	/// Returns an error when request signing, HTTP transport, or S3 fails.
	pub async fn head_object(&self, key: &str) -> Result<Option<ObjectMetadata>> {
		let response = self
			.signed_request(Method::HEAD, key, Bytes::new(), HeaderMap::new())
			.await?;

		if response.status() == StatusCode::NOT_FOUND {
			return Ok(None);
		}
		if !response.status().is_success() {
			return Err(service_error(response).await);
		}

		Ok(Some(metadata_from_response(&response)))
	}

	/// Create a presigned `GET Object` URL.
	///
	/// # Errors
	///
	/// Returns an error when credentials are missing, the URL cannot be built, or
	/// the expiry exceeds S3's seven-day SigV4 limit.
	pub async fn presigned_get_url(&self, key: &str, expires: Duration) -> Result<String> {
		if expires.as_secs() > 604_800 {
			return Err(ProviderError::Config(
				"S3 presigned URLs cannot expire after more than seven days".to_string(),
			));
		}

		let signing_config = self.resolve_signing_config().await?;
		let credentials = &signing_config.credentials;
		let (mut url, canonical_uri) = self.object_url(key, &signing_config.region)?;
		let host = canonical_host(&url)?;
		let now = Utc::now();
		let date = now.format("%Y%m%d").to_string();
		let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
		let credential_scope = self.credential_scope(&date, &signing_config.region);
		let credential = format!("{}/{}", credentials.access_key_id(), credential_scope);

		let mut query = BTreeMap::new();
		query.insert(
			"X-Amz-Algorithm".to_string(),
			"AWS4-HMAC-SHA256".to_string(),
		);
		query.insert("X-Amz-Credential".to_string(), credential);
		query.insert("X-Amz-Date".to_string(), amz_date.clone());
		query.insert("X-Amz-Expires".to_string(), expires.as_secs().to_string());
		query.insert("X-Amz-SignedHeaders".to_string(), "host".to_string());
		if let Some(token) = credentials.session_token() {
			query.insert("X-Amz-Security-Token".to_string(), token.to_string());
		}

		let canonical_query_string = canonical_query(&query);
		let canonical_request = format!(
			"GET\n{canonical_uri}\n{canonical_query_string}\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD"
		);
		let string_to_sign =
			self.string_to_sign(&canonical_request, &date, &amz_date, &signing_config.region)?;
		let signature = sign_hex(
			&signing_key(
				credentials.secret_access_key(),
				&date,
				&signing_config.region,
			),
			&string_to_sign,
		);

		query.insert("X-Amz-Signature".to_string(), signature);
		url.set_query(Some(&canonical_query(&query)));

		Ok(url.to_string())
	}

	async fn signed_request(
		&self,
		method: Method,
		key: &str,
		body: Bytes,
		additional_headers: HeaderMap,
	) -> Result<Response> {
		let signing_config = self.resolve_signing_config().await?;
		let credentials = &signing_config.credentials;
		let (url, canonical_uri) = self.object_url(key, &signing_config.region)?;
		let host = canonical_host(&url)?;
		let now = Utc::now();
		let date = now.format("%Y%m%d").to_string();
		let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
		let payload_hash = if body.is_empty() {
			EMPTY_SHA256.to_string()
		} else {
			sha256_hex(&body)
		};
		let additional_headers = normalize_headers(additional_headers)?;
		let headers = self.authorization_headers(SigningRequest {
			method: &method,
			canonical_uri: &canonical_uri,
			canonical_query: "",
			host: &host,
			date: &date,
			amz_date: &amz_date,
			payload_hash: &payload_hash,
			credentials,
			region: &signing_config.region,
			additional_headers: &additional_headers,
		})?;

		Ok(self
			.http
			.request(method, url)
			.headers(headers)
			.body(body)
			.send()
			.await?)
	}

	fn authorization_headers(&self, request: SigningRequest<'_>) -> Result<HeaderMap> {
		let mut canonical_headers = BTreeMap::new();
		canonical_headers.insert("host".to_string(), request.host.to_string());
		canonical_headers.insert(
			"x-amz-content-sha256".to_string(),
			request.payload_hash.to_string(),
		);
		canonical_headers.insert("x-amz-date".to_string(), request.amz_date.to_string());

		if let Some(token) = request.credentials.session_token() {
			canonical_headers.insert("x-amz-security-token".to_string(), token.to_string());
		}

		for (name, value) in request.additional_headers {
			let name = name.as_str().to_ascii_lowercase();
			let value = normalized_header_value(value)?;
			canonical_headers
				.entry(name)
				.and_modify(|existing: &mut String| {
					existing.push(',');
					existing.push_str(&value);
				})
				.or_insert(value);
		}

		let canonical_headers = canonical_headers
			.iter()
			.map(|(name, value)| format!("{name}:{value}\n"))
			.collect::<String>();
		let signed_headers = canonical_headers
			.lines()
			.filter_map(|line| line.split_once(':').map(|(name, _)| name))
			.collect::<Vec<_>>()
			.join(";");

		let canonical_request = format!(
			"{}\n{}\n{}\n{}\n{}\n{}",
			request.method.as_str(),
			request.canonical_uri,
			request.canonical_query,
			canonical_headers,
			signed_headers,
			request.payload_hash
		);
		let string_to_sign = self.string_to_sign(
			&canonical_request,
			request.date,
			request.amz_date,
			request.region,
		)?;
		let signature = sign_hex(
			&signing_key(
				request.credentials.secret_access_key(),
				request.date,
				request.region,
			),
			&string_to_sign,
		);
		let authorization = format!(
			"AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
			request.credentials.access_key_id(),
			self.credential_scope(request.date, request.region),
			signed_headers,
			signature
		);

		let mut headers = HeaderMap::new();
		insert_header(&mut headers, "host", request.host)?;
		insert_header(&mut headers, "x-amz-date", request.amz_date)?;
		insert_header(&mut headers, "x-amz-content-sha256", request.payload_hash)?;
		insert_header(&mut headers, "authorization", authorization)?;
		if let Some(token) = request.credentials.session_token() {
			insert_header(&mut headers, "x-amz-security-token", token)?;
		}
		headers.extend(request.additional_headers.clone());

		Ok(headers)
	}

	fn string_to_sign(
		&self,
		canonical_request: &str,
		date: &str,
		amz_date: &str,
		region: &str,
	) -> Result<String> {
		let hashed_request = sha256_hex(canonical_request.as_bytes());
		Ok(format!(
			"AWS4-HMAC-SHA256\n{}\n{}\n{}",
			amz_date,
			self.credential_scope(date, region),
			hashed_request
		))
	}

	fn credential_scope(&self, date: &str, region: &str) -> String {
		format!("{date}/{region}/{S3_SERVICE}/{AWS4_REQUEST}")
	}

	async fn resolve_signing_config(&self) -> Result<ResolvedS3SigningConfig> {
		let resolved = self.config.credentials.resolve().await?;
		let region = self
			.config
			.region
			.clone()
			.or(resolved.region)
			.unwrap_or_else(|| "us-east-1".to_string());

		Ok(ResolvedS3SigningConfig {
			credentials: resolved.credentials,
			region,
		})
	}

	fn object_url(&self, key: &str, region: &str) -> Result<(Url, String)> {
		let encoded_key = uri_encode(key, false);

		if self.config.endpoint.is_some() || self.config.force_path_style {
			let endpoint = self
				.config
				.endpoint
				.as_deref()
				.unwrap_or("https://s3.amazonaws.com");
			let endpoint_url = Url::parse(endpoint)?;
			let base_path = endpoint_url.path().trim_end_matches('/');
			let bucket = uri_encode(&self.config.bucket, true);
			let canonical_uri = format!("{base_path}/{bucket}/{encoded_key}");
			let mut origin = endpoint_url;
			origin.set_path("");
			origin.set_query(None);
			origin.set_fragment(None);
			let url = Url::parse(&format!(
				"{}{}",
				origin.as_str().trim_end_matches('/'),
				canonical_uri
			))?;
			return Ok((url, canonical_uri));
		}

		let host = format!("{}.s3.{}.amazonaws.com", self.config.bucket, region);
		let canonical_uri = format!("/{encoded_key}");
		let url = Url::parse(&format!("https://{host}{canonical_uri}"))?;
		Ok((url, canonical_uri))
	}

	async fn expect_success(&self, response: Response, key: &str) -> Result<()> {
		if response.status() == StatusCode::NOT_FOUND {
			return Err(ProviderError::NotFound(key.to_string()));
		}
		if !response.status().is_success() {
			return Err(service_error(response).await);
		}
		Ok(())
	}
}

fn normalize_headers(headers: HeaderMap) -> Result<HeaderMap> {
	let mut normalized = HeaderMap::new();
	for (name, value) in &headers {
		let name = HeaderName::from_bytes(name.as_str().to_ascii_lowercase().as_bytes())
			.map_err(|err| ProviderError::Header(err.to_string()))?;
		let value = HeaderValue::from_str(&normalized_header_value(value)?)
			.map_err(|err| ProviderError::Header(err.to_string()))?;
		normalized.append(name, value);
	}
	Ok(normalized)
}

fn normalized_header_value(value: &HeaderValue) -> Result<String> {
	let value = value
		.to_str()
		.map_err(|err| ProviderError::Header(err.to_string()))?;
	Ok(value.split_ascii_whitespace().collect::<Vec<_>>().join(" "))
}

fn metadata_from_response(response: &Response) -> ObjectMetadata {
	let headers = response.headers();
	let size = headers
		.get("content-length")
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.parse::<u64>().ok());
	let last_modified = headers
		.get("last-modified")
		.and_then(|value| value.to_str().ok())
		.and_then(|value| DateTime::parse_from_rfc2822(value).ok())
		.map(|value| value.with_timezone(&Utc));
	let etag = headers
		.get("etag")
		.and_then(|value| value.to_str().ok())
		.map(ToOwned::to_owned);

	ObjectMetadata {
		size,
		last_modified,
		etag,
	}
}

async fn service_error(response: Response) -> ProviderError {
	let status = response.status();
	let message = response
		.text()
		.await
		.unwrap_or_else(|err| format!("failed to read provider error body: {err}"));

	if status == StatusCode::FORBIDDEN {
		return ProviderError::PermissionDenied(message);
	}

	ProviderError::Service {
		status: status.as_u16(),
		message,
	}
}

fn canonical_host(url: &Url) -> Result<String> {
	let host = url
		.host_str()
		.ok_or_else(|| ProviderError::Config("S3 URL is missing a host".to_string()))?;
	let include_port = match (url.scheme(), url.port()) {
		("http", Some(80)) | ("https", Some(443)) | (_, None) => false,
		(_, Some(_)) => true,
	};

	if include_port {
		Ok(format!("{}:{}", host, url.port().expect("port checked")))
	} else {
		Ok(host.to_string())
	}
}

fn canonical_query(query: &BTreeMap<String, String>) -> String {
	query
		.iter()
		.map(|(key, value)| format!("{}={}", uri_encode(key, true), uri_encode(value, true)))
		.collect::<Vec<_>>()
		.join("&")
}

fn insert_header(
	headers: &mut HeaderMap,
	name: &'static str,
	value: impl AsRef<str>,
) -> Result<()> {
	let value = HeaderValue::from_str(value.as_ref())
		.map_err(|err| ProviderError::Header(err.to_string()))?;
	headers.insert(HeaderName::from_static(name), value);
	Ok(())
}

fn sha256_hex(input: impl AsRef<[u8]>) -> String {
	let hash = Sha256::digest(input.as_ref());
	hex_lower(&hash)
}

fn signing_key(secret_access_key: &str, date: &str, region: &str) -> Vec<u8> {
	let date_key = hmac_sha256(
		format!("AWS4{secret_access_key}").as_bytes(),
		date.as_bytes(),
	);
	let date_region_key = hmac_sha256(&date_key, region.as_bytes());
	let date_region_service_key = hmac_sha256(&date_region_key, S3_SERVICE.as_bytes());
	hmac_sha256(&date_region_service_key, AWS4_REQUEST.as_bytes())
}

fn sign_hex(key: &[u8], message: &str) -> String {
	let signature = hmac_sha256(key, message.as_bytes());
	hex_lower(&signature)
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
	let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any size");
	mac.update(message);
	mac.finalize().into_bytes().to_vec()
}

fn hex_lower(bytes: &[u8]) -> String {
	let mut out = String::with_capacity(bytes.len() * 2);
	for byte in bytes {
		write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
	}
	out
}

fn uri_encode(input: &str, encode_slash: bool) -> String {
	let mut out = String::with_capacity(input.len());
	for byte in input.bytes() {
		if is_unreserved(byte) || (!encode_slash && byte == b'/') {
			out.push(byte as char);
		} else {
			write!(&mut out, "%{byte:02X}").expect("writing to String cannot fail");
		}
	}
	out
}

fn is_unreserved(byte: u8) -> bool {
	matches!(
		byte,
		b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~'
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use wiremock::matchers::{header, method, path};
	use wiremock::{Mock, MockServer, ResponseTemplate};

	fn test_client(endpoint: String) -> S3Client {
		S3Client::new(S3ClientConfig {
			bucket: "bucket".to_string(),
			region: Some("us-east-1".to_string()),
			endpoint: Some(endpoint),
			credentials: AwsCredentialsSource::Static(AwsCredentials::new("test", "test")),
			force_path_style: true,
		})
	}

	#[tokio::test]
	async fn put_object_if_absent_sends_a_signed_precondition() {
		let server = MockServer::start().await;
		Mock::given(method("PUT"))
			.and(path("/bucket/avatars/a.png"))
			.and(header("if-none-match", "*"))
			.respond_with(ResponseTemplate::new(200))
			.mount(&server)
			.await;
		let client = test_client(server.uri());

		client
			.put_object_if_absent("avatars/a.png", b"content".as_slice())
			.await
			.expect("conditional object put should succeed");

		let requests = server
			.received_requests()
			.await
			.expect("wiremock should record the request");
		let authorization = requests[0]
			.headers
			.get("authorization")
			.expect("request should include authorization")
			.to_str()
			.expect("authorization should be valid header text");
		assert!(
			authorization
				.contains("SignedHeaders=host;if-none-match;x-amz-content-sha256;x-amz-date")
		);
	}

	#[tokio::test]
	async fn put_object_if_absent_preserves_precondition_service_errors() {
		for status in [StatusCode::CONFLICT, StatusCode::PRECONDITION_FAILED] {
			let server = MockServer::start().await;
			Mock::given(method("PUT"))
				.respond_with(ResponseTemplate::new(status.as_u16()).set_body_string("exists"))
				.mount(&server)
				.await;
			let client = test_client(server.uri());

			let err = client
				.put_object_if_absent("avatars/a.png", b"content".as_slice())
				.await
				.expect_err("conditional PUT should report the S3 precondition failure");
			assert!(matches!(
				err,
				ProviderError::Service { status: actual, message }
					if actual == status.as_u16() && message == "exists"
			));
		}
	}

	#[test]
	fn uri_encode_preserves_key_slashes() {
		assert_eq!(
			uri_encode("path/to/file name.txt", false),
			"path/to/file%20name.txt"
		);
	}

	#[test]
	fn uri_encode_escapes_query_slashes() {
		assert_eq!(
			uri_encode("AKIA/20260612/us-east-1/s3/aws4_request", true),
			"AKIA%2F20260612%2Fus-east-1%2Fs3%2Faws4_request"
		);
	}

	#[test]
	fn empty_sha256_constant_matches_hash() {
		assert_eq!(sha256_hex([]), EMPTY_SHA256);
	}

	#[test]
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
}
