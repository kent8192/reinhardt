use reinhardt_db::migrations::{
	MigrationError,
	introspect::{GeneratedFile, GeneratedOutput, write_generated_files_atomically},
};
use std::{fs, io, path::Path};

fn generated_output(files: Vec<GeneratedFile>) -> GeneratedOutput {
	GeneratedOutput { files }
}

fn directory_entries(path: &Path) -> Vec<String> {
	let mut entries: Vec<_> = fs::read_dir(path)
		.expect("output directory should remain readable")
		.map(|entry| {
			entry
				.expect("directory entry should remain readable")
				.file_name()
				.into_string()
				.expect("test file names should be UTF-8")
		})
		.collect();
	entries.sort();
	entries
}

#[test]
fn existing_destination_without_force_is_rejected_before_any_mutation() {
	let temp_dir = tempfile::Builder::new()
		.prefix("inspectdb-output-")
		.tempdir_in("/tmp")
		.expect("temporary directory should be created");
	let new_destination = temp_dir.path().join("new.rs");
	let existing_destination = temp_dir.path().join("existing.rs");
	fs::write(&existing_destination, b"original bytes").expect("existing file should be created");
	let output = generated_output(vec![
		GeneratedFile::new(&new_destination, "new bytes"),
		GeneratedFile::new(&existing_destination, "replacement bytes"),
	]);

	let error = write_generated_files_atomically(&output, false)
		.expect_err("an existing destination should reject the entire output");

	match error {
		MigrationError::IoError(error) => {
			assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
		}
		other => panic!("expected an I/O error, got {other:?}"),
	}
	assert!(!new_destination.exists());
	assert_eq!(
		fs::read(&existing_destination).expect("existing file should remain readable"),
		b"original bytes"
	);
	assert_eq!(
		directory_entries(temp_dir.path()),
		vec!["existing.rs".to_string()]
	);
}

#[test]
fn current_directory_aliases_are_rejected_before_mutation() {
	let temp_dir = tempfile::Builder::new()
		.prefix("inspectdb-output-")
		.tempdir_in("/tmp")
		.expect("temporary directory should be created");
	let destination = temp_dir.path().join("model.rs");
	let dot_alias = temp_dir.path().join(".").join("model.rs");
	let output = generated_output(vec![
		GeneratedFile::new(&destination, "first bytes"),
		GeneratedFile::new(&dot_alias, "second bytes"),
	]);

	let error = write_generated_files_atomically(&output, false)
		.expect_err("lexical aliases should be rejected");

	match error {
		MigrationError::IoError(error) => {
			assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
		}
		other => panic!("expected an I/O error, got {other:?}"),
	}
	assert!(!destination.exists());
	assert_eq!(directory_entries(temp_dir.path()), Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn symlink_parent_aliases_are_rejected_before_mutation() {
	use std::os::unix::fs::symlink;

	let temp_dir = tempfile::Builder::new()
		.prefix("inspectdb-output-")
		.tempdir_in("/tmp")
		.expect("temporary directory should be created");
	let real_parent = temp_dir.path().join("real");
	let alias_parent = temp_dir.path().join("alias");
	fs::create_dir(&real_parent).expect("real parent should be created");
	symlink(&real_parent, &alias_parent).expect("parent alias should be created");
	let real_destination = real_parent.join("model.rs");
	let alias_destination = alias_parent.join("model.rs");
	let output = generated_output(vec![
		GeneratedFile::new(&real_destination, "first bytes"),
		GeneratedFile::new(&alias_destination, "second bytes"),
	]);

	let error = write_generated_files_atomically(&output, false)
		.expect_err("destinations resolving to the same path should be rejected");

	match error {
		MigrationError::IoError(error) => {
			assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
		}
		other => panic!("expected an I/O error, got {other:?}"),
	}
	assert!(!real_destination.exists());
	assert_eq!(directory_entries(&real_parent), Vec::<String>::new());
}

#[test]
fn parent_traversal_destination_is_rejected_before_mutation() {
	let temp_dir = tempfile::Builder::new()
		.prefix("inspectdb-output-")
		.tempdir_in("/tmp")
		.expect("temporary directory should be created");
	let nested = temp_dir.path().join("nested");
	fs::create_dir(&nested).expect("nested directory should be created");
	let escaped_destination = nested.join("..").join("escaped.rs");
	let output = generated_output(vec![GeneratedFile::new(
		&escaped_destination,
		"escaped bytes",
	)]);

	let error = write_generated_files_atomically(&output, false)
		.expect_err("parent traversal should be rejected");

	match error {
		MigrationError::IoError(error) => {
			assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
		}
		other => panic!("expected an I/O error, got {other:?}"),
	}
	assert!(!temp_dir.path().join("escaped.rs").exists());
	assert_eq!(
		directory_entries(temp_dir.path()),
		vec!["nested".to_string()]
	);
}

#[test]
fn normalized_ancestor_destinations_are_rejected_before_mutation() {
	let temp_dir = tempfile::Builder::new()
		.prefix("inspectdb-output-")
		.tempdir_in("/tmp")
		.expect("temporary directory should be created");
	let ancestor = temp_dir.path().join("generated");
	let descendant = temp_dir.path().join(".").join("generated").join("model.rs");
	let output = generated_output(vec![
		GeneratedFile::new(&ancestor, "ancestor bytes"),
		GeneratedFile::new(&descendant, "descendant bytes"),
	]);

	let error = write_generated_files_atomically(&output, false)
		.expect_err("ancestor destinations should be rejected after normalization");

	match error {
		MigrationError::IoError(error) => {
			assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
		}
		other => panic!("expected an I/O error, got {other:?}"),
	}
	assert!(!ancestor.exists());
	assert_eq!(directory_entries(temp_dir.path()), Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn successful_force_write_preserves_permissions_and_removes_artifacts() {
	use std::os::unix::fs::PermissionsExt;

	let temp_dir = tempfile::Builder::new()
		.prefix("inspectdb-output-")
		.tempdir_in("/tmp")
		.expect("temporary directory should be created");
	let existing = temp_dir.path().join("existing.rs");
	let new_destination = temp_dir.path().join("new.rs");
	fs::write(&existing, b"original bytes").expect("existing file should be created");
	fs::set_permissions(&existing, fs::Permissions::from_mode(0o640))
		.expect("existing permissions should be set");
	let output = generated_output(vec![
		GeneratedFile::new(&existing, "replacement bytes"),
		GeneratedFile::new(&new_destination, "new bytes"),
	]);

	write_generated_files_atomically(&output, true)
		.expect("all generated files should be installed");

	assert_eq!(
		fs::read(&existing).expect("existing output should be readable"),
		b"replacement bytes"
	);
	assert_eq!(
		fs::metadata(&existing)
			.expect("existing output metadata should be readable")
			.permissions()
			.mode() & 0o777,
		0o640
	);
	assert_eq!(
		fs::read(&new_destination).expect("new output should be readable"),
		b"new bytes"
	);
	assert_eq!(
		directory_entries(temp_dir.path()),
		vec!["existing.rs".to_string(), "new.rs".to_string()]
	);
}

#[cfg(unix)]
#[test]
fn successful_force_write_preserves_unix_special_permission_bits() {
	use std::os::unix::fs::PermissionsExt;

	let temp_dir = tempfile::Builder::new()
		.prefix("inspectdb-output-")
		.tempdir_in("/tmp")
		.expect("temporary directory should be created");
	let destination = temp_dir.path().join("special-mode.rs");
	fs::write(&destination, b"original bytes").expect("existing file should be created");
	fs::set_permissions(&destination, fs::Permissions::from_mode(0o6751))
		.expect("special permissions should be set");
	let expected_mode = fs::metadata(&destination)
		.expect("existing metadata should be readable")
		.permissions()
		.mode()
		& 0o7777;
	assert_eq!(expected_mode & 0o4777, 0o4751);
	let output = generated_output(vec![GeneratedFile::new(&destination, "replacement bytes")]);

	write_generated_files_atomically(&output, true)
		.expect("generated file should replace the destination");

	assert_eq!(
		fs::read(&destination).expect("generated output should be readable"),
		b"replacement bytes"
	);
	assert_eq!(
		fs::metadata(&destination)
			.expect("generated metadata should be readable")
			.permissions()
			.mode() & 0o7777,
		expected_mode
	);
}
