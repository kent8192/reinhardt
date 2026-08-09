//! Sorting functionality for tables

/// Sort direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
	/// Ascending order
	Ascending,
	/// Descending order
	Descending,
}

impl SortDirection {
	/// Returns the opposite direction
	pub fn toggle(&self) -> Self {
		match self {
			Self::Ascending => Self::Descending,
			Self::Descending => Self::Ascending,
		}
	}

	/// Parses a sort direction from a query parameter
	///
	/// Returns `Ascending` for positive values and `Descending` for negative values
	/// (e.g., "name" -> Ascending, "-name" -> Descending)
	pub fn parse_from_query(s: &str) -> (Self, &str) {
		if let Some(field) = s.strip_prefix('-') {
			(Self::Descending, field)
		} else {
			(Self::Ascending, s)
		}
	}
}

/// Trait for sortable tables
pub trait Sortable {
	/// Sorts the table by the specified field and direction
	fn sort_by(&mut self, field: &str, direction: SortDirection);

	/// Returns the current sort field and direction
	fn current_sort(&self) -> Option<(&str, SortDirection)>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn sorting_and_table_defaults_match_public_contract() {
		// Arrange
		let expected = [
			(SortDirection::Ascending, ""),
			(SortDirection::Descending, "name"),
			(SortDirection::Descending, "-name"),
		];

		// Act
		let parsed = [
			SortDirection::parse_from_query(""),
			SortDirection::parse_from_query("-name"),
			SortDirection::parse_from_query("--name"),
		];

		// Assert
		assert_eq!(parsed, expected);
		assert_eq!(SortDirection::Ascending.toggle(), SortDirection::Descending);
		assert_eq!(SortDirection::Descending.toggle(), SortDirection::Ascending);
	}
}
