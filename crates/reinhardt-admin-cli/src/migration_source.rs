//! Upgrade generated migration source files to the current source format.

use std::fs::{self, OpenOptions};
use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Args;
use reinhardt_db::migrations::{source_format::has_source_format_marker, upgrade_source};
use syn::{File, Item};
use thiserror::Error;
use walkdir::WalkDir;

/// Arguments for `reinhardt-admin migrations upgrade-source`.
#[derive(Args, Debug)]
pub struct UpgradeSourceArgs {
	/// Migration directory or source file to inspect.
	#[arg(default_value = "migrations", value_name = "PATH")]
	pub path: PathBuf,

	/// Check for upgrades without writing files; exits unsuccessfully when drift exists.
	#[arg(long)]
	pub check: bool,
}

/// Errors produced while upgrading migration source files.
#[derive(Debug, Error)]
pub enum MigrationSourceError {
	/// The requested path does not exist or cannot be read.
	#[error("I/O operation failed: {0}")]
	Io(#[from] std::io::Error),
	/// A source tree entry could not be enumerated.
	#[error("failed to walk migration sources: {0}")]
	Walk(#[from] walkdir::Error),
	/// One or more files failed the preflight pass.
	#[error("migration source upgrade preflight failed:\n{0}")]
	Preflight(String),
	/// A write failed after preflight completed.
	#[error("failed to write upgraded migration source: {0}")]
	Write(String),
}

/// Result type for migration source upgrades.
pub type Result<T> = std::result::Result<T, MigrationSourceError>;

struct PlannedUpgrade {
	path: PathBuf,
	relative_path: PathBuf,
	original_source: String,
	source: String,
}

struct TemporaryFileGuard {
	path: PathBuf,
	committed: bool,
}

impl TemporaryFileGuard {
	fn new(path: PathBuf) -> Self {
		Self {
			path,
			committed: false,
		}
	}

	fn commit(&mut self) {
		self.committed = true;
	}
}

struct DestinationPermissionsGuard {
	path: PathBuf,
	permissions: fs::Permissions,
	armed: bool,
}

impl DestinationPermissionsGuard {
	fn new(path: &Path, permissions: fs::Permissions) -> Self {
		Self {
			path: path.to_path_buf(),
			permissions,
			armed: true,
		}
	}

	fn make_writable(path: &Path, permissions: fs::Permissions) -> std::io::Result<Self> {
		Self::make_writable_with(path, permissions, |path, permissions| {
			fs::set_permissions(path, permissions)
		})
	}

	fn make_writable_with(
		path: &Path,
		permissions: fs::Permissions,
		set_permissions: impl FnOnce(&Path, fs::Permissions) -> std::io::Result<()>,
	) -> std::io::Result<Self> {
		let guard = Self::new(path, permissions);
		let writable_permissions = make_permissions_writable(&guard.permissions);
		set_permissions(&guard.path, writable_permissions)?;
		Ok(guard)
	}

	fn disarm(&mut self) {
		self.armed = false;
	}
}

fn make_permissions_writable(permissions: &fs::Permissions) -> fs::Permissions {
	let mut writable_permissions = permissions.clone();
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;

		writable_permissions.set_mode(writable_permissions.mode() | 0o200);
	}
	#[cfg(not(unix))]
	writable_permissions.set_readonly(false);
	writable_permissions
}

impl Drop for DestinationPermissionsGuard {
	fn drop(&mut self) {
		if self.armed {
			let _ = fs::set_permissions(&self.path, self.permissions.clone());
		}
	}
}

impl Drop for TemporaryFileGuard {
	fn drop(&mut self) {
		if !self.committed {
			let _ = fs::remove_file(&self.path);
		}
	}
}

/// Upgrade all generated migration sources below `args.path`.
///
/// The complete tree is parsed and converted before any destination is
/// touched. `--check` performs the same preflight and reports drift without
/// writing files.
pub fn run(args: UpgradeSourceArgs) -> Result<()> {
	let root = validate_root(&args.path)?;
	let files = find_source_files(&root, args.path.is_file())?;
	let mut plans = Vec::new();
	let mut diagnostics = Vec::new();

	for path in files {
		let relative_path = path
			.strip_prefix(&root)
			.ok()
			.filter(|relative| !relative.as_os_str().is_empty())
			.map(Path::to_path_buf)
			.unwrap_or_else(|| path.file_name().map_or_else(PathBuf::new, PathBuf::from));
		let source = match fs::read_to_string(&path) {
			Ok(source) => source,
			Err(error) => {
				diagnostics.push(format!("{}: {error}", relative_path.display()));
				continue;
			}
		};
		let file = match syn::parse_file(&source) {
			Ok(file) => file,
			Err(error) => {
				diagnostics.push(format!(
					"{}: failed to parse Rust source: {error}",
					relative_path.display()
				));
				continue;
			}
		};
		let has_source_marker = match has_source_format_marker(&source) {
			Ok(has_source_marker) => has_source_marker,
			Err(error) => {
				diagnostics.push(format!("{}: {error}", relative_path.display()));
				continue;
			}
		};
		if !has_source_marker && !defines_migration(&file) {
			continue;
		}
		match upgrade_source(&source) {
			Ok(result) if result.changed => plans.push(PlannedUpgrade {
				path,
				relative_path,
				original_source: source,
				source: result.source,
			}),
			Ok(_) => {}
			Err(error) => diagnostics.push(format!("{}: {error}", relative_path.display())),
		}
	}

	if !diagnostics.is_empty() {
		diagnostics.sort();
		return Err(MigrationSourceError::Preflight(diagnostics.join("\n")));
	}

	plans.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
	if args.check {
		for plan in &plans {
			println!("would upgrade: {}", plan.relative_path.display());
		}
		if plans.is_empty() {
			println!("migration source format is current");
			return Ok(());
		}
		return Err(MigrationSourceError::Preflight(format!(
			"{} migration source file(s) require upgrade",
			plans.len()
		)));
	}

	for plan in &plans {
		current_metadata(&plan.path, &plan.original_source)?;
	}
	for plan in &plans {
		write_atomically(&plan.path, &plan.original_source, &plan.source)?;
		println!("upgraded: {}", plan.relative_path.display());
	}
	println!("{} migration source file(s) upgraded", plans.len());
	Ok(())
}

fn validate_root(path: &Path) -> Result<PathBuf> {
	let metadata = fs::symlink_metadata(path)?;
	if metadata.file_type().is_symlink() {
		return Err(MigrationSourceError::Preflight(format!(
			"{}: symlinks are not accepted",
			path.display()
		)));
	}
	let root = path.canonicalize()?;
	if !root.is_dir() && !root.is_file() {
		return Err(MigrationSourceError::Preflight(format!(
			"{}: expected a directory or Rust source file",
			path.display()
		)));
	}
	if root.is_dir() && root.join("Cargo.toml").is_file() {
		return Err(MigrationSourceError::Preflight(format!(
			"{}: refusing to scan a repository root; pass a migrations directory or source file",
			root.display()
		)));
	}
	if root.is_file() && root.extension().is_none_or(|extension| extension != "rs") {
		return Err(MigrationSourceError::Preflight(format!(
			"{}: expected a .rs source file",
			root.display()
		)));
	}
	Ok(root)
}

fn find_source_files(root: &Path, single_file: bool) -> Result<Vec<PathBuf>> {
	if single_file {
		return Ok(vec![root.to_path_buf()]);
	}

	let mut files = Vec::new();
	for entry in WalkDir::new(root).follow_links(false) {
		let entry = entry?;
		let path = entry.path();
		if path != root && entry.file_type().is_symlink() {
			return Err(MigrationSourceError::Preflight(format!(
				"{}: symlinks are not accepted",
				path.display()
			)));
		}
		if !entry.file_type().is_file()
			|| path.extension().is_none_or(|extension| extension != "rs")
		{
			continue;
		}
		let canonical = path.canonicalize()?;
		if !canonical.starts_with(root) {
			return Err(MigrationSourceError::Preflight(format!(
				"{}: path escapes the requested root",
				path.display()
			)));
		}
		files.push(canonical);
	}
	files.sort();
	Ok(files)
}

fn defines_migration(file: &File) -> bool {
	file.items
		.iter()
		.any(|item| matches!(item, Item::Fn(function) if function.sig.ident == "migration"))
}

fn current_metadata(path: &Path, expected_source: &str) -> Result<fs::Metadata> {
	verify_current_source(path, expected_source)
		.map_err(|error| MigrationSourceError::Write(format!("{}: {error}", path.display())))
}

fn verify_current_source(path: &Path, expected_source: &str) -> std::io::Result<fs::Metadata> {
	let metadata = fs::symlink_metadata(path)?;
	if metadata.file_type().is_symlink() {
		return Err(Error::new(
			ErrorKind::InvalidInput,
			"destination became a symlink",
		));
	}
	let current_source = fs::read_to_string(path)?;
	if current_source != expected_source {
		return Err(Error::new(
			ErrorKind::InvalidData,
			"source changed after preflight",
		));
	}
	Ok(metadata)
}

fn write_atomically(path: &Path, expected_source: &str, content: &str) -> Result<()> {
	let mut metadata = current_metadata(path, expected_source)?;
	let parent = path.parent().ok_or_else(|| {
		MigrationSourceError::Write(format!("{}: destination has no parent", path.display()))
	})?;
	let name = path
		.file_name()
		.and_then(|name| name.to_str())
		.ok_or_else(|| {
			MigrationSourceError::Write(format!(
				"{}: destination has no valid file name",
				path.display()
			))
		})?;
	let nonce = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_nanos();
	let mut temporary = TemporaryFileGuard::new(parent.join(format!(
		".{name}.{}.{}.tmp",
		std::process::id(),
		nonce
	)));
	let mut destination_permissions_guard = None;
	let write_result = (|| -> std::io::Result<()> {
		let mut file = OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&temporary.path)?;
		file.write_all(content.as_bytes())?;
		file.sync_all()?;
		if !metadata.permissions().readonly() {
			file.set_permissions(metadata.permissions())?;
			file.sync_all()?;
		}
		metadata = verify_current_source(path, expected_source)?;
		if metadata.permissions().readonly() {
			let original_permissions = metadata.permissions();
			destination_permissions_guard = Some(DestinationPermissionsGuard::make_writable(
				path,
				original_permissions,
			)?);
		}
		fs::rename(&temporary.path, path)?;
		// The rename transferred ownership of the temporary path to the destination.
		temporary.commit();
		fs::set_permissions(path, metadata.permissions())?;
		if let Some(guard) = destination_permissions_guard.as_mut() {
			guard.disarm();
		}
		Ok(())
	})();
	if let Err(error) = write_result {
		return Err(MigrationSourceError::Write(format!(
			"{}: {error}",
			path.display()
		)));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	const LEGACY_DROP_COLUMN: &str = r#"fn migration() -> Migration {
    Migration::new("0002_remove_owner", "app").add_operation(Operation::DropColumn {
        table: "items".into(), column: "owner_id".into()
    })
}
"#;

	#[test]
	fn legacy_drop_column_check_upgrade_and_repeat() {
		let directory = tempfile::tempdir().unwrap();
		let path = directory.path().join("0002_remove_owner.rs");
		fs::write(&path, LEGACY_DROP_COLUMN).unwrap();

		let error = run(UpgradeSourceArgs {
			path: directory.path().into(),
			check: true,
		})
		.unwrap_err();
		assert_eq!(
			error.to_string(),
			MigrationSourceError::Preflight("1 migration source file(s) require upgrade".into())
				.to_string()
		);
		assert_eq!(fs::read(&path).unwrap(), LEGACY_DROP_COLUMN.as_bytes());
		run(UpgradeSourceArgs {
			path: directory.path().into(),
			check: false,
		})
		.unwrap();
		let upgraded = fs::read(&path).unwrap();
		assert_ne!(upgraded, LEGACY_DROP_COLUMN.as_bytes());
		for check in [true, false] {
			run(UpgradeSourceArgs {
				path: directory.path().into(),
				check,
			})
			.unwrap();
			assert_eq!(fs::read(&path).unwrap(), upgraded);
		}
		assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
	}

	#[rstest]
	#[case::check(true)]
	#[case::write(false)]
	fn malformed_current_drop_column_prevents_all_writes(#[case] check: bool) {
		let directory = tempfile::tempdir().unwrap();
		let valid = directory.path().join("0001_legacy.rs");
		let invalid = directory.path().join("0002_inconsistent.rs");
		let inconsistent = format!("// reinhardt-migration-source: 1\n{LEGACY_DROP_COLUMN}");
		fs::write(&valid, LEGACY_DROP_COLUMN).unwrap();
		fs::write(&invalid, &inconsistent).unwrap();
		let before = [fs::read(&valid).unwrap(), fs::read(&invalid).unwrap()];

		let error = run(UpgradeSourceArgs {
			path: directory.path().into(),
			check,
		})
		.unwrap_err();

		assert_eq!(
			error.to_string(),
			MigrationSourceError::Preflight(
				"0002_inconsistent.rs: Invalid migration: source format marker is current but legacy struct-literal syntax remains".into()
			)
			.to_string()
		);
		assert_eq!(
			[fs::read(&valid).unwrap(), fs::read(&invalid).unwrap()],
			before
		);
		assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
	}

	#[test]
	fn check_reports_drift_without_writing() {
		let directory = tempfile::tempdir().expect("temporary directory");
		let path = directory.path().join("0001_initial.rs");
		let original = "fn migration() -> Migration { Migration { name: \"0001\".into(), app_label: \"app\".into(), operations: vec![], dependencies: vec![], replaces: vec![], atomic: true, initial: None, state_only: false, database_only: false, swappable_dependencies: vec![], optional_dependencies: vec![] } }\n";
		fs::write(&path, original).expect("write fixture");

		let error = run(UpgradeSourceArgs {
			path: directory.path().to_path_buf(),
			check: true,
		})
		.expect_err("check must report legacy drift");

		assert!(error.to_string().contains("require upgrade"));
		assert_eq!(fs::read_to_string(path).expect("read fixture"), original);
	}

	#[rstest]
	#[case::current_format(
		"// reinhardt-migration-source: 1\nfn renamed_migration() -> Migration { Migration::new(\"0001\", \"app\") }\n",
		"missing migration() entrypoint"
	)]
	#[case::future_format(
		"// reinhardt-migration-source: 2\nfn renamed_migration() -> Migration { Migration::new(\"0001\", \"app\") }\n",
		"requires a newer Reinhardt tool"
	)]
	fn check_rejects_marked_sources_without_migration_entrypoints(
		#[case] source: &str,
		#[case] expected: &str,
	) {
		let directory = tempfile::tempdir().expect("temporary directory");
		let path = directory.path().join("0001_initial.rs");
		fs::write(&path, source).expect("write fixture");

		let error = run(UpgradeSourceArgs {
			path: directory.path().to_path_buf(),
			check: true,
		})
		.expect_err("marked sources must be validated");

		assert!(error.to_string().contains(expected));
		assert_eq!(fs::read_to_string(path).expect("read fixture"), source);
	}

	#[test]
	fn temporary_file_guard_removes_files_during_unwinding() {
		let directory = tempfile::tempdir().expect("temporary directory");
		let path = directory.path().join("migration.tmp");
		fs::write(&path, "incomplete").expect("write temporary file");

		let result = std::panic::catch_unwind(|| {
			let _guard = TemporaryFileGuard::new(path.clone());
			panic!("abort the write");
		});

		assert!(result.is_err());
		assert!(!path.exists());
	}

	#[test]
	fn destination_permissions_guard_restores_during_unwinding() {
		let directory = tempfile::tempdir().expect("temporary directory");
		let path = directory.path().join("migration.rs");
		fs::write(&path, "source").expect("write destination");
		let mut original_permissions = fs::metadata(&path)
			.expect("read destination metadata")
			.permissions();
		original_permissions.set_readonly(true);
		fs::set_permissions(&path, original_permissions.clone())
			.expect("make destination readonly");
		let mut writable_permissions = original_permissions.clone();
		writable_permissions.set_readonly(false);
		fs::set_permissions(&path, writable_permissions).expect("make destination writable");

		let result = std::panic::catch_unwind(|| {
			let _guard = DestinationPermissionsGuard::new(&path, original_permissions.clone());
			panic!("abort the replacement");
		});

		assert!(result.is_err());
		assert!(
			fs::metadata(path)
				.expect("read destination metadata")
				.permissions()
				.readonly()
		);
	}

	#[test]
	fn destination_permissions_guard_restores_when_writable_transition_errors() {
		let directory = tempfile::tempdir().expect("temporary directory");
		let path = directory.path().join("migration.rs");
		fs::write(&path, "source").expect("write destination");
		let mut original_permissions = fs::metadata(&path)
			.expect("read destination metadata")
			.permissions();
		original_permissions.set_readonly(true);
		fs::set_permissions(&path, original_permissions.clone())
			.expect("make destination readonly");

		let result = DestinationPermissionsGuard::make_writable_with(
			&path,
			original_permissions,
			|path, writable_permissions| {
				fs::set_permissions(path, writable_permissions)?;
				Err(Error::other("injected transition failure"))
			},
		);

		assert!(matches!(result, Err(error) if error.kind() == ErrorKind::Other));
		assert!(
			fs::metadata(path)
				.expect("read destination metadata")
				.permissions()
				.readonly()
		);
	}

	#[test]
	fn atomic_upgrade_rejects_changed_destination() {
		let directory = tempfile::tempdir().expect("temporary directory");
		let path = directory.path().join("0001_initial.rs");
		fs::write(&path, "newer").expect("write destination");

		let error = write_atomically(&path, "preflight", "upgraded")
			.expect_err("changed destination must abort replacement");

		assert!(error.to_string().contains("source changed after preflight"));
		assert_eq!(fs::read_to_string(path).expect("read destination"), "newer");
	}

	#[test]
	fn atomic_upgrade_replaces_readonly_destinations() {
		let directory = tempfile::tempdir().expect("temporary directory");
		let path = directory.path().join("0001_initial.rs");
		fs::write(&path, "old").expect("write destination");
		let mut permissions = fs::metadata(&path)
			.expect("read destination metadata")
			.permissions();
		permissions.set_readonly(true);
		fs::set_permissions(&path, permissions).expect("set readonly permissions");

		write_atomically(&path, "old", "new").expect("replace readonly destination");

		assert_eq!(fs::read_to_string(&path).expect("read destination"), "new");
		assert!(
			fs::metadata(path)
				.expect("read destination metadata")
				.permissions()
				.readonly()
		);
	}

	#[cfg(unix)]
	#[test]
	fn atomic_upgrade_preserves_destination_permissions() {
		use std::os::unix::fs::PermissionsExt;

		let directory = tempfile::tempdir().expect("temporary directory");
		let path = directory.path().join("0001_initial.rs");
		fs::write(&path, "old").expect("write destination");
		fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("set permissions");

		write_atomically(&path, "old", "new").expect("replace destination");

		let mode = fs::metadata(path)
			.expect("read destination metadata")
			.permissions()
			.mode() & 0o777;
		assert_eq!(mode, 0o640);
	}
}
