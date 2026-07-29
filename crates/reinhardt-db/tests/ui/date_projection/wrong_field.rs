#![allow(dead_code, unexpected_cfgs)]

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
