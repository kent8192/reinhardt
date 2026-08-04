//! Cross-crate JWT settings secret contract.

use reinhardt_auth::JwtSessionSettings;
use reinhardt_conf::settings::schema::{FieldRef, SettingsNode, SettingsPathBuf};
use rstest::rstest;

#[rstest]
fn jwt_settings_preserve_string_reference_and_secret_metadata() {
	let settings: JwtSessionSettings = serde_json::from_value(serde_json::json!({
		"secret": { "secret": "super-secret-signing-key" }
	}))
	.expect("secret source should deserialize");
	let schema = JwtSessionSettings::schema_at::<JwtSessionSettings>(SettingsPathBuf::new());
	let _: FieldRef<JwtSessionSettings, String> = schema.secret;
	let mut secret_paths = Vec::new();
	JwtSessionSettings::node_schema().collect_secret_paths(&mut secret_paths);

	assert_eq!(settings.secret, "super-secret-signing-key");
	assert_eq!(secret_paths.len(), 1);
	assert_eq!(secret_paths[0].to_string(), "secret");
}
