use reinhardt_test::static_files::{assertions, config_helpers, integration_helpers};
use reinhardt_utils::staticfiles::handler::StaticError;
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use tempfile::NamedTempFile;

#[rstest]
#[tokio::test]
async fn static_integration_helpers_find_serve_and_reject_traversal() {
	// Arrange
	let setup = integration_helpers::IntegrationTestSetup::with_multiple_dirs();
	let first_path = setup.create_test_file("nested/app.css", b"body { color: teal; }");
	let second_path = setup.temp_dirs[1].path().join("images/logo.svg");
	fs::create_dir_all(second_path.parent().unwrap()).unwrap();
	fs::write(&second_path, b"<svg viewBox=\"0 0 1 1\"/>").unwrap();
	let outside_file = NamedTempFile::new_in(setup.temp_dirs[0].path().parent().unwrap()).unwrap();
	fs::write(outside_file.path(), b"secret").unwrap();
	let outside_name = outside_file
		.path()
		.file_name()
		.unwrap()
		.to_str()
		.unwrap()
		.to_owned();
	let traversal_path = format!("../{outside_name}");

	// Act
	let found_first = setup.finder.find("nested/app.css").unwrap();
	let found_second = setup.finder.find("/images/logo.svg").unwrap();
	let served = setup.handler.serve("nested/app.css").await.unwrap();
	let missing = setup.handler.serve("missing.css").await.unwrap_err();
	let traversal = setup.handler.serve(&traversal_path).await.unwrap_err();

	// Assert
	assert_eq!(setup.config.static_root, setup.temp_dirs[0].path());
	assert_eq!(setup.config.static_url, "/static/");
	assert_eq!(setup.config.staticfiles_dirs.len(), 2);
	assert_eq!(
		fs::read(&found_first).unwrap(),
		fs::read(&first_path).unwrap()
	);
	assert_eq!(found_first.file_name().unwrap(), "app.css");
	assert_eq!(
		fs::read(&found_second).unwrap(),
		fs::read(&second_path).unwrap()
	);
	assert_eq!(found_second.file_name().unwrap(), "logo.svg");
	assert_eq!(fs::read(&served.path).unwrap(), b"body { color: teal; }");
	assert_eq!(served.path.file_name().unwrap(), "app.css");
	assert_eq!(served.content, b"body { color: teal; }");
	assert_eq!(served.mime_type, "text/css");
	match missing {
		StaticError::NotFound(path) => assert_eq!(path, "missing.css"),
		other => panic!("expected not found error, got {other:?}"),
	}
	match traversal {
		StaticError::DirectoryTraversal(path) => assert_eq!(path, traversal_path),
		other => panic!("expected traversal error, got {other:?}"),
	}
	match setup.finder.find(&traversal_path) {
		Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::NotFound),
		Ok(path) => panic!(
			"expected traversal lookup to fail, found {}",
			path.display()
		),
	}
}

#[rstest]
#[tokio::test]
async fn static_helpers_build_configs_and_assert_single_directory_results() {
	// Arrange
	let default_config = config_helpers::create_default_config();
	let setup = integration_helpers::IntegrationTestSetup::default();
	let root = setup.temp_dirs[0].path().to_path_buf();
	let custom_config =
		config_helpers::create_custom_config(root.clone(), "/assets/".into(), vec![root.clone()]);
	let created = setup.create_test_file("index.html", b"<h1>static</h1>");
	let outside_file = NamedTempFile::new_in(root.parent().unwrap()).unwrap();
	fs::write(outside_file.path(), b"private").unwrap();
	let outside_name = outside_file
		.path()
		.file_name()
		.unwrap()
		.to_str()
		.unwrap()
		.to_owned();
	let traversal_path = format!("../{outside_name}");

	// Act
	let found = setup.finder.find("index.html").unwrap();
	let served = setup.handler.serve("index.html").await;
	let missing = setup.handler.serve("missing.html").await;
	let missing_for_success_rejection = setup.handler.serve("missing.html").await;
	let traversal = setup.handler.serve(&traversal_path).await;
	let served_for_not_found_rejection = setup.handler.serve("index.html").await;
	let served_for_traversal_rejection = setup.handler.serve("index.html").await;
	let wrong_root = root.join("wrong");
	let root_assertion_rejection = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		config_helpers::assert_config_properties(&custom_config, &wrong_root, "/assets/", 1);
	}));
	let url_assertion_rejection = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		config_helpers::assert_config_properties(&custom_config, &root, "/wrong/", 1);
	}));
	let directory_count_assertion_rejection =
		std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			config_helpers::assert_config_properties(&custom_config, &root, "/assets/", 2);
		}));

	// Assert
	assert_eq!(default_config.static_root, PathBuf::from("static"));
	assert_eq!(default_config.static_url, "/static/");
	assert_eq!(default_config.staticfiles_dirs, Vec::<PathBuf>::new());
	assert_eq!(default_config.media_url, None);
	config_helpers::assert_config_properties(&custom_config, &root, "/assets/", 1);
	assert!(root_assertion_rejection.is_err());
	assert!(url_assertion_rejection.is_err());
	assert!(directory_count_assertion_rejection.is_err());
	assert_eq!(custom_config.staticfiles_dirs, vec![root]);
	assert_eq!(fs::read(found).unwrap(), fs::read(created).unwrap());
	let success_assertion_rejection =
		std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			assertions::assert_file_served_successfully(
				missing_for_success_rejection,
				b"wrong content",
			)
		}));
	let not_found_assertion_rejection =
		std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			assertions::assert_file_not_found_error(served_for_not_found_rejection)
		}));
	let traversal_assertion_rejection =
		std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			assertions::assert_directory_traversal_blocked(served_for_traversal_rejection)
		}));
	assert!(success_assertion_rejection.is_err());
	assert!(not_found_assertion_rejection.is_err());
	assert!(traversal_assertion_rejection.is_err());
	assertions::assert_file_served_successfully(served, b"<h1>static</h1>");
	assertions::assert_file_not_found_error(missing);
	assertions::assert_directory_traversal_blocked(traversal);
}
