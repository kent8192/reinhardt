//! Cross-target file metadata captured from an event target.
//!
//! Core owns the canonical [`EventFile`] type. The public
//! `reinhardt_pages::event::EventFile` path remains a re-export of this type.

/// Owned metadata for a file selected by an event target.
///
/// This is a P2 API. Native targets expose metadata captured from a native
/// event target, while WASM targets expose the same metadata captured from a
/// browser `File`.
#[derive(Clone)]
pub struct EventFile {
	name: String,
	media_type: String,
	size: u64,
	last_modified: i64,
	#[cfg(wasm)]
	raw: web_sys::File,
}

impl EventFile {
	/// Returns the file name without a path.
	///
	/// This is a P2 API with the same metadata behavior on native and WASM
	/// targets.
	#[must_use]
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Returns the reported media type.
	///
	/// This is a P2 API with the same metadata behavior on native and WASM
	/// targets.
	#[must_use]
	pub fn media_type(&self) -> &str {
		&self.media_type
	}

	/// Returns the file size in bytes.
	///
	/// This is a P2 API with the same metadata behavior on native and WASM
	/// targets.
	#[must_use]
	pub const fn size(&self) -> u64 {
		self.size
	}

	/// Returns the last-modified timestamp in milliseconds since the Unix epoch.
	///
	/// This is a P2 API with the same metadata behavior on native and WASM
	/// targets.
	#[must_use]
	pub const fn last_modified(&self) -> i64 {
		self.last_modified
	}

	/// Returns the source browser file.
	///
	/// This is a P0 API: the browser `File` is only available on WASM targets.
	#[cfg(wasm)]
	#[must_use]
	pub const fn raw(&self) -> &web_sys::File {
		&self.raw
	}
}

#[cfg(native)]
impl From<&super::NativeEventFile> for EventFile {
	fn from(file: &super::NativeEventFile) -> Self {
		Self {
			name: file.name.clone(),
			media_type: file.media_type.clone(),
			size: file.size,
			last_modified: file.last_modified,
		}
	}
}

#[cfg(wasm)]
impl From<web_sys::File> for EventFile {
	fn from(file: web_sys::File) -> Self {
		Self {
			name: file.name(),
			media_type: file.type_(),
			size: file.size() as u64,
			last_modified: file.last_modified() as i64,
			raw: file,
		}
	}
}

impl std::fmt::Debug for EventFile {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("EventFile")
			.field("name", &self.name)
			.field("media_type", &self.media_type)
			.field("size", &self.size)
			.field("last_modified", &self.last_modified)
			.finish()
	}
}

impl PartialEq for EventFile {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name
			&& self.media_type == other.media_type
			&& self.size == other.size
			&& self.last_modified == other.last_modified
	}
}

impl Eq for EventFile {}
