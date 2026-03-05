//! Media file and image management
//!
//! Image uploads, renditions (resize/crop), metadata, and folder organization.
//! Inspired by Wagtail's image management system.

use crate::error::{CmsError, CmsResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Media file identifier
pub type MediaId = Uuid;

/// Represents an uploaded media file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
	/// Unique media identifier
	pub id: MediaId,

	/// Original filename
	pub filename: String,

	/// File size in bytes
	pub size: u64,

	/// MIME type
	pub mime_type: String,

	/// Storage path
	pub path: String,

	/// Upload timestamp
	pub uploaded_at: chrono::DateTime<chrono::Utc>,

	/// Image width in pixels (if image)
	pub width: Option<u32>,
	/// Image height in pixels (if image)
	pub height: Option<u32>,

	/// Folder ID (for organization)
	pub folder_id: Option<Uuid>,

	/// User-provided title
	pub title: Option<String>,

	/// Alt text for accessibility
	pub alt_text: Option<String>,
}

/// Rendition specification (resize/crop parameters)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenditionSpec {
	/// Target width
	pub width: Option<u32>,

	/// Target height
	pub height: Option<u32>,

	/// Crop mode (fill, fit, crop, etc.)
	pub mode: CropMode,

	/// Image format (JPEG, PNG, WebP)
	pub format: Option<ImageFormat>,

	/// Quality (1-100)
	pub quality: Option<u8>,
}

/// Crop mode for renditions
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CropMode {
	/// Scale to fit within dimensions
	Fit,

	/// Scale to fill dimensions, crop overflow
	Fill,

	/// Crop to exact dimensions
	Crop,

	/// Scale to width, ignore height
	Width,

	/// Scale to height, ignore width
	Height,
}

/// Image format
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ImageFormat {
	/// JPEG format
	Jpeg,

	/// PNG format
	Png,

	/// WebP format
	WebP,

	/// AVIF format
	Avif,
}

/// A generated image rendition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRendition {
	/// Rendition ID
	pub id: Uuid,

	/// Original media file ID
	pub media_id: MediaId,

	/// Rendition spec
	pub spec: RenditionSpec,

	/// Storage path of rendered image
	pub path: String,

	/// Rendition width
	pub width: u32,

	/// Rendition height
	pub height: u32,

	/// File size in bytes
	pub size: u64,

	/// Creation timestamp
	pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Media manager for file operations
pub struct MediaManager {
	/// Storage for uploaded media files
	media_storage: std::collections::HashMap<MediaId, MediaFile>,
	/// Storage for generated renditions
	rendition_storage: std::collections::HashMap<uuid::Uuid, ImageRendition>,
}

impl MediaManager {
	/// Create a new media manager
	pub fn new() -> Self {
		Self {
			media_storage: std::collections::HashMap::new(),
			rendition_storage: std::collections::HashMap::new(),
		}
	}

	/// Upload a new media file
	pub async fn upload(&mut self, filename: String, data: Vec<u8>) -> CmsResult<MediaFile> {
		use chrono::Utc;

		let id = MediaId::new_v4();
		let size = data.len() as u64;

		// Detect MIME type from filename extension
		let mime_type = Self::detect_mime_type(&filename);

		// Determine if the file is an image and extract dimensions
		let (width, height) = if mime_type.starts_with("image/") {
			// TODO: Extract actual image dimensions using image crate
			(Some(0), Some(0))
		} else {
			(None, None)
		};

		let media = MediaFile {
			id,
			filename,
			size,
			mime_type,
			path: format!("/media/{}", id),
			uploaded_at: Utc::now(),
			width,
			height,
			folder_id: None,
			title: None,
			alt_text: None,
		};

		self.media_storage.insert(id, media.clone());
		Ok(media)
	}

	/// Get a media file by ID
	pub async fn get(&self, id: MediaId) -> CmsResult<MediaFile> {
		self.media_storage
			.get(&id)
			.cloned()
			.ok_or_else(|| CmsError::MediaNotFound(id.to_string()))
	}

	/// Generate or retrieve a rendition
	pub async fn get_rendition(
		&mut self,
		media_id: MediaId,
		spec: RenditionSpec,
	) -> CmsResult<ImageRendition> {
		// Check if media file exists
		let media = self.get(media_id).await?;

		// Check if rendition already exists
		for rendition in self.rendition_storage.values() {
			if rendition.media_id == media_id
				&& rendition.spec.width == spec.width
				&& rendition.spec.height == spec.height
				&& std::mem::discriminant(&rendition.spec.mode)
					== std::mem::discriminant(&spec.mode)
			{
				return Ok(rendition.clone());
			}
		}

		// Generate new rendition
		// TODO: Implement actual image processing using image crate
		let id = uuid::Uuid::new_v4();
		let width = spec.width.unwrap_or(media.width.unwrap_or(0));
		let height = spec.height.unwrap_or(media.height.unwrap_or(0));

		let rendition = ImageRendition {
			id,
			media_id,
			spec: spec.clone(),
			path: format!("/media/renditions/{}", id),
			width,
			height,
			size: 0, // TODO: Calculate actual size after processing
			created_at: chrono::Utc::now(),
		};

		self.rendition_storage.insert(id, rendition.clone());

		Ok(rendition)
	}

	/// Delete a media file and its renditions
	pub async fn delete(&mut self, id: MediaId) -> CmsResult<()> {
		// Remove the media file
		self.media_storage
			.remove(&id)
			.ok_or_else(|| CmsError::MediaNotFound(id.to_string()))?;

		// Remove all associated renditions
		self.rendition_storage
			.retain(|_, rendition| rendition.media_id != id);

		Ok(())
	}

	/// Detect MIME type from filename extension
	fn detect_mime_type(filename: &str) -> String {
		let extension = filename.rsplit('.').next().unwrap_or("").to_lowercase();

		match extension.as_str() {
			"jpg" | "jpeg" => "image/jpeg",
			"png" => "image/png",
			"gif" => "image/gif",
			"webp" => "image/webp",
			"svg" => "image/svg+xml",
			"pdf" => "application/pdf",
			"txt" => "text/plain",
			"html" => "text/html",
			"json" => "application/json",
			_ => "application/octet-stream",
		}
		.to_string()
	}
}

impl Default for MediaManager {
	fn default() -> Self {
		Self::new()
	}
}
