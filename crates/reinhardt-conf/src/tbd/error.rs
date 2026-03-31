//! Error types for the TBD DSL parser and evaluator.

use std::fmt;
use std::path::PathBuf;

/// Byte offset range within source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
	/// Start byte offset (inclusive).
	pub start: usize,
	/// End byte offset (exclusive).
	pub end: usize,
}

/// Errors produced during TBD DSL parsing, type-checking, and evaluation.
#[derive(Debug, Clone)]
pub enum TbdError {
	/// A syntax error encountered during parsing.
	ParseError {
		/// Human-readable description of the parse failure.
		message: String,
		/// Location of the invalid syntax in the source.
		span: Span,
	},
	/// A type mismatch detected during type-checking.
	TypeError {
		/// The type that was expected.
		expected: String,
		/// The type that was actually found.
		found: String,
		/// Location of the mismatched expression.
		span: Span,
	},
	/// A runtime error during expression evaluation.
	EvalError {
		/// The specific kind of evaluation failure.
		kind: EvalErrorKind,
		/// Location of the expression that failed.
		span: Span,
	},
	/// An error associated with processing a specific file.
	ProcessError {
		/// Path to the file being processed.
		path: PathBuf,
		/// The underlying error.
		source: Box<TbdError>,
	},
}

/// Specific kinds of evaluation errors.
#[derive(Debug, Clone)]
pub enum EvalErrorKind {
	/// Attempted division by zero.
	DivisionByZero,
	/// A regex pattern string was invalid.
	InvalidRegexPattern(String),
	/// An integer range had `min > max`.
	RangeInvalid {
		/// The minimum value of the range.
		min: i64,
		/// The maximum value of the range.
		max: i64,
	},
	/// A function name was not recognised.
	UnknownFunction(String),
	/// No generation strategy was specified where one was required.
	MissingStrategy,
}

impl fmt::Display for EvalErrorKind {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::DivisionByZero => write!(f, "division by zero"),
			Self::InvalidRegexPattern(pat) => {
				write!(f, "invalid regex pattern: {pat}")
			}
			Self::RangeInvalid { min, max } => {
				write!(f, "invalid range: min ({min}) > max ({max})")
			}
			Self::UnknownFunction(name) => {
				write!(f, "unknown function: {name}")
			}
			Self::MissingStrategy => {
				write!(f, "no generation strategy specified")
			}
		}
	}
}

impl fmt::Display for TbdError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::ParseError { message, span } => {
				write!(f, "parse error at {}-{}: {message}", span.start, span.end)
			}
			Self::TypeError {
				expected,
				found,
				span,
			} => {
				write!(
					f,
					"type error at {}-{}: expected {expected}, found {found}",
					span.start, span.end
				)
			}
			Self::EvalError { kind, span } => {
				write!(f, "eval error at {}-{}: {kind}", span.start, span.end)
			}
			Self::ProcessError { path, source } => {
				write!(f, "error processing {}: {source}", path.display())
			}
		}
	}
}

impl std::error::Error for TbdError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::ProcessError { source, .. } => Some(source.as_ref()),
			_ => None,
		}
	}
}
