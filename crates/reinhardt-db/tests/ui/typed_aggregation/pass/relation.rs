#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::{AggregateKind, TypedExpression, func};
use support::ModelRecord;

fn main() {
	let _: TypedExpression<ModelRecord, i64, AggregateKind> = func::count(ModelRecord::field_i64());
	let _: TypedExpression<ModelRecord, i64, AggregateKind> =
		func::count(ModelRecord::rel_related().field(RelatedRecord::field_i64()));
	let _: TypedExpression<ModelRecord, i64, AggregateKind> =
		func::count(ModelRecord::rel_related());
}
