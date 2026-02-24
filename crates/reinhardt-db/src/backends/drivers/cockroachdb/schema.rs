/// CockroachDB-specific schema editor
///
/// This module provides CockroachDB-specific DDL operations, including:
/// - LOCALITY support for multi-region tables
/// - PARTITION BY for table partitioning
/// - AS OF SYSTEM TIME for historical queries
/// - Region-aware index creation
///
/// CockroachDB is PostgreSQL-compatible, so it inherits PostgreSQL schema operations
/// and extends them with distributed database features.
///
/// # Example
///
/// ```rust,ignore
/// # use reinhardt_db::backends::drivers::cockroachdb::schema::CockroachDBSchemaEditor;
/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
/// let pg_editor = PostgreSQLSchemaEditor::new();
/// let editor = CockroachDBSchemaEditor::new(pg_editor);
///
/// // Create a region-aware table
/// let sql = editor.create_table_with_locality_sql(
///     "users",
///     &[("id", "UUID PRIMARY KEY"), ("name", "VARCHAR(100)")],
///     "REGIONAL BY ROW"
/// );
/// assert!(sql.contains("LOCALITY REGIONAL BY ROW"));
/// ```
use crate::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
use crate::backends::schema::{BaseDatabaseSchemaEditor, SchemaEditorResult};

/// Quote an identifier with double quotes
///
/// Always adds double quotes around identifiers for CockroachDB,
/// ensuring compatibility with reserved words and special characters.
fn quote_ident(ident: &str) -> String {
	format!("\"{}\"", ident.replace("\"", "\"\""))
}

/// CockroachDB-specific schema editor
///
/// Extends PostgreSQL schema editor with CockroachDB-specific features
pub struct CockroachDBSchemaEditor {
	/// PostgreSQL schema editor for base operations
	pub pg_editor: PostgreSQLSchemaEditor,
}

impl CockroachDBSchemaEditor {
	/// Create a new CockroachDB schema editor
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::schema::CockroachDBSchemaEditor;
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let editor = CockroachDBSchemaEditor::new(pg_editor);
	/// ```
	pub fn new(pg_editor: PostgreSQLSchemaEditor) -> Self {
		Self { pg_editor }
	}

	/// Generate CREATE TABLE with LOCALITY clause
	///
	/// CockroachDB supports multi-region tables with LOCALITY options:
	/// - REGIONAL BY ROW: Rows are stored in the region specified by a column
	/// - REGIONAL BY TABLE: Entire table stored in one region
	/// - GLOBAL: Replicated across all regions
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::schema::CockroachDBSchemaEditor;
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let editor = CockroachDBSchemaEditor::new(pg_editor);
	///
	/// let sql = editor.create_table_with_locality_sql(
	///     "users",
	///     &[("id", "UUID PRIMARY KEY"), ("name", "VARCHAR(100)")],
	///     "REGIONAL BY ROW"
	/// );
	/// assert!(sql.contains("CREATE TABLE"));
	/// assert!(sql.contains("LOCALITY REGIONAL BY ROW"));
	/// ```
	pub fn create_table_with_locality_sql(
		&self,
		table: &str,
		columns: &[(&str, &str)],
		locality: &str,
	) -> String {
		let quoted_table = quote_ident(table);
		let column_defs: Vec<String> = columns
			.iter()
			.map(|(name, def)| format!("{} {}", quote_ident(name), def))
			.collect();

		format!(
			"CREATE TABLE {} ({}) LOCALITY {}",
			quoted_table,
			column_defs.join(", "),
			locality
		)
	}

	/// Generate ALTER TABLE to set LOCALITY
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::schema::CockroachDBSchemaEditor;
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let editor = CockroachDBSchemaEditor::new(pg_editor);
	///
	/// let sql = editor.alter_table_locality_sql("users", "REGIONAL BY TABLE IN \"us-east-1\"");
	/// assert!(sql.contains("ALTER TABLE"));
	/// assert!(sql.contains("SET LOCALITY"));
	/// ```
	pub fn alter_table_locality_sql(&self, table: &str, locality: &str) -> String {
		format!(
			"ALTER TABLE {} SET LOCALITY {}",
			quote_ident(table),
			locality
		)
	}

	/// Generate PARTITION BY clause for CREATE TABLE
	///
	/// CockroachDB supports table partitioning for better data locality.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::schema::CockroachDBSchemaEditor;
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let editor = CockroachDBSchemaEditor::new(pg_editor);
	///
	/// let sql = editor.create_partitioned_table_sql(
	///     "events",
	///     &[("id", "UUID"), ("region", "VARCHAR(50)"), ("data", "JSONB")],
	///     "region",
	///     &[("us-east", "'us-east-1', 'us-east-2'"), ("us-west", "'us-west-1', 'us-west-2'")]
	/// );
	/// assert!(sql.contains("PARTITION BY LIST"));
	/// ```
	pub fn create_partitioned_table_sql(
		&self,
		table: &str,
		columns: &[(&str, &str)],
		partition_column: &str,
		partitions: &[(&str, &str)],
	) -> String {
		let quoted_table = quote_ident(table);
		let column_defs: Vec<String> = columns
			.iter()
			.map(|(name, def)| format!("{} {}", quote_ident(name), def))
			.collect();

		let partition_defs: Vec<String> = partitions
			.iter()
			.map(|(name, values)| format!("PARTITION {} VALUES IN ({})", quote_ident(name), values))
			.collect();

		format!(
			"CREATE TABLE {} ({}) PARTITION BY LIST ({}) ({})",
			quoted_table,
			column_defs.join(", "),
			quote_ident(partition_column),
			partition_defs.join(", ")
		)
	}

	/// Generate region-aware index SQL
	///
	/// Create an index with STORING clause for covering indexes.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::schema::CockroachDBSchemaEditor;
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let editor = CockroachDBSchemaEditor::new(pg_editor);
	///
	/// let sql = editor.create_index_with_storing_sql(
	///     "idx_email",
	///     "users",
	///     &["email"],
	///     &["name", "created_at"],
	///     false,
	///     None
	/// );
	/// assert!(sql.contains("STORING"));
	/// ```
	pub fn create_index_with_storing_sql(
		&self,
		name: &str,
		table: &str,
		columns: &[&str],
		storing: &[&str],
		unique: bool,
		condition: Option<&str>,
	) -> String {
		let unique_keyword = if unique { "UNIQUE " } else { "" };
		let quoted_columns: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();

		let mut sql = format!(
			"CREATE {}INDEX {} ON {} ({})",
			unique_keyword,
			quote_ident(name),
			quote_ident(table),
			quoted_columns.join(", ")
		);

		if !storing.is_empty() {
			let storing_cols: Vec<String> = storing.iter().map(|c| quote_ident(c)).collect();
			sql.push_str(&format!(" STORING ({})", storing_cols.join(", ")));
		}

		if let Some(cond) = condition {
			sql.push_str(&format!(" WHERE {}", cond));
		}

		sql
	}

	/// Generate AS OF SYSTEM TIME query SQL
	///
	/// CockroachDB supports historical queries using AS OF SYSTEM TIME.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::schema::CockroachDBSchemaEditor;
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let editor = CockroachDBSchemaEditor::new(pg_editor);
	///
	/// let sql = editor.as_of_system_time_sql("SELECT * FROM users WHERE id = $1", "-5s");
	/// assert!(sql.contains("AS OF SYSTEM TIME"));
	/// ```
	pub fn as_of_system_time_sql(&self, query: &str, timestamp: &str) -> String {
		format!("{} AS OF SYSTEM TIME {}", query, timestamp)
	}

	/// Generate SHOW REGIONS SQL
	///
	/// Query available regions in the CockroachDB cluster.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::schema::CockroachDBSchemaEditor;
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let editor = CockroachDBSchemaEditor::new(pg_editor);
	///
	/// let sql = editor.show_regions_sql();
	/// assert_eq!(sql, "SHOW REGIONS");
	/// ```
	pub fn show_regions_sql(&self) -> String {
		"SHOW REGIONS".to_string()
	}

	/// Generate SHOW SURVIVAL GOAL SQL
	///
	/// Query the survival goal for the current database.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::schema::CockroachDBSchemaEditor;
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let editor = CockroachDBSchemaEditor::new(pg_editor);
	///
	/// let sql = editor.show_survival_goal_sql();
	/// assert_eq!(sql, "SHOW SURVIVAL GOAL");
	/// ```
	pub fn show_survival_goal_sql(&self) -> String {
		"SHOW SURVIVAL GOAL".to_string()
	}

	/// Generate ALTER DATABASE SET PRIMARY REGION SQL
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::cockroachdb::schema::CockroachDBSchemaEditor;
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let editor = CockroachDBSchemaEditor::new(pg_editor);
	///
	/// let sql = editor.set_primary_region_sql("mydb", "us-east-1");
	/// assert!(sql.contains("SET PRIMARY REGION"));
	/// ```
	pub fn set_primary_region_sql(&self, database: &str, region: &str) -> String {
		format!(
			"ALTER DATABASE {} SET PRIMARY REGION {}",
			quote_ident(database),
			quote_ident(region)
		)
	}
}

#[async_trait::async_trait]
impl BaseDatabaseSchemaEditor for CockroachDBSchemaEditor {
	fn database_type(&self) -> crate::backends::types::DatabaseType {
		// CockroachDB is PostgreSQL-compatible
		crate::backends::types::DatabaseType::Postgres
	}

	async fn execute(&mut self, sql: &str) -> SchemaEditorResult<()> {
		self.pg_editor.execute(sql).await
	}
}
