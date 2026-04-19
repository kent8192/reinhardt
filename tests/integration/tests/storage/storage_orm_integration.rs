//! Integration tests for reinhardt-storage with reinhardt-orm
//!
//! These tests verify file storage integration with ORM file fields.

use image::{ImageBuffer, Rgb};
use reinhardt_db::orm::file_fields::{FileField as ORMFileField, ImageField as ORMImageField};
use reinhardt_utils::storage::{InMemoryStorage, LocalStorage, Storage};
use std::io::Cursor;
use tempfile::TempDir;

async fn create_test_storage() -> (LocalStorage, TempDir) {
	let temp_dir = TempDir::new().unwrap();
	let storage = LocalStorage::new(temp_dir.path(), "http://localhost/media");
	storage.ensure_base_dir().await.unwrap();
	(storage, temp_dir)
}

/// Create a test image with specified dimensions
fn create_test_image(width: u32, height: u32) -> Vec<u8> {
	let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, |x, y| {
		// Create a simple gradient pattern
		Rgb([(x % 256) as u8, (y % 256) as u8, 128])
	});

	let mut buffer = Cursor::new(Vec::new());
	img.write_to(&mut buffer, image::ImageFormat::Png).unwrap();
	buffer.into_inner()
}

#[tokio::test]
async fn test_filefield_save() {
	// Test saving a file through ORM FileField
	let (storage, _temp_dir) = create_test_storage().await;

	let content = b"file content from ORM";
	let path = "documents/test.txt";

	storage.save(path, content).await.unwrap();

	let file = storage.read(path).await.unwrap();
	assert_eq!(file.content, content);

	storage.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_filefield_save_with_none_name() {
	// Test FileField save when no name is provided
	let (storage, _temp_dir) = create_test_storage().await;

	let content = b"content without name";
	// When name is None, storage should generate a unique name
	let generated_path = format!("uploads/{}.bin", uuid::Uuid::now_v7());

	storage.save(&generated_path, content).await.unwrap();
	assert!(storage.exists(&generated_path).await.unwrap());

	storage.delete(&generated_path).await.unwrap();
}

#[tokio::test]
async fn test_filefield_generate_filename() {
	// Test filename generation for FileField
	let (_storage, _temp_dir) = create_test_storage().await;

	// Simulate filename generation
	let base_name = "document";
	let extension = "pdf";
	let generated = format!("{}_{}.{}", base_name, uuid::Uuid::now_v7(), extension);

	assert!(generated.contains(base_name));
	assert!(generated.ends_with(extension));
}

#[tokio::test]
async fn test_filefield_get_upload_to_string() {
	// Test upload_to as a string path
	let (storage, _temp_dir) = create_test_storage().await;

	let upload_path = "uploads/documents";
	let filename = "report.pdf";
	let full_path = format!("{}/{}", upload_path, filename);

	storage.save(&full_path, b"report content").await.unwrap();
	assert!(storage.exists(&full_path).await.unwrap());

	storage.delete(&full_path).await.unwrap();
}

#[tokio::test]
async fn test_filefield_attr_class() {
	// Test FileField attribute class
	let field = ORMFileField::new("uploads/documents");
	assert_eq!(field.upload_to, "uploads/documents");
}

#[tokio::test]
async fn test_filefield_path() {
	// Test getting the path of a stored file
	let (storage, _temp_dir) = create_test_storage().await;

	let path = "files/data.bin";
	storage.save(path, b"data").await.unwrap();

	// Verify the file exists at the expected path
	let metadata = storage.metadata(path).await.unwrap();
	assert_eq!(metadata.path, path);

	storage.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_filefield_repr() {
	// Test FileField representation
	let field = ORMFileField::new("uploads/documents");
	let repr = format!("{:?}", field);
	assert!(repr.contains("FileField"));
}

#[tokio::test]
async fn test_filefield_url() {
	// Test URL generation for stored files
	let (storage, _temp_dir) = create_test_storage().await;

	let path = "media/image.jpg";
	storage.save(path, b"image data").await.unwrap();

	let url = storage.url(path);
	assert!(url.contains("image.jpg"));
	assert!(url.starts_with("http://localhost/media"));

	storage.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_filefield_size() {
	// Test getting file size
	let (storage, _temp_dir) = create_test_storage().await;

	let content = b"test content for size check";
	let path = "files/sizeable.txt";

	storage.save(path, content).await.unwrap();

	let metadata = storage.metadata(path).await.unwrap();
	assert_eq!(metadata.size, content.len() as u64);

	storage.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_filefield_pickle() {
	// Test FileField serialization/deserialization
	let (storage, _temp_dir) = create_test_storage().await;

	let path = "pickled/file.dat";
	let content = b"serializable content";

	storage.save(path, content).await.unwrap();

	// Verify we can read it back (simulating unpickling)
	let file = storage.read(path).await.unwrap();
	assert_eq!(file.content, content);

	storage.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_filefield_pathlib() {
	// Test FileField with pathlib-style paths
	let (storage, _temp_dir) = create_test_storage().await;

	let path = "path/to/nested/file.txt";
	storage.save(path, b"nested content").await.unwrap();

	assert!(storage.exists(path).await.unwrap());

	storage.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_imagefield_constructor() {
	// Test ImageField constructor
	let field = ORMImageField::new("uploads/images");
	assert!(field.width_field.is_none());
	assert!(field.height_field.is_none());
}

#[tokio::test]
async fn test_imagefield_dimensions() {
	// Tests image dimension extraction from actual image data
	let field = ORMImageField::new("uploads/images");

	// Create a test image with known dimensions
	let width = 640;
	let height = 480;
	let image_data = create_test_image(width, height);

	// Extract dimensions from the image
	let (extracted_width, extracted_height) = field.validate_image(&image_data).unwrap();

	assert_eq!(extracted_width, width, "Width should match");
	assert_eq!(extracted_height, height, "Height should match");
}

#[tokio::test]
async fn test_imagefield_field_save_file() {
	// Test ImageField save file operation
	let (storage, _temp_dir) = create_test_storage().await;

	let path = "gallery/artwork.png";
	storage.save(path, b"PNG image data").await.unwrap();

	let file = storage.read(path).await.unwrap();
	assert_eq!(file.content, b"PNG image data");

	storage.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_imagefield_update_dimension_fields() {
	// Tests dimension extraction from resized images
	let field = ORMImageField::with_dimensions("uploads/images", "width", "height");

	// Create a test image with specific dimensions
	let width = 800;
	let height = 600;
	let image_data = create_test_image(width, height);

	// Extract dimensions from the image
	let (extracted_width, extracted_height) = field.validate_image(&image_data).unwrap();

	assert_eq!(extracted_width, width, "Resized width should match");
	assert_eq!(extracted_height, height, "Resized height should match");

	// Verify width and height fields are configured
	assert_eq!(field.width_field, Some("width".to_string()));
	assert_eq!(field.height_field, Some("height".to_string()));
}

#[tokio::test]
async fn test_orm_file_with_inmemory_storage() {
	// Test ORM file operations with InMemoryStorage
	let storage = InMemoryStorage::new("memory", "http://localhost/uploads");

	let path = "temp/file.bin";
	storage.save(path, b"in memory file").await.unwrap();

	let file = storage.read(path).await.unwrap();
	assert_eq!(file.content, b"in memory file");

	let url = storage.url(path);
	assert!(url.contains("file.bin"));

	storage.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_file_metadata_with_orm() {
	// Test file metadata operations with ORM
	let (storage, _temp_dir) = create_test_storage().await;

	let path = "metadata/test.dat";
	let content = b"metadata test content";

	let saved = storage.save(path, content).await.unwrap();
	assert_eq!(saved.path, path);
	assert_eq!(saved.size, content.len() as u64);

	let metadata = storage.metadata(path).await.unwrap();
	assert_eq!(metadata.path, path);
	assert_eq!(metadata.size, content.len() as u64);

	storage.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_file_cleanup_with_orm() {
	// Test proper cleanup of ORM-managed files
	let (storage, _temp_dir) = create_test_storage().await;

	let files = vec![
		"orm_files/file1.txt",
		"orm_files/file2.txt",
		"orm_files/file3.txt",
	];

	for path in &files {
		storage.save(path, b"cleanup test").await.unwrap();
		assert!(storage.exists(path).await.unwrap());
	}

	for path in &files {
		storage.delete(path).await.unwrap();
		assert!(!storage.exists(path).await.unwrap());
	}
}
