use std::env;

use rstest::rstest;
use serial_test::serial;

use super::*;

const AWS_ENV_KEYS: &[&str] = &[
	"AWS_ACCESS_KEY_ID",
	"AWS_SECRET_ACCESS_KEY",
	"AWS_SESSION_TOKEN",
	"AWS_REGION",
	"AWS_DEFAULT_REGION",
	"AWS_EC2_METADATA_DISABLED",
	"AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
	"AWS_CONTAINER_CREDENTIALS_FULL_URI",
	"AWS_WEB_IDENTITY_TOKEN_FILE",
	"AWS_ROLE_ARN",
];

struct EnvGuard {
	originals: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
	fn capture(keys: &[&'static str]) -> Self {
		Self {
			originals: keys.iter().map(|key| (*key, env::var(key).ok())).collect(),
		}
	}

	fn replace(values: &[(&'static str, Option<&str>)]) -> Self {
		let guard = Self::capture(AWS_ENV_KEYS);
		// SAFETY: Every caller is serialized with #[serial(aws_credentials_env)].
		unsafe {
			for key in AWS_ENV_KEYS {
				env::remove_var(key);
			}
			for (key, value) in values {
				if let Some(value) = value {
					env::set_var(key, value);
				}
			}
		}
		guard
	}
}

impl Drop for EnvGuard {
	fn drop(&mut self) {
		for (key, value) in self.originals.iter().rev() {
			// SAFETY: Tests using this guard are serialized with
			// #[serial(aws_credentials_env)].
			unsafe {
				if let Some(value) = value {
					env::set_var(key, value);
				} else {
					env::remove_var(key);
				}
			}
		}
	}
}

#[rstest]
fn credentials_expose_static_values() {
	let credentials =
		AwsCredentials::new("access-key", "secret-key").with_session_token("session-token");

	assert_eq!(credentials.access_key_id(), "access-key");
	assert_eq!(credentials.secret_access_key(), "secret-key");
	assert_eq!(credentials.session_token(), Some("session-token"));
}

#[rstest]
fn credentials_without_session_token_return_none() {
	let credentials = AwsCredentials::new("access-key", "secret-key");

	assert_eq!(credentials.session_token(), None);
}

#[rstest]
fn debug_output_redacts_every_secret() {
	let credentials =
		AwsCredentials::new("access-key", "secret-key").with_session_token("session-token");
	let source = AwsCredentialsSource::Static(credentials.clone());
	let resolved = AwsSigningConfig {
		credentials: credentials.clone(),
		region: Some("us-east-1".to_string()),
	};

	assert_eq!(
		format!("{credentials:?}"),
		"AwsCredentials { access_key_id: \"<redacted>\", secret_access_key: \"<redacted>\", session_token: Some(\"<redacted>\") }"
	);
	assert_eq!(format!("{source:?}"), "Static(\"<redacted credentials>\")");
	assert_eq!(
		format!("{resolved:?}"),
		"AwsSigningConfig { credentials: \"<redacted credentials>\", region: Some(\"us-east-1\") }"
	);
}

#[rstest]
#[case(Some("access"), None, None)]
#[case(None, Some("secret"), None)]
#[case(None, None, Some("token"))]
#[case(Some("access"), None, Some("token"))]
#[case(None, Some("secret"), Some("token"))]
#[serial(aws_credentials_env)]
fn from_env_optional_rejects_incomplete_static_shapes(
	#[case] access: Option<&str>,
	#[case] secret: Option<&str>,
	#[case] token: Option<&str>,
) {
	let _guard = EnvGuard::replace(&[
		("AWS_ACCESS_KEY_ID", access),
		("AWS_SECRET_ACCESS_KEY", secret),
		("AWS_SESSION_TOKEN", token),
	]);

	let result = AwsCredentials::from_env_optional();

	match result {
		Err(ProviderError::Config(message)) => assert_eq!(
			message,
			"AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, and AWS_SESSION_TOKEN must form a complete static credential set"
		),
		other => panic!("unexpected credential result: {other:?}"),
	}
}

#[rstest]
#[serial(aws_credentials_env)]
fn from_env_optional_returns_none_for_absent_credentials() {
	let _guard = EnvGuard::replace(&[]);

	assert_eq!(
		AwsCredentials::from_env_optional().expect("valid absence"),
		None
	);
}

#[rstest]
#[serial(aws_credentials_env)]
fn from_env_requires_access_key_and_secret_access_key() {
	let _guard = EnvGuard::replace(&[]);

	let result = AwsCredentials::from_env();

	match result {
		Err(ProviderError::Config(message)) => assert_eq!(
			message,
			"AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY must be set"
		),
		other => panic!("unexpected credential result: {other:?}"),
	}
}

#[rstest]
#[case(None)]
#[case(Some("session-token"))]
#[serial(aws_credentials_env)]
fn from_env_optional_loads_complete_static_credentials(#[case] token: Option<&str>) {
	let _guard = EnvGuard::replace(&[
		("AWS_ACCESS_KEY_ID", Some("access-key")),
		("AWS_SECRET_ACCESS_KEY", Some("secret-key")),
		("AWS_SESSION_TOKEN", token),
	]);

	let credentials = AwsCredentials::from_env_optional()
		.expect("complete credentials are valid")
		.expect("complete credentials are present");

	assert_eq!(credentials.access_key_id(), "access-key");
	assert_eq!(credentials.secret_access_key(), "secret-key");
	assert_eq!(credentials.session_token(), token);
}

#[rstest]
#[tokio::test]
async fn static_source_resolves_without_region() {
	let credentials =
		AwsCredentials::new("access-key", "secret-key").with_session_token("session-token");
	let source = AwsCredentialsSource::Static(credentials.clone());

	let resolved = source.resolve().await.expect("static credentials resolve");

	assert_eq!(resolved.credentials, credentials);
	assert_eq!(resolved.region, None);
}

#[rstest]
#[case(None, "ap-northeast-1")]
#[case(Some("eu-west-1"), "eu-west-1")]
#[tokio::test]
#[serial(aws_credentials_env)]
async fn default_chain_resolves_environment_credentials_and_region_offline(
	#[case] region_override: Option<&str>,
	#[case] expected_region: &str,
) {
	let _guard = EnvGuard::replace(&[
		("AWS_ACCESS_KEY_ID", Some("chain-access")),
		("AWS_SECRET_ACCESS_KEY", Some("chain-secret")),
		("AWS_SESSION_TOKEN", Some("chain-token")),
		("AWS_REGION", Some("ap-northeast-1")),
		("AWS_EC2_METADATA_DISABLED", Some("true")),
	]);
	let source = AwsCredentialsSource::default_chain(region_override.map(str::to_owned));

	let resolved = source.resolve().await.expect("environment chain resolves");

	assert_eq!(resolved.credentials.access_key_id(), "chain-access");
	assert_eq!(resolved.credentials.secret_access_key(), "chain-secret");
	assert_eq!(resolved.credentials.session_token(), Some("chain-token"));
	assert_eq!(resolved.region.as_deref(), Some(expected_region));
}
