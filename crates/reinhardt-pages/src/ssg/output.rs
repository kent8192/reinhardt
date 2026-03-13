//! SSG build output statistics.

use std::path::PathBuf;

/// Statistics and results from an SSG build.
#[derive(Debug, Clone)]
pub struct SsgOutput {
	/// Number of HTML files written.
	pub files_written: usize,
	/// Total bytes written across all files.
	pub total_bytes: u64,
	/// Paths of all generated files (relative to output directory).
	pub generated_files: Vec<PathBuf>,
	/// Whether a sitemap was generated.
	pub sitemap_generated: bool,
	/// The output directory used.
	pub output_dir: PathBuf,
}

impl SsgOutput {
	/// Creates a new empty output.
	pub fn new(output_dir: PathBuf) -> Self {
		Self {
			files_written: 0,
			total_bytes: 0,
			generated_files: Vec::new(),
			sitemap_generated: false,
			output_dir,
		}
	}

	/// Records a file that was generated.
	pub fn record_file(&mut self, relative_path: PathBuf, bytes: u64) {
		self.files_written += 1;
		self.total_bytes += bytes;
		self.generated_files.push(relative_path);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_ssg_output_new() {
		// Arrange
		let dir = PathBuf::from("/tmp/dist");

		// Act
		let output = SsgOutput::new(dir.clone());

		// Assert
		assert_eq!(output.files_written, 0);
		assert_eq!(output.total_bytes, 0);
		assert!(output.generated_files.is_empty());
		assert!(!output.sitemap_generated);
		assert_eq!(output.output_dir, dir);
	}

	#[rstest]
	fn test_ssg_output_record_file() {
		// Arrange
		let mut output = SsgOutput::new(PathBuf::from("/tmp/dist"));

		// Act
		output.record_file(PathBuf::from("index.html"), 1024);
		output.record_file(PathBuf::from("about/index.html"), 512);

		// Assert
		assert_eq!(output.files_written, 2);
		assert_eq!(output.total_bytes, 1536);
		assert_eq!(output.generated_files.len(), 2);
		assert_eq!(output.generated_files[0], PathBuf::from("index.html"));
		assert_eq!(output.generated_files[1], PathBuf::from("about/index.html"));
	}
}
