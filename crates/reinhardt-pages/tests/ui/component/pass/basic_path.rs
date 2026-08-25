use reinhardt_pages::{Page, Path, Query, component, page};

#[component("/users/{id}/", name = "user-detail")]
fn user_page(Path(id): Path<i64>, Query(logs): Query<Option<i64>>) -> Page {
	page!(|id: i64, logs: Option<i64>| {
		div { { format!("{id}:{logs:?}") } }
	})(id, logs)
}

fn main() {
	let _ = UserPageProps::builder().id(7).build();
	let _ = page!(|| {
		UserPage {
			id: 7,
		}
	})();
}
