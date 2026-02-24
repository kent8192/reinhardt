//! Resource limits for admin panel operations
//!
//! This module defines configurable limits to prevent DoS attacks
//! through resource exhaustion (memory, CPU, database).

/// Maximum number of records that can be exported in a single request
///
/// This prevents memory exhaustion when exporting large tables.
/// Default: 10,000 records
pub const MAX_EXPORT_RECORDS: u64 = 10_000;

/// Maximum import file size in bytes
///
/// This prevents memory exhaustion from large file uploads.
/// Default: 10 MB
pub const MAX_IMPORT_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Maximum number of records that can be imported in a single request
///
/// This prevents database overload from bulk inserts.
/// Default: 1,000 records
pub const MAX_IMPORT_RECORDS: usize = 1_000;

/// Maximum page size for list views
///
/// This prevents memory exhaustion from large page requests.
/// Default: 500 records per page
pub const MAX_PAGE_SIZE: u64 = 500;

/// Default page size when not specified
pub const DEFAULT_PAGE_SIZE: u64 = 25;

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn export_records_limit_is_within_reasonable_bounds() {
		// Arrange
		let min = 1_u64;
		let max = 100_000_u64;

		// Act & Assert
		assert!(MAX_EXPORT_RECORDS >= min);
		assert!(MAX_EXPORT_RECORDS <= max);
	}

	#[rstest]
	fn import_file_size_limit_is_within_reasonable_bounds() {
		// Arrange
		let min = 1_usize;
		let max = 100 * 1024 * 1024_usize; // 100 MB

		// Act & Assert
		assert!(MAX_IMPORT_FILE_SIZE >= min);
		assert!(MAX_IMPORT_FILE_SIZE <= max);
	}

	#[rstest]
	fn import_records_limit_is_within_reasonable_bounds() {
		// Arrange
		let min = 1_usize;
		let max = 10_000_usize;

		// Act & Assert
		assert!(MAX_IMPORT_RECORDS >= min);
		assert!(MAX_IMPORT_RECORDS <= max);
	}

	#[rstest]
	fn page_size_limit_is_within_reasonable_bounds() {
		// Arrange
		let min = 1_u64;
		let max = 1_000_u64;

		// Act & Assert
		assert!(MAX_PAGE_SIZE >= min);
		assert!(MAX_PAGE_SIZE <= max);
	}

	#[rstest]
	fn default_page_size_does_not_exceed_max() {
		// Act & Assert
		assert!(DEFAULT_PAGE_SIZE > 0);
		assert!(DEFAULT_PAGE_SIZE <= MAX_PAGE_SIZE);
	}

	#[rstest]
	fn export_limit_is_expected_value() {
		// Assert
		assert_eq!(MAX_EXPORT_RECORDS, 10_000);
	}

	#[rstest]
	fn import_file_size_limit_is_10mb() {
		// Assert
		assert_eq!(MAX_IMPORT_FILE_SIZE, 10 * 1024 * 1024);
	}

	#[rstest]
	fn import_records_limit_is_expected_value() {
		// Assert
		assert_eq!(MAX_IMPORT_RECORDS, 1_000);
	}

	#[rstest]
	fn page_size_limit_is_expected_value() {
		// Assert
		assert_eq!(MAX_PAGE_SIZE, 500);
	}

	#[rstest]
	fn default_page_size_is_expected_value() {
		// Assert
		assert_eq!(DEFAULT_PAGE_SIZE, 25);
	}
}
