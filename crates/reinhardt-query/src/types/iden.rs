//! Identifier traits and types for SQL entities.
//!
//! This module provides the core traits for identifying SQL entities:
//!
//! - [`Iden`]: Trait for SQL identifiers (tables, columns, schemas)
//! - [`IdenStatic`]: Marker trait for compile-time identifiers
//! - [`DynIden`]: Type-erased identifier for heterogeneous collections
//! - [`IntoIden`]: Conversion trait for identifier types

use std::fmt::Write;

/// Thread-safe reference-counted pointer.
///
/// With the `thread-safe` feature enabled, this uses `Arc`.
/// Otherwise, it uses `Rc` for lower overhead in single-threaded contexts.
#[cfg(feature = "thread-safe")]
pub type SeaRc<T> = std::sync::Arc<T>;

/// Reference-counted smart pointer for identifier storage.
///
/// When the `thread-safe` feature is disabled, this uses `Rc` for lower overhead
/// in single-threaded contexts.
#[cfg(not(feature = "thread-safe"))]
pub type SeaRc<T> = std::rc::Rc<T>;

/// Type-erased identifier for heterogeneous collections.
///
/// This allows storing different types implementing `Iden` in the same collection.
pub type DynIden = SeaRc<dyn Iden>;

/// Trait for SQL identifiers.
///
/// This trait represents any SQL identifier such as table names, column names,
/// or schema names. Implementors must provide methods to write the identifier
/// in both quoted and unquoted forms.
///
/// # Example
///
/// ```rust
/// use reinhardt_query::Iden;
///
/// #[derive(Debug, Clone, Copy)]
/// enum Users {
///     Table,
///     Id,
///     Name,
///     Email,
/// }
///
/// impl Iden for Users {
///     fn unquoted(&self, s: &mut dyn std::fmt::Write) {
///         let name = match self {
///             Self::Table => "users",
///             Self::Id => "id",
///             Self::Name => "name",
///             Self::Email => "email",
///         };
///         write!(s, "{}", name).unwrap();
///     }
/// }
/// ```
pub trait Iden: Send + Sync + std::fmt::Debug {
	/// Write the identifier without quotes.
	///
	/// This method writes the raw identifier name to the given writer.
	fn unquoted(&self, s: &mut dyn Write);

	/// Write the identifier with quotes for the specified quote character.
	///
	/// The default implementation wraps the unquoted identifier with the quote character,
	/// escaping any embedded quote characters by doubling them.
	fn quoted(&self, q: char, s: &mut dyn Write) {
		write!(s, "{}", q).unwrap();
		let mut tmp = String::new();
		self.unquoted(&mut tmp);
		// Escape embedded quotes by doubling them
		for c in tmp.chars() {
			if c == q {
				write!(s, "{}{}", q, q).unwrap();
			} else {
				write!(s, "{}", c).unwrap();
			}
		}
		write!(s, "{}", q).unwrap();
	}

	/// Return the identifier as an unquoted string.
	fn to_string(&self) -> String {
		let mut s = String::new();
		self.unquoted(&mut s);
		s
	}
}

/// Marker trait for compile-time static identifiers.
///
/// This trait is used to mark identifiers that can be determined at compile time,
/// typically used with enums representing database schema elements.
pub trait IdenStatic: Iden + Copy {
	/// Returns the identifier as a static string.
	fn as_str(&self) -> &'static str;
}

/// Conversion trait for identifier types.
///
/// This trait provides conversion from various types to `DynIden`.
pub trait IntoIden {
	/// Convert this type into a `DynIden`.
	fn into_iden(self) -> DynIden;
}

// Blanket implementation for all types implementing Iden
impl<I> IntoIden for I
where
	I: Iden + 'static,
{
	fn into_iden(self) -> DynIden {
		SeaRc::new(self)
	}
}

// Implementation for DynIden itself
impl IntoIden for DynIden {
	fn into_iden(self) -> DynIden {
		self
	}
}

// Iden implementation for static &str
impl Iden for &'static str {
	fn unquoted(&self, s: &mut dyn Write) {
		write!(s, "{}", self).unwrap();
	}
}

// Iden implementation for String
impl Iden for String {
	fn unquoted(&self, s: &mut dyn Write) {
		write!(s, "{}", self).unwrap();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[allow(dead_code)]
	#[derive(Debug, Clone, Copy)]
	enum TestTable {
		Table,
		Id,
		Name,
	}

	impl Iden for TestTable {
		fn unquoted(&self, s: &mut dyn Write) {
			let name = match self {
				Self::Table => "test_table",
				Self::Id => "id",
				Self::Name => "name",
			};
			write!(s, "{}", name).unwrap();
		}
	}

	#[rstest]
	fn test_iden_unquoted() {
		assert_eq!(TestTable::Table.to_string(), "test_table");
		assert_eq!(TestTable::Id.to_string(), "id");
	}

	#[rstest]
	fn test_iden_quoted() {
		let mut s = String::new();
		TestTable::Table.quoted('"', &mut s);
		assert_eq!(s, "\"test_table\"");
	}

	#[rstest]
	fn test_iden_quoted_with_escape() {
		#[derive(Debug)]
		struct Quoted;
		impl Iden for Quoted {
			fn unquoted(&self, s: &mut dyn Write) {
				write!(s, "table\"with\"quotes").unwrap();
			}
		}

		let mut s = String::new();
		Quoted.quoted('"', &mut s);
		assert_eq!(s, "\"table\"\"with\"\"quotes\"");
	}

	#[rstest]
	fn test_str_iden() {
		assert_eq!("my_column".to_string(), "my_column");
	}

	#[rstest]
	fn test_into_iden() {
		let _dyn_iden: DynIden = TestTable::Table.into_iden();
		let _dyn_iden2: DynIden = "column_name".into_iden();
	}
}
