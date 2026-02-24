//! URL configuration for examples-tutorial-basis project
//!
//! The `routes` function defines all URL patterns for this project.

use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::prelude::*;
use reinhardt::routes;

// Import server_fn marker modules (snake_case + ::marker)
use crate::server_fn::polls::{
	get_question_detail, get_question_results, get_questions, get_vote_form_metadata, submit_vote,
	vote,
};

#[routes]
pub fn routes() -> UnifiedRouter {
	// Register all server functions explicitly via .server() closure
	UnifiedRouter::new()
		.server(|s| {
			s.server_fn(get_questions::marker)
				.server_fn(get_question_detail::marker)
				.server_fn(get_question_results::marker)
				.server_fn(vote::marker)
				.server_fn(get_vote_form_metadata::marker)
				.server_fn(submit_vote::marker)
		})
		// Mount polls routes
		.mount("/polls/", crate::apps::polls::urls::routes())
}
