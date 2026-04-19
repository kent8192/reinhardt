//! File change classification for HMR.

use std::path::Path;

/// Classifies a file change to determine the appropriate reload strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
	/// Rust source file changed - requires full page reload.
	Rust,
	/// CSS file changed - can be hot-swapped without reload.
	Css,
	/// Template file changed - requires full page reload.
	Template,
	/// Static asset changed (images, fonts, etc.) - requires full page reload.
	Asset,
	/// Unknown file type - requires full page reload as a safe default.
	Unknown,
}

impl ChangeKind {
	/// Classifies a file path into a change kind.
	///
	/// Uses the file extension to determine the type of change.
	pub fn from_path(path: &Path) -> Self {
		match path.extension().and_then(|ext| ext.to_str()) {
			Some("rs") => Self::Rust,
			Some("css") => Self::Css,
			Some("html") | Some("hbs") | Some("tera") | Some("jinja") | Some("j2") => {
				Self::Template
			}
			Some("js") | Some("ts") | Some("jsx") | Some("tsx") => Self::Asset,
			Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("svg") | Some("ico")
			| Some("webp") | Some("woff") | Some("woff2") | Some("ttf") | Some("eot") => Self::Asset,
			_ => Self::Unknown,
		}
	}

	/// Returns whether this change kind supports hot-swapping without a full reload.
	pub fn supports_hot_swap(self) -> bool {
		matches!(self, Self::Css)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("src/main.rs", ChangeKind::Rust)]
	#[case("src/lib.rs", ChangeKind::Rust)]
	#[case("styles/main.css", ChangeKind::Css)]
	#[case("templates/base.html", ChangeKind::Template)]
	#[case("templates/index.tera", ChangeKind::Template)]
	#[case("templates/page.hbs", ChangeKind::Template)]
	#[case("templates/email.jinja", ChangeKind::Template)]
	#[case("templates/layout.j2", ChangeKind::Template)]
	#[case("static/logo.png", ChangeKind::Asset)]
	#[case("static/icon.svg", ChangeKind::Asset)]
	#[case("static/app.js", ChangeKind::Asset)]
	#[case("static/font.woff2", ChangeKind::Asset)]
	#[case("Makefile", ChangeKind::Unknown)]
	#[case("README.md", ChangeKind::Unknown)]
	fn test_change_kind_from_path(#[case] path: &str, #[case] expected: ChangeKind) {
		// Arrange
		let path = Path::new(path);

		// Act
		let kind = ChangeKind::from_path(path);

		// Assert
		assert_eq!(kind, expected);
	}

	#[rstest]
	#[case(ChangeKind::Css, true)]
	#[case(ChangeKind::Rust, false)]
	#[case(ChangeKind::Template, false)]
	#[case(ChangeKind::Asset, false)]
	#[case(ChangeKind::Unknown, false)]
	fn test_supports_hot_swap(#[case] kind: ChangeKind, #[case] expected: bool) {
		// Act & Assert
		assert_eq!(kind.supports_hot_swap(), expected);
	}

	// --- Edge cases and boundary values ---

	#[rstest]
	// No extension
	#[case("Makefile", ChangeKind::Unknown)]
	#[case(".gitignore", ChangeKind::Unknown)]
	#[case(".env", ChangeKind::Unknown)]
	// Uppercase extensions are NOT recognized (case-sensitive matching)
	#[case("styles/MAIN.CSS", ChangeKind::Unknown)]
	#[case("src/app.RS", ChangeKind::Unknown)]
	#[case("templates/PAGE.HTML", ChangeKind::Unknown)]
	// Directory name contains a dot, extension still detected correctly
	#[case("src.bak/main.rs", ChangeKind::Rust)]
	#[case("styles.v2/app.css", ChangeKind::Css)]
	// Preprocessor stylesheets — not hot-swappable (unknown)
	#[case("styles/main.scss", ChangeKind::Unknown)]
	#[case("styles/main.sass", ChangeKind::Unknown)]
	#[case("styles/main.less", ChangeKind::Unknown)]
	// Additional image/font formats
	#[case("static/photo.jpeg", ChangeKind::Asset)]
	#[case("static/icon.ico", ChangeKind::Asset)]
	#[case("static/anim.gif", ChangeKind::Asset)]
	#[case("static/image.webp", ChangeKind::Asset)]
	#[case("static/font.ttf", ChangeKind::Asset)]
	#[case("static/font.eot", ChangeKind::Asset)]
	#[case("static/font.woff", ChangeKind::Asset)]
	// TypeScript/JSX variants
	#[case("src/app.ts", ChangeKind::Asset)]
	#[case("src/component.tsx", ChangeKind::Asset)]
	#[case("src/component.jsx", ChangeKind::Asset)]
	// Jinja2 alias
	#[case("templates/page.j2", ChangeKind::Template)]
	fn test_change_kind_edge_cases(#[case] path: &str, #[case] expected: ChangeKind) {
		// Arrange
		let path = Path::new(path);

		// Act
		let kind = ChangeKind::from_path(path);

		// Assert
		assert_eq!(kind, expected);
	}

	#[rstest]
	fn test_change_kind_empty_path() {
		// Arrange
		let path = Path::new("");

		// Act
		let kind = ChangeKind::from_path(path);

		// Assert
		assert_eq!(kind, ChangeKind::Unknown);
	}

	#[rstest]
	fn test_change_kind_only_dot() {
		// Arrange — path that is just "."
		let path = Path::new(".");

		// Act
		let kind = ChangeKind::from_path(path);

		// Assert
		assert_eq!(kind, ChangeKind::Unknown);
	}

	#[rstest]
	fn test_change_kind_debug_clone() {
		let kind = ChangeKind::Css;
		let cloned = kind;
		assert_eq!(format!("{:?}", kind), "Css");
		assert_eq!(kind, cloned);
	}
}
