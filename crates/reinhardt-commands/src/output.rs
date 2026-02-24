//! Output wrapper for command output
//!
//! Provides a wrapper around `Write` implementations to ensure proper buffering
//! and flushing of command output.

use std::io::{BufWriter, Result, Write};

/// Wrapper around a `Write` implementation that provides buffering and ensures
/// proper flushing of output.
///
/// This is similar to Django's OutputWrapper, which ensures that output is properly
/// flushed even when exceptions occur.
///
/// # Examples
///
/// ```rust
/// use reinhardt_commands::OutputWrapper;
/// use std::io;
///
/// let stdout = io::stdout();
/// let mut output = OutputWrapper::new(stdout);
///
/// output.write("Hello, ").unwrap();
/// output.write("world!").unwrap();
/// output.writeln("").unwrap();
///
/// // Ensure output is flushed
/// output.flush().unwrap();
/// ```
pub struct OutputWrapper<W: Write> {
	writer: Option<BufWriter<W>>,
}

impl<W: Write> OutputWrapper<W> {
	/// Create a new OutputWrapper wrapping the given writer.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_commands::OutputWrapper;
	/// use std::io;
	///
	/// let stdout = io::stdout();
	/// let output = OutputWrapper::new(stdout);
	/// ```
	pub fn new(writer: W) -> Self {
		Self {
			writer: Some(BufWriter::new(writer)),
		}
	}

	/// Consume the OutputWrapper and return the inner writer.
	///
	/// This will flush the buffer before returning the writer.
	///
	/// # Errors
	///
	/// Returns an error if flushing fails.
	pub fn into_inner(mut self) -> Result<W> {
		self.writer
			.take()
			.expect("writer already consumed by into_inner()")
			.into_inner()
			.map_err(|e| e.into_error())
	}

	/// Write a string to the output.
	///
	/// # Errors
	///
	/// Returns an error if the write operation fails.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_commands::OutputWrapper;
	/// use std::io;
	///
	/// let stdout = io::stdout();
	/// let mut output = OutputWrapper::new(stdout);
	/// output.write("Hello").unwrap();
	/// ```
	pub fn write(&mut self, s: &str) -> Result<()> {
		self.writer
			.as_mut()
			.expect("writer already consumed by into_inner()")
			.write_all(s.as_bytes())
	}

	/// Write a string to the output followed by a newline.
	///
	/// # Errors
	///
	/// Returns an error if the write operation fails.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_commands::OutputWrapper;
	/// use std::io;
	///
	/// let stdout = io::stdout();
	/// let mut output = OutputWrapper::new(stdout);
	/// output.writeln("Hello, world!").unwrap();
	/// ```
	pub fn writeln(&mut self, s: &str) -> Result<()> {
		let writer = self
			.writer
			.as_mut()
			.expect("writer already consumed by into_inner()");
		writer.write_all(s.as_bytes())?;
		writer.write_all(b"\n")
	}

	/// Flush the output buffer.
	///
	/// This ensures that all buffered data is written to the underlying writer.
	///
	/// # Errors
	///
	/// Returns an error if the flush operation fails.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_commands::OutputWrapper;
	/// use std::io;
	///
	/// let stdout = io::stdout();
	/// let mut output = OutputWrapper::new(stdout);
	/// output.write("Important message").unwrap();
	/// output.flush().unwrap(); // Ensure the message is written immediately
	/// ```
	pub fn flush(&mut self) -> Result<()> {
		self.writer
			.as_mut()
			.expect("writer already consumed by into_inner()")
			.flush()
	}
}

// Implement Drop to ensure flush on drop (similar to Django's behavior)
impl<W: Write> Drop for OutputWrapper<W> {
	fn drop(&mut self) {
		// Best-effort flush - ignore errors in drop
		if let Some(ref mut writer) = self.writer {
			writer.flush().ok();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Cursor;
	use std::sync::{Arc, Mutex};

	#[test]
	fn test_output_wrapper_write() {
		let buffer = Vec::new();
		let mut output = OutputWrapper::new(buffer);

		output.write("Hello").unwrap();
		output.write(", ").unwrap();
		output.write("world!").unwrap();

		output.flush().unwrap();

		let buffer = output.into_inner().unwrap();
		assert_eq!(String::from_utf8(buffer).unwrap(), "Hello, world!");
	}

	#[test]
	fn test_output_wrapper_writeln() {
		let buffer = Vec::new();
		let mut output = OutputWrapper::new(buffer);

		output.writeln("Line 1").unwrap();
		output.writeln("Line 2").unwrap();
		output.writeln("Line 3").unwrap();

		output.flush().unwrap();

		let buffer = output.into_inner().unwrap();
		assert_eq!(
			String::from_utf8(buffer).unwrap(),
			"Line 1\nLine 2\nLine 3\n"
		);
	}

	#[test]
	fn test_output_wrapper_mixed() {
		let buffer = Vec::new();
		let mut output = OutputWrapper::new(buffer);

		output.write("Hello").unwrap();
		output.writeln(", world!").unwrap();
		output.write("Second ").unwrap();
		output.writeln("line").unwrap();

		output.flush().unwrap();

		let buffer = output.into_inner().unwrap();
		assert_eq!(
			String::from_utf8(buffer).unwrap(),
			"Hello, world!\nSecond line\n"
		);
	}

	#[test]
	fn test_output_wrapper_flush() {
		let buffer = Vec::new();
		let mut output = OutputWrapper::new(buffer);

		output.write("Test").unwrap();
		output.flush().unwrap();

		// After flush, data should be in the buffer
		let buffer = output.into_inner().unwrap();
		assert_eq!(String::from_utf8(buffer).unwrap(), "Test");
	}

	#[test]
	fn test_output_wrapper_empty() {
		let buffer = Vec::new();
		let mut output = OutputWrapper::new(buffer);

		output.flush().unwrap();

		let buffer = output.into_inner().unwrap();
		assert_eq!(buffer.len(), 0);
	}

	#[test]
	fn test_output_wrapper_cursor() {
		let cursor = Cursor::new(Vec::new());
		let mut output = OutputWrapper::new(cursor);

		output.writeln("Test line").unwrap();
		output.flush().unwrap();

		let cursor = output.into_inner().unwrap();
		assert_eq!(
			String::from_utf8(cursor.into_inner()).unwrap(),
			"Test line\n"
		);
	}

	#[test]
	fn test_output_wrapper_drop_flushes() {
		use std::sync::{Arc, Mutex};

		// Use Arc<Mutex<Vec<u8>>> to track writes after drop
		let buffer = Arc::new(Mutex::new(Vec::new()));
		let buffer_clone = buffer.clone();

		{
			let writer = BufferWriter {
				buffer: buffer.clone(),
			};
			let mut output = OutputWrapper::new(writer);
			output.write("Test").unwrap();
			// Drop happens here, should trigger flush
		}

		// Verify data was written (flush was called on drop)
		let data = buffer_clone.lock().unwrap();
		assert_eq!(String::from_utf8(data.clone()).unwrap(), "Test");
	}

	// Helper struct for testing Drop behavior
	struct BufferWriter {
		buffer: Arc<Mutex<Vec<u8>>>,
	}

	impl Write for BufferWriter {
		fn write(&mut self, buf: &[u8]) -> Result<usize> {
			let mut data = self.buffer.lock().unwrap();
			data.extend_from_slice(buf);
			Ok(buf.len())
		}

		fn flush(&mut self) -> Result<()> {
			// No-op, data is already in the shared buffer
			Ok(())
		}
	}
}
