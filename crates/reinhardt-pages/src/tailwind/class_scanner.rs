//! RSX Template Class Name Scanner
//!
//! Scans Rust source files containing `page!` macro invocations to extract
//! CSS class names used in RSX templates. This enables Tailwind CSS to generate
//! only the utilities that are actually referenced in the code.
//!
//! ## Scanning Strategy
//!
//! The scanner uses regex-based extraction to find class attribute values in
//! RSX templates. It handles both static and common dynamic patterns:
//!
//! ```ignore
//! // Static classes (always detected)
//! div { class: "flex items-center gap-4" }
//!
//! // String literals in format expressions (detected)
//! div { class: format!("bg-{} text-white", color) }
//! ```

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use regex::Regex;

use super::TailwindResult;

/// Scanner for extracting CSS class names from RSX templates.
///
/// Processes Rust source files matching the configured glob patterns
/// and extracts class names from `class:` attributes within `page!` macros.
pub struct ClassScanner {
	/// Glob patterns for files to scan.
	content_paths: Vec<String>,
}

impl ClassScanner {
	/// Create a new scanner with the given content path patterns.
	pub fn new(content_paths: &[String]) -> Self {
		Self {
			content_paths: content_paths.to_vec(),
		}
	}

	/// Collect all source file paths matching the content patterns.
	///
	/// Uses glob to expand the patterns into concrete file paths.
	pub fn collect_source_files(&self) -> TailwindResult<Vec<PathBuf>> {
		let mut files = Vec::new();
		for pattern in &self.content_paths {
			match glob::glob(pattern) {
				Ok(paths) => {
					for entry in paths.flatten() {
						files.push(entry);
					}
				}
				Err(_) => {
					// Invalid glob pattern, skip it
					eprintln!("warning: invalid glob pattern: {}", pattern);
				}
			}
		}
		Ok(files)
	}

	/// Extract CSS class names from a single source file.
	///
	/// Parses the file content and looks for `class:` attribute values
	/// in RSX template syntax.
	///
	/// ## Patterns Detected
	///
	/// - `class: "..."` - Static class strings
	/// - `class: "... {}"` - Format strings (static portions extracted)
	pub fn extract_classes_from_file(path: &Path) -> TailwindResult<BTreeSet<String>> {
		let content = std::fs::read_to_string(path)?;
		Ok(Self::extract_classes_from_str(&content))
	}

	/// Extract CSS class names from a string of Rust source code.
	///
	/// This is the core extraction logic, separated for testability.
	pub fn extract_classes_from_str(content: &str) -> BTreeSet<String> {
		let mut classes = BTreeSet::new();

		// Match `class: "..."` or `class: format!("..."` patterns in RSX
		// This regex captures the string literal content after `class:`
		let class_attr_re =
			Regex::new(r#"class\s*:\s*"([^"]+)""#).expect("class attribute regex should compile");

		for cap in class_attr_re.captures_iter(content) {
			if let Some(class_value) = cap.get(1) {
				// Split the class string on whitespace and add each individual class
				for class_name in class_value.as_str().split_whitespace() {
					// Skip format placeholders like {} or {variable}
					if class_name.starts_with('{') && class_name.ends_with('}') {
						continue;
					}
					// Skip entries that are clearly not CSS classes
					if class_name.is_empty() || class_name.contains('{') || class_name.contains('}')
					{
						continue;
					}
					classes.insert(class_name.to_string());
				}
			}
		}

		// Also match class attributes in HTML-like format! strings:
		// format!("... class=\"...\" ...")
		let format_class_re =
			Regex::new(r#"class=\\?"([^"\\]+)\\?""#).expect("format class regex should compile");

		for cap in format_class_re.captures_iter(content) {
			if let Some(class_value) = cap.get(1) {
				for class_name in class_value.as_str().split_whitespace() {
					if class_name.starts_with('{') && class_name.ends_with('}') {
						continue;
					}
					if class_name.is_empty() || class_name.contains('{') || class_name.contains('}')
					{
						continue;
					}
					classes.insert(class_name.to_string());
				}
			}
		}

		classes
	}

	/// Scan all matching files and return the union of all extracted classes.
	pub fn scan_all(&self) -> TailwindResult<BTreeSet<String>> {
		let files = self.collect_source_files()?;
		let mut all_classes = BTreeSet::new();

		for file in &files {
			let classes = Self::extract_classes_from_file(file)?;
			all_classes.extend(classes);
		}

		Ok(all_classes)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_extract_simple_class() {
		// Arrange
		let source = r#"
			page!(|| {
				div { class: "flex items-center gap-4" }
			})
		"#;

		// Act
		let classes = ClassScanner::extract_classes_from_str(source);

		// Assert
		assert!(classes.contains("flex"));
		assert!(classes.contains("items-center"));
		assert!(classes.contains("gap-4"));
	}

	#[rstest]
	fn test_extract_multiple_class_attrs() {
		// Arrange
		let source = r#"
			page!(|| {
				div { class: "container mx-auto" }
				p { class: "text-gray-700 text-sm" }
			})
		"#;

		// Act
		let classes = ClassScanner::extract_classes_from_str(source);

		// Assert
		assert!(classes.contains("container"));
		assert!(classes.contains("mx-auto"));
		assert!(classes.contains("text-gray-700"));
		assert!(classes.contains("text-sm"));
	}

	#[rstest]
	fn test_skip_format_placeholders() {
		// Arrange
		let source = r#"
			div { class: "bg-blue-500 {dynamic_class} text-white" }
		"#;

		// Act
		let classes = ClassScanner::extract_classes_from_str(source);

		// Assert
		assert!(classes.contains("bg-blue-500"));
		assert!(classes.contains("text-white"));
		assert!(!classes.contains("{dynamic_class}"));
	}

	#[rstest]
	fn test_empty_source() {
		// Arrange
		let source = "";

		// Act
		let classes = ClassScanner::extract_classes_from_str(source);

		// Assert
		assert!(classes.is_empty());
	}

	#[rstest]
	fn test_no_class_attributes() {
		// Arrange
		let source = r#"
			page!(|| {
				div { id: "main" }
				p { "Hello, world!" }
			})
		"#;

		// Act
		let classes = ClassScanner::extract_classes_from_str(source);

		// Assert
		assert!(classes.is_empty());
	}

	#[rstest]
	fn test_classes_are_deduplicated() {
		// Arrange
		let source = r#"
			div { class: "flex" }
			span { class: "flex" }
		"#;

		// Act
		let classes = ClassScanner::extract_classes_from_str(source);

		// Assert
		assert_eq!(classes.len(), 1);
		assert!(classes.contains("flex"));
	}

	#[rstest]
	fn test_tailwind_responsive_classes() {
		// Arrange
		let source = r#"
			div { class: "sm:flex md:grid lg:hidden" }
		"#;

		// Act
		let classes = ClassScanner::extract_classes_from_str(source);

		// Assert
		assert!(classes.contains("sm:flex"));
		assert!(classes.contains("md:grid"));
		assert!(classes.contains("lg:hidden"));
	}

	#[rstest]
	fn test_tailwind_arbitrary_values() {
		// Arrange
		let source = r#"
			div { class: "w-[200px] bg-[#1da1f2] grid-cols-[1fr_2fr]" }
		"#;

		// Act
		let classes = ClassScanner::extract_classes_from_str(source);

		// Assert
		assert!(classes.contains("w-[200px]"));
		assert!(classes.contains("bg-[#1da1f2]"));
		assert!(classes.contains("grid-cols-[1fr_2fr]"));
	}

	#[rstest]
	fn test_collect_source_files_empty_patterns() {
		// Arrange
		let scanner = ClassScanner::new(&[]);

		// Act
		let files = scanner.collect_source_files().unwrap();

		// Assert
		assert!(files.is_empty());
	}

	#[rstest]
	fn test_extract_classes_from_file_nonexistent() {
		// Arrange
		let path = Path::new("/tmp/nonexistent-reinhardt-test-file.rs");

		// Act
		let result = ClassScanner::extract_classes_from_file(path);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_extract_from_real_file() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		let file_path = dir.path().join("test_component.rs");
		std::fs::write(
			&file_path,
			r#"
use reinhardt_pages::{page, Page};

fn my_component() -> Page {
	page!(|| {
		div { class: "flex items-center justify-between p-4",
			h1 { class: "text-2xl font-bold", "Title" }
			button { class: "bg-blue-500 hover:bg-blue-700 text-white px-4 py-2 rounded",
				"Click me"
			}
		}
	})()
}
"#,
		)
		.unwrap();

		// Act
		let classes = ClassScanner::extract_classes_from_file(&file_path).unwrap();

		// Assert
		assert!(classes.contains("flex"));
		assert!(classes.contains("items-center"));
		assert!(classes.contains("justify-between"));
		assert!(classes.contains("p-4"));
		assert!(classes.contains("text-2xl"));
		assert!(classes.contains("font-bold"));
		assert!(classes.contains("bg-blue-500"));
		assert!(classes.contains("hover:bg-blue-700"));
		assert!(classes.contains("text-white"));
		assert!(classes.contains("px-4"));
		assert!(classes.contains("py-2"));
		assert!(classes.contains("rounded"));
	}

	#[rstest]
	fn test_scan_all_with_glob_pattern() {
		// Arrange
		let dir = tempfile::tempdir().unwrap();
		let file1 = dir.path().join("comp1.rs");
		let file2 = dir.path().join("comp2.rs");

		std::fs::write(&file1, r#"div { class: "flex gap-2" }"#).unwrap();
		std::fs::write(&file2, r#"div { class: "grid gap-4" }"#).unwrap();

		let pattern = format!("{}/*.rs", dir.path().display());
		let scanner = ClassScanner::new(&[pattern]);

		// Act
		let classes = scanner.scan_all().unwrap();

		// Assert
		assert!(classes.contains("flex"));
		assert!(classes.contains("gap-2"));
		assert!(classes.contains("grid"));
		assert!(classes.contains("gap-4"));
	}
}
