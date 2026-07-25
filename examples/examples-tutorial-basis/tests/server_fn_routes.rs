//! Native route-registration regression tests for the tutorial apps.

#![cfg(server)]

#[cfg(with_reinhardt)]
mod tests {
	use examples_tutorial_basis::apps::polls::{
		server_fn::{
			create_choice, create_question, delete_choice, delete_question, get_question_detail,
			get_question_results, get_questions, submit_vote, update_choice, update_question, vote,
		},
		urls as polls_urls,
	};
	use examples_tutorial_basis::apps::users::{
		server_fn::{current_user, login, logout, register},
		urls as users_urls,
	};
	use examples_tutorial_basis::config::urls::routes;
	use reinhardt::ServerRouter;
	use reinhardt::pages::server_fn::ServerFnMetadata;
	use std::collections::BTreeSet;

	fn registered_paths(router: ServerRouter) -> BTreeSet<String> {
		router
			.registered_endpoints()
			.into_iter()
			.map(|endpoint| endpoint.path)
			.collect()
	}

	#[test]
	fn routes_collects_each_apps_server_functions_without_cross_app_leakage() {
		// Arrange
		let polls_paths = BTreeSet::from([
			get_questions::marker::PATH.to_string(),
			get_question_detail::marker::PATH.to_string(),
			get_question_results::marker::PATH.to_string(),
			vote::marker::PATH.to_string(),
			submit_vote::marker::PATH.to_string(),
			create_question::marker::PATH.to_string(),
			update_question::marker::PATH.to_string(),
			delete_question::marker::PATH.to_string(),
			create_choice::marker::PATH.to_string(),
			update_choice::marker::PATH.to_string(),
			delete_choice::marker::PATH.to_string(),
		]);
		let users_paths = BTreeSet::from([
			login::marker::PATH.to_string(),
			register::marker::PATH.to_string(),
			logout::marker::PATH.to_string(),
			current_user::marker::PATH.to_string(),
		]);

		// Act
		let project_router = routes();
		let project_server_router = project_router.server_ref();
		let project_paths = project_server_router
			.get_all_routes()
			.into_iter()
			.map(|(path, _, _, _)| path)
			.collect::<BTreeSet<_>>();
		let registered_polls_paths = registered_paths(polls_urls::server_url_patterns());
		let registered_users_paths = registered_paths(users_urls::server_url_patterns());

		// Assert
		assert_eq!(registered_polls_paths, polls_paths);
		assert_eq!(registered_users_paths, users_paths);
		assert!(registered_polls_paths.is_disjoint(&registered_users_paths));
		for path in registered_polls_paths.union(&registered_users_paths) {
			assert!(
				project_paths.contains(path),
				"project routes() must mount server-function endpoint {path}"
			);
		}
	}
}
