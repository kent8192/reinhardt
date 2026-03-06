use std::path::Path;

use syn::{Item, UseTree};
use walkdir::WalkDir;

use crate::config::NoSqlEngine;
use crate::detection::feature_flags::FeatureDetectionResult;
use crate::error::{DeployError, DeployResult};

/// Recursively collect fully-qualified path segments from a [`UseTree`].
///
/// Each returned `Vec<String>` represents one import path. For example,
/// `use reinhardt::{db::Model, cache::Backend}` produces two paths:
/// `["reinhardt", "db", "Model"]` and `["reinhardt", "cache", "Backend"]`.
fn collect_use_paths(tree: &UseTree, prefix: &[String]) -> Vec<Vec<String>> {
	match tree {
		UseTree::Path(use_path) => {
			let mut new_prefix = prefix.to_vec();
			new_prefix.push(use_path.ident.to_string());
			collect_use_paths(&use_path.tree, &new_prefix)
		}
		UseTree::Name(use_name) => {
			let mut path = prefix.to_vec();
			path.push(use_name.ident.to_string());
			vec![path]
		}
		UseTree::Rename(use_rename) => {
			let mut path = prefix.to_vec();
			path.push(use_rename.ident.to_string());
			vec![path]
		}
		UseTree::Glob(_) => {
			vec![prefix.to_vec()]
		}
		UseTree::Group(use_group) => use_group
			.items
			.iter()
			.flat_map(|item| collect_use_paths(item, prefix))
			.collect(),
	}
}

/// Classify a single import path and set the corresponding feature flags.
///
/// Only paths starting with `["reinhardt", ...]` with at least two segments
/// are considered. The second segment determines the feature category.
fn classify_import(segments: &[String], result: &mut FeatureDetectionResult) {
	if segments.len() < 2 || segments[0] != "reinhardt" {
		return;
	}

	match segments[1].as_str() {
		"db" | "orm" => result.database = true,
		"channels" | "websocket" => result.websockets = true,
		"cache" => result.cache = true,
		"pages" => {
			if segments.get(2).is_some_and(|s| s == "wasm") {
				result.wasm = true;
			} else {
				result.frontend = true;
			}
		}
		"views" if segments.get(2).is_some_and(|s| s == "template") => result.frontend = true,
		"static_files" => result.static_files = true,
		"media" => result.media = true,
		"tasks" | "celery" => result.background_tasks = true,
		"mail" => result.mail = true,
		"wasm" => result.wasm = true,
		"nosql" => {
			result.nosql = true;
			if let Some(engine) = segments.get(2) {
				let nosql_engine = match engine.as_str() {
					"mongodb" => Some(NoSqlEngine::MongoDb),
					"dynamodb" => Some(NoSqlEngine::DynamoDb),
					"firestore" => Some(NoSqlEngine::Firestore),
					_ => None,
				};
				if let Some(e) = nosql_engine
					&& !result.nosql_engines.contains(&e)
				{
					result.nosql_engines.push(e);
				}
			}
		}
		_ => {}
	}
}

/// Parse a single Rust source file and update feature detection results.
///
/// Uses `syn::parse_file` to build an AST, then inspects `use` items for
/// reinhardt imports and struct/enum items for `#[model(...)]` attributes.
fn analyze_file(
	content: &str,
	path: &Path,
	result: &mut FeatureDetectionResult,
) -> DeployResult<()> {
	let file = syn::parse_file(content).map_err(|e| DeployError::Detection {
		message: format!("failed to parse {}: {e}", path.display()),
	})?;

	for item in &file.items {
		match item {
			Item::Use(item_use) => {
				let paths = collect_use_paths(&item_use.tree, &[]);
				for path in &paths {
					classify_import(path, result);
				}
			}
			Item::Struct(item_struct) => {
				for attr in &item_struct.attrs {
					if attr.path().is_ident("model") {
						result.database = true;
						result.model_count += 1;
					}
				}
			}
			Item::Enum(item_enum) => {
				for attr in &item_enum.attrs {
					if attr.path().is_ident("model") {
						result.database = true;
						result.model_count += 1;
					}
				}
			}
			_ => {}
		}
	}

	Ok(())
}

/// Analyze source code to detect infrastructure requirements.
///
/// Walks the `src/` directory recursively, scanning `.rs` files using
/// `syn` AST parsing to detect `use reinhardt::*` import patterns and
/// `#[model(` attribute patterns. Returns a [`FeatureDetectionResult`]
/// with confidence 0.8 (code analysis provides moderate confidence since
/// imports may not reflect runtime usage).
///
/// If the `src/` directory does not exist, returns an empty result with
/// confidence 0.0.
pub fn analyze_code(project_root: &Path) -> DeployResult<FeatureDetectionResult> {
	let src_dir = project_root.join("src");

	if !src_dir.exists() {
		return Ok(FeatureDetectionResult {
			database: false,
			database_engine: None,
			nosql: false,
			nosql_engines: Vec::new(),
			cache: false,
			websockets: false,
			frontend: false,
			static_files: false,
			media: false,
			background_tasks: false,
			mail: false,
			wasm: false,
			ambiguous: false,
			model_count: 0,
			confidence: 0.0,
		});
	}

	let mut result = FeatureDetectionResult {
		database: false,
		database_engine: None,
		nosql: false,
		nosql_engines: Vec::new(),
		cache: false,
		websockets: false,
		frontend: false,
		static_files: false,
		media: false,
		background_tasks: false,
		mail: false,
		wasm: false,
		ambiguous: false,
		model_count: 0,
		confidence: 0.8,
	};

	for entry in WalkDir::new(&src_dir).into_iter() {
		let entry = entry.map_err(|e| DeployError::Detection {
			message: format!("failed to walk directory: {e}"),
		})?;
		if !entry.file_type().is_file() || entry.path().extension().is_none_or(|ext| ext != "rs") {
			continue;
		}
		let content =
			std::fs::read_to_string(entry.path()).map_err(|e| DeployError::Detection {
				message: format!("failed to read {}: {}", entry.path().display(), e),
			})?;

		analyze_file(&content, entry.path(), &mut result)?;
	}

	Ok(result)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn detect_database_from_use_statements() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			r#"
use reinhardt::db::models::Model;
use reinhardt::db::QuerySet;

fn main() {}
"#,
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.database);
	}

	#[rstest]
	fn detect_database_from_orm_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(src_dir.join("main.rs"), "use reinhardt::orm::QuerySet;\n").unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.database);
	}

	#[rstest]
	fn detect_websockets_from_use_statements() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			r#"
use reinhardt::channels::WebSocketConsumer;

fn main() {}
"#,
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.websockets);
	}

	#[rstest]
	fn detect_websockets_from_websocket_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::websocket::Connection;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.websockets);
	}

	#[rstest]
	fn detect_cache_from_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::cache::CacheBackend;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.cache);
	}

	#[rstest]
	fn detect_frontend_from_pages_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::pages::TemplateView;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.frontend);
	}

	#[rstest]
	fn detect_frontend_from_views_template_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::views::template::Render;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.frontend);
	}

	#[rstest]
	fn detect_static_files_from_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::static_files::StaticHandler;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.static_files);
	}

	#[rstest]
	fn detect_media_from_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::media::FileStorage;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.media);
	}

	#[rstest]
	fn detect_background_tasks_from_tasks_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::tasks::TaskRunner;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.background_tasks);
	}

	#[rstest]
	fn detect_background_tasks_from_celery_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(src_dir.join("main.rs"), "use reinhardt::celery::Worker;\n").unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.background_tasks);
	}

	#[rstest]
	fn detect_mail_from_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::mail::EmailBackend;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.mail);
	}

	#[rstest]
	fn detect_model_attribute() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("models.rs"),
			r#"
#[model(table_name = "users")]
pub struct User {
    pub id: i64,
    pub name: String,
}
"#,
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.database);
		assert!(result.model_count > 0);
	}

	#[rstest]
	fn detect_multiple_models() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("models.rs"),
			r#"
#[model(table_name = "users")]
pub struct User {
    pub id: i64,
    pub name: String,
}

#[model(table_name = "posts")]
pub struct Post {
    pub id: i64,
    pub title: String,
}

#[model(table_name = "comments")]
pub struct Comment {
    pub id: i64,
    pub body: String,
}
"#,
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.database);
		assert_eq!(result.model_count, 3);
	}

	#[rstest]
	fn detect_nosql_mongodb_from_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::nosql::mongodb::Collection;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.nosql);
		assert_eq!(result.nosql_engines, vec![NoSqlEngine::MongoDb]);
	}

	#[rstest]
	fn detect_nosql_dynamodb_from_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::nosql::dynamodb::Table;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.nosql);
		assert_eq!(result.nosql_engines, vec![NoSqlEngine::DynamoDb]);
	}

	#[rstest]
	fn detect_nosql_firestore_from_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::nosql::firestore::Document;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.nosql);
		assert_eq!(result.nosql_engines, vec![NoSqlEngine::Firestore]);
	}

	#[rstest]
	fn detect_wasm_from_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::wasm::WasmComponent;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.wasm);
	}

	#[rstest]
	fn detect_wasm_from_pages_wasm_use() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::pages::wasm::WasmApp;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.wasm);
	}

	#[rstest]
	fn no_src_dir_returns_empty_result() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		// No src/ directory created

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(!result.database);
		assert!(!result.nosql);
		assert!(!result.cache);
		assert!(!result.websockets);
		assert!(!result.frontend);
		assert!(!result.static_files);
		assert!(!result.media);
		assert!(!result.background_tasks);
		assert!(!result.mail);
		assert_eq!(result.model_count, 0);
		assert_eq!(result.confidence, 0.0);
	}

	#[rstest]
	fn empty_src_dir_returns_default_result() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(!result.database);
		assert!(!result.nosql);
		assert!(!result.cache);
		assert!(!result.websockets);
		assert_eq!(result.model_count, 0);
		assert_eq!(result.confidence, 0.8);
	}

	#[rstest]
	fn detect_features_across_multiple_files() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(src_dir.join("db.rs"), "use reinhardt::db::models::Model;\n").unwrap();
		std::fs::write(
			src_dir.join("ws.rs"),
			"use reinhardt::channels::Consumer;\n",
		)
		.unwrap();
		std::fs::write(src_dir.join("email.rs"), "use reinhardt::mail::Mailer;\n").unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.database);
		assert!(result.websockets);
		assert!(result.mail);
		assert_eq!(result.confidence, 0.8);
	}

	#[rstest]
	fn detect_features_in_nested_directories() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let nested_dir = tmp.path().join("src").join("app").join("handlers");
		std::fs::create_dir_all(&nested_dir).unwrap();
		std::fs::write(
			nested_dir.join("cache_handler.rs"),
			"use reinhardt::cache::RedisBackend;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.cache);
	}

	#[rstest]
	fn non_rs_files_are_ignored() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("config.toml"),
			"use reinhardt::db::models::Model;\n",
		)
		.unwrap();
		std::fs::write(
			src_dir.join("readme.md"),
			"use reinhardt::cache::Backend;\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(!result.database);
		assert!(!result.cache);
	}

	// --- New AST-specific tests ---

	#[rstest]
	fn detect_group_imports() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			r#"
use reinhardt::{db::Model, cache::Backend};

fn main() {}
"#,
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.database);
		assert!(result.cache);
	}

	#[rstest]
	fn detect_nested_group_imports() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			r#"
use reinhardt::nosql::{mongodb::Collection, dynamodb::Table};

fn main() {}
"#,
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.nosql);
		assert_eq!(result.nosql_engines.len(), 2);
		assert!(result.nosql_engines.contains(&NoSqlEngine::MongoDb));
		assert!(result.nosql_engines.contains(&NoSqlEngine::DynamoDb));
	}

	#[rstest]
	fn detect_glob_imports() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::db::*;\n\nfn main() {}\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.database);
	}

	#[rstest]
	fn detect_rename_imports() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(
			src_dir.join("main.rs"),
			"use reinhardt::cache::Backend as CacheBackend;\n\nfn main() {}\n",
		)
		.unwrap();

		// Act
		let result = analyze_code(tmp.path()).unwrap();

		// Assert
		assert!(result.cache);
	}

	#[rstest]
	fn syntax_error_returns_detection_error() {
		// Arrange
		let tmp = tempfile::tempdir().unwrap();
		let src_dir = tmp.path().join("src");
		std::fs::create_dir_all(&src_dir).unwrap();
		std::fs::write(src_dir.join("broken.rs"), "fn main() { let x = ;\n").unwrap();

		// Act
		let err = analyze_code(tmp.path()).unwrap_err();

		// Assert
		assert!(
			matches!(err, DeployError::Detection { .. }),
			"expected DeployError::Detection, got {err:?}"
		);
	}
}
