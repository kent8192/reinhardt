//! Tests for media management

use reinhardt_cms::error::CmsError;
use reinhardt_cms::media::{CropMode, MediaFile, MediaManager, RenditionSpec};
use rstest::rstest;
use uuid::Uuid;

#[rstest]
#[tokio::test]
async fn test_media_upload_and_retrieve() {
	let mut manager = MediaManager::new();

	// Upload a file
	let filename = "test.jpg".to_string();
	let data = vec![0u8; 1024]; // 1KB dummy data
	let media = manager.upload(filename.clone(), data).await.unwrap();

	// Verify upload
	assert_eq!(media.filename, filename);
	assert_eq!(media.size, 1024);
	assert_eq!(media.mime_type, "image/jpeg");

	// Retrieve the file
	let retrieved = manager.get(media.id).await.unwrap();
	assert_eq!(retrieved.id, media.id);
	assert_eq!(retrieved.filename, filename);
}

#[rstest]
#[tokio::test]
async fn test_media_delete() {
	let mut manager = MediaManager::new();

	// Upload a file
	let filename = "test.png".to_string();
	let data = vec![0u8; 512];
	let media = manager.upload(filename, data).await.unwrap();

	// Delete the file
	manager.delete(media.id).await.unwrap();

	// Verify deletion
	let result = manager.get(media.id).await;
	assert!(result.is_err());
}

#[rstest]
#[tokio::test]
async fn test_rendition_generation() {
	let mut manager = MediaManager::new();

	// Upload an image
	let filename = "test.jpg".to_string();
	let data = vec![0u8; 2048];
	let media = manager.upload(filename, data).await.unwrap();

	// Create rendition spec
	let spec = RenditionSpec {
		width: Some(800),
		height: Some(600),
		mode: CropMode::Fill,
		format: None,
		quality: Some(85),
	};

	// Get rendition
	let rendition = manager.get_rendition(media.id, spec.clone()).await.unwrap();
	assert_eq!(rendition.media_id, media.id);
	assert_eq!(rendition.spec.width, Some(800));
	assert_eq!(rendition.spec.height, Some(600));

	// Get the same rendition again (should return existing one)
	let rendition2 = manager.get_rendition(media.id, spec).await.unwrap();
	assert_eq!(rendition.id, rendition2.id);
}

#[rstest]
#[tokio::test]
async fn test_mime_type_detection() {
	let mut manager = MediaManager::new();

	// Test various file types
	let test_cases = vec![
		("test.jpg", "image/jpeg"),
		("test.png", "image/png"),
		("test.pdf", "application/pdf"),
		("test.txt", "text/plain"),
		("test.unknown", "application/octet-stream"),
	];

	for (filename, expected_mime) in test_cases {
		let data = vec![0u8; 100];
		let media = manager.upload(filename.to_string(), data).await.unwrap();
		assert_eq!(media.mime_type, expected_mime);
	}
}

// --- Happy Path ---

#[rstest]
#[tokio::test]
async fn test_upload_non_image_file_no_dimensions() {
	// Arrange
	let mut manager = MediaManager::new();

	// Act
	let media = manager
		.upload("readme.txt".to_string(), vec![0u8; 64])
		.await
		.unwrap();

	// Assert
	assert_eq!(media.width, None);
	assert_eq!(media.height, None);
}

// --- Error Path ---

#[rstest]
#[tokio::test]
async fn test_get_nonexistent_media() {
	// Arrange
	let manager = MediaManager::new();
	let fake_id = Uuid::new_v4();

	// Act
	let result = manager.get(fake_id).await;

	// Assert
	assert!(matches!(result, Err(CmsError::MediaNotFound(_))));
}

#[rstest]
#[tokio::test]
async fn test_delete_nonexistent_media() {
	// Arrange
	let mut manager = MediaManager::new();
	let fake_id = Uuid::new_v4();

	// Act
	let result = manager.delete(fake_id).await;

	// Assert
	assert!(matches!(result, Err(CmsError::MediaNotFound(_))));
}

#[rstest]
#[tokio::test]
async fn test_rendition_for_nonexistent_media() {
	// Arrange
	let mut manager = MediaManager::new();
	let fake_id = Uuid::new_v4();
	let spec = RenditionSpec {
		width: Some(100),
		height: Some(100),
		mode: CropMode::Fit,
		format: None,
		quality: None,
	};

	// Act
	let result = manager.get_rendition(fake_id, spec).await;

	// Assert
	assert!(matches!(result, Err(CmsError::MediaNotFound(_))));
}

// --- Edge Cases ---

#[rstest]
#[tokio::test]
async fn test_upload_empty_data() {
	// Arrange
	let mut manager = MediaManager::new();

	// Act
	let media = manager
		.upload("empty.bin".to_string(), vec![])
		.await
		.unwrap();

	// Assert
	assert_eq!(media.size, 0);
}

#[rstest]
#[tokio::test]
async fn test_upload_filename_without_extension() {
	// Arrange
	let mut manager = MediaManager::new();

	// Act
	let media = manager
		.upload("README".to_string(), vec![0u8; 10])
		.await
		.unwrap();

	// Assert
	assert_eq!(media.mime_type, "application/octet-stream");
}

#[rstest]
#[tokio::test]
async fn test_upload_filename_dot_only() {
	// Arrange
	let mut manager = MediaManager::new();

	// Act
	let media = manager
		.upload(".".to_string(), vec![0u8; 10])
		.await
		.unwrap();

	// Assert
	assert_eq!(media.mime_type, "application/octet-stream");
}

// --- Sanity ---

#[rstest]
#[tokio::test]
async fn test_media_manager_default_trait() {
	// Arrange
	let mut manager = MediaManager::default();

	// Act
	let media = manager
		.upload("test.jpg".to_string(), vec![0u8; 100])
		.await
		.unwrap();

	// Assert
	assert_eq!(media.filename, "test.jpg");
	assert_eq!(media.size, 100);
}

#[rstest]
#[tokio::test]
async fn test_media_file_serialization_roundtrip() {
	// Arrange
	let mut manager = MediaManager::new();
	let media = manager
		.upload("photo.png".to_string(), vec![0u8; 256])
		.await
		.unwrap();

	// Act
	let json = serde_json::to_string(&media).unwrap();
	let deserialized: MediaFile = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.id, media.id);
	assert_eq!(deserialized.filename, media.filename);
	assert_eq!(deserialized.size, media.size);
	assert_eq!(deserialized.mime_type, media.mime_type);
	assert_eq!(deserialized.path, media.path);
	assert_eq!(deserialized.width, media.width);
	assert_eq!(deserialized.height, media.height);
}

// --- Equivalence Partitioning ---

#[rstest]
#[case("img.gif", "image/gif")]
#[case("img.webp", "image/webp")]
#[case("img.svg", "image/svg+xml")]
#[tokio::test]
async fn test_mime_type_detection_image_types(#[case] filename: &str, #[case] expected: &str) {
	// Arrange
	let mut manager = MediaManager::new();

	// Act
	let media = manager
		.upload(filename.to_string(), vec![0u8; 10])
		.await
		.unwrap();

	// Assert
	assert_eq!(media.mime_type, expected);
}

#[rstest]
#[case("page.html", "text/html")]
#[case("data.json", "application/json")]
#[tokio::test]
async fn test_mime_type_detection_application_types(
	#[case] filename: &str,
	#[case] expected: &str,
) {
	// Arrange
	let mut manager = MediaManager::new();

	// Act
	let media = manager
		.upload(filename.to_string(), vec![0u8; 10])
		.await
		.unwrap();

	// Assert
	assert_eq!(media.mime_type, expected);
}

#[rstest]
#[case(CropMode::Fit)]
#[case(CropMode::Fill)]
#[case(CropMode::Crop)]
#[case(CropMode::Width)]
#[case(CropMode::Height)]
#[tokio::test]
async fn test_crop_mode_rendition_variants(#[case] mode: CropMode) {
	// Arrange
	let mut manager = MediaManager::new();
	let media = manager
		.upload("test.jpg".to_string(), vec![0u8; 100])
		.await
		.unwrap();
	let spec = RenditionSpec {
		width: Some(200),
		height: Some(200),
		mode,
		format: None,
		quality: None,
	};

	// Act
	let rendition = manager.get_rendition(media.id, spec).await.unwrap();

	// Assert
	assert_eq!(rendition.media_id, media.id);
}

// --- Boundary Value ---

#[rstest]
#[case(0)]
#[case(1)]
#[case(1024)]
#[case(1_048_576)]
#[tokio::test]
async fn test_media_upload_size_boundaries(#[case] size: usize) {
	// Arrange
	let mut manager = MediaManager::new();
	let data = vec![0u8; size];

	// Act
	let media = manager.upload("file.bin".to_string(), data).await.unwrap();

	// Assert
	assert_eq!(media.size, size as u64);
}

#[rstest]
#[case(Some(0), Some(0))]
#[case(Some(1), Some(1))]
#[case(None, None)]
#[case(Some(10000), Some(10000))]
#[tokio::test]
async fn test_rendition_dimension_boundaries(
	#[case] width: Option<u32>,
	#[case] height: Option<u32>,
) {
	// Arrange
	let mut manager = MediaManager::new();
	let media = manager
		.upload("test.jpg".to_string(), vec![0u8; 100])
		.await
		.unwrap();
	let spec = RenditionSpec {
		width,
		height,
		mode: CropMode::Fit,
		format: None,
		quality: None,
	};

	// Act
	let rendition = manager.get_rendition(media.id, spec).await.unwrap();

	// Assert
	assert_eq!(rendition.media_id, media.id);
	assert_eq!(rendition.spec.width, width);
	assert_eq!(rendition.spec.height, height);
}

#[rstest]
#[case(Some(0))]
#[case(Some(1))]
#[case(Some(50))]
#[case(Some(100))]
#[case(Some(255))]
#[case(None)]
#[tokio::test]
async fn test_rendition_quality_boundaries(#[case] quality: Option<u8>) {
	// Arrange
	let mut manager = MediaManager::new();
	let media = manager
		.upload("test.jpg".to_string(), vec![0u8; 100])
		.await
		.unwrap();
	let spec = RenditionSpec {
		width: Some(100),
		height: Some(100),
		mode: CropMode::Fit,
		format: None,
		quality,
	};

	// Act
	let rendition = manager.get_rendition(media.id, spec).await.unwrap();

	// Assert
	assert_eq!(rendition.media_id, media.id);
	assert_eq!(rendition.spec.quality, quality);
}

// --- Combination ---

#[rstest]
#[tokio::test]
async fn test_media_with_multiple_rendition_specs_different_modes() {
	// Arrange
	let mut manager = MediaManager::new();
	let media = manager
		.upload("test.jpg".to_string(), vec![0u8; 100])
		.await
		.unwrap();
	let modes = [
		CropMode::Fit,
		CropMode::Fill,
		CropMode::Crop,
		CropMode::Width,
		CropMode::Height,
	];

	// Act
	let mut rendition_ids = Vec::new();
	for mode in modes {
		let spec = RenditionSpec {
			width: Some(300),
			height: Some(300),
			mode,
			format: None,
			quality: None,
		};
		let rendition = manager.get_rendition(media.id, spec).await.unwrap();
		rendition_ids.push(rendition.id);
	}

	// Assert - All rendition IDs should be unique
	let unique_count = {
		let mut sorted = rendition_ids.clone();
		sorted.sort();
		sorted.dedup();
		sorted.len()
	};
	assert_eq!(unique_count, 5);
}

#[rstest]
#[tokio::test]
async fn test_rendition_cache_hit_same_spec() {
	// Arrange
	let mut manager = MediaManager::new();
	let media = manager
		.upload("test.jpg".to_string(), vec![0u8; 100])
		.await
		.unwrap();
	let spec = RenditionSpec {
		width: Some(400),
		height: Some(300),
		mode: CropMode::Fill,
		format: None,
		quality: Some(85),
	};

	// Act - Request same spec twice
	let rendition1 = manager.get_rendition(media.id, spec.clone()).await.unwrap();
	let rendition2 = manager.get_rendition(media.id, spec).await.unwrap();

	// Assert - Cache hit: same ID returned
	assert_eq!(rendition1.id, rendition2.id);

	// Act - Request different spec
	let different_spec = RenditionSpec {
		width: Some(800),
		height: Some(600),
		mode: CropMode::Fill,
		format: None,
		quality: Some(85),
	};
	let rendition3 = manager
		.get_rendition(media.id, different_spec)
		.await
		.unwrap();

	// Assert - Cache miss: different ID returned
	assert_ne!(rendition1.id, rendition3.id);
}

// --- Decision Table ---

#[rstest]
#[case("test.jpg", "image/jpeg")]
#[case("test.jpeg", "image/jpeg")]
#[case("test.png", "image/png")]
#[case("test.gif", "image/gif")]
#[case("test.webp", "image/webp")]
#[case("test.svg", "image/svg+xml")]
#[case("test.pdf", "application/pdf")]
#[case("test.txt", "text/plain")]
#[case("test.html", "text/html")]
#[case("test.json", "application/json")]
#[case("test.xyz", "application/octet-stream")]
#[tokio::test]
async fn test_mime_type_complete_decision_table(#[case] filename: &str, #[case] expected: &str) {
	// Arrange
	let mut manager = MediaManager::new();

	// Act
	let media = manager
		.upload(filename.to_string(), vec![0u8; 10])
		.await
		.unwrap();

	// Assert
	assert_eq!(media.mime_type, expected);
}

#[rstest]
#[case(
	Some(100),
	Some(100),
	CropMode::Fit,
	Some(100),
	Some(100),
	CropMode::Fit,
	true
)]
#[case(
	Some(100),
	Some(100),
	CropMode::Fit,
	Some(200),
	Some(100),
	CropMode::Fit,
	false
)]
#[case(
	Some(100),
	Some(100),
	CropMode::Fit,
	Some(100),
	Some(200),
	CropMode::Fit,
	false
)]
#[case(
	Some(100),
	Some(100),
	CropMode::Fit,
	Some(100),
	Some(100),
	CropMode::Fill,
	false
)]
#[tokio::test]
async fn test_rendition_cache_hit_decision_table(
	#[case] w1: Option<u32>,
	#[case] h1: Option<u32>,
	#[case] m1: CropMode,
	#[case] w2: Option<u32>,
	#[case] h2: Option<u32>,
	#[case] m2: CropMode,
	#[case] expect_cache_hit: bool,
) {
	// Arrange
	let mut manager = MediaManager::new();
	let media = manager
		.upload("test.jpg".to_string(), vec![0u8; 100])
		.await
		.unwrap();
	let spec1 = RenditionSpec {
		width: w1,
		height: h1,
		mode: m1,
		format: None,
		quality: None,
	};
	let spec2 = RenditionSpec {
		width: w2,
		height: h2,
		mode: m2,
		format: None,
		quality: None,
	};

	// Act
	let rendition1 = manager.get_rendition(media.id, spec1).await.unwrap();
	let rendition2 = manager.get_rendition(media.id, spec2).await.unwrap();

	// Assert
	assert_eq!(rendition1.id == rendition2.id, expect_cache_hit);
}

#[rstest]
#[case("photo.jpg", true)]
#[case("image.png", true)]
#[case("readme.txt", false)]
#[case("document.pdf", false)]
#[case("data.xyz", false)]
#[tokio::test]
async fn test_upload_image_detection_decision_table(
	#[case] filename: &str,
	#[case] is_image: bool,
) {
	// Arrange
	let mut manager = MediaManager::new();

	// Act
	let media = manager
		.upload(filename.to_string(), vec![0u8; 50])
		.await
		.unwrap();

	// Assert
	if is_image {
		assert_eq!(media.width, Some(0));
		assert_eq!(media.height, Some(0));
	} else {
		assert_eq!(media.width, None);
		assert_eq!(media.height, None);
	}
}
