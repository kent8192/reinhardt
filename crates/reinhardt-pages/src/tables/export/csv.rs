//! CSV export implementation

use super::ExportError;
use std::io::Write;

/// Exports data to CSV format
///
/// The first row of `data` is treated as the header row.
/// Subsequent rows are data rows.
pub fn export_csv<W: Write>(writer: &mut W, data: &[Vec<String>]) -> Result<(), ExportError> {
	for row in data {
		for (j, field) in row.iter().enumerate() {
			if j > 0 {
				writer.write_all(b",")?;
			}
			// Escape fields containing commas, quotes, or newlines
			if field.contains(',') || field.contains('"') || field.contains('\n') {
				writer.write_all(b"\"")?;
				writer.write_all(field.replace('"', "\"\"").as_bytes())?;
				writer.write_all(b"\"")?;
			} else {
				writer.write_all(field.as_bytes())?;
			}
		}
		writer.write_all(b"\n")?;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	struct FailingWriter;

	impl Write for FailingWriter {
		fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
			Err(std::io::Error::new(
				std::io::ErrorKind::BrokenPipe,
				"writer failed",
			))
		}

		fn flush(&mut self) -> std::io::Result<()> {
			Ok(())
		}
	}

	#[rstest]
	fn csv_and_json_exports_escape_and_shape_rows_exactly() {
		// Arrange
		let data = vec![
			vec!["name".to_string(), "note".to_string()],
			vec![
				"Ada, Lovelace".to_string(),
				"She said \"hello\"".to_string(),
			],
			vec!["Grace".to_string(), "line one\nline two".to_string()],
		];
		let mut output = Vec::new();

		// Act
		export_csv(&mut output, &data).expect("CSV export should write to an in-memory buffer");
		let error =
			export_csv(&mut FailingWriter, &data).expect_err("failing writer should be reported");
		let flush_result = FailingWriter.flush();

		// Assert
		assert_eq!(
			output,
			b"name,note\n\"Ada, Lovelace\",\"She said \"\"hello\"\"\"\nGrace,\"line one\nline two\"\n"
		);
		assert_eq!(error.to_string(), "I/O error: writer failed");
		flush_result.expect("failing writer flush should remain a no-op");
	}
}
