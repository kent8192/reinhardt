// The fixture defines fields solely to make the generated field accessor fail,
// so the compiler cannot observe reads from every field.
#![allow(dead_code)]
// The model macro emits its documented custom cfg names while trybuild invokes
// rustc directly, without Cargo's check-cfg configuration.
#![allow(unexpected_cfgs)]

use chrono::NaiveDate;
use reinhardt_core::macros::model;
use reinhardt_db::orm::{DateProjectionOrder, DateTruncKind, Model};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(app_label = "default", table_name = "projection_events")]
struct ProjectionEvent {
	#[field(primary_key = true)]
	id: i64,
	event_date: NaiveDate,
	#[field(max_length = 255)]
	title: String,
}

async fn rejects_non_date_field() -> reinhardt_core::exception::Result<()> {
	let _ = ProjectionEvent::objects()
		.dates(
			ProjectionEvent::field_title(),
			DateTruncKind::Day,
			DateProjectionOrder::Asc,
		)
		.await?;
	Ok(())
}

fn main() {}
