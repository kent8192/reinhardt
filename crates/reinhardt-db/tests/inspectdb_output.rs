use reinhardt_db::migrations::{
	MigrationError,
	introspect::{
		GeneratedFile, GeneratedOutput, write_generated_files_atomically,
		write_generated_files_atomically_with_commit_hook,
	},
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

#[cfg(unix)]
#[test]
fn force_failure_restores_prior_bytes_and_permissions() {
	use std::os::unix::fs::PermissionsExt;

	let temp_dir = tempfile::Builder::new()
		.prefix("inspectdb-output-")
		.tempdir_in("/tmp")
		.expect("temporary directory should be created");
	let first = temp_dir.path().join("first.rs");
	let second = temp_dir.path().join("second.rs");
	fs::write(&first, b"first original").expect("first file should be created");
	fs::write(&second, b"second original").expect("second file should be created");
	fs::set_permissions(&first, fs::Permissions::from_mode(0o640))
		.expect("first permissions should be set");
	fs::set_permissions(&second, fs::Permissions::from_mode(0o604))
		.expect("second permissions should be set");
	let output = generated_output(vec![
		GeneratedFile::new(&first, "first replacement"),
		GeneratedFile::new(&second, "second replacement"),
	]);

	let error =
		write_generated_files_atomically_with_commit_hook(&output, true, |index, _destination| {
			if index == 1 {
				Err(io::Error::other("injected second commit failure"))
			} else {
				Ok(())
			}
		})
		.expect_err("the injected second commit failure should be returned");

	match error {
		MigrationError::IoError(error) => {
			assert_eq!(error.kind(), io::ErrorKind::Other);
			assert_eq!(error.to_string(), "injected second commit failure");
		}
		other => panic!("expected an I/O error, got {other:?}"),
	}
	assert_eq!(
		fs::read(&first).expect("first file should remain readable"),
		b"first original"
	);
	assert_eq!(
		fs::read(&second).expect("second file should remain readable"),
		b"second original"
	);
	assert_eq!(
		fs::metadata(&first)
			.expect("first metadata should remain readable")
			.permissions()
			.mode() & 0o777,
		0o640
	);
	assert_eq!(
		fs::metadata(&second)
			.expect("second metadata should remain readable")
			.permissions()
			.mode() & 0o777,
		0o604
	);
	assert_eq!(
		directory_entries(temp_dir.path()),
		vec!["first.rs".to_string(), "second.rs".to_string()]
	);
}

#[test]
fn force_failure_removes_newly_created_partial_files() {
	let temp_dir = tempfile::Builder::new()
		.prefix("inspectdb-output-")
		.tempdir_in("/tmp")
		.expect("temporary directory should be created");
	let nested_directory = temp_dir.path().join("generated").join("models");
	let first = nested_directory.join("first.rs");
	let second = temp_dir.path().join("second.rs");
	let output = generated_output(vec![
		GeneratedFile::new(&first, "first generated file"),
		GeneratedFile::new(&second, "second generated file"),
	]);

	let error =
		write_generated_files_atomically_with_commit_hook(&output, true, |index, _destination| {
			if index == 1 {
				Err(io::Error::other("injected second commit failure"))
			} else {
				Ok(())
			}
		})
		.expect_err("the injected second commit failure should be returned");

	match error {
		MigrationError::IoError(error) => {
			assert_eq!(error.kind(), io::ErrorKind::Other);
			assert_eq!(error.to_string(), "injected second commit failure");
		}
		other => panic!("expected an I/O error, got {other:?}"),
	}
	assert!(!first.exists());
	assert!(!second.exists());
	assert!(!nested_directory.exists());
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
