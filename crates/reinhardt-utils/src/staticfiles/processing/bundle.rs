//! Asset bundling and concatenation
//!
//! Combines multiple CSS or JavaScript files into single bundles,
//! resolving dependencies and maintaining proper order.

use super::ProcessingResult;
use crate::staticfiles::DependencyGraph;
use std::collections::HashMap;
use std::path::PathBuf;

/// Asset bundler
///
/// Combines multiple files into a single bundle, resolving dependencies.
pub struct AssetBundler {
	/// Dependency graph for resolving file order
	graph: DependencyGraph,
	/// File contents cache
	files: HashMap<PathBuf, Vec<u8>>,
}

impl AssetBundler {
	/// Create a new asset bundler
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::processing::bundle::AssetBundler;
	///
	/// let bundler = AssetBundler::new();
	/// ```
	pub fn new() -> Self {
		Self {
			graph: DependencyGraph::new(),
			files: HashMap::new(),
		}
	}

	/// Add a file to the bundle
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::processing::bundle::AssetBundler;
	/// use std::path::PathBuf;
	///
	/// let mut bundler = AssetBundler::new();
	/// bundler.add_file(PathBuf::from("app.js"), b"console.log('hello');".to_vec());
	/// ```
	pub fn add_file(&mut self, path: PathBuf, content: Vec<u8>) {
		self.graph.add_file(path.to_string_lossy().to_string());
		self.files.insert(path, content);
	}

	/// Add a dependency between files
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::processing::bundle::AssetBundler;
	/// use std::path::PathBuf;
	///
	/// let mut bundler = AssetBundler::new();
	/// bundler.add_file(PathBuf::from("main.js"), b"import './utils.js';".to_vec());
	/// bundler.add_file(PathBuf::from("utils.js"), b"export const fn = () => {};".to_vec());
	/// bundler.add_dependency(
	///     PathBuf::from("main.js"),
	///     PathBuf::from("utils.js")
	/// );
	/// ```
	pub fn add_dependency(&mut self, from: PathBuf, to: PathBuf) {
		self.graph.add_dependency(
			from.to_string_lossy().to_string(),
			to.to_string_lossy().to_string(),
		);
	}

	/// Bundle all files in dependency order
	///
	/// # Returns
	///
	/// The bundled content with all files concatenated in the correct order.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::processing::bundle::AssetBundler;
	/// use std::path::PathBuf;
	///
	/// let mut bundler = AssetBundler::new();
	/// bundler.add_file(PathBuf::from("a.js"), b"const a = 1;".to_vec());
	/// bundler.add_file(PathBuf::from("b.js"), b"const b = 2;".to_vec());
	///
	/// let bundle = bundler.bundle().unwrap();
	/// assert!(bundle.len() > 0);
	/// ```
	pub fn bundle(&self) -> ProcessingResult<Vec<u8>> {
		let order = self.graph.resolve_order();
		let mut result = Vec::new();

		for file_name in order {
			let path = PathBuf::from(&file_name);
			if let Some(content) = self.files.get(&path) {
				// Add separator comment
				let separator = format!("\n/* {} */\n", file_name);
				result.extend_from_slice(separator.as_bytes());
				result.extend_from_slice(content);
				result.push(b'\n');
			}
		}

		Ok(result)
	}

	/// Bundle specific files (ignoring dependencies)
	///
	/// # Arguments
	///
	/// * `paths` - The files to bundle, in the order specified
	///
	/// # Returns
	///
	/// The bundled content
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::processing::bundle::AssetBundler;
	/// use std::path::PathBuf;
	///
	/// let mut bundler = AssetBundler::new();
	/// bundler.add_file(PathBuf::from("a.js"), b"const a = 1;".to_vec());
	/// bundler.add_file(PathBuf::from("b.js"), b"const b = 2;".to_vec());
	///
	/// let bundle = bundler.bundle_files(&[
	///     PathBuf::from("b.js"),
	///     PathBuf::from("a.js"),
	/// ]).unwrap();
	/// ```
	pub fn bundle_files(&self, paths: &[PathBuf]) -> ProcessingResult<Vec<u8>> {
		let mut result = Vec::new();

		for path in paths {
			if let Some(content) = self.files.get(path) {
				let separator = format!("\n/* {} */\n", path.display());
				result.extend_from_slice(separator.as_bytes());
				result.extend_from_slice(content);
				result.push(b'\n');
			}
		}

		Ok(result)
	}

	/// Get the number of files in the bundler
	pub fn len(&self) -> usize {
		self.files.len()
	}

	/// Check if the bundler is empty
	pub fn is_empty(&self) -> bool {
		self.files.is_empty()
	}
}

impl Default for AssetBundler {
	fn default() -> Self {
		Self::new()
	}
}

/// Bundle configuration
#[derive(Debug, Clone)]
pub struct BundleConfig {
	/// Output file name
	pub output: PathBuf,
	/// Files to include in the bundle
	pub files: Vec<PathBuf>,
	/// Enable minification
	pub minify: bool,
	/// Include source maps
	pub source_map: bool,
}

impl BundleConfig {
	/// Create a new bundle configuration
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::processing::bundle::BundleConfig;
	/// use std::path::PathBuf;
	///
	/// let config = BundleConfig::new(PathBuf::from("bundle.js"));
	/// ```
	pub fn new(output: PathBuf) -> Self {
		Self {
			output,
			files: Vec::new(),
			minify: false,
			source_map: false,
		}
	}

	/// Add a file to the bundle
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::processing::bundle::BundleConfig;
	/// use std::path::PathBuf;
	///
	/// let mut config = BundleConfig::new(PathBuf::from("bundle.js"));
	/// config.add_file(PathBuf::from("app.js"));
	/// config.add_file(PathBuf::from("utils.js"));
	/// ```
	pub fn add_file(&mut self, path: PathBuf) {
		self.files.push(path);
	}

	/// Enable minification
	pub fn with_minify(mut self, enable: bool) -> Self {
		self.minify = enable;
		self
	}

	/// Enable source maps
	pub fn with_source_map(mut self, enable: bool) -> Self {
		self.source_map = enable;
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_bundler_creation() {
		let bundler = AssetBundler::new();
		assert!(bundler.is_empty());
		assert_eq!(bundler.len(), 0);
	}

	#[test]
	fn test_bundler_add_file() {
		let mut bundler = AssetBundler::new();
		bundler.add_file(PathBuf::from("test.js"), b"const x = 1;".to_vec());
		assert_eq!(bundler.len(), 1);
		assert!(!bundler.is_empty());
	}

	#[test]
	fn test_bundler_simple_bundle() {
		let mut bundler = AssetBundler::new();
		bundler.add_file(PathBuf::from("a.js"), b"const a = 1;".to_vec());
		bundler.add_file(PathBuf::from("b.js"), b"const b = 2;".to_vec());

		let result = bundler.bundle().unwrap();
		let output = String::from_utf8(result).unwrap();

		assert!(output.contains("const a = 1;"));
		assert!(output.contains("const b = 2;"));
	}

	#[test]
	fn test_bundler_with_dependencies() {
		let mut bundler = AssetBundler::new();
		bundler.add_file(PathBuf::from("main.js"), b"// main".to_vec());
		bundler.add_file(PathBuf::from("utils.js"), b"// utils".to_vec());
		bundler.add_dependency(PathBuf::from("main.js"), PathBuf::from("utils.js"));

		let result = bundler.bundle().unwrap();
		let output = String::from_utf8(result).unwrap();

		// utils.js should come before main.js due to dependency
		let utils_pos = output.find("// utils").unwrap();
		let main_pos = output.find("// main").unwrap();
		assert!(utils_pos < main_pos);
	}

	#[test]
	fn test_bundler_bundle_files_custom_order() {
		let mut bundler = AssetBundler::new();
		bundler.add_file(PathBuf::from("a.js"), b"const a = 1;".to_vec());
		bundler.add_file(PathBuf::from("b.js"), b"const b = 2;".to_vec());

		let result = bundler
			.bundle_files(&[PathBuf::from("b.js"), PathBuf::from("a.js")])
			.unwrap();
		let output = String::from_utf8(result).unwrap();

		// b.js should come before a.js
		let b_pos = output.find("const b = 2;").unwrap();
		let a_pos = output.find("const a = 1;").unwrap();
		assert!(b_pos < a_pos);
	}

	#[test]
	fn test_bundler_includes_separators() {
		let mut bundler = AssetBundler::new();
		bundler.add_file(PathBuf::from("test.js"), b"code".to_vec());

		let result = bundler.bundle().unwrap();
		let output = String::from_utf8(result).unwrap();

		assert!(output.contains("/* test.js */"));
	}

	#[test]
	fn test_bundle_config_creation() {
		let config = BundleConfig::new(PathBuf::from("bundle.js"));
		assert_eq!(config.output, PathBuf::from("bundle.js"));
		assert!(config.files.is_empty());
		assert!(!config.minify);
		assert!(!config.source_map);
	}

	#[test]
	fn test_bundle_config_add_file() {
		let mut config = BundleConfig::new(PathBuf::from("bundle.js"));
		config.add_file(PathBuf::from("a.js"));
		config.add_file(PathBuf::from("b.js"));
		assert_eq!(config.files.len(), 2);
	}

	#[test]
	fn test_bundle_config_builder() {
		let config = BundleConfig::new(PathBuf::from("bundle.js"))
			.with_minify(true)
			.with_source_map(true);
		assert!(config.minify);
		assert!(config.source_map);
	}

	#[test]
	fn test_bundler_multiple_files() {
		let mut bundler = AssetBundler::new();
		for i in 0..5 {
			bundler.add_file(
				PathBuf::from(format!("file{}.js", i)),
				format!("const x{} = {};", i, i).into_bytes(),
			);
		}

		assert_eq!(bundler.len(), 5);
		let result = bundler.bundle().unwrap();
		assert!(!result.is_empty());
	}

	#[test]
	fn test_bundler_empty_bundle() {
		let bundler = AssetBundler::new();
		let result = bundler.bundle().unwrap();
		assert!(result.is_empty());
	}
}
