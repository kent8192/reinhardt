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
