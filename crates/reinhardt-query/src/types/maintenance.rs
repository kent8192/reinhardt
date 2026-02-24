//! Database maintenance types
//!
//! This module provides types for database maintenance operations:
//!
//! - [`VacuumOption`]: Options for VACUUM statement
//! - [`OptimizeTableOption`]: Options for OPTIMIZE TABLE statement (MySQL-only)
//! - [`RepairTableOption`]: Options for REPAIR TABLE statement (MySQL-only)
//! - [`CheckTableOption`]: Options for CHECK TABLE statement (MySQL-only)

use crate::types::{DynIden, IntoIden};

/// VACUUM statement options
///
/// This struct represents options for the VACUUM statement.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::maintenance::VacuumOption;
///
/// // Basic VACUUM
/// let opt = VacuumOption::new();
///
/// // VACUUM FULL
/// let opt = VacuumOption::new().full(true);
///
/// // VACUUM FULL ANALYZE
/// let opt = VacuumOption::new().full(true).analyze(true);
/// ```
#[derive(Debug, Clone, Default)]
pub struct VacuumOption {
	pub(crate) full: bool,
	pub(crate) freeze: bool,
	pub(crate) verbose: bool,
	pub(crate) analyze: bool,
}

impl VacuumOption {
	/// Create a new VACUUM option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::VacuumOption;
	///
	/// let opt = VacuumOption::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set FULL option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::VacuumOption;
	///
	/// let opt = VacuumOption::new().full(true);
	/// ```
	pub fn full(mut self, full: bool) -> Self {
		self.full = full;
		self
	}

	/// Set FREEZE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::VacuumOption;
	///
	/// let opt = VacuumOption::new().freeze(true);
	/// ```
	pub fn freeze(mut self, freeze: bool) -> Self {
		self.freeze = freeze;
		self
	}

	/// Set VERBOSE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::VacuumOption;
	///
	/// let opt = VacuumOption::new().verbose(true);
	/// ```
	pub fn verbose(mut self, verbose: bool) -> Self {
		self.verbose = verbose;
		self
	}

	/// Set ANALYZE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::VacuumOption;
	///
	/// let opt = VacuumOption::new().analyze(true);
	/// ```
	pub fn analyze(mut self, analyze: bool) -> Self {
		self.analyze = analyze;
		self
	}
}

/// Table specification for ANALYZE statement
///
/// This struct represents a table and its optional columns for ANALYZE.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::maintenance::AnalyzeTable;
///
/// // Analyze entire table
/// let tbl = AnalyzeTable::new("users");
///
/// // Analyze specific columns
/// let tbl = AnalyzeTable::new("users")
///     .add_column("email")
///     .add_column("name");
/// ```
#[derive(Debug, Clone)]
pub struct AnalyzeTable {
	// Allow dead_code: table field will be used when Phase B ANALYZE implementation is completed
	#[allow(dead_code)]
	pub(crate) table: DynIden,
	pub(crate) columns: Vec<DynIden>,
}

impl AnalyzeTable {
	/// Create a new ANALYZE table specification
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::AnalyzeTable;
	///
	/// let tbl = AnalyzeTable::new("users");
	/// ```
	pub fn new<T: IntoIden>(table: T) -> Self {
		Self {
			table: table.into_iden(),
			columns: Vec::new(),
		}
	}

	/// Add a column to analyze
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::AnalyzeTable;
	///
	/// let tbl = AnalyzeTable::new("users")
	///     .add_column("email");
	/// ```
	pub fn add_column<C: IntoIden>(mut self, column: C) -> Self {
		self.columns.push(column.into_iden());
		self
	}
}

/// OPTIMIZE TABLE statement options (MySQL-only)
///
/// This struct represents options for the OPTIMIZE TABLE statement.
/// OPTIMIZE TABLE reorganizes the physical storage of table data and associated index data.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::maintenance::OptimizeTableOption;
///
/// // Basic OPTIMIZE TABLE
/// let opt = OptimizeTableOption::new();
///
/// // OPTIMIZE NO_WRITE_TO_BINLOG TABLE
/// let opt = OptimizeTableOption::new().no_write_to_binlog(true);
///
/// // OPTIMIZE LOCAL TABLE
/// let opt = OptimizeTableOption::new().local(true);
/// ```
#[derive(Debug, Clone, Default)]
pub struct OptimizeTableOption {
	pub(crate) no_write_to_binlog: bool,
	pub(crate) local: bool,
}

impl OptimizeTableOption {
	/// Create a new OPTIMIZE TABLE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::OptimizeTableOption;
	///
	/// let opt = OptimizeTableOption::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set NO_WRITE_TO_BINLOG option
	///
	/// Suppresses binary logging for this operation (same as LOCAL).
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::OptimizeTableOption;
	///
	/// let opt = OptimizeTableOption::new().no_write_to_binlog(true);
	/// ```
	pub fn no_write_to_binlog(mut self, no_write_to_binlog: bool) -> Self {
		self.no_write_to_binlog = no_write_to_binlog;
		self
	}

	/// Set LOCAL option
	///
	/// Suppresses binary logging for this operation (same as NO_WRITE_TO_BINLOG).
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::OptimizeTableOption;
	///
	/// let opt = OptimizeTableOption::new().local(true);
	/// ```
	pub fn local(mut self, local: bool) -> Self {
		self.local = local;
		self
	}
}

/// REPAIR TABLE statement options (MySQL-only)
///
/// This struct represents options for the REPAIR TABLE statement.
/// REPAIR TABLE repairs a possibly corrupted table.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::maintenance::RepairTableOption;
///
/// // Basic REPAIR TABLE
/// let opt = RepairTableOption::new();
///
/// // REPAIR TABLE with QUICK option
/// let opt = RepairTableOption::new().quick(true);
///
/// // REPAIR TABLE with EXTENDED option
/// let opt = RepairTableOption::new().extended(true);
/// ```
#[derive(Debug, Clone, Default)]
pub struct RepairTableOption {
	pub(crate) no_write_to_binlog: bool,
	pub(crate) local: bool,
	pub(crate) quick: bool,
	pub(crate) extended: bool,
	pub(crate) use_frm: bool,
}

impl RepairTableOption {
	/// Create a new REPAIR TABLE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::RepairTableOption;
	///
	/// let opt = RepairTableOption::new();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set NO_WRITE_TO_BINLOG option
	///
	/// Suppresses binary logging for this operation.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::RepairTableOption;
	///
	/// let opt = RepairTableOption::new().no_write_to_binlog(true);
	/// ```
	pub fn no_write_to_binlog(mut self, no_write_to_binlog: bool) -> Self {
		self.no_write_to_binlog = no_write_to_binlog;
		self
	}

	/// Set LOCAL option
	///
	/// Suppresses binary logging for this operation (same as NO_WRITE_TO_BINLOG).
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::RepairTableOption;
	///
	/// let opt = RepairTableOption::new().local(true);
	/// ```
	pub fn local(mut self, local: bool) -> Self {
		self.local = local;
		self
	}

	/// Set QUICK option
	///
	/// Tries to repair only the index file, not the data file.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::RepairTableOption;
	///
	/// let opt = RepairTableOption::new().quick(true);
	/// ```
	pub fn quick(mut self, quick: bool) -> Self {
		self.quick = quick;
		self
	}

	/// Set EXTENDED option
	///
	/// Creates the index row by row instead of creating one index at a time with sorting.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::RepairTableOption;
	///
	/// let opt = RepairTableOption::new().extended(true);
	/// ```
	pub fn extended(mut self, extended: bool) -> Self {
		self.extended = extended;
		self
	}

	/// Set USE_FRM option
	///
	/// Uses the table definition from the .frm file to recreate the index file.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::maintenance::RepairTableOption;
	///
	/// let opt = RepairTableOption::new().use_frm(true);
	/// ```
	pub fn use_frm(mut self, use_frm: bool) -> Self {
		self.use_frm = use_frm;
		self
	}
}

/// CHECK TABLE statement options (MySQL-only)
///
/// This enum represents the check option for the CHECK TABLE statement.
/// CHECK TABLE checks a table or tables for errors.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::maintenance::CheckTableOption;
///
/// // Default check (MEDIUM)
/// let opt = CheckTableOption::default();
///
/// // Quick check
/// let opt = CheckTableOption::Quick;
///
/// // Extended check
/// let opt = CheckTableOption::Extended;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckTableOption {
	/// Check for version compatibility
	ForUpgrade,
	/// Quick check, skip scanning rows for incorrect links
	Quick,
	/// Fast check, check only tables that haven't been closed properly
	Fast,
	/// Medium check (default), scan rows to verify deleted links are valid
	#[default]
	Medium,
	/// Extended check, do a full key lookup for all keys
	Extended,
	/// Check only tables that have been changed since last check or not closed properly
	Changed,
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	// VacuumOption tests
	#[rstest]
	fn test_vacuum_option_default() {
		let opt = VacuumOption::new();
		assert!(!opt.full);
		assert!(!opt.freeze);
		assert!(!opt.verbose);
		assert!(!opt.analyze);
	}

	#[rstest]
	fn test_vacuum_option_full() {
		let opt = VacuumOption::new().full(true);
		assert!(opt.full);
		assert!(!opt.freeze);
		assert!(!opt.verbose);
		assert!(!opt.analyze);
	}

	#[rstest]
	fn test_vacuum_option_freeze() {
		let opt = VacuumOption::new().freeze(true);
		assert!(!opt.full);
		assert!(opt.freeze);
		assert!(!opt.verbose);
		assert!(!opt.analyze);
	}

	#[rstest]
	fn test_vacuum_option_verbose() {
		let opt = VacuumOption::new().verbose(true);
		assert!(!opt.full);
		assert!(!opt.freeze);
		assert!(opt.verbose);
		assert!(!opt.analyze);
	}

	#[rstest]
	fn test_vacuum_option_analyze() {
		let opt = VacuumOption::new().analyze(true);
		assert!(!opt.full);
		assert!(!opt.freeze);
		assert!(!opt.verbose);
		assert!(opt.analyze);
	}

	#[rstest]
	fn test_vacuum_option_combined() {
		let opt = VacuumOption::new()
			.full(true)
			.freeze(true)
			.verbose(true)
			.analyze(true);
		assert!(opt.full);
		assert!(opt.freeze);
		assert!(opt.verbose);
		assert!(opt.analyze);
	}

	// AnalyzeTable tests
	#[rstest]
	fn test_analyze_table_basic() {
		let tbl = AnalyzeTable::new("users");
		assert_eq!(tbl.table.to_string(), "users");
		assert!(tbl.columns.is_empty());
	}

	#[rstest]
	fn test_analyze_table_with_column() {
		let tbl = AnalyzeTable::new("users").add_column("email");
		assert_eq!(tbl.table.to_string(), "users");
		assert_eq!(tbl.columns.len(), 1);
		assert_eq!(tbl.columns[0].to_string(), "email");
	}

	#[rstest]
	fn test_analyze_table_with_multiple_columns() {
		let tbl = AnalyzeTable::new("users")
			.add_column("email")
			.add_column("name")
			.add_column("age");
		assert_eq!(tbl.table.to_string(), "users");
		assert_eq!(tbl.columns.len(), 3);
		assert_eq!(tbl.columns[0].to_string(), "email");
		assert_eq!(tbl.columns[1].to_string(), "name");
		assert_eq!(tbl.columns[2].to_string(), "age");
	}

	// OptimizeTableOption tests
	#[rstest]
	fn test_optimize_table_option_default() {
		let opt = OptimizeTableOption::new();
		assert!(!opt.no_write_to_binlog);
		assert!(!opt.local);
	}

	#[rstest]
	fn test_optimize_table_option_no_write_to_binlog() {
		let opt = OptimizeTableOption::new().no_write_to_binlog(true);
		assert!(opt.no_write_to_binlog);
		assert!(!opt.local);
	}

	#[rstest]
	fn test_optimize_table_option_local() {
		let opt = OptimizeTableOption::new().local(true);
		assert!(!opt.no_write_to_binlog);
		assert!(opt.local);
	}

	// RepairTableOption tests
	#[rstest]
	fn test_repair_table_option_default() {
		let opt = RepairTableOption::new();
		assert!(!opt.no_write_to_binlog);
		assert!(!opt.local);
		assert!(!opt.quick);
		assert!(!opt.extended);
		assert!(!opt.use_frm);
	}

	#[rstest]
	fn test_repair_table_option_quick() {
		let opt = RepairTableOption::new().quick(true);
		assert!(opt.quick);
		assert!(!opt.extended);
	}

	#[rstest]
	fn test_repair_table_option_extended() {
		let opt = RepairTableOption::new().extended(true);
		assert!(!opt.quick);
		assert!(opt.extended);
	}

	#[rstest]
	fn test_repair_table_option_use_frm() {
		let opt = RepairTableOption::new().use_frm(true);
		assert!(opt.use_frm);
	}

	// CheckTableOption tests
	#[rstest]
	fn test_check_table_option_default() {
		let opt = CheckTableOption::default();
		assert!(matches!(opt, CheckTableOption::Medium));
	}

	#[rstest]
	fn test_check_table_option_variants() {
		assert!(matches!(
			CheckTableOption::ForUpgrade,
			CheckTableOption::ForUpgrade
		));
		assert!(matches!(CheckTableOption::Quick, CheckTableOption::Quick));
		assert!(matches!(CheckTableOption::Fast, CheckTableOption::Fast));
		assert!(matches!(CheckTableOption::Medium, CheckTableOption::Medium));
		assert!(matches!(
			CheckTableOption::Extended,
			CheckTableOption::Extended
		));
		assert!(matches!(
			CheckTableOption::Changed,
			CheckTableOption::Changed
		));
	}
}
