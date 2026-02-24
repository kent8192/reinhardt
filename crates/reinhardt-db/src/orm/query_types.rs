//! Query abstraction layer
//!
//! Type definitions for passing reinhardt-query objects instead of SQL strings

use reinhardt_query::prelude::{
	AlterTableStatement, CreateIndexStatement, CreateTableStatement, DeleteStatement,
	DropIndexStatement, DropTableStatement, InsertStatement, MySqlQueryBuilder,
	PostgresQueryBuilder, QueryBuilder, SelectStatement, SqliteQueryBuilder, UpdateStatement,
	Values,
};

/// Database backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbBackend {
	Postgres,
	Mysql,
	Sqlite,
}

impl DbBackend {
	/// Get query builder for this backend
	pub fn query_builder(&self) -> DbQueryBuilder {
		match self {
			DbBackend::Postgres => DbQueryBuilder::Postgres,
			DbBackend::Mysql => DbQueryBuilder::Mysql,
			DbBackend::Sqlite => DbQueryBuilder::Sqlite,
		}
	}
}

/// Query builder type
#[derive(Debug, Clone, Copy)]
pub enum DbQueryBuilder {
	Postgres,
	Mysql,
	Sqlite,
}

/// Unified query statement enum
///
/// Wraps reinhardt-query objects for passing around instead of SQL strings
#[derive(Debug, Clone)]
pub enum QueryStatement {
	Select(SelectStatement),
	Insert(InsertStatement),
	Update(UpdateStatement),
	Delete(DeleteStatement),
	CreateTable(CreateTableStatement),
	AlterTable(AlterTableStatement),
	DropTable(DropTableStatement),
	RenameTable(AlterTableStatement),
	CreateIndex(CreateIndexStatement),
	DropIndex(DropIndexStatement),
}

impl QueryStatement {
	/// Build SQL string and bind values according to database backend
	pub fn build(&self, backend: DbBackend) -> (String, Values) {
		let pg = PostgresQueryBuilder::new();
		let mysql = MySqlQueryBuilder::new();
		let sqlite = SqliteQueryBuilder::new();

		match (self, backend) {
			(QueryStatement::Select(stmt), DbBackend::Postgres) => pg.build_select(stmt),
			(QueryStatement::Select(stmt), DbBackend::Mysql) => mysql.build_select(stmt),
			(QueryStatement::Select(stmt), DbBackend::Sqlite) => sqlite.build_select(stmt),
			(QueryStatement::Insert(stmt), DbBackend::Postgres) => pg.build_insert(stmt),
			(QueryStatement::Insert(stmt), DbBackend::Mysql) => mysql.build_insert(stmt),
			(QueryStatement::Insert(stmt), DbBackend::Sqlite) => sqlite.build_insert(stmt),
			(QueryStatement::Update(stmt), DbBackend::Postgres) => pg.build_update(stmt),
			(QueryStatement::Update(stmt), DbBackend::Mysql) => mysql.build_update(stmt),
			(QueryStatement::Update(stmt), DbBackend::Sqlite) => sqlite.build_update(stmt),
			(QueryStatement::Delete(stmt), DbBackend::Postgres) => pg.build_delete(stmt),
			(QueryStatement::Delete(stmt), DbBackend::Mysql) => mysql.build_delete(stmt),
			(QueryStatement::Delete(stmt), DbBackend::Sqlite) => sqlite.build_delete(stmt),
			(QueryStatement::CreateTable(stmt), DbBackend::Postgres) => pg.build_create_table(stmt),
			(QueryStatement::CreateTable(stmt), DbBackend::Mysql) => mysql.build_create_table(stmt),
			(QueryStatement::CreateTable(stmt), DbBackend::Sqlite) => {
				sqlite.build_create_table(stmt)
			}
			(QueryStatement::AlterTable(stmt), DbBackend::Postgres) => pg.build_alter_table(stmt),
			(QueryStatement::AlterTable(stmt), DbBackend::Mysql) => mysql.build_alter_table(stmt),
			(QueryStatement::AlterTable(stmt), DbBackend::Sqlite) => sqlite.build_alter_table(stmt),
			(QueryStatement::DropTable(stmt), DbBackend::Postgres) => pg.build_drop_table(stmt),
			(QueryStatement::DropTable(stmt), DbBackend::Mysql) => mysql.build_drop_table(stmt),
			(QueryStatement::DropTable(stmt), DbBackend::Sqlite) => sqlite.build_drop_table(stmt),
			(QueryStatement::RenameTable(stmt), DbBackend::Postgres) => pg.build_alter_table(stmt),
			(QueryStatement::RenameTable(stmt), DbBackend::Mysql) => mysql.build_alter_table(stmt),
			(QueryStatement::RenameTable(stmt), DbBackend::Sqlite) => {
				sqlite.build_alter_table(stmt)
			}
			(QueryStatement::CreateIndex(stmt), DbBackend::Postgres) => pg.build_create_index(stmt),
			(QueryStatement::CreateIndex(stmt), DbBackend::Mysql) => mysql.build_create_index(stmt),
			(QueryStatement::CreateIndex(stmt), DbBackend::Sqlite) => {
				sqlite.build_create_index(stmt)
			}
			(QueryStatement::DropIndex(stmt), DbBackend::Postgres) => pg.build_drop_index(stmt),
			(QueryStatement::DropIndex(stmt), DbBackend::Mysql) => mysql.build_drop_index(stmt),
			(QueryStatement::DropIndex(stmt), DbBackend::Sqlite) => sqlite.build_drop_index(stmt),
		}
	}

	/// Build SQL string only (without bind values)
	pub fn to_string(&self, backend: DbBackend) -> String {
		let (sql, _) = self.build(backend);
		sql
	}
}
