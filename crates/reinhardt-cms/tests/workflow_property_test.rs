//! Property-based tests for workflow engine

use proptest::prelude::*;
use reinhardt_cms::error::CmsError;
use reinhardt_cms::workflow::{PageState, WorkflowEngine, WorkflowTransition};
use uuid::Uuid;

/// Helper to get a page to a specific state via valid transitions
async fn set_page_state(
	engine: &mut WorkflowEngine,
	page_id: Uuid,
	user_id: Uuid,
	target: PageState,
) {
	match target {
		PageState::Draft => {} // Default state, no transitions needed
		PageState::InReview => {
			engine
				.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
				.await
				.unwrap();
		}
		PageState::Approved => {
			engine
				.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
				.await
				.unwrap();
			engine
				.transition(page_id, WorkflowTransition::Approve, user_id)
				.await
				.unwrap();
		}
		PageState::Published => {
			engine
				.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
				.await
				.unwrap();
			engine
				.transition(page_id, WorkflowTransition::Approve, user_id)
				.await
				.unwrap();
			engine
				.transition(page_id, WorkflowTransition::Publish, user_id)
				.await
				.unwrap();
		}
		PageState::Rejected => {
			engine
				.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
				.await
				.unwrap();
			engine
				.transition(page_id, WorkflowTransition::Reject, user_id)
				.await
				.unwrap();
		}
		PageState::Archived => {
			engine
				.transition(page_id, WorkflowTransition::Archive, user_id)
				.await
				.unwrap();
		}
	}
}

const ALL_STATES: [PageState; 6] = [
	PageState::Draft,
	PageState::InReview,
	PageState::Approved,
	PageState::Published,
	PageState::Rejected,
	PageState::Archived,
];

const ALL_TRANSITIONS: [WorkflowTransition; 7] = [
	WorkflowTransition::SubmitForReview,
	WorkflowTransition::Approve,
	WorkflowTransition::Reject,
	WorkflowTransition::Publish,
	WorkflowTransition::Unpublish,
	WorkflowTransition::Archive,
	WorkflowTransition::Restore,
];

proptest! {
	#[test]
	fn prop_version_numbers_are_sequential(count in 1u32..=50) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			// Arrange
			let mut engine = WorkflowEngine::new();
			let page_id = Uuid::new_v4();
			let author_id = Uuid::new_v4();

			// Act
			for i in 1..=count {
				let content = serde_json::json!({"version": i});
				engine
					.create_version(page_id, author_id, content, None)
					.await
					.unwrap();
			}

			// Assert
			let versions = engine.get_versions(page_id).await.unwrap();
			assert_eq!(versions.len() as u32, count);
			for (i, version) in versions.iter().enumerate() {
				assert_eq!(version.version_number, (i as u32) + 1);
			}
		});
	}

	#[test]
	fn prop_invalid_transition_does_not_change_state(
		state_idx in 0usize..6,
		transition_idx in 0usize..7,
	) {
		let state = ALL_STATES[state_idx];
		let transition = ALL_TRANSITIONS[transition_idx];
		let engine_check = WorkflowEngine::new();

		// Only test invalid transitions
		if !engine_check.is_valid_transition(state, transition) {
			let rt = tokio::runtime::Runtime::new().unwrap();
			rt.block_on(async {
				// Arrange
				let mut engine = WorkflowEngine::new();
				let page_id = Uuid::new_v4();
				let user_id = Uuid::new_v4();
				set_page_state(&mut engine, page_id, user_id, state).await;

				// Act
				let result = engine.transition(page_id, transition, user_id).await;

				// Assert
				let err = result.unwrap_err();
				assert!(matches!(err, CmsError::InvalidWorkflowTransition(_)));
				let current_state = engine.get_state(page_id).await.unwrap();
				assert_eq!(current_state, state);
			});
		}
	}

	#[test]
	fn fuzz_workflow_random_transitions(
		transition_indices in prop::collection::vec(0usize..7, 1..100),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			// Arrange
			let mut engine = WorkflowEngine::new();
			let page_id = Uuid::new_v4();
			let user_id = Uuid::new_v4();

			// Act - Apply random transitions, ignoring errors
			for &idx in &transition_indices {
				let _ = engine.transition(page_id, ALL_TRANSITIONS[idx], user_id).await;
			}

			// Assert - Engine is in a valid state and does not panic
			let state = engine.get_state(page_id).await.unwrap();
			assert!(matches!(
				state,
				PageState::Draft
					| PageState::InReview
					| PageState::Approved
					| PageState::Published
					| PageState::Rejected
					| PageState::Archived
			));
		});
	}
}
