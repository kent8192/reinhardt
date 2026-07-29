use crate::orm::connection::{DatabaseBackend, OrmExecutor, Row};
use crate::orm::custom_manager::CustomManager;
use crate::orm::manager::decode_model_row;
use crate::orm::upsert::assignment::{UpsertCreate, UpsertWrite};
use crate::orm::upsert::plan::UpsertPlan;
use crate::orm::upsert::sql;
use reinhardt_core::exception::{DatabaseErrorKind, Error, Result};

pub(crate) async fn execute_get_or_create<C, E>(
	manager: &C,
	mut plan: UpsertPlan<C::Model>,
	executor: &mut E,
) -> Result<(C::Model, bool)>
where
	C: CustomManager,
	E: OrmExecutor + ?Sized,
{
	let backend = executor.backend();
	let select = sql::select_by_lookup(&plan, backend, false)?;
	let rows = executor.fetch_all(&select.sql, select.params).await?;
	match decode_lookup_rows(rows)? {
		Some(model) => return Ok((model, false)),
		None => {}
	}

	manager.before_upsert_write(&mut UpsertWrite::Create(UpsertCreate {
		lookup: &plan.lookup,
		values: &mut plan.create,
	}))?;

	let insert = sql::insert(&plan, backend)?;
	match executor.execute(&insert.sql, insert.params).await {
		Ok(result) => {
			let created = match (backend, result.rows_affected) {
				(DatabaseBackend::Postgres | DatabaseBackend::Sqlite, 0) => false,
				(_, 1) => true,
				_ => {
					return Err(Error::Conflict(format!(
						"get_or_create INSERT affected {} rows for {backend:?}; expected one",
						result.rows_affected
					)));
				}
			};
			reload_lookup(&plan, executor, created, None).await
		}
		Err(error)
			if backend == DatabaseBackend::MySql
				&& error.database_kind() == Some(DatabaseErrorKind::UniqueViolation) =>
		{
			reload_lookup(&plan, executor, false, Some(error)).await
		}
		Err(error) => Err(error),
	}
}

async fn reload_lookup<M, E>(
	plan: &UpsertPlan<M>,
	executor: &mut E,
	created: bool,
	original_race_error: Option<Error>,
) -> Result<(M, bool)>
where
	M: crate::orm::model::Model,
	E: OrmExecutor + ?Sized,
{
	let select = sql::select_by_lookup(plan, executor.backend(), false)?;
	let rows = executor.fetch_all(&select.sql, select.params).await?;
	match decode_lookup_rows(rows)? {
		Some(model) => Ok((model, created)),
		None => match original_race_error {
			Some(error) => Err(error),
			None => Err(Error::Conflict(
				"get_or_create write completed without exactly one row matching the full lookup"
					.to_owned(),
			)),
		},
	}
}

fn decode_lookup_rows<M: crate::orm::model::Model>(rows: Vec<Row>) -> Result<Option<M>> {
	match rows.len() {
		0 => Ok(None),
		1 => rows.into_iter().next().map(decode_model_row).transpose(),
		count => Err(Error::Conflict(format!(
			"get_or_create full lookup matched {count} rows; expected at most one"
		))),
	}
}
