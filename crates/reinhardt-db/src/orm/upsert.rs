//! Typed builders, assignment views, and normalized plans for atomic upserts.

pub(crate) mod assignment;
pub(crate) mod execution;
pub(crate) mod plan;
pub(crate) mod sql;

use crate::orm::connection::OrmExecutor;
use crate::orm::custom_manager::CustomManager;
use crate::orm::expressions::FieldRef;
use crate::orm::field_codec::{DatabaseField, IntoFieldValue};
use crate::orm::upsert::assignment::TypedAssignment;
use crate::orm::upsert::execution::execute_get_or_create;
use crate::orm::upsert::plan::{UpsertMode, normalize};
use reinhardt_core::exception::{Error, Result};

pub use assignment::{UpsertCreate, UpsertWrite};

/// Typed builder for retrieving an existing row or creating it atomically.
pub struct GetOrCreateBuilder<C: CustomManager> {
	manager: C,
	lookup: Vec<TypedAssignment<C::Model>>,
	defaults: Vec<TypedAssignment<C::Model>>,
	error: Option<Error>,
}

enum GetAssignmentRole {
	Lookup,
	Default,
}

impl<C: CustomManager> GetOrCreateBuilder<C> {
	pub(crate) fn new(manager: C) -> Self {
		Self {
			manager,
			lookup: Vec::new(),
			defaults: Vec::new(),
			error: None,
		}
	}

	/// Adds a field whose value identifies the row.
	pub fn lookup<T, V>(self, field: FieldRef<C::Model, T>, value: V) -> Self
	where
		T: DatabaseField,
		V: IntoFieldValue<T>,
	{
		self.push_assignment(
			TypedAssignment::new(field, value),
			GetAssignmentRole::Lookup,
		)
	}

	/// Adds a value used only when the row must be created.
	pub fn default<T, V>(self, field: FieldRef<C::Model, T>, value: V) -> Self
	where
		T: DatabaseField,
		V: IntoFieldValue<T>,
	{
		self.push_assignment(
			TypedAssignment::new(field, value),
			GetAssignmentRole::Default,
		)
	}

	/// Executes through the configured global ORM connection.
	///
	/// Builder encoding and plan validation finish before a connection is acquired.
	pub async fn execute(self) -> Result<(C::Model, bool)> {
		let (manager, plan) = self.into_plan()?;
		let mut executor = crate::orm::manager::get_connection().await?;
		execute_get_or_create(&manager, plan, &mut executor).await
	}

	/// Executes through a caller-owned ORM executor.
	pub async fn execute_with<E>(self, executor: &mut E) -> Result<(C::Model, bool)>
	where
		E: OrmExecutor + ?Sized,
	{
		let (manager, plan) = self.into_plan()?;
		execute_get_or_create(&manager, plan, executor).await
	}

	fn assignments_mut(&mut self, role: GetAssignmentRole) -> &mut Vec<TypedAssignment<C::Model>> {
		match role {
			GetAssignmentRole::Lookup => &mut self.lookup,
			GetAssignmentRole::Default => &mut self.defaults,
		}
	}

	fn push_assignment(
		mut self,
		result: Result<TypedAssignment<C::Model>>,
		role: GetAssignmentRole,
	) -> Self {
		match result {
			Ok(value) if self.error.is_none() => self.assignments_mut(role).push(value),
			Err(error) if self.error.is_none() => self.error = Some(error),
			_ => {}
		}
		self
	}

	fn into_plan(self) -> Result<(C, plan::UpsertPlan<C::Model>)> {
		if let Some(error) = self.error {
			return Err(error);
		}
		let plan = normalize(
			self.lookup,
			self.defaults,
			Vec::new(),
			UpsertMode::GetOrCreate,
		)?;
		Ok((self.manager, plan))
	}
}
