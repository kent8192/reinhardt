//! Session compression support
//!
//! This module provides compression support for session data to reduce storage size
//! and network bandwidth. Multiple compression algorithms are available:
//!
//! - **Zstd** (feature: `compression-zstd`): Recommended - balanced speed and compression ratio
//! - **Gzip** (feature: `compression-gzip`): Wide compatibility
//! - **Brotli** (feature: `compression-brotli`): Maximum compression ratio
//!
//! ## Compression Strategy
//!
//! The `CompressedSessionBackend` wrapper automatically compresses session data
//! when it exceeds a configurable threshold (default: 512 bytes). Smaller payloads
//! are stored uncompressed to avoid overhead.
//!
//! ## Example
//!
//! ```rust,no_run,ignore
//! use reinhardt_auth::sessions::compression::{CompressedSessionBackend, ZstdCompressor};
//! use reinhardt_auth::sessions::backends::InMemorySessionBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = InMemorySessionBackend::new();
//! let compressor = ZstdCompressor::new();
//! let compressed_backend = CompressedSessionBackend::new(backend, compressor);
//!
//! // Session data is automatically compressed when it exceeds the threshold
//! # Ok(())
//! # }
//! ```

use crate::sessions::backends::{SessionBackend, SessionError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Submodules
#[cfg(feature = "compression-zstd")]
mod zstd;
#[cfg(feature = "compression-zstd")]
pub use self::zstd::ZstdCompressor;

#[cfg(feature = "compression-gzip")]
mod gzip;
#[cfg(feature = "compression-gzip")]
pub use self::gzip::GzipCompressor;

#[cfg(feature = "compression-brotli")]
mod brotli;
#[cfg(feature = "compression-brotli")]
pub use self::brotli::BrotliCompressor;

/// Compression errors
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum CompressionError {
	/// Compression failed
	#[error("Compression failed: {0}")]
	CompressionFailed(String),

	/// Decompression failed
	#[error("Decompression failed: {0}")]
	DecompressionFailed(String),
}

/// Compressor trait for different compression algorithms
///
/// # Example
///
/// ```rust,no_run,ignore
/// use reinhardt_auth::sessions::compression::{Compressor, ZstdCompressor};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let compressor = ZstdCompressor::new();
///
/// let data = b"Hello, World!";
/// let compressed = compressor.compress(data)?;
/// let decompressed = compressor.decompress(&compressed)?;
///
/// assert_eq!(data, decompressed.as_slice());
/// # Ok(())
/// # }
/// ```
pub trait Compressor: Send + Sync + Clone {
	/// Compress data
	fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

	/// Decompress data
	fn decompress(&self, compressed: &[u8]) -> Result<Vec<u8>, CompressionError>;

	/// Get compressor name
	fn name(&self) -> &'static str;
}

/// Compressed data envelope
///
/// This structure wraps the actual payload and includes metadata about
/// whether the data is compressed.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct CompressedData {
	/// The actual payload (compressed or uncompressed)
	payload: Vec<u8>,
	/// Whether the payload is compressed
	is_compressed: bool,
}

/// Compressed session backend wrapper
///
/// This wrapper automatically compresses session data when it exceeds
/// a configurable threshold. Smaller payloads are stored uncompressed
/// to avoid overhead.
///
/// # Example
///
/// ```rust,no_run,ignore
/// use reinhardt_auth::sessions::compression::{CompressedSessionBackend, ZstdCompressor};
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = InMemorySessionBackend::new();
/// let compressor = ZstdCompressor::new();
///
/// // Default threshold: 512 bytes
/// let compressed_backend = CompressedSessionBackend::new(backend, compressor);
///
/// // Custom threshold: 1024 bytes
/// let custom_backend = CompressedSessionBackend::with_threshold(
///     InMemorySessionBackend::new(),
///     ZstdCompressor::new(),
///     1024,
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CompressedSessionBackend<B, C> {
	backend: B,
	compressor: C,
	threshold_bytes: usize,
}

impl<B, C> CompressedSessionBackend<B, C>
where
	B: SessionBackend,
	C: Compressor,
{
	/// Create a new compressed session backend with default threshold (512 bytes)
	///
	/// # Example
	///
	/// ```rust,no_run,ignore
	/// use reinhardt_auth::sessions::compression::{CompressedSessionBackend, ZstdCompressor};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let compressor = ZstdCompressor::new();
	/// let compressed_backend = CompressedSessionBackend::new(backend, compressor);
	/// ```
	pub fn new(backend: B, compressor: C) -> Self {
		Self {
			backend,
			compressor,
			threshold_bytes: 512, // Default: 512 bytes
		}
	}

	/// Create a new compressed session backend with custom threshold
	///
	/// # Example
	///
	/// ```rust,no_run,ignore
	/// use reinhardt_auth::sessions::compression::{CompressedSessionBackend, ZstdCompressor};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let compressor = ZstdCompressor::new();
	/// let compressed_backend = CompressedSessionBackend::with_threshold(
	///     backend,
	///     compressor,
	///     1024, // 1KB threshold
	/// );
	/// ```
	pub fn with_threshold(backend: B, compressor: C, threshold_bytes: usize) -> Self {
		Self {
			backend,
			compressor,
			threshold_bytes,
		}
	}

	/// Get the compression threshold in bytes
	pub fn threshold(&self) -> usize {
		self.threshold_bytes
	}
}

#[async_trait]
impl<B, C> SessionBackend for CompressedSessionBackend<B, C>
where
	B: SessionBackend,
	C: Compressor,
{
	async fn load<T>(&self, session_key: &str) -> Result<Option<T>, SessionError>
	where
		T: for<'de> Deserialize<'de> + Send,
	{
		// Load compressed data envelope
		let envelope: Option<CompressedData> = self.backend.load(session_key).await?;

		match envelope {
			Some(envelope) => {
				let payload = if envelope.is_compressed {
					// Decompress the payload
					self.compressor
						.decompress(&envelope.payload)
						.map_err(|e| SessionError::SerializationError(e.to_string()))?
				} else {
					envelope.payload
				};

				// Deserialize the payload
				let data: T = serde_json::from_slice(&payload)
					.map_err(|e| SessionError::SerializationError(e.to_string()))?;

				Ok(Some(data))
			}
			None => Ok(None),
		}
	}

	async fn save<T>(
		&self,
		session_key: &str,
		data: &T,
		ttl: Option<u64>,
	) -> Result<(), SessionError>
	where
		T: Serialize + Send + Sync,
	{
		// Serialize the data
		let serialized = serde_json::to_vec(data)
			.map_err(|e| SessionError::SerializationError(e.to_string()))?;

		// Compress if above threshold
		let (payload, is_compressed) = if serialized.len() > self.threshold_bytes {
			let compressed = self
				.compressor
				.compress(&serialized)
				.map_err(|e| SessionError::SerializationError(e.to_string()))?;
			(compressed, true)
		} else {
			(serialized, false)
		};

		// Create envelope
		let envelope = CompressedData {
			payload,
			is_compressed,
		};

		// Save envelope
		self.backend.save(session_key, &envelope, ttl).await
	}

	async fn delete(&self, session_key: &str) -> Result<(), SessionError> {
		self.backend.delete(session_key).await
	}

	async fn exists(&self, session_key: &str) -> Result<bool, SessionError> {
		self.backend.exists(session_key).await
	}
}

#[cfg(test)]
mod tests {
	#[cfg(feature = "compression-zstd")]
	#[tokio::test]
	async fn test_compressed_backend_above_threshold() {
		use super::{CompressedSessionBackend, ZstdCompressor};
		use crate::sessions::{InMemorySessionBackend, SessionBackend};

		let backend = InMemorySessionBackend::new();
		let compressor = ZstdCompressor::new();
		let compressed_backend = CompressedSessionBackend::with_threshold(backend, compressor, 10);

		// Large data (above threshold)
		let data = serde_json::json!({
			"key": "value_with_many_characters_to_exceed_threshold",
		});

		compressed_backend
			.save("test_key", &data, None)
			.await
			.unwrap();

		let loaded: Option<serde_json::Value> = compressed_backend.load("test_key").await.unwrap();
		assert_eq!(loaded.unwrap(), data);
	}

	#[cfg(feature = "compression-zstd")]
	#[tokio::test]
	async fn test_compressed_backend_below_threshold() {
		use super::{CompressedSessionBackend, ZstdCompressor};
		use crate::sessions::{InMemorySessionBackend, SessionBackend};

		let backend = InMemorySessionBackend::new();
		let compressor = ZstdCompressor::new();
		let compressed_backend =
			CompressedSessionBackend::with_threshold(backend, compressor, 1000);

		// Small data (below threshold)
		let data = serde_json::json!({"key": "value"});

		compressed_backend
			.save("test_key", &data, None)
			.await
			.unwrap();

		let loaded: Option<serde_json::Value> = compressed_backend.load("test_key").await.unwrap();
		assert_eq!(loaded.unwrap(), data);
	}

	#[cfg(feature = "compression-zstd")]
	#[tokio::test]
	async fn test_compressed_backend_delete() {
		use super::{CompressedSessionBackend, ZstdCompressor};
		use crate::sessions::{InMemorySessionBackend, SessionBackend};

		let backend = InMemorySessionBackend::new();
		let compressor = ZstdCompressor::new();
		let compressed_backend = CompressedSessionBackend::new(backend, compressor);

		let data = serde_json::json!({"key": "value"});

		compressed_backend
			.save("test_key", &data, None)
			.await
			.unwrap();

		assert!(compressed_backend.exists("test_key").await.unwrap());

		compressed_backend.delete("test_key").await.unwrap();

		assert!(!compressed_backend.exists("test_key").await.unwrap());
	}
}
