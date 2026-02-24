//! Integration tests for types module.

use super::*;
use rstest::rstest;

#[rstest]
fn test_types_integration() {
	// Create an alias
	let alias = Alias::new("my_table");

	// Use it as a table reference
	let table = TableRef::table(alias.clone());

	// Create a column reference
	let column = ColumnRef::table_column(alias.clone(), Alias::new("id"));

	// Verify the types are correct
	if let TableRef::Table(iden) = &table {
		assert_eq!(iden.to_string(), "my_table");
	}

	if let ColumnRef::TableColumn(tbl, col) = &column {
		assert_eq!(tbl.to_string(), "my_table");
		assert_eq!(col.to_string(), "id");
	}
}

#[rstest]
fn test_join_with_types() {
	// Create a join expression
	let join = JoinExpr::new(TableRef::table(Alias::new("posts")))
		.join_type(JoinType::LeftJoin)
		.on_columns(
			join::ColumnSpec::table_column(Alias::new("users"), Alias::new("id")),
			join::ColumnSpec::table_column(Alias::new("posts"), Alias::new("user_id")),
		);

	assert_eq!(join.join, JoinType::LeftJoin);
}

#[rstest]
fn test_order_with_types() {
	// Create an order expression
	let order = OrderExpr::new(Alias::new("created_at"))
		.order(Order::Desc)
		.nulls(NullOrdering::Last);

	assert_eq!(order.order, Order::Desc);
	assert_eq!(order.nulls, Some(NullOrdering::Last));
}
