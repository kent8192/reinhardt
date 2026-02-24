//! COMMENT ON statement builder
//!
//! This module provides the `CommentStatement` type for building SQL COMMENT ON queries.

use crate::{backend::QueryBuilder, types::CommentTarget};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// COMMENT ON statement builder
///
/// This struct provides a fluent API for constructing COMMENT ON queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
/// use reinhardt_query::types::CommentTarget;
///
/// // COMMENT ON TABLE "users" IS 'User account information'
/// let query = Query::comment()
///     .target(CommentTarget::Table("users".into_iden()))
///     .comment("User account information");
///
/// // COMMENT ON COLUMN "users"."email" IS 'User email address'
/// let query = Query::comment()
///     .target(CommentTarget::Column("users".into_iden(), "email".into_iden()))
///     .comment("User email address");
///
/// // COMMENT ON TABLE "users" IS NULL (remove comment)
/// let query = Query::comment()
///     .target(CommentTarget::Table("users".into_iden()))
///     .comment_null();
/// ```
#[derive(Debug, Clone)]
pub struct CommentStatement {
	pub(crate) target: Option<CommentTarget>,
	pub(crate) comment: Option<String>,
	pub(crate) is_null: bool,
}

impl CommentStatement {
	/// Create a new COMMENT ON statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::comment();
	/// ```
	pub fn new() -> Self {
		Self {
			target: None,
			comment: None,
			is_null: false,
		}
	}

	/// Take the ownership of data in the current [`CommentStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			target: self.target.take(),
			comment: self.comment.take(),
			is_null: self.is_null,
		}
	}

	/// Set the target object for the comment
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::CommentTarget;
	///
	/// let query = Query::comment()
	///     .target(CommentTarget::Table("users".into_iden()));
	/// ```
	pub fn target(mut self, target: CommentTarget) -> Self {
		self.target = Some(target);
		self
	}

	/// Set the comment text
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::CommentTarget;
	///
	/// let query = Query::comment()
	///     .target(CommentTarget::Table("users".into_iden()))
	///     .comment("User account information");
	/// ```
	pub fn comment<S: Into<String>>(mut self, comment: S) -> Self {
		self.comment = Some(comment.into());
		self.is_null = false;
		self
	}

	/// Set the comment to NULL (remove comment)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::CommentTarget;
	///
	/// let query = Query::comment()
	///     .target(CommentTarget::Table("users".into_iden()))
	///     .comment_null();
	/// ```
	pub fn comment_null(mut self) -> Self {
		self.comment = None;
		self.is_null = true;
		self
	}
}

impl Default for CommentStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CommentStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast QueryBuilderTrait to concrete QueryBuilder types
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_comment(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_comment(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_comment(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_comment(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CommentStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{backend::PostgresQueryBuilder, types::IntoIden};
	use rstest::*;

	#[fixture]
	fn builder() -> PostgresQueryBuilder {
		PostgresQueryBuilder
	}

	#[rstest]
	fn test_comment_on_table(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::Table("users".into_iden()))
			.comment("User account information");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(
			sql,
			"COMMENT ON TABLE \"users\" IS 'User account information'"
		);
	}

	#[rstest]
	fn test_comment_on_column(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::Column(
				"users".into_iden(),
				"email".into_iden(),
			))
			.comment("User email address");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(
			sql,
			"COMMENT ON COLUMN \"users\".\"email\" IS 'User email address'"
		);
	}

	#[rstest]
	fn test_comment_on_index(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::Index("idx_users_email".into_iden()))
			.comment("Email index");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(sql, "COMMENT ON INDEX \"idx_users_email\" IS 'Email index'");
	}

	#[rstest]
	fn test_comment_on_view(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::View("active_users".into_iden()))
			.comment("Active users view");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(
			sql,
			"COMMENT ON VIEW \"active_users\" IS 'Active users view'"
		);
	}

	#[rstest]
	fn test_comment_on_materialized_view(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::MaterializedView("user_stats".into_iden()))
			.comment("User statistics");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(
			sql,
			"COMMENT ON MATERIALIZED VIEW \"user_stats\" IS 'User statistics'"
		);
	}

	#[rstest]
	fn test_comment_on_sequence(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::Sequence("user_id_seq".into_iden()))
			.comment("User ID sequence");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(
			sql,
			"COMMENT ON SEQUENCE \"user_id_seq\" IS 'User ID sequence'"
		);
	}

	#[rstest]
	fn test_comment_on_schema(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::Schema("public".into_iden()))
			.comment("Public schema");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(sql, "COMMENT ON SCHEMA \"public\" IS 'Public schema'");
	}

	#[rstest]
	fn test_comment_on_database(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::Database("mydb".into_iden()))
			.comment("My database");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(sql, "COMMENT ON DATABASE \"mydb\" IS 'My database'");
	}

	#[rstest]
	fn test_comment_on_function(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::Function("calculate_total".into_iden()))
			.comment("Calculate total amount");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(
			sql,
			"COMMENT ON FUNCTION \"calculate_total\" IS 'Calculate total amount'"
		);
	}

	#[rstest]
	fn test_comment_on_trigger(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::Trigger(
				"update_timestamp".into_iden(),
				"users".into_iden(),
			))
			.comment("Update timestamp trigger");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(
			sql,
			"COMMENT ON TRIGGER \"update_timestamp\" ON \"users\" IS 'Update timestamp trigger'"
		);
	}

	#[rstest]
	fn test_comment_on_type(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::Type("user_status".into_iden()))
			.comment("User status enum");
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(sql, "COMMENT ON TYPE \"user_status\" IS 'User status enum'");
	}

	#[rstest]
	fn test_comment_null(builder: PostgresQueryBuilder) {
		let query = CommentStatement::new()
			.target(CommentTarget::Table("users".into_iden()))
			.comment_null();
		let (sql, _) = builder.build_comment(&query);
		assert_eq!(sql, "COMMENT ON TABLE \"users\" IS NULL");
	}
}
