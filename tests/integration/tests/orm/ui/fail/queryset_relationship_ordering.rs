// The model macro emits native-only cfgs evaluated in this standalone trybuild crate.
#![allow(unexpected_cfgs)]
//! Fail case: virtual relationship fields cannot prove SQL ordering validity.

use reinhardt::db::associations::ForeignKeyField;
use reinhardt::db::orm::OrderingField;
use reinhardt::model;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(app_label = "projects", table_name = "projects")]
struct Project {
	#[field(primary_key = true)]
	id: i64,
}

#[model(app_label = "jobs", table_name = "jobs")]
#[derive(Serialize, Deserialize)]
struct Job {
	#[field(primary_key = true)]
	id: i64,
	#[rel(foreign_key)]
	project: ForeignKeyField<Project>,
}

fn accepts_job_ordering(_: &[OrderingField<Job>]) {}

fn main() {
	accepts_job_ordering(&[Job::field_project().ordering()]);
}
