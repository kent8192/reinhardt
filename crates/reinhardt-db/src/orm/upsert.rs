//! Typed builders, assignment views, and normalized plans for atomic upserts.
//!
//! Generated model [`FieldRef`] accessors bind every assignment to its model
//! and database field type. Lookups must cover a primary key, a field declared
//! `unique = true`, or every field of an immediate, unconditional unique
//! constraint. A lookup field cannot also be a default or update assignment.
//!
//! # Example
//!
//! ```no_run
//! # // Permit cfg names emitted by the model macro in this isolated doctest.
//! # #![allow(unexpected_cfgs)]
//! # mod migrations { pub use reinhardt_db::migrations::*; }
//! # mod orm { pub use reinhardt_db::orm::*; }
//! use reinhardt_core::exception::Error;
//! use reinhardt_core::macros::model;
//! use reinhardt_db::orm::{CustomManager, Model};
//! use serde::{Deserialize, Serialize};
//!
//! #[model(app_label = "typed_upsert_docs", table_name = "typed_upsert_tags")]
//! #[derive(Serialize, Deserialize)]
//! struct Tag {
//!     #[field(primary_key = true)]
//!     id: Option<i64>,
//!     #[field(max_length = 64, unique = true)]
//!     slug: String,
//!     display_order: i32,
//! }
//!
//! # async fn example() -> Result<(), Error> {
//! let (tag, created) = Tag::objects()
//!     .get_or_create()
//!     .lookup(Tag::field_slug(), "rust")
//!     .default(Tag::field_display_order(), 10_i32)
//!     .execute()
//!     .await?;
//! # let _ = (tag, created);
//! # Ok(())
//! # }
//! # fn main() {}
//! ```

pub(crate) mod assignment;
pub(crate) mod execution;
pub(crate) mod plan;
pub(crate) mod sql;

use crate::orm::connection::OrmExecutor;
use crate::orm::custom_manager::CustomManager;
use crate::orm::expressions::FieldRef;
use crate::orm::field_codec::{DatabaseField, IntoFieldValue};
use crate::orm::transaction::AtomicTransaction;
use crate::orm::upsert::assignment::TypedAssignment;
use crate::orm::upsert::execution::{execute_get_or_create, execute_update_or_create};
use crate::orm::upsert::plan::{UpsertMode, normalize};
use reinhardt_core::exception::{Error, Result};

pub use assignment::{UpsertCreate, UpsertWrite};

/// Typed builder for retrieving an existing row or creating it atomically.
///
/// A successful insert returns `true`. If another invocation wins a concurrent
/// insert race, this builder reloads that row and returns `false`.
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
	///
	/// The complete lookup must cover supported immediate uniqueness.
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
	///
	/// A default cannot target a lookup field.
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
	///
	/// An autocommit connection is accepted directly. A caller-owned transaction
	/// must have been obtained from
	/// [`crate::orm::connection::DatabaseConnection::atomic_write`]; ordinary
	/// atomic transactions are rejected before the initial lookup.
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

/// Typed builder for updating a locked row or creating it atomically.
///
/// Existing rows are locked before their update values and manager hook are
/// applied. If another invocation wins a concurrent insert race, this builder
/// locks and updates the winner, then returns `false`.
///
/// # Example
///
/// ```no_run
/// # // Permit the `native` cfg emitted by the model macro in this isolated doctest.
/// # #![allow(unexpected_cfgs)]
/// # mod migrations { pub use reinhardt_db::migrations::*; }
/// # mod orm { pub use reinhardt_db::orm::*; }
/// use reinhardt_core::exception::Error;
/// use reinhardt_core::macros::model;
/// use reinhardt_db::orm::{CustomManager, Model};
/// use serde::{Deserialize, Serialize};
///
/// struct User {
///     id: i64,
/// }
///
/// #[model(app_label = "typed_upsert_docs", table_name = "typed_upsert_profiles")]
/// #[derive(Serialize, Deserialize)]
/// struct Profile {
///     #[field(primary_key = true)]
///     id: Option<i64>,
///     #[field(unique = true)]
///     user_id: i64,
///     last_seen: i64,
///     created_at: i64,
/// }
///
/// # async fn example() -> Result<(), Error> {
/// let user = User { id: 7 };
/// let now = 1_725_000_000_i64;
/// let (profile, created) = Profile::objects()
///     .update_or_create()
///     .lookup(Profile::field_user_id(), user.id)
///     .set(Profile::field_last_seen(), now)
///     .create_default(Profile::field_created_at(), now)
///     .execute()
///     .await?;
/// # let _ = (profile, created);
/// # Ok(())
/// # }
/// # fn main() {}
/// ```
pub struct UpdateOrCreateBuilder<C: CustomManager> {
	manager: C,
	lookup: Vec<TypedAssignment<C::Model>>,
	set: Vec<TypedAssignment<C::Model>>,
	create_defaults: Vec<TypedAssignment<C::Model>>,
	error: Option<Error>,
}

enum UpdateAssignmentRole {
	Lookup,
	Set,
	CreateDefault,
}

impl<C: CustomManager> UpdateOrCreateBuilder<C> {
	pub(crate) fn new(manager: C) -> Self {
		Self {
			manager,
			lookup: Vec::new(),
			set: Vec::new(),
			create_defaults: Vec::new(),
			error: None,
		}
	}

	/// Adds a field whose value identifies the row.
	///
	/// The complete lookup must cover supported immediate uniqueness.
	pub fn lookup<T, V>(self, field: FieldRef<C::Model, T>, value: V) -> Self
	where
		T: DatabaseField,
		V: IntoFieldValue<T>,
	{
		self.push_assignment(
			TypedAssignment::new(field, value),
			UpdateAssignmentRole::Lookup,
		)
	}

	/// Adds a value applied to both the update and create branches.
	///
	/// A set assignment cannot target a lookup field.
	pub fn set<T, V>(self, field: FieldRef<C::Model, T>, value: V) -> Self
	where
		T: DatabaseField,
		V: IntoFieldValue<T>,
	{
		self.push_assignment(
			TypedAssignment::new(field, value),
			UpdateAssignmentRole::Set,
		)
	}

	/// Adds a value used only by the create branch.
	///
	/// A create default cannot target a lookup field.
	pub fn create_default<T, V>(self, field: FieldRef<C::Model, T>, value: V) -> Self
	where
		T: DatabaseField,
		V: IntoFieldValue<T>,
	{
		self.push_assignment(
			TypedAssignment::new(field, value),
			UpdateAssignmentRole::CreateDefault,
		)
	}

	/// Executes through a write-intent transaction on the global connection.
	///
	/// On SQLite, the write-intent transaction issues `BEGIN IMMEDIATE` before
	/// the lookup. MySQL uses a `READ COMMITTED` write-intent transaction to
	/// avoid missing-row gap-lock deadlocks while unique constraints serialize
	/// competing inserts.
	pub async fn execute(self) -> Result<(C::Model, bool)> {
		let (manager, plan) = self.into_plan()?;
		let connection = crate::orm::manager::get_connection().await?;
		connection
			.atomic_write(async |transaction| {
				execute_update_or_create(&manager, plan, transaction).await
			})
			.await
	}

	/// Executes without finishing a caller-owned write-intent transaction.
	///
	/// Obtain the transaction from [`crate::orm::connection::DatabaseConnection::atomic_write`].
	/// An ordinary [`crate::orm::connection::DatabaseConnection::atomic`] transaction
	/// is rejected before any SQL is issued.
	pub async fn execute_with(
		self,
		transaction: &mut AtomicTransaction,
	) -> Result<(C::Model, bool)> {
		let (manager, plan) = self.into_plan()?;
		execute_update_or_create(&manager, plan, transaction).await
	}

	fn assignments_mut(
		&mut self,
		role: UpdateAssignmentRole,
	) -> &mut Vec<TypedAssignment<C::Model>> {
		match role {
			UpdateAssignmentRole::Lookup => &mut self.lookup,
			UpdateAssignmentRole::Set => &mut self.set,
			UpdateAssignmentRole::CreateDefault => &mut self.create_defaults,
		}
	}

	fn push_assignment(
		mut self,
		result: Result<TypedAssignment<C::Model>>,
		role: UpdateAssignmentRole,
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
			self.create_defaults,
			self.set,
			UpsertMode::UpdateOrCreate,
		)?;
		Ok((self.manager, plan))
	}
}
