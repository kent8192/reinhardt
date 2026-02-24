//! Page components
//!
//! This module re-exports page-level components for the polling application.
//! Each page function returns a View that can be rendered.

use reinhardt::pages::component::View;

/// Index page - List all polls
pub fn index_page() -> View {
	crate::client::components::polls::polls_index()
}

/// Poll detail page - Show question and voting form
pub fn polls_detail_page(question_id: i64) -> View {
	crate::client::components::polls::polls_detail(question_id)
}

/// Poll results page - Show voting results
pub fn polls_results_page(question_id: i64) -> View {
	crate::client::components::polls::polls_results(question_id)
}
