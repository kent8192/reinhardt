#[path = "../support.rs"]
mod support;

use reinhardt_db::orm::{AggregateKind, HavingPredicate, TypedExpression, TypedPredicate, func};
use support::ModelRecord;

fn main() {
	let _: TypedExpression<ModelRecord, i64, AggregateKind> = func::count_all();
	let _: TypedExpression<ModelRecord, i64, AggregateKind> =
		func::count(ModelRecord::field_i32()).distinct();
	let _: TypedExpression<ModelRecord, i64, AggregateKind> =
		func::sum(ModelRecord::field_i32()).distinct();
	let _: TypedExpression<ModelRecord, f64, AggregateKind> = func::avg(ModelRecord::field_i32());
	let _: TypedExpression<ModelRecord, i64, AggregateKind> = func::min(ModelRecord::field_i64());
	let _: TypedExpression<ModelRecord, i64, AggregateKind> = func::max(ModelRecord::field_i64());
	let _: TypedExpression<ModelRecord, f64, AggregateKind> = func::sum(ModelRecord::field_f32());
	let _: TypedExpression<ModelRecord, f64, AggregateKind> = func::avg(ModelRecord::field_f32());
	let _: TypedExpression<ModelRecord, f64, AggregateKind> = func::sum(ModelRecord::field_f64());
	let _: TypedExpression<ModelRecord, f64, AggregateKind> = func::avg(ModelRecord::field_f64());
	let _: TypedExpression<ModelRecord, rust_decimal::Decimal, AggregateKind> =
		func::sum(ModelRecord::field_decimal());
	let _: TypedExpression<ModelRecord, rust_decimal::Decimal, AggregateKind> =
		func::avg(ModelRecord::field_decimal());
	let _: TypedExpression<ModelRecord, i64, AggregateKind> =
		func::sum(ModelRecord::field_optional_i64());
	let _: TypedExpression<ModelRecord, f64, AggregateKind> =
		func::avg(ModelRecord::field_optional_i64());

	let scalar: TypedExpression<ModelRecord, i64> = ModelRecord::field_i64().into();
	let _: TypedPredicate<ModelRecord> = scalar.eq(7_i64);
	let _: HavingPredicate<ModelRecord> = func::sum(ModelRecord::field_i32()).gt(7_i64);
}
