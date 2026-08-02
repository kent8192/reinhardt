use reinhardt_pages::prelude::*;
use reinhardt_pages::ClientLauncher;

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
	let _launcher = ClientLauncher::new("#root").query_defaults(QueryDefaults::new());
	let _: Page = prelude_page();
}
