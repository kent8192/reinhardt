#![allow(dead_code, unexpected_cfgs)]

use chrono::{DateTime, NaiveDate, Utc};
use reinhardt_core::macros::model;
use reinhardt_db::orm::{
	DateProjectionOrder, DateTimeTruncKind, DateTruncKind, Model,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(app_label = "default", table_name = "projection_events")]
struct ProjectionEvent {
	#[field(primary_key = true)]
	id: i64,
	event_date: Option<NaiveDate>,
	occurred_at: DateTime<Utc>,
}

async fn typed_date_projections() -> reinhardt_core::exception::Result<()> {
	let _dates = ProjectionEvent::objects()
		.dates(
			ProjectionEvent::field_event_date(),
			DateTruncKind::Week,
			DateProjectionOrder::Asc,
		)
		.await?;
	let _datetimes = ProjectionEvent::objects()
		.datetimes(
			ProjectionEvent::field_occurred_at(),
			DateTimeTruncKind::Hour,
			DateProjectionOrder::Desc,
			Some(chrono_tz::Asia::Tokyo),
		)
		.await?;
	Ok(())
}

fn main() {}
