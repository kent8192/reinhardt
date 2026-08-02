use reinhardt_db::orm::DatabaseConnection;
use reinhardt_pages_macros::server_fn;

fn consume(_db: DatabaseConnection) {}

#[server_fn]
async fn copy_database_connection(
	#[inject] db: DatabaseConnection,
) -> Result<(), reinhardt_pages::server_fn::ServerFnError> {
	let first = move || consume(db);
	let second = move || consume(db);
	first();
	second();
	consume(db);
	Ok(())
}

fn main() {}
