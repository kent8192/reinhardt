//! Structured failures returned by native model-backed forms.

use std::collections::HashMap;

use reinhardt_core::exception::DatabaseError;
use thiserror::Error;

/// A failure encountered while validating, constructing, or saving a model form.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModelFormError {
	/// A submitted wire field is excluded by the active form policy.
	#[error("model form field '{field}' is forbidden")]
	ForbiddenInput {
		/// The field rejected by the policy.
		field: &'static str,
	},
	/// One or more native form fields failed validation.
	#[error("model form field validation failed")]
	FieldValidation {
		/// Validation messages grouped by form field name.
		errors: HashMap<String, Vec<String>>,
	},
	/// A required model field could not be resolved while creating an instance.
	#[error("required model field '{field}' is missing")]
	MissingModelField {
		/// The unresolved model field.
		field: &'static str,
	},
	/// The candidate failed model-level validation.
	#[error("model validation failed")]
	ModelValidation {
		/// Model-level validation messages.
		errors: Vec<String>,
	},
	/// The database rejected the persistence operation.
	#[error("model form persistence failed: {source}")]
	Persistence {
		/// The structured database failure.
		source: DatabaseError,
	},
	/// The database inserted the record, but Reinhardt could not hydrate it.
	///
	/// Retrying the same create would duplicate the row. Reload the persisted
	/// record before attempting a subsequent update.
	#[error("model form create persisted but hydration failed: {source}")]
	PersistenceAfterCreate {
		/// The structured database failure.
		source: DatabaseError,
	},
	/// A caller-owned transaction has not finalized a prior form write yet.
	#[error("model form persistence is waiting for its transaction outcome")]
	TransactionOutcomePending,
}

impl ModelFormError {
	/// Returns the structured database failure retained by a persistence error.
	pub fn database_error(&self) -> Option<&DatabaseError> {
		match self {
			Self::Persistence { source } | Self::PersistenceAfterCreate { source } => Some(source),
			Self::TransactionOutcomePending => None,
			_ => None,
		}
	}
}
