//! Secure filename and logical-key construction.

use crate::settings::is_valid_named_storage_alias;
use crate::{FileStorageError, StorageError};
use chrono::{DateTime, Utc};
use unicode_normalization::UnicodeNormalization;

const COLLISION_SUFFIX_LENGTH: usize = 17;
const MAX_STORAGE_COMPONENT_BYTES: usize = 255;
const MAX_STORAGE_KEY_BYTES: usize = 1024;

/// Validate a storage alias using the registry's canonical grammar.
///
/// # Errors
///
/// Returns [`StorageError::ConfigError`] when the alias is invalid or when the
/// reserved `default` alias is not allowed at this call site.
pub fn validate_storage_alias(
	alias: &str,
	allow_default: bool,
) -> std::result::Result<(), StorageError> {
	if (allow_default && alias == "default") || is_valid_named_storage_alias(alias) {
		Ok(())
	} else {
		Err(StorageError::ConfigError(format!(
			"invalid storage alias `{alias}`"
		)))
	}
}

/// Validate a relative upload-directory template.
///
/// Only `%Y`, `%m`, `%d`, `%H`, `%M`, and `%S` UTC substitutions are accepted.
///
/// # Errors
///
/// Returns [`FileStorageError::InvalidUploadTemplate`] for unsafe structure or
/// unsupported substitutions.
pub fn validate_upload_template(template: &str) -> std::result::Result<(), FileStorageError> {
	if template.is_empty() {
		return Err(invalid_template("template is empty"));
	}
	if template.starts_with('/') {
		return Err(invalid_template("rooted templates are not allowed"));
	}
	if template.contains('\\') {
		return Err(invalid_template("backslashes are not allowed"));
	}
	if has_drive_prefix(template) {
		return Err(invalid_template("drive prefixes are not allowed"));
	}
	if template.contains('\0') {
		return Err(invalid_template("NUL is not allowed"));
	}
	if template.chars().any(char::is_control) {
		return Err(invalid_template("control characters are not allowed"));
	}

	for component in template.split('/') {
		if component.is_empty() {
			return Err(invalid_template("empty components are not allowed"));
		}
		if component == "." {
			return Err(invalid_template("dot components are not allowed"));
		}
		if component == ".." {
			return Err(invalid_template("parent components are not allowed"));
		}
		validate_template_tokens(component)?;
		let structural = substitute_template_tokens(component, "2000")?;
		validate_windows_forbidden_characters(&structural).map_err(invalid_template)?;
		validate_component_structure(&structural).map_err(invalid_template)?;
	}

	Ok(())
}

/// Normalize a single client-supplied filename without accepting path syntax.
///
/// The output is NFC-normalized, preserves Unicode letters and numbers plus
/// `_`, `-`, and `.`, and replaces all other non-structural characters with
/// `_`.
///
/// # Errors
///
/// Returns [`FileStorageError::UnsafeFilename`] when the input is empty or has
/// unsafe path, control-character, trailing-character, or device-name syntax.
pub fn normalize_client_filename(filename: &str) -> std::result::Result<String, FileStorageError> {
	validate_raw_filename(filename)?;

	let normalized: String = filename
		.nfc()
		.map(|character| {
			if character.is_alphanumeric() || matches!(character, '_' | '-' | '.') {
				character
			} else {
				'_'
			}
		})
		.collect();
	validate_normalized_filename(&normalized)?;
	Ok(normalized)
}

/// Validate a portable `/`-separated relative logical storage key.
///
/// # Errors
///
/// Returns [`FileStorageError::UnsafeFilename`] when the key is empty, rooted,
/// contains host-specific separators, or has unsafe components.
pub fn validate_logical_key(path: &str) -> std::result::Result<(), FileStorageError> {
	if path.is_empty() {
		return Err(unsafe_filename("logical key is empty"));
	}
	if path.starts_with('/') || has_drive_prefix(path) {
		return Err(unsafe_filename("logical keys must be relative"));
	}
	if path.contains('\\') {
		return Err(unsafe_filename("logical keys must use `/` separators"));
	}
	if path.contains('\0') {
		return Err(unsafe_filename("NUL is not allowed"));
	}
	if path.chars().any(char::is_control) {
		return Err(unsafe_filename("control characters are not allowed"));
	}

	for component in path.split('/') {
		if component.is_empty() {
			return Err(unsafe_filename("logical key components must be non-empty"));
		}
		if component == "." {
			return Err(unsafe_filename("dot components are not allowed"));
		}
		if component == ".." {
			return Err(unsafe_filename("parent components are not allowed"));
		}
		validate_windows_forbidden_characters(component).map_err(unsafe_filename)?;
		validate_component_structure(component).map_err(unsafe_filename)?;
	}

	Ok(())
}

pub(crate) fn expand_upload_template(
	template: &str,
	now: DateTime<Utc>,
) -> std::result::Result<String, FileStorageError> {
	validate_upload_template(template)?;
	let mut expanded = String::with_capacity(template.len());
	let mut characters = template.chars();
	while let Some(character) = characters.next() {
		if character != '%' {
			expanded.push(character);
			continue;
		}

		let token = characters
			.next()
			.ok_or_else(|| invalid_template("incomplete UTC token"))?;
		let value = match token {
			'Y' => now.format("%Y").to_string(),
			'm' => now.format("%m").to_string(),
			'd' => now.format("%d").to_string(),
			'H' => now.format("%H").to_string(),
			'M' => now.format("%M").to_string(),
			'S' => now.format("%S").to_string(),
			_ => {
				return Err(invalid_template(format!(
					"unsupported UTC token `%{token}`"
				)));
			}
		};
		expanded.push_str(&value);
	}
	validate_logical_key(&expanded)?;
	Ok(expanded)
}

pub(crate) fn prepare_upload_key(
	directory: &str,
	filename: &str,
	max_length: usize,
) -> std::result::Result<String, FileStorageError> {
	let (stem, extension) = split_extension(filename);
	let fixed_length = directory.chars().count()
		+ usize::from(!directory.is_empty())
		+ extension.chars().count()
		+ COLLISION_SUFFIX_LENGTH;
	let available_stem = max_length
		.checked_sub(fixed_length)
		.filter(|available| *available > 0)
		.ok_or(FileStorageError::PathTooLong { max_length })?;
	let available_component_bytes = extension
		.len()
		.checked_add(COLLISION_SUFFIX_LENGTH)
		.and_then(|fixed| MAX_STORAGE_COMPONENT_BYTES.checked_sub(fixed))
		.ok_or(FileStorageError::PathTooLong { max_length })?;
	let available_key_bytes = directory
		.len()
		.checked_add(usize::from(!directory.is_empty()))
		.and_then(|fixed| fixed.checked_add(extension.len()))
		.and_then(|fixed| fixed.checked_add(COLLISION_SUFFIX_LENGTH))
		.and_then(|fixed| MAX_STORAGE_KEY_BYTES.checked_sub(fixed))
		.ok_or(FileStorageError::PathTooLong { max_length })?;
	let available_stem_bytes = available_component_bytes.min(available_key_bytes);
	let mut used_stem_bytes = 0;
	let shortened_stem: String = stem
		.chars()
		.take(available_stem)
		.take_while(|character| {
			let character_bytes = character.len_utf8();
			if used_stem_bytes + character_bytes > available_stem_bytes {
				return false;
			}
			used_stem_bytes += character_bytes;
			true
		})
		.collect();
	if shortened_stem.is_empty() {
		return Err(FileStorageError::PathTooLong { max_length });
	}
	let key = if directory.is_empty() {
		format!("{shortened_stem}{extension}")
	} else {
		format!("{directory}/{shortened_stem}{extension}")
	};
	validate_logical_key(&key)?;
	if key.len() > MAX_STORAGE_KEY_BYTES
		|| key
			.split('/')
			.any(|component| component.len() > MAX_STORAGE_COMPONENT_BYTES)
	{
		return Err(FileStorageError::PathTooLong { max_length });
	}
	Ok(key)
}

pub(crate) fn collision_candidate(original: &str, random: [u8; 10]) -> String {
	let (directory, filename) = original.rsplit_once('/').unwrap_or(("", original));
	let (stem, extension) = split_extension(filename);
	let suffix = encode_base32(random);
	if directory.is_empty() {
		format!("{stem}_{suffix}{extension}")
	} else {
		format!("{directory}/{stem}_{suffix}{extension}")
	}
}

fn validate_raw_filename(filename: &str) -> std::result::Result<(), FileStorageError> {
	if filename.is_empty() {
		return Err(unsafe_filename("filename is empty"));
	}
	if filename.contains('\0') {
		return Err(unsafe_filename("NUL is not allowed"));
	}
	if filename.contains(['/', '\\']) {
		return Err(unsafe_filename("path separators are not allowed"));
	}
	if has_drive_prefix(filename) {
		return Err(unsafe_filename("drive prefixes are not allowed"));
	}
	if filename.chars().any(char::is_control) {
		return Err(unsafe_filename("control characters are not allowed"));
	}
	validate_component_structure(filename).map_err(unsafe_filename)
}

fn validate_normalized_filename(filename: &str) -> std::result::Result<(), FileStorageError> {
	if filename.is_empty() {
		return Err(unsafe_filename("normalized filename is empty"));
	}
	validate_component_structure(filename).map_err(unsafe_filename)
}

fn validate_component_structure(component: &str) -> std::result::Result<(), &'static str> {
	if matches!(component, "." | "..") {
		return Err("dot components are not allowed");
	}
	if component.ends_with(['.', ' ']) {
		return Err("trailing dots or spaces are not allowed");
	}
	if is_windows_device_basename(component) {
		return Err("reserved Windows device basename");
	}
	Ok(())
}

fn validate_windows_forbidden_characters(component: &str) -> std::result::Result<(), &'static str> {
	if component.contains(['<', '>', ':', '"', '|', '?', '*']) {
		Err("Windows-forbidden characters are not allowed")
	} else {
		Ok(())
	}
}

fn validate_template_tokens(component: &str) -> std::result::Result<(), FileStorageError> {
	let mut characters = component.chars();
	while let Some(character) = characters.next() {
		if character != '%' {
			continue;
		}
		let token = characters
			.next()
			.ok_or_else(|| invalid_template("incomplete UTC token"))?;
		if !matches!(token, 'Y' | 'm' | 'd' | 'H' | 'M' | 'S') {
			return Err(invalid_template(format!(
				"unsupported UTC token `%{token}`"
			)));
		}
	}
	Ok(())
}

fn substitute_template_tokens(
	component: &str,
	replacement: &str,
) -> std::result::Result<String, FileStorageError> {
	let mut substituted = String::with_capacity(component.len());
	let mut characters = component.chars();
	while let Some(character) = characters.next() {
		if character == '%' {
			characters
				.next()
				.ok_or_else(|| invalid_template("incomplete UTC token"))?;
			substituted.push_str(replacement);
		} else {
			substituted.push(character);
		}
	}
	Ok(substituted)
}

fn has_drive_prefix(value: &str) -> bool {
	let bytes = value.as_bytes();
	bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn is_windows_device_basename(filename: &str) -> bool {
	let basename = filename.split('.').next().unwrap_or(filename);
	let uppercase = basename.to_ascii_uppercase();
	matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL")
		|| uppercase
			.strip_prefix("COM")
			.or_else(|| uppercase.strip_prefix("LPT"))
			.is_some_and(|number| {
				matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
			})
}

fn split_extension(filename: &str) -> (&str, &str) {
	match filename.rfind('.') {
		Some(index) if index > 0 => filename.split_at(index),
		_ => (filename, ""),
	}
}

fn encode_base32(bytes: [u8; 10]) -> String {
	const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
	let mut encoded = String::with_capacity(16);
	let mut buffer = 0_u32;
	let mut bits = 0_u8;
	for byte in bytes {
		buffer = (buffer << 8) | u32::from(byte);
		bits += 8;
		while bits >= 5 {
			bits -= 5;
			let index = ((buffer >> bits) & 0x1f) as usize;
			encoded.push(char::from(ALPHABET[index]));
		}
	}
	encoded
}

fn unsafe_filename(reason: impl Into<String>) -> FileStorageError {
	FileStorageError::UnsafeFilename(reason.into())
}

fn invalid_template(reason: impl Into<String>) -> FileStorageError {
	FileStorageError::InvalidUploadTemplate(reason.into())
}

#[cfg(test)]
mod tests {
	use super::{collision_candidate, expand_upload_template, prepare_upload_key};
	use chrono::{TimeZone, Utc};
	use rstest::rstest;

	#[rstest]
	fn expands_every_supported_token_from_one_utc_timestamp() {
		let now = Utc.with_ymd_and_hms(2026, 8, 8, 12, 34, 56).unwrap();

		assert_eq!(
			expand_upload_template("uploads/%Y/%m/%d/%H/%M/%S", now).unwrap(),
			"uploads/2026/08/08/12/34/56"
		);
	}

	#[rstest]
	fn shortening_counts_unicode_scalars_and_preserves_suffix_allowance() {
		assert_eq!(
			prepare_upload_key("画像", "猫猫猫猫猫.PNG", 26).unwrap(),
			"画像/猫猫.PNG"
		);
	}

	#[rstest]
	fn shortening_respects_storage_byte_limits() {
		let key = prepare_upload_key("uploads", &"猫".repeat(255), 255).unwrap();
		assert!(key.len() <= 255);
		assert!(key.split('/').all(|component| component.len() <= 255));

		let collision = collision_candidate(&key, [0; 10]);
		assert!(collision.len() <= 1024);
		assert!(collision.split('/').all(|component| component.len() <= 255));
	}

	#[rstest]
	#[case([0; 10], "avatars/photo_aaaaaaaaaaaaaaaa.PNG")]
	#[case([0xff; 10], "avatars/photo_7777777777777777.PNG")]
	#[case([0, 1, 2, 3, 4, 5, 6, 7, 8, 9], "avatars/photo_aaaqeayeaudaocaj.PNG")]
	fn collision_suffix_encodes_exactly_eighty_bits(
		#[case] random: [u8; 10],
		#[case] expected: &str,
	) {
		assert_eq!(collision_candidate("avatars/photo.PNG", random), expected);
	}
}
