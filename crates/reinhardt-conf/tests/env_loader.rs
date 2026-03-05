//! Integration tests for environment variable loading in reinhardt-conf.
//!
//! Tests cover:
//! - EnvLoader: .env file parsing, quoting, export prefix, variable interpolation,
//!   overwrite semantics, load_optional, missing files
//! - Env: string/int/bool/list/path/database_url reading with and without defaults,
//!   prefix support, missing-variable errors, parse errors
//! - env_parser helpers: parse_bool, parse_list, parse_dict, parse_database_url, parse_cache_url
//! - load_env / load_env_optional convenience functions
//! - validate_env_var_name validation rules

use reinhardt_conf::settings::env::{Env, EnvError};
use reinhardt_conf::settings::env_loader::{EnvLoader, load_env, load_env_optional};
use reinhardt_conf::settings::env_parser::{
	parse_bool, parse_cache_url, parse_database_url, parse_dict, parse_list,
};
use rstest::rstest;
use serial_test::serial;
use std::fs::File;
use std::io::Write as IoWrite;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a `.env`-style file to a temp directory and return both the dir and
/// the file path so the directory is not dropped early.
fn write_env_file(content: &str) -> (TempDir, std::path::PathBuf) {
	let dir = TempDir::new().expect("failed to create temp dir");
	let path = dir.path().join(".env");
	let mut f = File::create(&path).expect("failed to create .env file");
	f.write_all(content.as_bytes())
		.expect("failed to write .env file");
	(dir, path)
}

// ---------------------------------------------------------------------------
// EnvLoader – basic .env file loading
// ---------------------------------------------------------------------------

#[rstest]
#[serial(env)]
fn env_loader_loads_simple_key_value() {
	// Arrange
	let (_dir, path) = write_env_file("SIMPLE_KEY=simple_value\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("SIMPLE_KEY").unwrap(), "simple_value");

	// Cleanup
	unsafe { std::env::remove_var("SIMPLE_KEY") };
}

#[rstest]
#[serial(env)]
fn env_loader_skips_comment_lines() {
	// Arrange
	let (_dir, path) = write_env_file("# This is a comment\nCOMMENT_TEST=ok\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("COMMENT_TEST").unwrap(), "ok");

	// Cleanup
	unsafe { std::env::remove_var("COMMENT_TEST") };
}

#[rstest]
#[serial(env)]
fn env_loader_skips_empty_lines() {
	// Arrange
	let (_dir, path) = write_env_file("\n\nEMPTY_LINE_TEST=val\n\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("EMPTY_LINE_TEST").unwrap(), "val");

	// Cleanup
	unsafe { std::env::remove_var("EMPTY_LINE_TEST") };
}

#[rstest]
#[serial(env)]
fn env_loader_single_quoted_value() {
	// Arrange
	let (_dir, path) = write_env_file("SQ_VAR='single quoted value'\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("SQ_VAR").unwrap(), "single quoted value");

	// Cleanup
	unsafe { std::env::remove_var("SQ_VAR") };
}

#[rstest]
#[serial(env)]
fn env_loader_double_quoted_value() {
	// Arrange
	let (_dir, path) = write_env_file("DQ_VAR=\"double quoted value\"\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("DQ_VAR").unwrap(), "double quoted value");

	// Cleanup
	unsafe { std::env::remove_var("DQ_VAR") };
}

#[rstest]
#[serial(env)]
fn env_loader_export_prefix() {
	// Arrange
	let (_dir, path) = write_env_file("export EXPORTED_KEY=exported_value\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("EXPORTED_KEY").unwrap(), "exported_value");

	// Cleanup
	unsafe { std::env::remove_var("EXPORTED_KEY") };
}

#[rstest]
#[serial(env)]
fn env_loader_does_not_overwrite_by_default() {
	// Arrange
	unsafe { std::env::set_var("EXISTING_KEY", "original") };
	let (_dir, path) = write_env_file("EXISTING_KEY=overwritten\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.load()
		.expect("load should succeed");

	// Assert – original value preserved
	assert_eq!(std::env::var("EXISTING_KEY").unwrap(), "original");

	// Cleanup
	unsafe { std::env::remove_var("EXISTING_KEY") };
}

#[rstest]
#[serial(env)]
fn env_loader_overwrite_replaces_existing_value() {
	// Arrange
	unsafe { std::env::set_var("OVERWRITE_KEY", "original") };
	let (_dir, path) = write_env_file("OVERWRITE_KEY=replaced\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.overwrite(true)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("OVERWRITE_KEY").unwrap(), "replaced");

	// Cleanup
	unsafe { std::env::remove_var("OVERWRITE_KEY") };
}

#[rstest]
#[serial(env)]
fn env_loader_variable_interpolation_dollar_syntax() {
	// Arrange
	unsafe { std::env::set_var("BASE_PATH", "/home/user") };
	let (_dir, path) = write_env_file("FULL_PATH=$BASE_PATH/projects\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.interpolate(true)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("FULL_PATH").unwrap(), "/home/user/projects");

	// Cleanup
	unsafe {
		std::env::remove_var("BASE_PATH");
		std::env::remove_var("FULL_PATH");
	};
}

#[rstest]
#[serial(env)]
fn env_loader_variable_interpolation_braces_syntax() {
	// Arrange
	unsafe { std::env::set_var("APP_NAME", "reinhardt") };
	let (_dir, path) = write_env_file("APP_LOG=${APP_NAME}.log\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.interpolate(true)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("APP_LOG").unwrap(), "reinhardt.log");

	// Cleanup
	unsafe {
		std::env::remove_var("APP_NAME");
		std::env::remove_var("APP_LOG");
	};
}

#[rstest]
#[serial(env)]
fn env_loader_escaped_dollar_not_expanded() {
	// Arrange
	let (_dir, path) = write_env_file("LITERAL_DOLLAR=\\$not_a_var\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.interpolate(true)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("LITERAL_DOLLAR").unwrap(), "$not_a_var");

	// Cleanup
	unsafe { std::env::remove_var("LITERAL_DOLLAR") };
}

#[rstest]
#[serial(env)]
fn env_loader_unescape_newline() {
	// Arrange
	let (_dir, path) = write_env_file("MULTILINE_VAL=line1\\nline2\n");

	// Act
	EnvLoader::new()
		.path(&path)
		.load()
		.expect("load should succeed");

	// Assert
	assert_eq!(std::env::var("MULTILINE_VAL").unwrap(), "line1\nline2");

	// Cleanup
	unsafe { std::env::remove_var("MULTILINE_VAL") };
}

#[rstest]
#[serial(env)]
fn env_loader_returns_error_for_missing_file() {
	// Arrange
	let path = std::path::PathBuf::from("/tmp/nonexistent_env_file_for_test.env");

	// Act
	let result = EnvLoader::new().path(path).load();

	// Assert
	assert!(result.is_err());
}

#[rstest]
#[serial(env)]
fn env_loader_load_optional_returns_false_for_missing_file() {
	// Arrange
	let path = std::path::PathBuf::from("/tmp/nonexistent_optional.env");

	// Act
	let loaded = EnvLoader::new()
		.path(path)
		.load_optional()
		.expect("load_optional should not error for missing file");

	// Assert
	assert!(!loaded);
}

#[rstest]
#[serial(env)]
fn env_loader_load_optional_returns_true_for_existing_file() {
	// Arrange
	let (_dir, path) = write_env_file("OPTIONAL_KEY=optional_value\n");

	// Act
	let loaded = EnvLoader::new()
		.path(&path)
		.load_optional()
		.expect("load_optional should succeed");

	// Assert
	assert!(loaded);
	assert_eq!(std::env::var("OPTIONAL_KEY").unwrap(), "optional_value");

	// Cleanup
	unsafe { std::env::remove_var("OPTIONAL_KEY") };
}

// ---------------------------------------------------------------------------
// load_env / load_env_optional convenience functions
// ---------------------------------------------------------------------------

#[rstest]
#[serial(env)]
fn load_env_convenience_function() {
	// Arrange
	let (_dir, path) = write_env_file("CONVENIENCE_KEY=conv_value\n");

	// Act
	load_env(&path).expect("load_env should succeed");

	// Assert
	assert_eq!(std::env::var("CONVENIENCE_KEY").unwrap(), "conv_value");

	// Cleanup
	unsafe { std::env::remove_var("CONVENIENCE_KEY") };
}

#[rstest]
#[serial(env)]
fn load_env_optional_convenience_function_missing() {
	// Arrange
	let path = std::path::PathBuf::from("/tmp/does_not_exist_optional_conv.env");

	// Act
	let loaded =
		load_env_optional(path).expect("load_env_optional should not error for missing file");

	// Assert
	assert!(!loaded);
}

// ---------------------------------------------------------------------------
// Env – string type
// ---------------------------------------------------------------------------

#[rstest]
#[serial(env)]
fn env_str_reads_existing_variable() {
	// Arrange
	unsafe { std::env::set_var("ENV_STR_TEST", "hello_world") };
	let env = Env::new();

	// Act
	let result = env.str("ENV_STR_TEST").expect("str() should succeed");

	// Assert
	assert_eq!(result, "hello_world");

	// Cleanup
	unsafe { std::env::remove_var("ENV_STR_TEST") };
}

#[rstest]
#[serial(env)]
fn env_str_returns_error_for_missing_variable() {
	// Arrange
	let env = Env::new();

	// Act
	let result = env.str("ENV_STR_TOTALLY_MISSING_XYZ_123");

	// Assert
	assert!(matches!(result, Err(EnvError::MissingVariable(_))));
}

#[rstest]
#[serial(env)]
fn env_str_with_default_uses_default_when_missing() {
	// Arrange
	let env = Env::new();

	// Act
	let result = env
		.str_with_default("ENV_STR_MISSING_DEF", Some("fallback"))
		.expect("str_with_default should succeed");

	// Assert
	assert_eq!(result, "fallback");
}

// ---------------------------------------------------------------------------
// Env – integer type
// ---------------------------------------------------------------------------

#[rstest]
#[serial(env)]
fn env_int_reads_positive_integer() {
	// Arrange
	unsafe { std::env::set_var("ENV_INT_POS", "42") };
	let env = Env::new();

	// Act
	let result = env.int("ENV_INT_POS").expect("int() should succeed");

	// Assert
	assert_eq!(result, 42);

	// Cleanup
	unsafe { std::env::remove_var("ENV_INT_POS") };
}

#[rstest]
#[serial(env)]
fn env_int_reads_negative_integer() {
	// Arrange
	unsafe { std::env::set_var("ENV_INT_NEG", "-7") };
	let env = Env::new();

	// Act
	let result = env.int("ENV_INT_NEG").expect("int() should succeed");

	// Assert
	assert_eq!(result, -7);

	// Cleanup
	unsafe { std::env::remove_var("ENV_INT_NEG") };
}

#[rstest]
#[serial(env)]
fn env_int_returns_parse_error_for_non_numeric() {
	// Arrange
	unsafe { std::env::set_var("ENV_INT_BAD", "not_a_number") };
	let env = Env::new();

	// Act
	let result = env.int("ENV_INT_BAD");

	// Assert
	assert!(matches!(result, Err(EnvError::ParseError { .. })));

	// Cleanup
	unsafe { std::env::remove_var("ENV_INT_BAD") };
}

#[rstest]
#[serial(env)]
fn env_int_with_default_uses_default_when_missing() {
	// Arrange
	let env = Env::new();

	// Act
	let result = env
		.int_with_default("ENV_INT_MISSING_DEF", Some(99))
		.expect("int_with_default should succeed");

	// Assert
	assert_eq!(result, 99);
}

// ---------------------------------------------------------------------------
// Env – boolean type
// ---------------------------------------------------------------------------

#[rstest]
#[serial(env)]
#[case("true", true)]
#[case("1", true)]
#[case("yes", true)]
#[case("on", true)]
#[case("false", false)]
#[case("0", false)]
#[case("no", false)]
#[case("off", false)]
fn env_bool_various_truthy_falsy_values(#[case] raw: &str, #[case] expected: bool) {
	// Arrange
	unsafe { std::env::set_var("ENV_BOOL_CASE", raw) };
	let env = Env::new();

	// Act
	let result = env.bool("ENV_BOOL_CASE").expect("bool() should succeed");

	// Assert
	assert_eq!(result, expected);

	// Cleanup
	unsafe { std::env::remove_var("ENV_BOOL_CASE") };
}

#[rstest]
#[serial(env)]
fn env_bool_returns_parse_error_for_invalid_value() {
	// Arrange
	unsafe { std::env::set_var("ENV_BOOL_INVALID", "maybe") };
	let env = Env::new();

	// Act
	let result = env.bool("ENV_BOOL_INVALID");

	// Assert
	assert!(matches!(result, Err(EnvError::ParseError { .. })));

	// Cleanup
	unsafe { std::env::remove_var("ENV_BOOL_INVALID") };
}

#[rstest]
#[serial(env)]
fn env_bool_with_default_true_when_missing() {
	// Arrange
	let env = Env::new();

	// Act
	let result = env
		.bool_with_default("ENV_BOOL_MISSING_DEF", Some(true))
		.expect("bool_with_default should succeed");

	// Assert
	assert!(result);
}

// ---------------------------------------------------------------------------
// Env – list type
// ---------------------------------------------------------------------------

#[rstest]
#[serial(env)]
fn env_list_reads_comma_separated_values() {
	// Arrange
	unsafe { std::env::set_var("ENV_LIST_TEST", "alpha,beta,gamma") };
	let env = Env::new();

	// Act
	let result = env.list("ENV_LIST_TEST").expect("list() should succeed");

	// Assert
	assert_eq!(result, vec!["alpha", "beta", "gamma"]);

	// Cleanup
	unsafe { std::env::remove_var("ENV_LIST_TEST") };
}

#[rstest]
#[serial(env)]
fn env_list_trims_whitespace_around_items() {
	// Arrange
	unsafe { std::env::set_var("ENV_LIST_SPACES", "  a , b , c  ") };
	let env = Env::new();

	// Act
	let result = env.list("ENV_LIST_SPACES").expect("list() should succeed");

	// Assert
	assert_eq!(result, vec!["a", "b", "c"]);

	// Cleanup
	unsafe { std::env::remove_var("ENV_LIST_SPACES") };
}

#[rstest]
#[serial(env)]
fn env_list_with_default_when_missing() {
	// Arrange
	let env = Env::new();
	let default = vec!["x".to_string(), "y".to_string()];

	// Act
	let result = env
		.list_with_default("ENV_LIST_MISSING_DEF", Some(default.clone()))
		.expect("list_with_default should succeed");

	// Assert
	assert_eq!(result, default);
}

// ---------------------------------------------------------------------------
// Env – path type
// ---------------------------------------------------------------------------

#[rstest]
#[serial(env)]
fn env_path_reads_path_variable() {
	// Arrange
	unsafe { std::env::set_var("ENV_PATH_TEST", "/tmp/my_project") };
	let env = Env::new();

	// Act
	let result = env.path("ENV_PATH_TEST").expect("path() should succeed");

	// Assert
	assert_eq!(result, std::path::PathBuf::from("/tmp/my_project"));

	// Cleanup
	unsafe { std::env::remove_var("ENV_PATH_TEST") };
}

#[rstest]
#[serial(env)]
fn env_path_with_default_when_missing() {
	// Arrange
	let env = Env::new();
	let default = std::path::PathBuf::from("/default/path");

	// Act
	let result = env
		.path_with_default("ENV_PATH_MISSING_DEF", Some(default.clone()))
		.expect("path_with_default should succeed");

	// Assert
	assert_eq!(result, default);
}

// ---------------------------------------------------------------------------
// Env – prefix support
// ---------------------------------------------------------------------------

#[rstest]
#[serial(env)]
fn env_with_prefix_reads_prefixed_variable() {
	// Arrange
	unsafe { std::env::set_var("MYAPP_PORT", "8080") };
	let env = Env::new().with_prefix("MYAPP_");

	// Act
	let result = env.int("PORT").expect("int() with prefix should succeed");

	// Assert
	assert_eq!(result, 8080);

	// Cleanup
	unsafe { std::env::remove_var("MYAPP_PORT") };
}

#[rstest]
#[serial(env)]
fn env_with_prefix_missing_returns_missing_variable_error() {
	// Arrange
	let env = Env::new().with_prefix("MYAPP_");

	// Act
	let result = env.str("TOTALLY_NONEXISTENT_SUFFIX");

	// Assert
	assert!(
		matches!(result, Err(EnvError::MissingVariable(key)) if key == "MYAPP_TOTALLY_NONEXISTENT_SUFFIX")
	);
}

// ---------------------------------------------------------------------------
// Env – database URL
// ---------------------------------------------------------------------------

#[rstest]
#[serial(env)]
fn env_database_url_reads_postgresql_url() {
	// Arrange
	unsafe {
		std::env::set_var(
			"DATABASE_URL_TEST",
			"postgresql://admin:secret@localhost:5432/testdb",
		)
	};
	let env = Env::new();

	// Act
	let db = env
		.database_url("DATABASE_URL_TEST")
		.expect("database_url() should succeed");

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.postgresql");
	assert_eq!(db.name, "testdb");
	assert_eq!(db.user.unwrap(), "admin");
	assert_eq!(db.password.unwrap(), "secret");
	assert_eq!(db.host.unwrap(), "localhost");
	assert_eq!(db.port.unwrap(), 5432);

	// Cleanup
	unsafe { std::env::remove_var("DATABASE_URL_TEST") };
}

#[rstest]
#[serial(env)]
fn env_database_url_with_default_sqlite_memory() {
	// Arrange
	let env = Env::new();

	// Act
	let db = env
		.database_url_with_default("DB_URL_MISSING", Some("sqlite::memory:"))
		.expect("database_url_with_default should succeed");

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.sqlite3");
	assert_eq!(db.name, ":memory:");
}

// ---------------------------------------------------------------------------
// parse_bool standalone tests
// ---------------------------------------------------------------------------

#[rstest]
#[case("ok", true)]
#[case("y", true)]
#[case("n", false)]
fn parse_bool_extended_truthy_falsy_values(#[case] input: &str, #[case] expected: bool) {
	// Act
	let result = parse_bool(input).expect("parse_bool should succeed");

	// Assert
	assert_eq!(result, expected);
}

#[rstest]
fn parse_bool_returns_error_for_unknown_value() {
	// Act
	let result = parse_bool("maybe");

	// Assert
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// parse_list standalone tests
// ---------------------------------------------------------------------------

#[rstest]
fn parse_list_empty_string_returns_empty_vec() {
	// Act
	let result = parse_list("");

	// Assert
	assert!(result.is_empty());
}

#[rstest]
fn parse_list_single_item_no_comma() {
	// Act
	let result = parse_list("only_item");

	// Assert
	assert_eq!(result, vec!["only_item"]);
}

#[rstest]
fn parse_list_filters_empty_segments() {
	// Arrange – trailing comma produces empty segment
	let result = parse_list("a,b,");

	// Assert
	assert_eq!(result, vec!["a", "b"]);
}

// ---------------------------------------------------------------------------
// parse_dict standalone tests
// ---------------------------------------------------------------------------

#[rstest]
fn parse_dict_basic_key_value_pairs() {
	// Act
	let dict = parse_dict("host=localhost,port=5432,dbname=mydb");

	// Assert
	assert_eq!(dict.get("host").unwrap(), "localhost");
	assert_eq!(dict.get("port").unwrap(), "5432");
	assert_eq!(dict.get("dbname").unwrap(), "mydb");
}

#[rstest]
fn parse_dict_trims_whitespace_from_keys_and_values() {
	// Act
	let dict = parse_dict(" host = localhost , port = 5432 ");

	// Assert
	assert_eq!(dict.get("host").unwrap(), "localhost");
	assert_eq!(dict.get("port").unwrap(), "5432");
}

// ---------------------------------------------------------------------------
// parse_database_url standalone tests
// ---------------------------------------------------------------------------

#[rstest]
fn parse_database_url_sqlite_memory() {
	// Act
	let db = parse_database_url("sqlite::memory:").expect("parse should succeed");

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.sqlite3");
	assert_eq!(db.name, ":memory:");
	assert!(db.host.is_none());
	assert!(db.port.is_none());
}

#[rstest]
fn parse_database_url_sqlite_file_path() {
	// Act
	let db = parse_database_url("sqlite:///var/data/app.db").expect("parse should succeed");

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.sqlite3");
	assert_eq!(db.name, "var/data/app.db");
}

#[rstest]
fn parse_database_url_mysql() {
	// Act
	let db =
		parse_database_url("mysql://root:pass@127.0.0.1:3306/shop").expect("parse should succeed");

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.mysql");
	assert_eq!(db.name, "shop");
	assert_eq!(db.user.unwrap(), "root");
	assert_eq!(db.port.unwrap(), 3306);
}

#[rstest]
fn parse_database_url_unsupported_scheme_returns_error() {
	// Act
	let result = parse_database_url("ftp://localhost/dbname");

	// Assert
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// parse_cache_url standalone tests
// ---------------------------------------------------------------------------

#[rstest]
fn parse_cache_url_locmem() {
	// Act
	let cache = parse_cache_url("locmem://").expect("parse should succeed");

	// Assert
	assert_eq!(cache.backend, "reinhardt.cache.backends.locmem.LocMemCache");
	assert!(cache.location.is_none());
}

#[rstest]
fn parse_cache_url_redis() {
	// Act
	let cache = parse_cache_url("redis://localhost:6379/1").expect("parse should succeed");

	// Assert
	assert_eq!(cache.backend, "reinhardt.cache.backends.redis.RedisCache");
	assert_eq!(
		cache.location.as_deref().unwrap(),
		"redis://localhost:6379/1"
	);
}

#[rstest]
fn parse_cache_url_memcached() {
	// Act
	let cache = parse_cache_url("memcached://localhost:11211").expect("parse should succeed");

	// Assert
	assert_eq!(
		cache.backend,
		"reinhardt.cache.backends.memcached.PyMemcacheCache"
	);
	assert_eq!(cache.location.as_deref().unwrap(), "localhost:11211");
}

#[rstest]
fn parse_cache_url_unsupported_scheme_returns_error() {
	// Act
	let result = parse_cache_url("unknown://host");

	// Assert
	assert!(result.is_err());
}
