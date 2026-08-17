use super::{VerificationFinding, VerificationRun, redacted_settings_path};
use crate::CommandResult;
use reinhardt_conf::settings::schema::{JsonKind, SettingsViolationKind};
use reinhardt_db::migrations::SchemaFinding;
use serde::Serialize;
use std::io::Write;

/// Version of the machine-readable verification report schema.
pub const VERIFICATION_REPORT_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Overall result of a verification run.
pub enum VerificationStatusV1 {
	/// No contract violations were found.
	Passed,
	/// Contract violations were found.
	Failed,
	/// Verification could not safely complete.
	Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Stable category of a verification violation.
pub enum VerificationClassV1 {
	/// Schema or migration contract violation.
	Schema,
	/// Endpoint authorization contract violation.
	Authorization,
	/// Settings contract violation.
	Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Severity assigned to a verification violation.
pub enum VerificationSeverityV1 {
	/// The violation prevents a passing verification result.
	Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
/// Redacted target associated with a verification violation.
pub enum VerificationTargetV1 {
	/// Model state change that requires a migration.
	ModelChange {
		/// Application owning the model change.
		app_label: String,
		/// Stable migration-name fragment for the change.
		name_fragment: String,
	},
	/// Migration missing from applied history.
	Migration {
		/// Application owning the migration.
		app_label: String,
		/// Migration name.
		migration_name: String,
	},
	/// Endpoint lacking an explicit authorization declaration.
	Endpoint {
		/// HTTP method.
		method: String,
		/// Route path.
		path: String,
		/// Endpoint module path.
		module_path: String,
		/// Endpoint function name.
		function_name: String,
	},
	/// Settings path with dynamic values redacted.
	Setting {
		/// Canonical, redacted settings path.
		path: String,
	},
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
/// Machine-readable version-one verification violation.
pub struct VerificationViolationV1 {
	/// Stable violation code.
	pub code: String,
	/// Violation category.
	pub class: VerificationClassV1,
	/// Violation severity.
	pub severity: VerificationSeverityV1,
	/// Redacted violation target.
	pub target: VerificationTargetV1,
	/// Optional source location.
	pub location: Option<String>,
	/// Human-readable evidence for the violation.
	pub evidence: String,
	/// Optional remediation guidance.
	pub suggested_fix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
/// Machine-readable version-one verification report.
pub struct VerificationReportV1 {
	/// Report schema version.
	pub schema_version: u8,
	/// Overall verification status.
	pub status: VerificationStatusV1,
	/// Canonically ordered violations.
	pub violations: Vec<VerificationViolationV1>,
}

impl VerificationReportV1 {
	/// Project a verification run into the stable version-one report schema.
	pub fn from_run(run: &VerificationRun) -> Self {
		if !run.check_errors.is_empty() {
			return Self::error();
		}
		let mut run = run.clone();
		run.sort_canonical();
		let status = if run.findings.is_empty() {
			VerificationStatusV1::Passed
		} else {
			VerificationStatusV1::Failed
		};
		Self {
			schema_version: VERIFICATION_REPORT_SCHEMA_VERSION,
			status,
			violations: run.findings.into_iter().map(violation_from).collect(),
		}
	}

	/// Build an error report that does not expose partial findings.
	pub fn error() -> Self {
		Self {
			schema_version: VERIFICATION_REPORT_SCHEMA_VERSION,
			status: VerificationStatusV1::Error,
			violations: Vec::new(),
		}
	}

	/// Serialize the report as pretty JSON terminated by a newline.
	pub fn write_json(&self, output: &mut dyn Write) -> CommandResult<()> {
		let mut document = serde_json::to_vec_pretty(self)?;
		document.push(b'\n');
		output.write_all(&document)?;
		output.flush()?;
		Ok(())
	}
}

fn violation_from(finding: VerificationFinding) -> VerificationViolationV1 {
	let (code, class, target, evidence, suggested_fix) = match finding {
		VerificationFinding::Schema(SchemaFinding::MissingMigration {
			app_label,
			name_fragment,
			description,
		}) => (
			"schema.missing_migration".to_owned(),
			VerificationClassV1::Schema,
			VerificationTargetV1::ModelChange {
				app_label,
				name_fragment,
			},
			format!("Model state requires migration operation: {description}"),
			"Create a migration for the model change".to_owned(),
		),
		VerificationFinding::Schema(SchemaFinding::UnappliedMigration { migration }) => (
			"schema.unapplied_migration".to_owned(),
			VerificationClassV1::Schema,
			VerificationTargetV1::Migration {
				app_label: migration.app_label.clone(),
				migration_name: migration.name.clone(),
			},
			format!(
				"Migration {}.{} is not applied",
				migration.app_label, migration.name
			),
			"Apply the migration to the selected database".to_owned(),
		),
		VerificationFinding::Authorization(value) => (
			"authorization.missing_declaration".to_owned(),
			VerificationClassV1::Authorization,
			VerificationTargetV1::Endpoint {
				method: value.method,
				path: value.path,
				module_path: value.module_path,
				function_name: value.function_name,
			},
			"Endpoint has no explicit authentication declaration".to_owned(),
			"Declare the endpoint as protected, optional, or public".to_owned(),
		),
		VerificationFinding::Settings(value) => {
			let path = redacted_settings_path(&value.path);
			let actual = actual_json_kind(value.actual);
			let (evidence, suggested_fix) = match &value.kind {
				SettingsViolationKind::MissingRequired => (
					format!("Required setting {path} is absent"),
					"Provide the required setting".to_owned(),
				),
				SettingsViolationKind::TypeMismatch => (
					format!(
						"Setting {path} expects {} but received {actual}",
						value.expected
					),
					"Provide a value matching the declared setting type".to_owned(),
				),
				SettingsViolationKind::MapKeyTypeMismatch => (
					format!(
						"Map key at {path} expects {} but received {actual}",
						value.expected
					),
					"Use map keys matching the declared key type".to_owned(),
				),
				SettingsViolationKind::DuplicateInput => (
					format!("Setting {path} received multiple accepted input keys"),
					"Provide exactly one accepted input key for the setting".to_owned(),
				),
			};
			(
				value.kind.code().to_owned(),
				VerificationClassV1::Settings,
				VerificationTargetV1::Setting { path },
				evidence,
				suggested_fix,
			)
		}
	};
	VerificationViolationV1 {
		code,
		class,
		severity: VerificationSeverityV1::Error,
		target,
		location: None,
		evidence,
		suggested_fix: Some(suggested_fix),
	}
}

fn actual_json_kind(kind: Option<JsonKind>) -> &'static str {
	match kind {
		None => "missing",
		Some(JsonKind::Null) => "null",
		Some(JsonKind::Boolean) => "boolean",
		Some(JsonKind::Number) => "number",
		Some(JsonKind::String) => "string",
		Some(JsonKind::Sequence) => "sequence",
		Some(JsonKind::Map) => "map",
	}
}
