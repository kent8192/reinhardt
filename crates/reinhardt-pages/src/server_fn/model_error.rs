use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind};
use reinhardt_db::{backends::error::database_table_matches_model, orm::Model};

use super::ServerFnError;

enum ValidationTarget {
	Field,
	Form,
}

struct ResolvedViolation {
	fields: Vec<String>,
	target: ValidationTarget,
	default_message: &'static str,
}

impl ServerFnError {
	/// Converts a proven database constraint violation into a safe validation error.
	pub fn try_from_model_error<M>(
		error: reinhardt_core::exception::Error,
	) -> Result<Self, reinhardt_core::exception::Error>
	where
		M: Model,
	{
		Self::try_from_model_error_with::<M, _>(error, |_, _| None)
	}

	/// Converts a proven database constraint violation using an optional safe message.
	pub fn try_from_model_error_with<M, F>(
		error: reinhardt_core::exception::Error,
		message: F,
	) -> Result<Self, reinhardt_core::exception::Error>
	where
		M: Model,
		F: FnOnce(&DatabaseError, &[&str]) -> Option<String>,
	{
		let resolved = {
			let Some(database_error) = error.database_error() else {
				return Err(error);
			};
			let Some(resolved) = resolve_model_violation::<M>(database_error) else {
				return Err(error);
			};
			resolved
		};

		let field_refs = resolved
			.fields
			.iter()
			.map(String::as_str)
			.collect::<Vec<_>>();
		let safe_message = message(
			error
				.database_error()
				.expect("resolved model violations always retain their database error"),
			&field_refs,
		)
		.unwrap_or_else(|| resolved.default_message.to_owned());

		match resolved.target {
			ValidationTarget::Field => Ok(Self::validation_with_message(
				safe_message.clone(),
				[(resolved.fields[0].clone(), safe_message)],
			)),
			ValidationTarget::Form => Ok(Self::validation_with_message(
				safe_message,
				std::iter::empty::<(String, String)>(),
			)),
		}
	}
}

fn resolve_model_violation<M: Model>(database_error: &DatabaseError) -> Option<ResolvedViolation> {
	if let Some(table) = database_error.table()
		&& !database_table_matches_model(M::table_name(), table)
	{
		return None;
	}

	let constraint_fields = match database_error.constraint() {
		Some(constraint) => Some(normalize_fields(M::constraint_fields(constraint)?)?),
		None => None,
	};
	let column_fields = if database_error.columns().is_empty() {
		None
	} else {
		Some(resolve_columns::<M>(database_error.columns())?)
	};

	if let (Some(constraint_fields), Some(column_fields)) = (&constraint_fields, &column_fields)
		&& constraint_fields != column_fields
	{
		return None;
	}

	match database_error.kind() {
		DatabaseErrorKind::UniqueViolation => {
			let fields = constraint_fields.as_ref().or(column_fields.as_ref())?;
			match fields.len() {
				1 => field(fields.clone(), "A record with this value already exists"),
				2.. if constraint_fields.is_some() => Some(form(
					fields.clone(),
					"A record with these values already exists",
				)),
				_ => None,
			}
		}
		DatabaseErrorKind::NotNullViolation => match column_fields.as_ref() {
			Some(fields) if fields.len() == 1 => field(fields.clone(), "This field is required"),
			_ => None,
		},
		DatabaseErrorKind::ForeignKeyViolation => match constraint_fields.as_ref() {
			Some(fields) if fields.len() == 1 => field(fields.clone(), "Select a valid value"),
			_ => None,
		},
		DatabaseErrorKind::CheckViolation => constraint_fields.as_ref().map(|fields| {
			form(
				fields.clone(),
				"The submitted values violate a data constraint",
			)
		}),
		_ => None,
	}
}

fn resolve_columns<M: Model>(columns: &[String]) -> Option<Vec<String>> {
	let metadata = M::field_metadata();
	let fields = columns
		.iter()
		.map(|column| {
			let mut matches = metadata
				.iter()
				.filter(|field| field.db_column_name() == column)
				.map(|field| field.name.clone());
			let field = matches.next()?;
			matches.next().is_none().then_some(field)
		})
		.collect::<Option<Vec<_>>>()?;
	normalize_fields(fields)
}

fn normalize_fields<I, S>(fields: I) -> Option<Vec<String>>
where
	I: IntoIterator<Item = S>,
	S: Into<String>,
{
	let mut fields = fields.into_iter().map(Into::into).collect::<Vec<_>>();
	fields.sort_unstable();
	fields
		.windows(2)
		.all(|pair| pair[0] != pair[1])
		.then_some(fields)
}

fn field(fields: Vec<String>, default_message: &'static str) -> Option<ResolvedViolation> {
	(fields.len() == 1).then_some(ResolvedViolation {
		fields,
		target: ValidationTarget::Field,
		default_message,
	})
}

fn form(fields: Vec<String>, default_message: &'static str) -> ResolvedViolation {
	ResolvedViolation {
		fields,
		target: ValidationTarget::Form,
		default_message,
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
	use reinhardt_db::orm::{FieldSelector, Manager, Model, inspection::FieldInfo};
	use serde::{Deserialize, Serialize};

	use crate::server_fn::{ServerFnError, ServerFnErrorKind};

	#[derive(Clone)]
	struct RecordFields;

	impl FieldSelector for RecordFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	#[derive(Clone, Debug, Deserialize, Serialize)]
	struct Record {
		id: Option<i64>,
		email: String,
		tenant_id: i64,
		owner_id: i64,
		status: String,
	}

	impl Model for Record {
		type PrimaryKey = i64;
		type Fields = RecordFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"records"
		}

		fn new_fields() -> Self::Fields {
			RecordFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn field_metadata() -> Vec<FieldInfo> {
			vec![
				field("email", "email_addr"),
				field("tenant_id", "tenant_id"),
				field("owner_id", "owner_id"),
				field("status", "status"),
			]
		}

		fn constraint_fields(constraint: &str) -> Option<Vec<&'static str>> {
			match constraint {
				"records_email_key" => Some(vec!["email"]),
				"records_email_tenant_key" => Some(vec!["email", "tenant_id"]),
				"records_owner_fkey" => Some(vec!["owner_id"]),
				"records_status_check" => Some(vec!["status"]),
				_ => None,
			}
		}
	}

	fn field(name: &str, db_column: &str) -> FieldInfo {
		FieldInfo {
			name: name.to_owned(),
			field_type: "CharField".to_owned(),
			storage_kind: None,
			domain: None,
			nullable: false,
			primary_key: false,
			unique: false,
			blank: false,
			editable: true,
			default: None,
			db_default: None,
			db_column: (name != db_column).then(|| db_column.to_owned()),
			choices: None,
			attributes: HashMap::new(),
		}
	}

	fn database_error(
		kind: DatabaseErrorKind,
		constraint: Option<&str>,
		columns: &[&str],
	) -> Error {
		let mut error = DatabaseError::new(
			kind,
			"private driver message containing rejected@example.com",
		)
		.with_code("PRIVATE")
		.with_table("records")
		.with_columns(columns.iter().copied());
		if let Some(constraint) = constraint {
			error = error.with_constraint(constraint);
		}
		Error::from(error.with_source(std::io::Error::other("private source")))
	}

	fn assert_field_error(error: ServerFnError, field: &str, message: &str) {
		assert_eq!(error.kind(), ServerFnErrorKind::Validation);
		assert_eq!(error.status(), Some(422));
		assert_eq!(error.user_message(), message);
		assert_eq!(error.field_errors().len(), 1);
		assert_eq!(error.field_errors()[0].field(), field);
		assert_eq!(error.field_errors()[0].message(), message);
	}

	fn assert_unchanged(error: Error) {
		let message = error.to_string();
		let metadata = error.database_error().map(|database_error| {
			(
				database_error.kind(),
				database_error.code().map(str::to_owned),
				database_error.constraint().map(str::to_owned),
				database_error.table().map(str::to_owned),
				database_error.columns().to_vec(),
			)
		});

		let returned = ServerFnError::try_from_model_error::<Record>(error)
			.expect_err("unproven errors must remain unchanged");
		assert_eq!(returned.to_string(), message);
		assert_eq!(
			returned.database_error().map(|database_error| (
				database_error.kind(),
				database_error.code().map(str::to_owned),
				database_error.constraint().map(str::to_owned),
				database_error.table().map(str::to_owned),
				database_error.columns().to_vec(),
			)),
			metadata
		);
	}

	#[test]
	fn maps_exact_default_messages_and_targets() {
		assert_field_error(
			ServerFnError::try_from_model_error::<Record>(database_error(
				DatabaseErrorKind::UniqueViolation,
				Some("records_email_key"),
				&["email_addr"],
			))
			.expect("single UNIQUE maps to its field"),
			"email",
			"A record with this value already exists",
		);

		let composite = ServerFnError::try_from_model_error::<Record>(database_error(
			DatabaseErrorKind::UniqueViolation,
			Some("records_email_tenant_key"),
			&["tenant_id", "email_addr"],
		))
		.expect("known composite UNIQUE maps to form-level validation");
		assert_eq!(
			composite.user_message(),
			"A record with these values already exists"
		);
		assert_eq!(composite.field_errors(), []);

		assert_field_error(
			ServerFnError::try_from_model_error::<Record>(database_error(
				DatabaseErrorKind::NotNullViolation,
				None,
				&["email_addr"],
			))
			.expect("NOT NULL maps to its field"),
			"email",
			"This field is required",
		);
		assert_field_error(
			ServerFnError::try_from_model_error::<Record>(database_error(
				DatabaseErrorKind::ForeignKeyViolation,
				Some("records_owner_fkey"),
				&[],
			))
			.expect("known FK maps to its field"),
			"owner_id",
			"Select a valid value",
		);

		let check = ServerFnError::try_from_model_error::<Record>(database_error(
			DatabaseErrorKind::CheckViolation,
			Some("records_status_check"),
			&[],
		))
		.expect("known CHECK maps to form-level validation");
		assert_eq!(
			check.user_message(),
			"The submitted values violate a data constraint"
		);
		assert_eq!(check.field_errors(), []);
	}

	#[test]
	fn callback_uses_safe_override_or_default() {
		let overridden = ServerFnError::try_from_model_error_with::<Record, _>(
			database_error(
				DatabaseErrorKind::UniqueViolation,
				Some("records_email_key"),
				&["email_addr"],
			),
			|_, fields| {
				assert_eq!(fields, ["email"]);
				Some("Choose another email address".to_owned())
			},
		)
		.expect("callback runs only after safe field routing is resolved");
		assert_field_error(overridden, "email", "Choose another email address");

		let defaulted = ServerFnError::try_from_model_error_with::<Record, _>(
			database_error(
				DatabaseErrorKind::UniqueViolation,
				Some("records_email_key"),
				&["email_addr"],
			),
			|_, _| None,
		)
		.expect("missing callback message uses the safe default");
		assert_field_error(
			defaulted,
			"email",
			"A record with this value already exists",
		);
	}

	#[test]
	fn fails_closed_for_unproven_database_metadata() {
		for error in [
			database_error(
				DatabaseErrorKind::UniqueViolation,
				Some("records_unknown_key"),
				&["email_addr"],
			),
			database_error(
				DatabaseErrorKind::NotNullViolation,
				None,
				&["unknown_column"],
			),
			database_error(
				DatabaseErrorKind::UniqueViolation,
				Some("records_email_key"),
				&["tenant_id"],
			),
			Error::from(
				DatabaseError::new(DatabaseErrorKind::ForeignKeyViolation, "private")
					.with_table("other_records")
					.with_constraint("records_owner_fkey"),
			),
			Error::from(
				DatabaseError::new(DatabaseErrorKind::UniqueViolation, "private")
					.with_table("other_records")
					.with_constraint("records_email_key"),
			),
		] {
			assert_unchanged(error);
		}
	}

	#[test]
	fn preserves_non_database_errors_and_serializes_only_safe_details() {
		assert_unchanged(Error::Internal("private".to_owned()));

		let error = ServerFnError::try_from_model_error::<Record>(database_error(
			DatabaseErrorKind::UniqueViolation,
			Some("records_email_key"),
			&["email_addr"],
		))
		.expect("known violation converts to a safe error");
		let serialized = serde_json::to_string(&error).expect("server error serializes");
		assert!(!serialized.contains("private driver message"));
		assert!(!serialized.contains("rejected@example.com"));
		assert!(!serialized.contains("PRIVATE"));
		assert!(!serialized.contains("records"));
		let round_trip: ServerFnError =
			serde_json::from_str(&serialized).expect("server error deserializes");
		assert_eq!(round_trip, error);
	}
}
