#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::{AggregateKind, TypedExpression, func};
use support::ModelRecord;

fn main() {
	let _: TypedExpression<ModelRecord, i64, AggregateKind> =
		func::sum(ModelRecord::field_custom_amount());
	let _: TypedExpression<ModelRecord, f64, AggregateKind> =
		func::avg(ModelRecord::field_custom_amount());
}
