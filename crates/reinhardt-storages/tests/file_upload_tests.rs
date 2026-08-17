//! Tests for secure file upload naming.

use reinhardt_core::parsers::UploadedFile;
use reinhardt_storages::{
	FileStorageError, normalize_client_filename, validate_logical_key, validate_storage_alias,
	validate_upload_template,
};
use rstest::rstest;

fn accepts_canonical_upload_type(_upload: Option<UploadedFile>) {}

#[rstest]
fn uploaded_file_is_available_from_the_canonical_parser_namespace() {
	accepts_canonical_upload_type(None);
}

#[rstest]
#[case("", "unsafe upload filename: filename is empty")]
#[case(
	"folder/file.txt",
	"unsafe upload filename: path separators are not allowed"
)]
#[case(
	"folder\\file.txt",
	"unsafe upload filename: path separators are not allowed"
)]
#[case(
	"/absolute.txt",
	"unsafe upload filename: path separators are not allowed"
)]
#[case(
	"C:drive.txt",
	"unsafe upload filename: drive prefixes are not allowed"
)]
#[case(
	"\\\\server\\share.txt",
	"unsafe upload filename: path separators are not allowed"
)]
#[case("nul\0byte.txt", "unsafe upload filename: NUL is not allowed")]
#[case(
	"line\nfeed.txt",
	"unsafe upload filename: control characters are not allowed"
)]
#[case(".", "unsafe upload filename: dot components are not allowed")]
#[case("..", "unsafe upload filename: dot components are not allowed")]
#[case(
	"trailing.",
	"unsafe upload filename: trailing dots or spaces are not allowed"
)]
#[case(
	"trailing ",
	"unsafe upload filename: trailing dots or spaces are not allowed"
)]
fn unsafe_client_filenames_are_rejected(#[case] input: &str, #[case] expected: &str) {
	assert_eq!(
		normalize_client_filename(input).unwrap_err().to_string(),
		expected
	);
}

#[rstest]
#[case("CON")]
#[case("con.txt")]
#[case("PrN.PDF")]
#[case("aux")]
#[case("NUL.json")]
#[case("com1")]
#[case("COM9.txt")]
#[case("lpt1")]
#[case("LPT9.tar.gz")]
fn windows_device_basenames_are_rejected(#[case] input: &str) {
	assert_eq!(
		normalize_client_filename(input).unwrap_err().to_string(),
		"unsafe upload filename: reserved Windows device basename"
	);
}

#[rstest]
#[case("Cafe\u{301}.TXT", "Café.TXT")]
#[case("東京_①-画像.PNG", "東京_①-画像.PNG")]
#[case("hello world+猫?.JpG", "hello_world_猫_.JpG")]
fn client_filenames_are_normalized_without_losing_safe_unicode_or_extension_case(
	#[case] input: &str,
	#[case] expected: &str,
) {
	assert_eq!(normalize_client_filename(input).unwrap(), expected);
}

#[rstest]
#[case("default", true, true)]
#[case("default", false, false)]
#[case("private_uploads", false, true)]
#[case("private-2", false, true)]
#[case("Private", false, false)]
#[case("private.files", false, false)]
#[case("-private", false, false)]
#[case("", false, false)]
fn storage_alias_validation_matches_registry_grammar(
	#[case] alias: &str,
	#[case] allow_default: bool,
	#[case] expected: bool,
) {
	assert_eq!(
		validate_storage_alias(alias, allow_default).is_ok(),
		expected
	);
}

#[rstest]
#[case("avatars/%Y/%m/%d/%H/%M/%S")]
#[case("画像/%Y-%m-%d")]
fn upload_templates_accept_every_supported_utc_token(#[case] template: &str) {
	assert_eq!(
		validate_upload_template(template).map_err(|error| error.to_string()),
		Ok(())
	);
}

#[rstest]
#[case("", "invalid upload template: template is empty")]
#[case("avatars/%Q", "invalid upload template: unsupported UTC token `%Q`")]
#[case("avatars/%", "invalid upload template: incomplete UTC token")]
#[case(
	"/avatars",
	"invalid upload template: rooted templates are not allowed"
)]
#[case("C:avatars", "invalid upload template: drive prefixes are not allowed")]
#[case(
	"\\\\server\\avatars",
	"invalid upload template: backslashes are not allowed"
)]
#[case(
	"avatars//daily",
	"invalid upload template: empty components are not allowed"
)]
#[case(
	"avatars/",
	"invalid upload template: empty components are not allowed"
)]
#[case(
	"avatars/../daily",
	"invalid upload template: parent components are not allowed"
)]
#[case(
	"avatars/./daily",
	"invalid upload template: dot components are not allowed"
)]
#[case(
	"avatars:daily",
	"invalid upload template: Windows-forbidden characters are not allowed"
)]
fn unsafe_upload_templates_are_rejected(#[case] template: &str, #[case] expected: &str) {
	assert_eq!(
		validate_upload_template(template).unwrap_err().to_string(),
		expected
	);
}

#[rstest]
#[case("avatars/2026/08/file.txt")]
#[case("画像/猫.PNG")]
fn logical_keys_use_portable_forward_slashes(#[case] path: &str) {
	assert_eq!(
		validate_logical_key(path).map_err(|error| error.to_string()),
		Ok(())
	);
}

#[rstest]
#[case("", "unsafe upload filename: logical key is empty")]
#[case("/rooted.txt", "unsafe upload filename: logical keys must be relative")]
#[case(
	"folder/",
	"unsafe upload filename: logical key components must be non-empty"
)]
#[case(
	"folder//file.txt",
	"unsafe upload filename: logical key components must be non-empty"
)]
#[case(
	"folder\\file.txt",
	"unsafe upload filename: logical keys must use `/` separators"
)]
#[case(
	"folder/../file.txt",
	"unsafe upload filename: parent components are not allowed"
)]
#[case(
	"folder/./file.txt",
	"unsafe upload filename: dot components are not allowed"
)]
#[case(
	"folder/file?.txt",
	"unsafe upload filename: Windows-forbidden characters are not allowed"
)]
fn unsafe_logical_keys_are_rejected(#[case] path: &str, #[case] expected: &str) {
	assert_eq!(
		validate_logical_key(path).unwrap_err().to_string(),
		expected
	);
}

#[rstest]
fn upload_error_type_is_the_registry_error_type() {
	let error = FileStorageError::MissingFilename;
	assert_eq!(error.to_string(), "the upload does not include a filename");
}
