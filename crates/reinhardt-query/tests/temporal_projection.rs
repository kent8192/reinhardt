use reinhardt_query::prelude::*;
use rstest::rstest;

fn projection_statement(
	kind: TemporalTruncKind,
	output: TemporalTruncOutput,
	time_zone: Option<TemporalTimeZone>,
	order: Order,
) -> SelectStatement {
	let source = Expr::col(match output {
		TemporalTruncOutput::Date => "occurred_on",
		TemporalTruncOutput::DateTime => "occurred_at",
	})
	.into_simple_expr();
	let projection = Func::temporal_trunc(source.clone(), kind, time_zone, output).unwrap();
	let mut statement = Query::select();
	statement
		.expr_as(projection.clone(), "value")
		.from("events")
		.and_where(source.is_not_null())
		.distinct()
		.order_by_expr(Expr::col("value"), order);
	statement.to_owned()
}

fn postgres_date_expr(kind: TemporalTruncKind) -> String {
	format!("DATE_TRUNC('{}', \"occurred_on\")::date", kind.as_str())
}

fn mysql_date_expr(kind: TemporalTruncKind) -> String {
	match kind {
		TemporalTruncKind::Year => {
			"CAST(DATE_FORMAT(`occurred_on`, '%Y-01-01') AS DATE)".to_string()
		}
		TemporalTruncKind::Month => {
			"CAST(DATE_FORMAT(`occurred_on`, '%Y-%m-01') AS DATE)".to_string()
		}
		TemporalTruncKind::Week => {
			"CAST(STR_TO_DATE(DATE_FORMAT(`occurred_on`, '%x-%v Monday'), '%x-%v %W') AS DATE)"
				.to_string()
		}
		TemporalTruncKind::Day => {
			"CAST(DATE_FORMAT(`occurred_on`, '%Y-%m-%d') AS DATE)".to_string()
		}
		_ => panic!("date matrix uses only date truncation kinds"),
	}
}

fn sqlite_date_expr(kind: TemporalTruncKind) -> String {
	match kind {
		TemporalTruncKind::Year => "DATE(\"occurred_on\", 'start of year')".to_string(),
		TemporalTruncKind::Month => "DATE(\"occurred_on\", 'start of month')".to_string(),
		TemporalTruncKind::Week => "DATE(\"occurred_on\", '-6 days', 'weekday 1')".to_string(),
		TemporalTruncKind::Day => "DATE(\"occurred_on\")".to_string(),
		_ => panic!("date matrix uses only date truncation kinds"),
	}
}

#[rstest]
fn date_projection_sql_covers_every_kind_and_order_for_all_backends() {
	let kinds = [
		TemporalTruncKind::Year,
		TemporalTruncKind::Month,
		TemporalTruncKind::Week,
		TemporalTruncKind::Day,
	];
	for kind in kinds {
		for order in [Order::Asc, Order::Desc] {
			let order_sql = match order {
				Order::Asc => "ASC",
				Order::Desc => "DESC",
			};
			let statement = projection_statement(kind, TemporalTruncOutput::Date, None, order);

			let (postgres_sql, postgres_values) = PostgresQueryBuilder
				.build_select_checked(&statement)
				.unwrap();
			let postgres_expr = postgres_date_expr(kind);
			assert_eq!(
				postgres_sql,
				format!(
					"SELECT DISTINCT {postgres_expr} AS \"value\" FROM \"events\" \
					 WHERE \"occurred_on\" IS NOT NULL ORDER BY \"value\" {order_sql}"
				)
			);
			assert_eq!(postgres_values.len(), 0);

			let (mysql_sql, mysql_values) =
				MySqlQueryBuilder.build_select_checked(&statement).unwrap();
			let mysql_expr = mysql_date_expr(kind);
			assert_eq!(
				mysql_sql,
				format!(
					"SELECT DISTINCT {mysql_expr} AS `value` FROM `events` \
					 WHERE `occurred_on` IS NOT NULL ORDER BY `value` {order_sql}"
				)
			);
			assert_eq!(mysql_values.len(), 0);

			let (sqlite_sql, sqlite_values) =
				SqliteQueryBuilder.build_select_checked(&statement).unwrap();
			let sqlite_expr = sqlite_date_expr(kind);
			assert_eq!(
				sqlite_sql,
				format!(
					"SELECT DISTINCT {sqlite_expr} AS \"value\" FROM \"events\" \
					 WHERE \"occurred_on\" IS NOT NULL ORDER BY \"value\" {order_sql}"
				)
			);
			assert_eq!(sqlite_values.len(), 0);
		}
	}
}

fn datetime_format(kind: TemporalTruncKind) -> &'static str {
	match kind {
		TemporalTruncKind::Year => "%Y-01-01 00:00:00",
		TemporalTruncKind::Month => "%Y-%m-01 00:00:00",
		TemporalTruncKind::Day => "%Y-%m-%d 00:00:00",
		TemporalTruncKind::Hour => "%Y-%m-%d %H:00:00",
		TemporalTruncKind::Minute => "%Y-%m-%d %H:%i:00",
		TemporalTruncKind::Second => "%Y-%m-%d %H:%i:%s",
		TemporalTruncKind::Week => panic!("week uses a Monday expression"),
	}
}

fn postgres_datetime_expr(kind: TemporalTruncKind) -> String {
	format!(
		"DATE_TRUNC('{}', \"occurred_at\" AT TIME ZONE $1) AT TIME ZONE $2",
		kind.as_str()
	)
}

fn mysql_datetime_expr(kind: TemporalTruncKind) -> String {
	if kind == TemporalTruncKind::Week {
		return concat!(
			"CAST(STR_TO_DATE(DATE_FORMAT(CONVERT_TZ(`occurred_at`, '+00:00', ?), ",
			"'%x-%v Monday'), '%x-%v %W') AS DATETIME)"
		)
		.to_string();
	}

	format!(
		"CAST(DATE_FORMAT(CONVERT_TZ(`occurred_at`, '+00:00', ?), '{}') AS DATETIME)",
		datetime_format(kind)
	)
}

fn sqlite_datetime_expr(kind: TemporalTruncKind) -> String {
	match kind {
		TemporalTruncKind::Year => "DATETIME(\"occurred_at\", 'start of year')".to_string(),
		TemporalTruncKind::Month => "DATETIME(\"occurred_at\", 'start of month')".to_string(),
		TemporalTruncKind::Week => {
			"DATETIME(\"occurred_at\", '-6 days', 'weekday 1', 'start of day')".to_string()
		}
		TemporalTruncKind::Day => "DATETIME(\"occurred_at\", 'start of day')".to_string(),
		TemporalTruncKind::Hour => {
			"DATETIME(strftime('%Y-%m-%d %H:00:00', \"occurred_at\"))".to_string()
		}
		TemporalTruncKind::Minute => {
			"DATETIME(strftime('%Y-%m-%d %H:%M:00', \"occurred_at\"))".to_string()
		}
		TemporalTruncKind::Second => {
			"DATETIME(strftime('%Y-%m-%d %H:%M:%S', \"occurred_at\"))".to_string()
		}
	}
}

#[rstest]
fn datetime_projection_sql_covers_every_kind_and_order_for_all_backends() {
	let kinds = [
		TemporalTruncKind::Year,
		TemporalTruncKind::Month,
		TemporalTruncKind::Week,
		TemporalTruncKind::Day,
		TemporalTruncKind::Hour,
		TemporalTruncKind::Minute,
		TemporalTruncKind::Second,
	];
	for kind in kinds {
		for order in [Order::Asc, Order::Desc] {
			let order_sql = match order {
				Order::Asc => "ASC",
				Order::Desc => "DESC",
			};
			let statement = projection_statement(
				kind,
				TemporalTruncOutput::DateTime,
				Some(TemporalTimeZone::Utc),
				order,
			);
			let (postgres_sql, postgres_values) = PostgresQueryBuilder
				.build_select_checked(&statement)
				.unwrap();
			assert_eq!(
				postgres_sql,
				format!(
					"SELECT DISTINCT {} AS \"value\" FROM \"events\" WHERE \"occurred_at\" IS NOT NULL ORDER BY \"value\" {order_sql}",
					postgres_datetime_expr(kind)
				)
			);
			assert_eq!(postgres_values.len(), 2);

			let (mysql_sql, mysql_values) =
				MySqlQueryBuilder.build_select_checked(&statement).unwrap();
			assert_eq!(
				mysql_sql,
				format!(
					"SELECT DISTINCT {} AS `value` FROM `events` WHERE `occurred_at` IS NOT NULL ORDER BY `value` {order_sql}",
					mysql_datetime_expr(kind)
				)
			);
			assert_eq!(mysql_values.len(), 1);

			let (sqlite_sql, sqlite_values) =
				SqliteQueryBuilder.build_select_checked(&statement).unwrap();
			assert_eq!(
				sqlite_sql,
				format!(
					"SELECT DISTINCT {} AS \"value\" FROM \"events\" WHERE \"occurred_at\" IS NOT NULL ORDER BY \"value\" {order_sql}",
					sqlite_datetime_expr(kind)
				)
			);
			assert_eq!(sqlite_values.len(), 0);
		}
	}
}

#[rstest]
fn named_time_zone_is_structural_and_capability_checked() {
	let statement = projection_statement(
		TemporalTruncKind::Hour,
		TemporalTruncOutput::DateTime,
		Some(TemporalTimeZone::Named("America/New_York".to_string())),
		Order::Asc,
	);
	let (postgres_sql, postgres_values) = PostgresQueryBuilder
		.build_select_checked(&statement)
		.unwrap();
	assert_eq!(postgres_values.len(), 2);
	assert_eq!(
		postgres_sql,
		concat!(
			"SELECT DISTINCT DATE_TRUNC('hour', \"occurred_at\" AT TIME ZONE $1) ",
			"AT TIME ZONE $2 AS \"value\" FROM \"events\" WHERE \"occurred_at\" ",
			"IS NOT NULL ORDER BY \"value\" ASC"
		)
	);

	let mysql_error = MySqlQueryBuilder
		.build_select_checked(&statement)
		.unwrap_err();
	assert_eq!(
		mysql_error.to_string(),
		"named time-zone conversion is not supported by the MySQL backend"
	);
	let sqlite_error = SqliteQueryBuilder
		.build_select_checked(&statement)
		.unwrap_err();
	assert_eq!(
		sqlite_error.to_string(),
		"named time-zone conversion is not supported by the SQLite backend"
	);
}

#[rstest]
fn temporal_time_zones_are_inlined_for_view_definitions() {
	let select = projection_statement(
		TemporalTruncKind::Hour,
		TemporalTruncOutput::DateTime,
		Some(TemporalTimeZone::Utc),
		Order::Asc,
	);
	let mut postgres_view = Query::create_view();
	postgres_view
		.name("hourly_events")
		.as_select(select.clone());
	let (postgres_sql, postgres_values) = postgres_view.build(PostgresQueryBuilder);
	assert_eq!(
		postgres_sql,
		concat!(
			"CREATE VIEW \"hourly_events\" AS SELECT DISTINCT DATE_TRUNC('hour', ",
			"\"occurred_at\" AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' AS \"value\" ",
			"FROM \"events\" WHERE \"occurred_at\" IS NOT NULL ORDER BY \"value\" ASC"
		)
	);
	assert_eq!(postgres_values.len(), 0);

	let mut mysql_view = Query::create_view();
	mysql_view.name("hourly_events").as_select(select);
	let (mysql_sql, mysql_values) = mysql_view.build(MySqlQueryBuilder);
	assert_eq!(
		mysql_sql,
		concat!(
			"CREATE VIEW `hourly_events` AS SELECT DISTINCT CAST(DATE_FORMAT(CONVERT_TZ(",
			"`occurred_at`, '+00:00', '+00:00'), '%Y-%m-%d %H:00:00') AS DATETIME) ",
			"AS `value` FROM `events` WHERE `occurred_at` IS NOT NULL ORDER BY `value` ASC"
		)
	);
	assert_eq!(mysql_values.len(), 0);
}
