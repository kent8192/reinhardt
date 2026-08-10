use super::*;
use chrono::TimeZone;

mod http;
mod signing;

pub(super) fn fixed_now() -> DateTime<Utc> {
	Utc.with_ymd_and_hms(2013, 5, 24, 0, 0, 0)
		.single()
		.expect("valid fixed signing time")
}

pub(super) fn test_config(endpoint: Option<String>) -> S3ClientConfig {
	S3ClientConfig {
		bucket: "examplebucket".to_string(),
		region: Some("us-east-1".to_string()),
		endpoint,
		credentials: AwsCredentialsSource::Static(AwsCredentials::new(
			"AKIAIOSFODNN7EXAMPLE",
			"wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
		)),
		force_path_style: false,
	}
}
