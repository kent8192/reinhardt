//! Environment variable parsing utilities
//!
//! Provides parsers for various data types including database URLs,
//! booleans, lists, and more.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;
/// Parse a boolean value from a string
///
/// Accepts: "true", "false", "1", "0", "yes", "no", "on", "off", "ok", "y", "n"
///
/// # Examples
///
/// ```
/// use reinhardt_conf::settings::env_parser::parse_bool;
///
/// assert!(parse_bool("true").unwrap());
/// assert!(parse_bool("1").unwrap());
/// assert!(parse_bool("yes").unwrap());
/// assert!(!parse_bool("false").unwrap());
/// assert!(!parse_bool("0").unwrap());
/// assert!(parse_bool("invalid").is_err());
/// ```
pub fn parse_bool(value: &str) -> Result<bool, String> {
	let normalized = value.trim().to_lowercase();

	match normalized.as_str() {
		"true" | "1" | "yes" | "on" | "ok" | "y" => Ok(true),
		"false" | "0" | "no" | "off" | "n" => Ok(false),
		_ => Err(format!("Invalid boolean value: {}", value)),
	}
}
/// Parse a comma-separated list
///
/// # Examples
///
/// ```
/// use reinhardt_conf::settings::env_parser::parse_list;
///
/// let list = parse_list("apple,banana,cherry");
/// assert_eq!(list, vec!["apple", "banana", "cherry"]);
///
/// let list_with_spaces = parse_list("foo, bar, baz");
/// assert_eq!(list_with_spaces, vec!["foo", "bar", "baz"]);
/// ```
pub fn parse_list(value: &str) -> Vec<String> {
	value
		.split(',')
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty())
		.collect()
}
/// Parse a dictionary-like string (key=value,key2=value2)
///
/// # Examples
///
/// ```
/// use reinhardt_conf::settings::env_parser::parse_dict;
///
/// let dict = parse_dict("host=localhost,port=5432,user=admin");
/// assert_eq!(dict.get("host").unwrap(), "localhost");
/// assert_eq!(dict.get("port").unwrap(), "5432");
/// assert_eq!(dict.get("user").unwrap(), "admin");
/// ```
pub fn parse_dict(value: &str) -> HashMap<String, String> {
	let mut map = HashMap::new();

	for pair in value.split(',') {
		if let Some((key, val)) = pair.split_once('=') {
			let key = key.trim().to_string();
			let val = val.trim().to_string();

			// Skip entries where both key and value are empty
			if !key.is_empty() || !val.is_empty() {
				map.insert(key, val);
			}
		}
	}

	map
}

/// Database URL configuration parsed from a connection string
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatabaseUrl {
	/// Database engine (postgresql, mysql, sqlite, etc.)
	pub engine: String,

	/// Database name
	pub name: String,

	/// Username (optional)
	pub user: Option<String>,

	/// Password (optional)
	pub password: Option<String>,

	/// Host (optional)
	pub host: Option<String>,

	/// Port (optional)
	pub port: Option<u16>,

	/// Query parameters as options
	pub options: HashMap<String, String>,

	/// Original URL string
	pub url: String,
}
/// Parse a database URL
///
/// Supports formats:
/// - sqlite:///path/to/db.sqlite3
/// - sqlite::memory:
/// - postgresql://user:pass@host:port/dbname
/// - mysql://user:pass@host:port/dbname
///
/// # Examples
///
/// ```
/// use reinhardt_conf::settings::env_parser::parse_database_url;
///
/// let db = parse_database_url("sqlite::memory:").unwrap();
/// assert_eq!(db.engine, "reinhardt.db.backends.sqlite3");
/// assert_eq!(db.name, ":memory:");
///
/// let db = parse_database_url("postgresql://user:pass@localhost:5432/mydb").unwrap();
/// assert_eq!(db.engine, "reinhardt.db.backends.postgresql");
/// assert_eq!(db.name, "mydb");
/// assert_eq!(db.user.unwrap(), "user");
/// ```
pub fn parse_database_url(url_str: &str) -> Result<DatabaseUrl, String> {
	// Handle SQLite special cases
	if url_str.starts_with("sqlite:") {
		return parse_sqlite_url(url_str);
	}

	let url = Url::parse(url_str).map_err(|e| format!("Invalid URL: {}", e))?;

	let scheme = url.scheme();
	let engine = match scheme {
		"postgresql" | "postgres" => "reinhardt.db.backends.postgresql",
		"mysql" | "mariadb" => "reinhardt.db.backends.mysql",
		"sqlite" => "reinhardt.db.backends.sqlite3",
		other => return Err(format!("Unsupported database scheme: {}", other)),
	};

	let name = url.path().trim_start_matches('/').to_string();

	if name.is_empty() && scheme != "sqlite" {
		return Err("Database name is required".to_string());
	}

	let user = if url.username().is_empty() {
		None
	} else {
		Some(url.username().to_string())
	};

	let password = url.password().map(|p| p.to_string());
	let host = url.host_str().map(|h| h.to_string());
	let port = url.port();

	// Parse query parameters
	let mut options = HashMap::new();
	for (key, value) in url.query_pairs() {
		options.insert(key.to_string(), value.to_string());
	}

	Ok(DatabaseUrl {
		engine: engine.to_string(),
		name,
		user,
		password,
		host,
		port,
		options,
		url: url_str.to_string(),
	})
}

/// Parse SQLite URL
fn parse_sqlite_url(url_str: &str) -> Result<DatabaseUrl, String> {
	let name = if url_str == "sqlite::memory:" || url_str == "sqlite://:memory:" {
		":memory:".to_string()
	} else if url_str.starts_with("sqlite:///") {
		url_str.trim_start_matches("sqlite:///").to_string()
	} else if url_str.starts_with("sqlite://") {
		url_str.trim_start_matches("sqlite://").to_string()
	} else if url_str.starts_with("sqlite:") {
		// Handle sqlite:db.sqlite3 format (single colon for relative paths)
		url_str.trim_start_matches("sqlite:").to_string()
	} else {
		return Err("Invalid SQLite URL format".to_string());
	};

	Ok(DatabaseUrl {
		engine: "reinhardt.db.backends.sqlite3".to_string(),
		name,
		user: None,
		password: None,
		host: None,
		port: None,
		options: HashMap::new(),
		url: url_str.to_string(),
	})
}

/// Cache URL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheUrl {
	pub backend: String,
	pub location: Option<String>,
	pub options: HashMap<String, String>,
}
/// Parse a cache URL
///
/// Supports:
/// - locmem://
/// - redis://host:port/db
/// - memcached://host:port
///
/// # Examples
///
/// ```
/// use reinhardt_conf::settings::env_parser::parse_cache_url;
///
/// let cache = parse_cache_url("locmem://").unwrap();
/// assert_eq!(cache.backend, "reinhardt.cache.backends.locmem.LocMemCache");
///
/// let cache = parse_cache_url("redis://localhost:6379/0").unwrap();
/// assert_eq!(cache.backend, "reinhardt.cache.backends.redis.RedisCache");
/// assert!(cache.location.is_some());
/// ```
pub fn parse_cache_url(url_str: &str) -> Result<CacheUrl, String> {
	if url_str == "locmem://" || url_str.starts_with("locmem://") {
		return Ok(CacheUrl {
			backend: "reinhardt.cache.backends.locmem.LocMemCache".to_string(),
			location: None,
			options: HashMap::new(),
		});
	}

	let url = Url::parse(url_str).map_err(|e| format!("Invalid cache URL: {}", e))?;

	let (backend, location) = match url.scheme() {
		"redis" => (
			"reinhardt.cache.backends.redis.RedisCache",
			Some(url_str.to_string()),
		),
		"memcached" => (
			"reinhardt.cache.backends.memcached.PyMemcacheCache",
			Some(format!(
				"{}:{}",
				url.host_str().unwrap_or("localhost"),
				url.port().unwrap_or(11211)
			)),
		),
		other => return Err(format!("Unsupported cache scheme: {}", other)),
	};

	let mut options = HashMap::new();
	for (key, value) in url.query_pairs() {
		options.insert(key.to_string(), value.to_string());
	}

	Ok(CacheUrl {
		backend: backend.to_string(),
		location,
		options,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_bool() {
		assert!(parse_bool("true").unwrap());
		assert!(parse_bool("True").unwrap());
		assert!(parse_bool("1").unwrap());
		assert!(parse_bool("yes").unwrap());
		assert!(parse_bool("on").unwrap());

		assert!(!parse_bool("false").unwrap());
		assert!(!parse_bool("False").unwrap());
		assert!(!parse_bool("0").unwrap());
		assert!(!parse_bool("no").unwrap());
		assert!(!parse_bool("off").unwrap());

		assert!(parse_bool("invalid").is_err());
	}

	#[test]
	fn test_parse_list() {
		assert_eq!(parse_list("a,b,c"), vec!["a", "b", "c"]);
		assert_eq!(parse_list("a, b, c"), vec!["a", "b", "c"]);
		assert_eq!(parse_list(""), Vec::<String>::new());
		assert_eq!(parse_list("single"), vec!["single"]);
	}

	#[test]
	fn test_parse_dict() {
		let dict = parse_dict("key1=value1,key2=value2");
		assert_eq!(dict.get("key1").unwrap(), "value1");
		assert_eq!(dict.get("key2").unwrap(), "value2");
	}

	#[test]
	fn test_parse_sqlite_memory() {
		let db = parse_database_url("sqlite::memory:").unwrap();
		assert_eq!(db.engine, "reinhardt.db.backends.sqlite3");
		assert_eq!(db.name, ":memory:");
		assert!(db.user.is_none());
	}

	#[test]
	fn test_parse_sqlite_file() {
		let db = parse_database_url("sqlite:///path/to/db.sqlite3").unwrap();
		assert_eq!(db.engine, "reinhardt.db.backends.sqlite3");
		assert_eq!(db.name, "path/to/db.sqlite3");
	}

	#[test]
	fn test_parse_postgresql() {
		let db = parse_database_url("postgresql://user:pass@localhost:5432/mydb").unwrap();
		assert_eq!(db.engine, "reinhardt.db.backends.postgresql");
		assert_eq!(db.name, "mydb");
		assert_eq!(db.user.unwrap(), "user");
		assert_eq!(db.password.unwrap(), "pass");
		assert_eq!(db.host.unwrap(), "localhost");
		assert_eq!(db.port.unwrap(), 5432);
	}

	#[test]
	fn test_parse_mysql() {
		let db = parse_database_url("mysql://root:secret@127.0.0.1:3306/testdb").unwrap();
		assert_eq!(db.engine, "reinhardt.db.backends.mysql");
		assert_eq!(db.name, "testdb");
		assert_eq!(db.user.unwrap(), "root");
		assert_eq!(db.password.unwrap(), "secret");
		assert_eq!(db.host.unwrap(), "127.0.0.1");
		assert_eq!(db.port.unwrap(), 3306);
	}

	#[test]
	fn test_parse_mariadb() {
		let db = parse_database_url("mariadb://root:secret@127.0.0.1:3306/testdb").unwrap();
		assert_eq!(db.engine, "reinhardt.db.backends.mysql");
		assert_eq!(db.name, "testdb");
		assert_eq!(db.user.unwrap(), "root");
		assert_eq!(db.password.unwrap(), "secret");
		assert_eq!(db.host.unwrap(), "127.0.0.1");
		assert_eq!(db.port.unwrap(), 3306);
	}

	#[test]
	fn test_parse_cache_locmem() {
		let cache = parse_cache_url("locmem://").unwrap();
		assert_eq!(cache.backend, "reinhardt.cache.backends.locmem.LocMemCache");
		assert!(cache.location.is_none());
	}

	#[test]
	fn test_parse_cache_redis() {
		let cache = parse_cache_url("redis://localhost:6379/0").unwrap();
		assert_eq!(cache.backend, "reinhardt.cache.backends.redis.RedisCache");
		assert_eq!(cache.location.unwrap(), "redis://localhost:6379/0");
	}
}
