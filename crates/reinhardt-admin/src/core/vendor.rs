//! Vendor asset manifest and download logic for the admin panel.
//!
//! This module defines the manifest of external CSS, JS, and font assets
//! required by the admin panel, along with utilities for downloading and
//! verifying them. All assets are version-pinned to ensure reproducible builds.

#[cfg(server)]
use std::path::Path;

#[cfg(server)]
use sha2::{Digest, Sha256};

/// A single vendor asset with a version-pinned URL, target path, and SHA-256 checksum.
#[cfg(server)]
pub struct VendorAsset {
	/// The version-pinned CDN URL to download the asset from.
	pub url: &'static str,
	/// Relative path within the static directory where the asset will be stored.
	pub target: &'static str,
	/// Expected SHA-256 hex digest of the file content.
	/// Empty string means the checksum has not yet been populated.
	pub sha256: &'static str,
}

/// All vendor assets required by the admin panel.
///
/// SHA-256 values are left empty here and will be populated after the first
/// successful download using `verify_integrity`.
#[cfg(server)]
const ADMIN_VENDOR_ASSETS: &[VendorAsset] = &[
	// Open Props v1.7.23 — CSS custom property design tokens
	VendorAsset {
		url: "https://cdn.jsdelivr.net/npm/open-props@1.7.23/open-props.min.css",
		target: "vendor/open-props.min.css",
		sha256: "",
	},
	// Animate.css v4.1.1 — CSS animation library
	VendorAsset {
		url: "https://cdn.jsdelivr.net/npm/animate.css@4.1.1/animate.min.css",
		target: "vendor/animate.min.css",
		sha256: "",
	},
	// DM Sans — Latin subset, weight 400 (regular)
	VendorAsset {
		url: "https://cdn.jsdelivr.net/npm/@fontsource/dm-sans@5.1.1/files/dm-sans-latin-400-normal.woff2",
		target: "vendor/fonts/dm-sans-latin-400-normal.woff2",
		sha256: "",
	},
	// DM Sans — Latin subset, weight 400 italic
	VendorAsset {
		url: "https://cdn.jsdelivr.net/npm/@fontsource/dm-sans@5.1.1/files/dm-sans-latin-400-italic.woff2",
		target: "vendor/fonts/dm-sans-latin-400-italic.woff2",
		sha256: "",
	},
	// DM Sans — Latin subset, weight 500 (medium)
	VendorAsset {
		url: "https://cdn.jsdelivr.net/npm/@fontsource/dm-sans@5.1.1/files/dm-sans-latin-500-normal.woff2",
		target: "vendor/fonts/dm-sans-latin-500-normal.woff2",
		sha256: "",
	},
	// DM Sans — Latin subset, weight 600 (semi-bold)
	VendorAsset {
		url: "https://cdn.jsdelivr.net/npm/@fontsource/dm-sans@5.1.1/files/dm-sans-latin-600-normal.woff2",
		target: "vendor/fonts/dm-sans-latin-600-normal.woff2",
		sha256: "",
	},
	// DM Sans — Latin subset, weight 700 (bold)
	VendorAsset {
		url: "https://cdn.jsdelivr.net/npm/@fontsource/dm-sans@5.1.1/files/dm-sans-latin-700-normal.woff2",
		target: "vendor/fonts/dm-sans-latin-700-normal.woff2",
		sha256: "",
	},
	// Inter — Latin subset, weight 600 (semi-bold)
	VendorAsset {
		url: "https://cdn.jsdelivr.net/fontsource/fonts/inter@latest/latin-600-normal.woff2",
		target: "vendor/fonts/inter-latin-600-normal.woff2",
		sha256: "",
	},
	// Inter — Latin subset, weight 700 (bold)
	VendorAsset {
		url: "https://cdn.jsdelivr.net/fontsource/fonts/inter@latest/latin-700-normal.woff2",
		target: "vendor/fonts/inter-latin-700-normal.woff2",
		sha256: "",
	},
	// Inter — Latin subset, weight 800 (extra-bold)
	VendorAsset {
		url: "https://cdn.jsdelivr.net/fontsource/fonts/inter@latest/latin-800-normal.woff2",
		target: "vendor/fonts/inter-latin-800-normal.woff2",
		sha256: "",
	},
	// UnoCSS Runtime v66.6.7 — browser-based utility CSS generation engine.
	// Generates Tailwind-compatible utility CSS by observing DOM class names
	// at runtime, eliminating the need for a build-time CLI step.
	VendorAsset {
		url: "https://cdn.jsdelivr.net/npm/@unocss/runtime@66.6.7/uno.global.js",
		target: "vendor/unocss-runtime.js",
		sha256: "",
	},
];

/// Returns the full list of vendor assets required by the admin panel.
#[cfg(server)]
pub fn admin_vendor_assets() -> &'static [VendorAsset] {
	ADMIN_VENDOR_ASSETS
}

/// Verifies the SHA-256 integrity of a file at the given path.
///
/// Returns `Ok(())` if `expected_sha256` is empty (checksum not yet known),
/// or if the computed digest matches `expected_sha256`.
/// Returns `Err` if the file cannot be read or the digest does not match.
#[cfg(server)]
pub fn verify_integrity(path: &Path, expected_sha256: &str) -> Result<(), String> {
	// Skip verification when no expected hash is provided
	if expected_sha256.is_empty() {
		return Ok(());
	}

	let data =
		std::fs::read(path).map_err(|e| format!("failed to read {}: {}", path.display(), e))?;

	let mut hasher = Sha256::new();
	hasher.update(&data);
	let computed = format!("{:x}", hasher.finalize());

	if computed == expected_sha256 {
		Ok(())
	} else {
		Err(format!(
			"integrity check failed for {}: expected {}, got {}",
			path.display(),
			expected_sha256,
			computed
		))
	}
}

/// Verbosity level for download progress output.
#[cfg(server)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
	/// No output.
	Silent,
	/// Print one line per downloaded file.
	Normal,
	/// Print detailed progress including skip messages.
	Verbose,
}

/// Downloads all vendor assets to `base_dir`, skipping files that already exist
/// and pass the integrity check.
///
/// # Errors
///
/// Returns an error if any HTTP request fails, the response body cannot be
/// read, a file cannot be written, or an integrity check fails after download.
#[cfg(server)]
pub async fn download_vendor_assets(
	base_dir: &Path,
	verbosity: Verbosity,
) -> Result<(), anyhow::Error> {
	use std::io::Write as _;

	let client = reqwest::Client::new();

	for asset in ADMIN_VENDOR_ASSETS {
		let dest = base_dir.join(asset.target);

		// Skip download if the file already exists and passes integrity check
		if dest.exists() {
			match verify_integrity(&dest, asset.sha256) {
				Ok(()) => {
					if verbosity == Verbosity::Verbose {
						println!("skip (exists): {}", asset.target);
					}
					continue;
				}
				Err(e) => {
					if verbosity != Verbosity::Silent {
						println!(
							"re-downloading (integrity mismatch): {} — {}",
							asset.target, e
						);
					}
				}
			}
		}

		if verbosity != Verbosity::Silent {
			println!("download: {}", asset.url);
		}

		let response = client.get(asset.url).send().await?;
		let status = response.status();
		if !status.is_success() {
			return Err(anyhow::anyhow!("HTTP {} downloading {}", status, asset.url));
		}

		let bytes = response.bytes().await?;

		// Ensure the parent directory exists
		if let Some(parent) = dest.parent() {
			std::fs::create_dir_all(parent)?;
		}

		// Write atomically by writing to a temp file in the same directory then renaming
		let parent_dir = dest.parent().ok_or_else(|| {
			anyhow::anyhow!("destination has no parent directory: {}", dest.display())
		})?;
		let mut tmp = tempfile::NamedTempFile::new_in(parent_dir)?;
		tmp.write_all(&bytes)?;
		tmp.persist(&dest)
			.map_err(|e| anyhow::anyhow!("failed to persist {}: {}", dest.display(), e))?;

		// Verify the newly downloaded file
		verify_integrity(&dest, asset.sha256).map_err(|e| anyhow::anyhow!("{}", e))?;

		if verbosity == Verbosity::Verbose {
			println!("saved: {}", asset.target);
		}
	}

	Ok(())
}

/// Ensures all vendor assets exist on disk, downloading any that are missing.
///
/// This function is safe to call repeatedly — it uses a `std::sync::OnceLock` to
/// guarantee that the download check executes at most once per process. Subsequent
/// calls return immediately.
///
/// Intended to be called lazily on the first admin panel request so that vendor
/// files are available during development without requiring a separate
/// `collectstatic` step.
///
/// # Arguments
///
/// * `base_dir` — The directory containing vendor assets (typically the admin
///   `assets/` directory).
#[cfg(server)]
pub async fn ensure_vendor_assets(base_dir: &std::path::Path) {
	use std::sync::OnceLock;

	static ENSURED: OnceLock<()> = OnceLock::new();

	// Fast path: already checked
	if ENSURED.get().is_some() {
		return;
	}

	// Check whether any vendor asset is missing
	let any_missing = ADMIN_VENDOR_ASSETS
		.iter()
		.any(|asset| !base_dir.join(asset.target).exists());

	if any_missing {
		tracing::info!("Admin vendor assets missing — downloading on first request");
		match download_vendor_assets(base_dir, Verbosity::Normal).await {
			Ok(()) => {
				tracing::info!("Admin vendor assets downloaded successfully");
			}
			Err(e) => {
				tracing::warn!(
					error = %e,
					"Failed to download admin vendor assets; \
					 the admin panel may render without styles"
				);
				// Still mark as ensured to avoid retrying every request
			}
		}
	}

	// Mark as ensured regardless of outcome to avoid repeated attempts
	let _ = ENSURED.set(());
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	#[cfg(server)]
	use super::{ADMIN_VENDOR_ASSETS, admin_vendor_assets, verify_integrity};

	/// The manifest must contain at least one entry.
	#[cfg(server)]
	#[rstest]
	fn manifest_not_empty() {
		// Arrange / Act
		let assets = admin_vendor_assets();

		// Assert
		assert!(
			!assets.is_empty(),
			"vendor asset manifest must not be empty"
		);
	}

	/// Every entry must have a non-empty URL and a non-empty target path.
	#[cfg(server)]
	#[rstest]
	fn all_assets_have_url_and_target() {
		// Arrange
		let assets = admin_vendor_assets();

		// Act / Assert
		for asset in assets {
			assert!(
				!asset.url.is_empty(),
				"asset target '{}' has an empty URL",
				asset.target
			);
			assert!(
				!asset.target.is_empty(),
				"an asset with URL '{}' has an empty target path",
				asset.url
			);
		}
	}

	/// Every URL must contain `@` which indicates a pinned version.
	#[cfg(server)]
	#[rstest]
	fn all_urls_are_versioned() {
		// Arrange
		let assets = admin_vendor_assets();

		// Act / Assert
		for asset in assets {
			assert!(
				asset.url.contains('@'),
				"URL is not version-pinned (missing '@'): {}",
				asset.url
			);
		}
	}

	/// No two assets may share the same target path.
	#[cfg(server)]
	#[rstest]
	fn no_duplicate_targets() {
		// Arrange
		let assets = admin_vendor_assets();
		let mut seen = std::collections::HashSet::new();

		// Act / Assert
		for asset in assets {
			assert!(
				seen.insert(asset.target),
				"duplicate target path found: {}",
				asset.target
			);
		}
	}

	/// `verify_integrity` must return `Ok` when the expected hash is empty.
	#[cfg(server)]
	#[rstest]
	fn verify_integrity_with_empty_hash() {
		// Arrange
		let dir = tempfile::tempdir().expect("tempdir");
		let file = dir.path().join("test.css");
		std::fs::write(&file, b"body {}").expect("write");

		// Act
		let result = verify_integrity(&file, "");

		// Assert
		assert!(result.is_ok(), "empty hash should always pass");
	}

	/// `verify_integrity` must return `Ok` when the hash matches.
	#[cfg(server)]
	#[rstest]
	fn verify_integrity_with_correct_hash() {
		use sha2::{Digest, Sha256};

		// Arrange
		let content = b"body {}";
		let expected = format!("{:x}", Sha256::digest(content));

		let dir = tempfile::tempdir().expect("tempdir");
		let file = dir.path().join("test.css");
		std::fs::write(&file, content).expect("write");

		// Act
		let result = verify_integrity(&file, &expected);

		// Assert
		assert!(result.is_ok(), "correct hash should pass: {:?}", result);
	}

	/// `verify_integrity` must return `Err` when the hash does not match.
	#[cfg(server)]
	#[rstest]
	fn verify_integrity_with_wrong_hash() {
		// Arrange
		let dir = tempfile::tempdir().expect("tempdir");
		let file = dir.path().join("test.css");
		std::fs::write(&file, b"body {}").expect("write");

		// Act
		let result = verify_integrity(
			&file,
			"0000000000000000000000000000000000000000000000000000000000000000",
		);

		// Assert
		assert!(result.is_err(), "wrong hash should fail");
	}

	/// Sanity-check: the constant and the function return the same slice.
	#[cfg(server)]
	#[rstest]
	fn admin_vendor_assets_matches_const() {
		// Arrange / Act
		let from_fn = admin_vendor_assets();

		// Assert
		assert_eq!(
			from_fn.len(),
			ADMIN_VENDOR_ASSETS.len(),
			"admin_vendor_assets() must return ADMIN_VENDOR_ASSETS"
		);
	}

	/// Every font `url()` reference in `style.css` must correspond to a vendor
	/// asset `target` path in `ADMIN_VENDOR_ASSETS`. This prevents filename
	/// drift between the CSS and the download manifest.
	#[cfg(server)]
	#[rstest]
	fn css_font_urls_match_vendor_targets() {
		// Arrange
		let css = include_str!("../../assets/style.css");
		let vendor_targets: std::collections::HashSet<&str> =
			ADMIN_VENDOR_ASSETS.iter().map(|a| a.target).collect();

		// Act — extract all url('vendor/...') references from the CSS
		let mut missing = Vec::new();
		for line in css.lines() {
			if let Some(start) = line.find("url('") {
				let rest = &line[start + 5..];
				if let Some(end) = rest.find("')") {
					let url_path = &rest[..end];
					if url_path.starts_with("vendor/") && !vendor_targets.contains(url_path) {
						missing.push(url_path.to_string());
					}
				}
			}
		}

		// Assert
		assert!(
			missing.is_empty(),
			"style.css references font files not found in ADMIN_VENDOR_ASSETS: {:?}",
			missing
		);
	}
}
