use reinhardt_db::migrations::{IndexType, Operation};

fn main() {
	let _operation = Operation::CreateIndex {
		table: "users".to_string(),
		columns: vec!["email".to_string()],
		unique: false,
		index_type: Some(IndexType::BTree),
		where_clause: None,
		concurrently: false,
		expressions: None,
		mysql_options: None,
		operator_class: None,
	};
}
