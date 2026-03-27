use std::env;
use std::path::PathBuf;

/// Provides multiple strategies for detecting the project root
pub struct PathResolver;

impl PathResolver {
	/// Multi-layer path resolution
	///
	/// Attempts resolution in the following priority order:
	/// 1. Use CARGO_MANIFEST_DIR environment variable
	/// 2. Search upward from current directory for Cargo.toml
	/// 3. Fall back to current directory
	///
	/// # Arguments
	///
	/// * `relative_path` - The relative path to resolve
	///
	/// # Returns
	///
	/// The resolved path. Returns as-is if the path is absolute.
	pub fn resolve_static_dir(relative_path: &str) -> PathBuf {
		let path = PathBuf::from(relative_path);

		// Return as-is if absolute path
		if path.is_absolute() {
			return path;
		}

		// Layer 1: Use CARGO_MANIFEST_DIR
		if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
			let candidate = PathBuf::from(manifest_dir).join(&path);
			if candidate.exists() {
				return candidate;
			}
		}

		// Layer 2: Find project root by searching for Cargo.toml
		if let Some(project_root) = Self::find_project_root() {
			let candidate = project_root.join(&path);
			if candidate.exists() {
				return candidate;
			}
		}

		// Layer 3: Resolve from current directory (existing behavior)
		env::current_dir()
			.ok()
			.and_then(|cwd| {
				let candidate = cwd.join(&path);
				if candidate.exists() {
					Some(candidate)
				} else {
					None
				}
			})
			.unwrap_or(path)
	}

	/// Searches for `Cargo.toml` upward from the current directory.
	///
	/// Starting from `std::env::current_dir()`, traverses parent directories
	/// until a `Cargo.toml` containing `[[bin]]` or `[package]` is found.
	/// Symlinks are not explicitly followed beyond default OS behaviour.
	///
	/// # Returns
	///
	/// The project root directory, or `None` if no matching `Cargo.toml` is found
	/// before reaching the filesystem root.
	pub fn find_project_root() -> Option<PathBuf> {
		let mut current = env::current_dir().ok()?;

		loop {
			let cargo_toml = current.join("Cargo.toml");
			if cargo_toml.exists() {
				// Check if Cargo.toml has [[bin]] or [package] section
				// (indicating this is the project root)
				if let Ok(content) = std::fs::read_to_string(&cargo_toml)
					&& (content.contains("[[bin]]") || content.contains("[package]"))
				{
					return Some(current);
				}
			}

			// Move to parent directory
			if !current.pop() {
				break;
			}
		}

		None
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_absolute_path_unchanged() {
		let absolute = "/tmp/static";
		let resolved = PathResolver::resolve_static_dir(absolute);
		assert_eq!(resolved, PathBuf::from(absolute));
	}

	#[test]
	fn test_relative_path_resolution() {
		// Results may vary depending on test environment, but verify it doesn't panic
		let resolved = PathResolver::resolve_static_dir("dist");
		assert!(resolved.to_string_lossy().contains("dist"));
	}
}
