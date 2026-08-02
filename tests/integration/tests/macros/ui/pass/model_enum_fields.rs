#![allow(unexpected_cfgs)]

use reinhardt::db::orm::expressions::{FieldRef, GeneratedModelField};
use reinhardt::{ModelEnum, model};
use serde::{Deserialize, Serialize};

#[derive(ModelEnum, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[model_enum(repr = "string")]
enum Status {
	#[model_enum(value = "queued")]
	Queued,
	#[model_enum(value = "running")]
	Running,
}

#[derive(ModelEnum, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[model_enum(repr = "i32")]
enum Priority {
	#[model_enum(value = 10)]
	Low,
	#[model_enum(value = 20)]
	High,
}

#[derive(ModelEnum, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[model_enum(repr = "string")]
enum Glyph {
	#[model_enum(value = "é")]
	AcuteE,
}

#[model(app_label = "jobs", table_name = "jobs")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Job {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 16)]
	status: Status,
	priority: Option<Priority>,
	#[field(skip = true)]
	transient_tags: Vec<String>,
}

#[model(app_label = "jobs", table_name = "glyphs")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct GlyphRecord {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 1)]
	glyph: Glyph,
}

fn main() {
	let _: FieldRef<Job, Status, GeneratedModelField> = Job::field_status();
	let _: FieldRef<Job, Option<Priority>, GeneratedModelField> = Job::field_priority();
	let _: FieldRef<GlyphRecord, Glyph, GeneratedModelField> = GlyphRecord::field_glyph();
}
