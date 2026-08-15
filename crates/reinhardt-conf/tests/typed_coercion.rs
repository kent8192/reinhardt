//! Integration tests for typed string coercion (issue #4226).

use reinhardt_conf::settings::typed_deserializer::{CoercionError, TypedSettingsDeserializer};

use rstest::rstest;
use serde::Deserialize;
use serde_json::json;

// --- bool / int coercion ----------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct Scalars {
	flag: bool,
	port: u16,
	count: i64,
}

#[rstest]
#[case::all_strings(
	json!({ "flag": "true", "port": "5432", "count": "-42" }),
	Scalars { flag: true, port: 5432, count: -42 }
)]
#[case::mixed(
	json!({ "flag": true, "port": "5432", "count": -42 }),
	Scalars { flag: true, port: 5432, count: -42 }
)]
#[case::all_native(
	json!({ "flag": false, "port": 8080, "count": 0 }),
	Scalars { flag: false, port: 8080, count: 0 }
)]
fn scalar_coerce_happy(#[case] v: serde_json::Value, #[case] expected: Scalars) {
	// Arrange
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let got: Scalars = Scalars::deserialize(de).expect("coerce should succeed");

	// Assert
	assert_eq!(got, expected);
}

#[rstest]
#[case::bad_int(json!({ "flag": "true", "port": "five-thousand", "count": "0" }), "port", "u16")]
#[case::bad_bool(json!({ "flag": "yep", "port": "1", "count": "0" }), "flag", "bool")]
fn scalar_coerce_error(
	#[case] v: serde_json::Value,
	#[case] expected_key: &str,
	#[case] expected_target: &str,
) {
	// Arrange
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let err = Scalars::deserialize(de).unwrap_err();

	// Assert
	let msg = err.to_string();
	assert!(matches!(err, CoercionError::Parse { .. }), "got: {err:?}");
	assert!(msg.contains(expected_key), "msg = {msg}");
	assert!(msg.contains(expected_target), "msg = {msg}");
}

// --- floats / char ---------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct FloatChar {
	rate: f64,
	sigil: char,
}

#[rstest]
#[case::strings(
	json!({ "rate": "1.5", "sigil": "x" }),
	FloatChar { rate: 1.5, sigil: 'x' }
)]
#[case::native(
	json!({ "rate": 2.25, "sigil": "z" }),
	FloatChar { rate: 2.25, sigil: 'z' }
)]
fn float_char_coerce_happy(#[case] v: serde_json::Value, #[case] expected: FloatChar) {
	// Arrange
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let got: FloatChar = FloatChar::deserialize(de).expect("ok");

	// Assert
	assert_eq!(got, expected);
}

#[rstest]
#[case::bad_float(json!({ "rate": "fast", "sigil": "x" }), "rate", "f64")]
#[case::char_too_long(json!({ "rate": "1.0", "sigil": "long" }), "sigil", "char")]
fn float_char_coerce_error(
	#[case] v: serde_json::Value,
	#[case] expected_key: &str,
	#[case] expected_target: &str,
) {
	// Arrange
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let err = FloatChar::deserialize(de).unwrap_err();

	// Assert
	let msg = err.to_string();
	assert!(matches!(err, CoercionError::Parse { .. }), "got: {err:?}");
	assert!(msg.contains(expected_key), "msg = {msg}");
	assert!(msg.contains(expected_target), "msg = {msg}");
}

// --- enum-unit -------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum LogLevel {
	Trace,
	Debug,
	Info,
	Warn,
	Error,
}

#[derive(Debug, Deserialize, PartialEq)]
struct WithLevel {
	level: LogLevel,
}

#[rstest]
fn enum_unit_coerce_from_string() {
	// Arrange
	let v = json!({ "level": "info" });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let got: WithLevel = WithLevel::deserialize(de).expect("ok");

	// Assert
	assert_eq!(
		got,
		WithLevel {
			level: LogLevel::Info
		}
	);
}

// --- Option<T> -------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct OptionalPort {
	port: Option<u16>,
}

#[rstest]
#[case::some_str(json!({ "port": "5432" }),  OptionalPort { port: Some(5432) })]
#[case::some_native(json!({ "port": 5432 }), OptionalPort { port: Some(5432) })]
#[case::none_empty(json!({ "port": "" }),    OptionalPort { port: None })]
#[case::none_null(json!({ "port": null }),   OptionalPort { port: None })]
#[case::none_missing(json!({}),              OptionalPort { port: None })]
fn option_coerce(#[case] v: serde_json::Value, #[case] expected: OptionalPort) {
	// Arrange
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let got: OptionalPort = OptionalPort::deserialize(de).expect("ok");

	// Assert
	assert_eq!(got, expected);
}

// --- bytes / Vec<u8> -------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithKey {
	#[serde(with = "serde_bytes")]
	key: Vec<u8>,
}

#[rstest]
fn bytes_coerce_from_base64() {
	// Arrange — "hello" in base64 = "aGVsbG8="
	let v = json!({ "key": "aGVsbG8=" });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let got: WithKey = WithKey::deserialize(de).expect("ok");

	// Assert
	assert_eq!(
		got,
		WithKey {
			key: b"hello".to_vec()
		}
	);
}

#[rstest]
fn bytes_coerce_invalid_base64_errors() {
	// Arrange
	let v = json!({ "key": "not-valid-base64!@#" });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let err = WithKey::deserialize(de).unwrap_err();

	// Assert
	let msg = err.to_string();
	assert!(matches!(err, CoercionError::Parse { .. }));
	assert!(
		msg.contains("bytes") || msg.contains("base64"),
		"msg = {msg}"
	);
}

// --- UnsupportedShape: struct from string ----------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct Endpoint {
	host: String,
	port: u16,
}

#[derive(Debug, Deserialize, PartialEq)]
struct WithEndpoint {
	endpoint: Endpoint,
}

#[rstest]
fn nested_struct_from_string_is_unsupported() {
	// Arrange
	let v = json!({ "endpoint": "{\"host\":\"localhost\",\"port\":5432}" });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let err = WithEndpoint::deserialize(de).unwrap_err();

	// Assert
	let msg = err.to_string();
	assert!(
		matches!(err, CoercionError::UnsupportedShape { .. }),
		"got: {err:?}"
	);
	assert!(msg.contains("struct"), "msg = {msg}");
	assert!(msg.contains("endpoint"), "msg = {msg}");
}

#[rstest]
fn nested_struct_from_object_works() {
	// Arrange — per-field interpolation is the recommended pattern
	let v = json!({ "endpoint": { "host": "localhost", "port": "5432" } });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let got: WithEndpoint = WithEndpoint::deserialize(de).expect("ok");

	// Assert
	assert_eq!(
		got,
		WithEndpoint {
			endpoint: Endpoint {
				host: "localhost".into(),
				port: 5432
			}
		}
	);
}

// --- UnsupportedShape: tuple from string -----------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct WithTuple {
	pair: (u16, u16),
}

#[rstest]
fn tuple_from_string_is_unsupported() {
	// Arrange
	let v = json!({ "pair": "(1, 2)" });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let err = WithTuple::deserialize(de).unwrap_err();

	// Assert
	let msg = err.to_string();
	assert!(
		matches!(err, CoercionError::UnsupportedShape { .. }),
		"got: {err:?}"
	);
	assert!(msg.contains("tuple"), "msg = {msg}");
	assert!(msg.contains("pair"), "msg = {msg}");
}

// --- UnsupportedShape: tuple_struct from string ----------------------

#[derive(Debug, Deserialize, PartialEq)]
struct Pair(u16, u16);

#[derive(Debug, Deserialize, PartialEq)]
struct WithTupleStruct {
	pair: Pair,
}

#[rstest]
fn tuple_struct_from_string_is_unsupported() {
	// Arrange
	let v = json!({ "pair": "(1, 2)" });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let err = WithTupleStruct::deserialize(de).unwrap_err();

	// Assert
	let msg = err.to_string();
	assert!(
		matches!(err, CoercionError::UnsupportedShape { .. }),
		"got: {err:?}"
	);
	assert!(
		msg.contains("tuple struct") || msg.contains("tuple"),
		"msg = {msg}"
	);
	assert!(msg.contains("pair"), "msg = {msg}");
}

// --- Vec<T> ----------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq)]
struct VecCases {
	ports: Vec<u16>,
	hosts: Vec<String>,
}

#[rstest]
#[case::vec_native(
	json!({ "ports": [5432, 5433], "hosts": ["a", "b"] }),
	VecCases { ports: vec![5432, 5433], hosts: vec!["a".into(), "b".into()] }
)]
#[case::vec_string_native_array(
	json!({ "ports": "[5432, 5433]", "hosts": "[\"a\", \"b\"]" }),
	VecCases { ports: vec![5432, 5433], hosts: vec!["a".into(), "b".into()] }
)]
#[case::vec_string_with_string_elements(
	json!({ "ports": "[\"5432\", \"5433\"]", "hosts": "[\"a\", \"b\"]" }),
	VecCases { ports: vec![5432, 5433], hosts: vec!["a".into(), "b".into()] }
)]
fn vec_coerce_happy(#[case] v: serde_json::Value, #[case] expected: VecCases) {
	// Arrange
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let got: VecCases = VecCases::deserialize(de).expect("ok");

	// Assert
	assert_eq!(got, expected);
}

#[rstest]
fn vec_invalid_json_errors() {
	// Arrange — a Vec<u16> field whose source is a string that doesn't parse as JSON array
	let v = json!({ "ports": "not-an-array", "hosts": [] });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let err = VecCases::deserialize(de).unwrap_err();

	// Assert
	assert!(matches!(err, CoercionError::Parse { .. }));
	let msg = err.to_string();
	assert!(
		msg.contains("array") || msg.contains("ports"),
		"msg = {msg}"
	);
}

#[rstest]
fn vec_string_with_object_inside_errors() {
	// Arrange — string contains JSON object (not array)
	let v = json!({ "ports": "{\"5432\":\"x\"}", "hosts": [] });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let err = VecCases::deserialize(de).unwrap_err();

	// Assert
	assert!(matches!(err, CoercionError::Parse { .. }));
}

// --- HashMap<K, V> ---------------------------------------------------

use std::collections::HashMap;

#[derive(Debug, Deserialize, PartialEq)]
struct WithWeights {
	weights: HashMap<String, i32>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct WithPortWeights {
	weights: HashMap<u16, i32>,
}

#[rstest]
#[case::map_native(
	json!({ "weights": { "a": 1, "b": 2 } }),
	WithWeights { weights: HashMap::from([("a".into(), 1), ("b".into(), 2)]) }
)]
#[case::map_string_with_native_values(
	json!({ "weights": "{\"a\": 1, \"b\": 2}" }),
	WithWeights { weights: HashMap::from([("a".into(), 1), ("b".into(), 2)]) }
)]
#[case::map_string_with_string_values(
	json!({ "weights": "{\"a\": \"1\", \"b\": \"2\"}" }),
	WithWeights { weights: HashMap::from([("a".into(), 1), ("b".into(), 2)]) }
)]
fn map_coerce_happy(#[case] v: serde_json::Value, #[case] expected: WithWeights) {
	// Arrange
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let got: WithWeights = WithWeights::deserialize(de).expect("ok");

	// Assert
	assert_eq!(got, expected);
}

#[rstest]
fn map_keys_coerce_to_the_declared_type() {
	// Arrange
	let settings = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value("weights", json!("{\"443\": \"1\"}")))
		.build()
		.expect("build");
	let expected = WithPortWeights {
		weights: HashMap::from([(443, 1)]),
	};

	// Act
	let got: WithPortWeights = settings.into_typed().expect("coerce");

	// Assert
	assert_eq!(got, expected);
}

#[rstest]
fn map_invalid_json_errors() {
	// Arrange — Map<String, i32> field whose source string isn't JSON
	let v = json!({ "weights": "not-json-at-all" });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let err = WithWeights::deserialize(de).unwrap_err();

	// Assert
	assert!(matches!(err, CoercionError::Parse { .. }));
	let msg = err.to_string();
	assert!(
		msg.contains("object") || msg.contains("weights"),
		"msg = {msg}"
	);
}

#[rstest]
fn map_string_with_array_inside_errors() {
	// Arrange — string contains JSON array (not object)
	let v = json!({ "weights": "[1, 2, 3]" });
	let de = TypedSettingsDeserializer::new(&v);

	// Act
	let err = WithWeights::deserialize(de).unwrap_err();

	// Assert
	assert!(matches!(err, CoercionError::Parse { .. }));
}

// --- builder integration --------------------------------------------

use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::sources::DefaultSource;

#[derive(Debug, Deserialize, PartialEq)]
struct DbSettings {
	host: String,
	port: u16,
	read_only: bool,
}

#[derive(Debug, Deserialize, PartialEq)]
struct WithDb {
	database: DbSettings,
}

#[rstest]
fn builder_coerces_strings_to_typed_fields() {
	// Arrange — DefaultSource pushes string-typed values that still
	// need to coerce into DbSettings.
	let settings = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value(
			"database",
			json!({
				"host": "localhost",
				"port": "5432",
				"read_only": "false",
			}),
		))
		.build()
		.expect("build");

	// Act
	let got: WithDb = settings.into_typed().expect("into_typed");

	// Assert
	assert_eq!(
		got,
		WithDb {
			database: DbSettings {
				host: "localhost".into(),
				port: 5432,
				read_only: false
			}
		}
	);
}

#[rstest]
fn builder_with_typed_coercion_disabled_falls_through_to_legacy() {
	// Arrange — same input as above, but coercion disabled.
	let result: Result<WithDb, _> = SettingsBuilder::new()
		.with_typed_coercion(false)
		.add_source(DefaultSource::new().with_value(
			"database",
			json!({
				"host": "localhost",
				"port": "5432",
				"read_only": "false",
			}),
		))
		.build()
		.unwrap()
		.into_typed();

	// Act / Assert — expect the legacy serde error path.
	assert!(
		result.is_err(),
		"legacy path should reject string-typed port"
	);
}

// --- hot reload (build path rejects coercion-breaking edits) -------

use reinhardt_conf::settings::sources::TomlFileSource;
use std::fs;
use tempfile::NamedTempFile;

#[rstest]
fn hot_reload_rejects_coercion_breaking_change_and_keeps_old_settings() {
	// Arrange — initial valid TOML that coerces cleanly.
	let tmp = NamedTempFile::new().expect("tempfile");
	fs::write(
		tmp.path(),
		r#"
[database]
host = "localhost"
port = "5432"
read_only = "false"
"#,
	)
	.expect("write initial");

	let initial: WithDb = SettingsBuilder::new()
		.add_source(TomlFileSource::new(tmp.path()))
		.build()
		.expect("initial build")
		.into_typed()
		.expect("initial typed");
	assert_eq!(initial.database.port, 5432);

	// Act — overwrite the file with a coercion-breaking value.
	fs::write(
		tmp.path(),
		r#"
[database]
host = "localhost"
port = "five-thousand"
read_only = "false"
"#,
	)
	.expect("write breaking");

	let result: Result<WithDb, _> = SettingsBuilder::new()
		.add_source(TomlFileSource::new(tmp.path()))
		.build()
		.expect("rebuild")
		.into_typed();

	// Assert — reload returns Err; previous in-memory settings stay
	// untouched (they are owned by this test; the watcher would
	// equivalently swap on success only).
	assert!(result.is_err(), "rebuild should reject coercion failure");
	assert_eq!(initial.database.port, 5432, "previous settings stay valid");
}
