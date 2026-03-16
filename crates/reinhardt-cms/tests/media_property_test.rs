//! Property-based tests for media management

use proptest::prelude::*;
use reinhardt_cms::media::{CropMode, MediaManager, RenditionSpec};

proptest! {
	#[test]
	fn prop_media_upload_size_matches_data_length(data_len in 0usize..10000) {
		// Arrange
		let rt = tokio::runtime::Runtime::new().unwrap();
		let data = vec![0u8; data_len];

		// Act
		let media = rt.block_on(async {
			let mut manager = MediaManager::new();
			manager.upload("test.bin".to_string(), data).await.unwrap()
		});

		// Assert
		prop_assert_eq!(media.size, data_len as u64);
	}

	#[test]
	fn prop_media_path_contains_id(data_len in 1usize..1000) {
		// Arrange
		let rt = tokio::runtime::Runtime::new().unwrap();
		let data = vec![0u8; data_len];

		// Act
		let media = rt.block_on(async {
			let mut manager = MediaManager::new();
			manager.upload("test.png".to_string(), data).await.unwrap()
		});

		// Assert
		let expected_path = format!("/media/{}", media.id);
		prop_assert_eq!(media.path, expected_path);
	}

	#[test]
	fn fuzz_media_upload_random_filenames(filename in "[a-zA-Z0-9._-]{0,100}") {
		// Arrange
		let rt = tokio::runtime::Runtime::new().unwrap();
		let expected = filename.clone();
		let data = vec![0u8; 10];

		// Act
		let media = rt.block_on(async {
			let mut manager = MediaManager::new();
			manager.upload(filename, data).await.unwrap()
		});

		// Assert
		prop_assert_eq!(media.filename, expected);
	}

	#[test]
	fn fuzz_media_upload_random_data_sizes(size in 0usize..100000) {
		// Arrange
		let rt = tokio::runtime::Runtime::new().unwrap();
		let data = vec![0u8; size];

		// Act
		let media = rt.block_on(async {
			let mut manager = MediaManager::new();
			manager.upload("test.jpg".to_string(), data).await.unwrap()
		});

		// Assert
		prop_assert_eq!(media.size, size as u64);
	}

	#[test]
	fn fuzz_rendition_spec_random_dimensions(
		w in proptest::option::of(0u32..10000),
		h in proptest::option::of(0u32..10000),
		mode_idx in 0u8..5,
		quality in proptest::option::of(0u8..=255u8),
	) {
		// Arrange
		let rt = tokio::runtime::Runtime::new().unwrap();
		let mode = match mode_idx {
			0 => CropMode::Fit,
			1 => CropMode::Fill,
			2 => CropMode::Crop,
			3 => CropMode::Width,
			_ => CropMode::Height,
		};
		let spec = RenditionSpec {
			width: w,
			height: h,
			mode,
			format: None,
			quality,
		};

		// Act
		let (media_id, rendition) = rt.block_on(async {
			let mut manager = MediaManager::new();
			let media = manager
				.upload("test.jpg".to_string(), vec![0u8; 100])
				.await
				.unwrap();
			let mid = media.id;
			let r = manager.get_rendition(mid, spec).await.unwrap();
			(mid, r)
		});

		// Assert
		prop_assert_eq!(rendition.media_id, media_id);
	}
}
