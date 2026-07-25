#![deny(unexpected_cfgs)]

use reinhardt::model;
use reinhardt_core::model_form::{AllEditableModelFields, ModelFormFieldKind, ModelFormSchema};
use serde::{Deserialize, Serialize};

#[model(app_label = "projects", table_name = "projects", info = false)]
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Project {
	#[field(primary_key = true, max_length = 64)]
	pub id: String,

	#[field(max_length = 120)]
	pub name: String,
}

#[model(app_label = "jobs", table_name = "jobs", form = true, info = false)]
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Job {
	#[field(primary_key = true)]
	pub id: i64,

	#[rel(foreign_key)]
	pub project: reinhardt::db::associations::ForeignKeyField<Project>,

	#[field(max_length = 120)]
	pub job_type: String,
}

#[model(app_label = "forms", table_name = "forms", form = true, info = false)]
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct FormProject {
	#[field(primary_key = true)]
	pub id: i64,

	#[field(max_length = 120)]
	pub title: String,
}

pub fn retry_preserves_project(job: &Job, retry: &Job) -> bool {
	job.project_id() == retry.project_id()
}

pub fn accepts_foreign_key_id(job: &Job) -> String {
	job.project_id()
}

pub fn foreign_key_form_kind_is_text() -> bool {
	matches!(
		JobFormSchema::project_id().kind,
		ModelFormFieldKind::Text {
			max_length: Some(64),
			multiline: false,
		}
	)
}

pub fn model_form_schema_fields() -> usize {
	FormProjectFormSchema::fields().len()
}

pub fn model_form_payload_has_title() -> bool {
	let mut payload = FormProjectModelFormData::<AllEditableModelFields>::empty();
	payload.set_title("draft".to_owned());
	payload.title().is_some_and(|title| title == "draft")
}
