use reinhardt_pages::{Path, Query, loader};

#[loader]
async fn project_tab_loader(
	Path(project_id): Path<i64>,
	Query(tab): Query<String>,
	Query(logs): Query<core::option::Option<i64>>,
) -> Result<String, String> {
	Ok(format!("{project_id}:{tab}:{logs:?}"))
}

fn main() {
	let _future = project_tab_loader(Path(42), Query("open".to_string()), Query(None));
}
