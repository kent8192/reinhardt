//! JSON export implementation

use super::ExportError;
use std::io::Write;

/// Exports data to JSON format
///
/// The first row of `data` is treated as the header row (field names).
/// Subsequent rows are data rows. Each row is represented as a JSON object
/// with header values as keys.
pub fn export_json<W: Write>(writer: &mut W, data: &[Vec<String>]) -> Result<(), ExportError> {
	if data.is_empty() {
		writer.write_all(b"[]")?;
		return Ok(());
	}

	let headers = &data[0];
	let rows: Vec<serde_json::Map<String, serde_json::Value>> = data[1..]
		.iter()
		.map(|row| {
			headers
				.iter()
				.zip(row.iter())
				.map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
				.collect()
		})
		.collect();

	let json = serde_json::to_string_pretty(&rows)
		.map_err(|e| ExportError::Serialization(e.to_string()))?;
	writer.write_all(json.as_bytes())?;
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
			vec!["name".to_string(), "role".to_string(), "unused".to_string()],
			vec!["Ada".to_string(), "maintainer".to_string()],
			vec!["Grace".to_string()],
		];
		let mut output = Vec::new();
		let mut empty_output = Vec::new();

		// Act
		export_json(&mut output, &data).expect("JSON export should write to an in-memory buffer");
		export_json(&mut empty_output, &[]).expect("empty JSON export should succeed");
		let error =
			export_json(&mut FailingWriter, &data).expect_err("failing writer should be reported");
		let flush_result = FailingWriter.flush();

		// Assert
		assert_eq!(
			output,
			br#"[
  {
    "name": "Ada",
    "role": "maintainer"
  },
  {
    "name": "Grace"
  }
]"#,
		);
		assert_eq!(empty_output, b"[]");
		assert_eq!(error.to_string(), "I/O error: writer failed");
		flush_result.expect("failing writer flush should remain a no-op");
	}
}
