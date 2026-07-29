use reinhardt_pages::prelude::*;

const JOBS: QueryFamily<u64, Vec<String>, String> = QueryFamily::new("ui.prelude.jobs");

#[client_page]
pub fn prelude_page() -> Page {
	let jobs = use_query(
		JOBS.query(42, || async { Ok(vec!["Index job".to_string()]) }),
		QueryOptions::new(),
	);
	let client: QueryClient = queries();
	client.invalidate(&JOBS.key(42));
	client.invalidate_family(JOBS);

	page!(|| {
		div { { jobs.data().unwrap_or_default().join(", ") } }
	})()
}

fn main() {
	let _: Page = prelude_page();
}
