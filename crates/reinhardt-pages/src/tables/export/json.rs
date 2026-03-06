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
