use reinhardt_pages::reactive::{QueryFamily, QueryOptions, use_query};

#[test]
#[should_panic(expected = "use_query requires an active QueryClient")]
fn use_query_rejects_missing_application_context() {
	let family = QueryFamily::<(), String, String>::new("tests.no-client");
	let _query = use_query(
		family.query((), || async { Ok("value".to_string()) }),
		QueryOptions::default(),
	);
}
