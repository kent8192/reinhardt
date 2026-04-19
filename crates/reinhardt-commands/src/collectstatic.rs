//! Collect static files command
//!
//! Django-style static file collection for production deployment

use crate::CommandResult;
use crate::{BaseCommand, CommandContext};
use async_trait::async_trait;
use reinhardt_utils::staticfiles::{StaticFilesConfig, StaticFilesFinder};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Options for the `collectstatic` management command.
#[derive(Debug, Clone)]
pub struct CollectStaticOptions {
	/// Whether to clear the destination directory before collecting.
	pub clear: bool,
	/// Whether to skip user confirmation prompts.
	pub no_input: bool,
	/// Whether to perform a dry run without copying files.
	pub dry_run: bool,
	/// Whether to prompt the user interactively.
	pub interactive: bool,
	/// Verbosity level (0 = quiet, 1 = normal, 2+ = verbose).
	pub verbosity: u8,
	/// Whether to create symbolic links instead of copying files.
	pub link: bool,
	/// Glob patterns for files to ignore during collection.
	pub ignore_patterns: Vec<String>,
	/// Whether to enable content-based hashing for cache busting.
	pub enable_hashing: bool,
	/// Whether to use fast (size-only) comparison instead of content hashing.
	pub fast_compare: bool,
}

impl Default for CollectStaticOptions {
	fn default() -> Self {
		Self {
			clear: false,
			no_input: false,
			dry_run: false,
			interactive: true,
			verbosity: 1,
			link: false,
			ignore_patterns: Vec::new(),
			enable_hashing: true,
			fast_compare: false,
		}
	}
}

/// Statistics from a `collectstatic` execution.
#[derive(Debug, Clone)]
pub struct CollectStaticStats {
	/// Number of files copied to the destination.
	pub copied: usize,
	/// Number of files skipped due to ignore patterns.
	pub skipped: usize,
	/// Number of files deleted during clear operation.
	pub deleted: usize,
	/// Number of files already up-to-date at the destination.
	pub unmodified: usize,
}

impl CollectStaticStats {
	/// Creates a new stats counter with all values set to zero.
	pub fn new() -> Self {
		Self {
			copied: 0,
			skipped: 0,
			deleted: 0,
			unmodified: 0,
		}
	}
}

impl Default for CollectStaticStats {
	fn default() -> Self {
		Self::new()
	}
}

/// Management command for collecting static files into a single directory.
///
/// Discovers static files from configured directories and installed apps,
/// then copies them to the `STATIC_ROOT` directory for production serving.
#[derive(Default)]
pub struct CollectStaticCommand {
	config: StaticFilesConfig,
	options: CollectStaticOptions,
	manifest: HashMap<String, String>,
	/// Source path for index.html (copies to static_root with template processing).
	/// Refs #2869
	index_source: Option<PathBuf>,
}

impl CollectStaticCommand {
	/// Creates a new command with the given static files configuration and options.
	pub fn new(config: StaticFilesConfig, options: CollectStaticOptions) -> Self {
		Self {
			config,
			options,
			manifest: HashMap::new(),
			index_source: None,
		}
	}

	/// Set the source path for index.html.
	///
	/// When set, `execute()` copies this file to `static_root/index.html`
	/// with `{{ static_url() }}` template processing applied.
	pub fn set_index_source(&mut self, path: Option<PathBuf>) {
		self.index_source = path;
	}

	/// Execute the collectstatic command
	pub fn execute(&mut self) -> Result<CollectStaticStats, io::Error> {
		let mut stats = CollectStaticStats::new();

		// Validate configuration
		self.validate_config()?;

		// Clear destination if requested
		if self.options.clear {
			self.clear_destination(&mut stats)?;
		}

		// Create destination directory if it doesn't exist
		if !self.options.dry_run {
			fs::create_dir_all(&self.config.static_root)?;
		}

		// Download vendor assets for the admin panel before collecting static files.
		// This step is only active when the "server" feature is enabled.
		//
		// The download target is resolved from the admin app's registered static_dir
		// (via inventory) to ensure consistency with the path that find_all() will
		// later walk. Falling back to a CARGO_MANIFEST_DIR-relative path would
		// silently diverge when the crate is consumed from the cargo registry or
		// across git worktrees.
		#[cfg(feature = "server")]
		{
			let admin_assets_dir: Option<PathBuf> = {
				let app_configs = ::reinhardt_apps::get_app_static_files();
				app_configs
					.iter()
					.find(|c| c.app_label == "admin")
					.map(|c| PathBuf::from(c.static_dir))
			};

			if let Some(admin_assets_dir) = admin_assets_dir {
				// Ensure the directory exists so vendor downloads have a target
				if !self.options.dry_run {
					fs::create_dir_all(&admin_assets_dir).ok();
				}

				use reinhardt_admin::core::vendor::{Verbosity, download_vendor_assets};

				let verbosity = match self.options.verbosity {
					0 => Verbosity::Silent,
					1 => Verbosity::Normal,
					_ => Verbosity::Verbose,
				};

				// Bridge sync->async: try the current tokio runtime handle first,
				// then fall back to a freshly created single-threaded runtime.
				let result: Result<(), String> = match tokio::runtime::Handle::try_current() {
					Ok(handle) => tokio::task::block_in_place(|| {
						handle
							.block_on(download_vendor_assets(&admin_assets_dir, verbosity))
							.map_err(|e| e.to_string())
					}),
					Err(_) => match tokio::runtime::Builder::new_current_thread()
						.enable_all()
						.build()
					{
						Ok(rt) => rt
							.block_on(download_vendor_assets(&admin_assets_dir, verbosity))
							.map_err(|e| e.to_string()),
						Err(e) => Err(format!("failed to create tokio runtime: {}", e)),
					},
				};

				match result {
					Ok(()) => {
						if self.options.verbosity > 0 {
							println!("All vendor assets up-to-date");
						}
					}
					Err(e) => {
						// Vendor download failures are non-fatal; warn and continue
						// with whatever assets already exist in the directory.
						eprintln!(
							"Warning: vendor asset download failed (continuing with existing files): {}",
							e
						);
					}
				}
			} else if self.options.verbosity > 0 {
				println!("Admin static files not registered; skipping vendor asset download");
			}
		}

		// Collect files from all source directories
		// Start with manually configured directories
		let mut all_dirs = self.config.staticfiles_dirs.clone();

		// Auto-discover static files from installed apps via inventory
		let app_static_configs = ::reinhardt_apps::get_app_static_files();

		for config in app_static_configs {
			// Convert &'static str to PathBuf
			let static_dir = std::path::PathBuf::from(config.static_dir);

			// Skip if already in staticfiles_dirs (manual config takes precedence)
			if !all_dirs.contains(&static_dir) {
				if self.options.verbosity > 1 {
					println!(
						"Auto-discovered static files from app '{}': {}",
						config.app_label,
						static_dir.display()
					);
				}
				all_dirs.push(static_dir);
			}
		}

		let finder = StaticFilesFinder::new(all_dirs.clone());
		let all_files = finder.find_all();

		if self.options.verbosity > 0 {
			println!("Found {} static files", all_files.len());
		}

		// Use a HashSet to track unique file paths and ensure we process each file only once
		// Later sources override earlier ones due to the order of find_all()
		let mut processed_files = std::collections::HashSet::new();
		let files_to_process: Vec<String> = all_files.into_iter().rev().collect();

		// Process each file (reversed so later sources are processed first)
		for file_path in &files_to_process {
			// Skip if already processed (handles duplicates from multiple source dirs)
			if !processed_files.insert(file_path.clone()) {
				continue;
			}

			if self.should_ignore(file_path) {
				if self.options.verbosity > 1 {
					println!("Ignoring: {}", file_path);
				}
				stats.skipped += 1;
				continue;
			}

			match self.copy_file(file_path, &all_dirs)? {
				CopyResult::Copied => stats.copied += 1,
				CopyResult::Unmodified => stats.unmodified += 1,
			}
		}

		// Save manifest if hashing is enabled
		if self.options.enable_hashing && !self.options.dry_run {
			self.save_manifest()?;
		}

		// Process index.html from explicit source path
		// Refs #2869: Copy index.html from project root to dist/ with template processing
		if let Some(ref index_source) = self.index_source {
			if index_source.exists() {
				let dest_path = self.config.static_root.join("index.html");
				if !self.options.dry_run {
					if let Some(parent) = dest_path.parent() {
						fs::create_dir_all(parent)?;
					}
					if self.options.link {
						self.create_symlink(index_source, &dest_path)?;
					} else {
						self.process_html_template(index_source, &dest_path)?;
					}
				}
				if self.options.verbosity > 0 {
					println!(
						"Index: {} → {}",
						index_source.display(),
						dest_path.display()
					);
				}
				stats.copied += 1;
			} else {
				return Err(io::Error::new(
					io::ErrorKind::NotFound,
					format!("Index source file not found: {}", index_source.display()),
				));
			}
		}

		// Print summary
		if self.options.verbosity > 0 {
			self.print_summary(&stats);
		}

		Ok(stats)
	}

	fn validate_config(&self) -> Result<(), io::Error> {
		if self.config.static_root.as_os_str().is_empty() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidInput,
				"STATIC_ROOT is not configured",
			));
		}

		Ok(())
	}

	fn clear_destination(&self, stats: &mut CollectStaticStats) -> Result<(), io::Error> {
		if !self.config.static_root.exists() {
			return Ok(());
		}

		if self.options.verbosity > 0 {
			println!(
				"Clearing existing files from {}",
				self.config.static_root.display()
			);
		}

		if !self.options.dry_run {
			for entry in fs::read_dir(&self.config.static_root)? {
				let entry = entry?;
				let path = entry.path();

				if path.is_file() {
					fs::remove_file(&path)?;
					stats.deleted += 1;
				} else if path.is_dir() {
					fs::remove_dir_all(&path)?;
					stats.deleted += 1;
				}
			}
		}

		Ok(())
	}

	fn find_in_reverse(&self, path: &str, all_dirs: &[PathBuf]) -> Result<PathBuf, io::Error> {
		// Search directories in reverse order to prioritize later sources
		// Now searches ALL directories (manual + auto-discovered from inventory)
		for dir in all_dirs.iter().rev() {
			let file_path = dir.join(path);
			if file_path.exists() {
				return Ok(file_path);
			}
		}
		Err(io::Error::new(
			io::ErrorKind::NotFound,
			format!("File not found in any directory: {}", path),
		))
	}

	fn should_ignore(&self, file_path: &str) -> bool {
		// Check if file is hidden (starts with dot)
		if let Some(file_name) = std::path::Path::new(file_path).file_name()
			&& let Some(name_str) = file_name.to_str()
			&& name_str.starts_with('.')
		{
			return true;
		}

		// Check ignore patterns
		for pattern in &self.options.ignore_patterns {
			// Simple wildcard matching: "*.ext" matches any file ending with .ext
			if pattern.starts_with("*.") {
				let ext = &pattern[1..]; // Remove the *
				if file_path.ends_with(ext) {
					return true;
				}
			} else if file_path.contains(pattern) {
				return true;
			}
		}
		false
	}

	fn copy_file(
		&mut self,
		relative_path: &str,
		all_dirs: &[PathBuf],
	) -> Result<CopyResult, io::Error> {
		// Find source file - search directories in reverse order to prioritize later sources
		let source_path = self.find_in_reverse(relative_path, all_dirs)?;

		// Special handling for index.html template processing
		if relative_path.ends_with("index.html") && self.options.enable_hashing {
			let dest_path = self.config.static_root.join(relative_path);

			if !self.options.dry_run {
				if let Some(parent) = dest_path.parent() {
					fs::create_dir_all(parent)?;
				}
				self.process_html_template(&source_path, &dest_path)?;
			}

			return Ok(CopyResult::Copied);
		}

		// Generate hashed filename if hashing is enabled
		let (_dest_filename, dest_path) = if self.options.enable_hashing {
			let hash = self.calculate_hash(&source_path)?;
			let hashed_name = self.get_hashed_filename(relative_path, &hash);

			// Record in manifest
			self.manifest
				.insert(relative_path.to_string(), hashed_name.clone());

			let dest = self.config.static_root.join(&hashed_name);
			(hashed_name, dest)
		} else {
			let dest = self.config.static_root.join(relative_path);
			(relative_path.to_string(), dest)
		};

		// Check if file exists and is identical
		if dest_path.exists() && !self.options.clear {
			let identical = if self.options.fast_compare {
				self.files_identical_fast(&source_path, &dest_path)?
			} else if self.options.enable_hashing {
				self.files_identical_hash(&source_path, &dest_path)?
			} else {
				self.files_identical(&source_path, &dest_path)?
			};

			if identical {
				if self.options.verbosity > 1 {
					println!("Unmodified: {}", relative_path);
				}
				return Ok(CopyResult::Unmodified);
			}
		}

		if self.options.verbosity > 1 {
			println!(
				"Copying: {} → {}",
				source_path.display(),
				dest_path.display()
			);
		}

		if self.options.dry_run {
			return Ok(CopyResult::Copied);
		}

		// Create parent directories
		if let Some(parent) = dest_path.parent() {
			fs::create_dir_all(parent)?;
		}

		// Copy or link file
		if self.options.link {
			self.create_symlink(&source_path, &dest_path)?;
		} else {
			fs::copy(&source_path, &dest_path)?;
		}

		Ok(CopyResult::Copied)
	}

	#[cfg(unix)]
	fn create_symlink(&self, source: &Path, dest: &Path) -> Result<(), io::Error> {
		use std::os::unix::fs::symlink;

		// Remove existing file/symlink
		if dest.exists() || dest.symlink_metadata().is_ok() {
			fs::remove_file(dest)?;
		}

		symlink(source, dest)
	}

	#[cfg(not(unix))]
	fn create_symlink(&self, source: &Path, dest: &Path) -> Result<(), io::Error> {
		// Fallback to copy on non-Unix systems
		if dest.exists() {
			fs::remove_file(dest)?;
		}
		fs::copy(source, dest)?;
		Ok(())
	}

	fn files_identical(&self, path1: &Path, path2: &Path) -> Result<bool, io::Error> {
		let meta1 = fs::metadata(path1)?;
		let meta2 = fs::metadata(path2)?;

		// Quick check: if sizes differ, files are different
		if meta1.len() != meta2.len() {
			return Ok(false);
		}

		// For small files, compare content
		if meta1.len() < 1024 * 1024 {
			// 1MB threshold
			let content1 = fs::read(path1)?;
			let content2 = fs::read(path2)?;
			return Ok(content1 == content2);
		}

		// For large files, assume identical if same size
		// (more sophisticated comparison could use checksums)
		Ok(true)
	}

	fn calculate_hash(&self, path: &Path) -> Result<String, io::Error> {
		use sha2::{Digest, Sha256};

		// Handle symlinks: resolve to target
		let canonical_path = fs::canonicalize(path)?;
		let content = fs::read(&canonical_path)?;

		let hash = Sha256::digest(&content);
		// Use first 8 characters of SHA-256 hash
		Ok(format!("{:x}", hash)[..8].to_string())
	}

	fn get_hashed_filename(&self, filename: &str, hash: &str) -> String {
		if let Some(dot_pos) = filename.rfind('.') {
			// Insert hash before extension
			format!("{}.{}{}", &filename[..dot_pos], hash, &filename[dot_pos..])
		} else {
			// No extension: append hash to end
			format!("{}.{}", filename, hash)
		}
	}

	fn save_manifest(&self) -> Result<(), io::Error> {
		let manifest_path = self.config.static_root.join("manifest.json");

		let manifest_data = serde_json::json!({
			"version": "1.0",
			"paths": self.manifest
		});

		let json = serde_json::to_string_pretty(&manifest_data)?;
		fs::write(manifest_path, json)?;

		if self.options.verbosity > 0 {
			println!("✓ Manifest saved: manifest.json");
		}

		Ok(())
	}

	fn files_identical_hash(&self, path1: &Path, path2: &Path) -> Result<bool, io::Error> {
		let hash1 = self.calculate_hash(path1)?;
		let hash2 = self.calculate_hash(path2)?;
		Ok(hash1 == hash2)
	}

	fn files_identical_fast(&self, path1: &Path, path2: &Path) -> Result<bool, io::Error> {
		let meta1 = fs::metadata(path1)?;
		let meta2 = fs::metadata(path2)?;

		// Check size
		if meta1.len() != meta2.len() {
			return Ok(false);
		}

		// Files <= 1MB: content comparison
		if meta1.len() < 1024 * 1024 {
			let content1 = fs::read(path1)?;
			let content2 = fs::read(path2)?;
			return Ok(content1 == content2);
		}

		// Files > 1MB: size-only comparison
		Ok(true)
	}

	fn process_html_template(&self, source: &Path, dest: &Path) -> Result<(), io::Error> {
		let content = fs::read_to_string(source)?;

		// Detect {{ static_url("path") }} pattern
		static STATIC_URL_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
			regex::Regex::new(r#"\{\{\s*static_url\("([^"]+)"\)\s*\}\}"#).unwrap()
		});

		let processed = STATIC_URL_RE.replace_all(&content, |caps: &regex::Captures| {
			let original_path = &caps[1];

			// Resolve from manifest
			if let Some(hashed_path) = self.manifest.get(original_path) {
				format!("{}{}", self.config.static_url, hashed_path)
			} else {
				if self.options.verbosity > 0 {
					eprintln!(
						"⚠️  Static file '{}' not in manifest, using original path",
						original_path
					);
				}
				format!("{}{}", self.config.static_url, original_path)
			}
		});

		fs::write(dest, processed.as_bytes())?;

		if self.options.verbosity > 1 {
			println!("✓ Processed HTML template: {}", source.display());
		}

		Ok(())
	}

	fn print_summary(&self, stats: &CollectStaticStats) {
		println!("\n{} static files copied", stats.copied);

		if stats.unmodified > 0 {
			println!("{} files unmodified", stats.unmodified);
		}

		if stats.skipped > 0 {
			println!("{} files skipped", stats.skipped);
		}

		if stats.deleted > 0 {
			println!("{} files deleted", stats.deleted);
		}
	}
}

#[derive(Debug, PartialEq)]
enum CopyResult {
	Copied,
	Unmodified,
}

#[async_trait]
impl BaseCommand for CollectStaticCommand {
	fn name(&self) -> &str {
		"collectstatic"
	}

	async fn execute(&self, _ctx: &CommandContext) -> CommandResult<()> {
		// BaseCommand requires async, but our logic is sync
		// We simply return Ok as the actual execution happens via the sync execute() method
		Ok(())
	}
}

impl Clone for CollectStaticCommand {
	fn clone(&self) -> Self {
		Self {
			config: self.config.clone(),
			options: self.options.clone(),
			manifest: HashMap::new(),
			index_source: self.index_source.clone(),
		}
	}
}
