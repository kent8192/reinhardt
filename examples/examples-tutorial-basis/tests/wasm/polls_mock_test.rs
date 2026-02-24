//! Polls Component WASM Tests with Mocking
//!
//! Layer 2 tests for polls components that interact with server functions.
//! These tests verify that actual polls components from `src/client/components/polls.rs`
//! render correctly and can interact with mocked server function responses.
//!
//! **Test Categories:**
//! - Pure rendering tests (no server_fn interaction)
//! - Structure validation tests (list elements, form elements)
//! - Mock infrastructure integration tests
//!
//! **Run with**: `cargo make wasm-test`

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// Import actual components from the application
use examples_tutorial_basis::client::components::polls::{
	polls_detail, polls_index, polls_results,
};
use examples_tutorial_basis::shared::types::{ChoiceInfo, QuestionInfo, VoteRequest};
use reinhardt::pages::component::View;
use reinhardt::pages::testing::{
	assert_server_fn_call_count, assert_server_fn_not_called, clear_mocks, mock_server_fn,
	mock_server_fn_error,
};

// ============================================================================
// Test Fixtures
// ============================================================================

/// Create a mock QuestionInfo for testing
fn mock_question() -> QuestionInfo {
	QuestionInfo {
		id: 1,
		question_text: "What is your favorite programming language?".to_string(),
		pub_date: chrono::Utc::now(),
	}
}

/// Create a mock list of QuestionInfo for testing
fn mock_questions_list() -> Vec<QuestionInfo> {
	vec![
		QuestionInfo {
			id: 1,
			question_text: "What is your favorite programming language?".to_string(),
			pub_date: chrono::Utc::now(),
		},
		QuestionInfo {
			id: 2,
			question_text: "Which web framework do you prefer?".to_string(),
			pub_date: chrono::Utc::now(),
		},
		QuestionInfo {
			id: 3,
			question_text: "What database do you use most?".to_string(),
			pub_date: chrono::Utc::now(),
		},
	]
}

/// Create a mock list of ChoiceInfo for testing
fn mock_choices() -> Vec<ChoiceInfo> {
	vec![
		ChoiceInfo {
			id: 1,
			question_id: 1,
			choice_text: "Rust".to_string(),
			votes: 42,
		},
		ChoiceInfo {
			id: 2,
			question_id: 1,
			choice_text: "Python".to_string(),
			votes: 30,
		},
		ChoiceInfo {
			id: 3,
			question_id: 1,
			choice_text: "JavaScript".to_string(),
			votes: 25,
		},
	]
}

/// Create a mock VoteRequest for testing
#[allow(dead_code)] // For future mock tests
fn mock_vote_request() -> VoteRequest {
	VoteRequest {
		question_id: 1,
		choice_id: 1,
	}
}

// ============================================================================
// Polls Index Rendering Tests
// ============================================================================

/// Test polls index renders as a View::Element
#[wasm_bindgen_test]
fn test_polls_index_renders() {
	let view = polls_index();
	assert!(matches!(view, View::Element(_)));
}

/// Test polls index contains title
#[wasm_bindgen_test]
fn test_polls_index_has_title() {
	let view = polls_index();

	if let View::Element(element) = view {
		let html = element.to_html();
		assert!(html.contains("Polls"), "Should have 'Polls' title");
		assert!(html.contains("<h1"), "Title should be in h1 tag");
	} else {
		panic!("Expected View::Element");
	}
}

/// Test polls index has container class
#[wasm_bindgen_test]
fn test_polls_index_has_container() {
	let view = polls_index();

	if let View::Element(element) = view {
		let html = element.to_html();
		assert!(
			html.contains("container"),
			"Should have Bootstrap container class"
		);
	} else {
		panic!("Expected View::Element");
	}
}

// ============================================================================
// Polls Detail Rendering Tests
// ============================================================================

/// Test polls detail renders as a View::Element
#[wasm_bindgen_test]
fn test_polls_detail_renders() {
	let view = polls_detail(1);
	assert!(matches!(view, View::Element(_)));
}

/// Test polls detail renders for different question IDs
#[wasm_bindgen_test]
fn test_polls_detail_different_ids() {
	// Test with various question IDs
	for id in [1, 2, 10, 100] {
		let view = polls_detail(id);
		assert!(
			matches!(view, View::Element(_)),
			"Should render for question_id = {}",
			id
		);
	}
}

// ============================================================================
// Polls Results Rendering Tests
// ============================================================================

/// Test polls results renders as a View::Element
#[wasm_bindgen_test]
fn test_polls_results_renders() {
	let view = polls_results(1);
	assert!(matches!(view, View::Element(_)));
}

/// Test polls results renders for different question IDs
#[wasm_bindgen_test]
fn test_polls_results_different_ids() {
	// Test with various question IDs
	for id in [1, 2, 10, 100] {
		let view = polls_results(id);
		assert!(
			matches!(view, View::Element(_)),
			"Should render for question_id = {}",
			id
		);
	}
}

// ============================================================================
// Mock Infrastructure Tests
// ============================================================================

/// Test mock infrastructure for get_questions endpoint
#[wasm_bindgen_test]
fn test_mock_get_questions_endpoint() {
	clear_mocks();

	let questions = mock_questions_list();
	mock_server_fn("/api/server_fn/get_questions", &questions);

	// Verify mock was registered (no calls yet)
	assert_server_fn_not_called("/api/server_fn/get_questions");
	assert_server_fn_call_count("/api/server_fn/get_questions", 0);

	clear_mocks();
}

/// Test mock infrastructure for get_question_detail endpoint
#[wasm_bindgen_test]
fn test_mock_get_question_detail_endpoint() {
	clear_mocks();

	let question = mock_question();
	let choices = mock_choices();
	mock_server_fn("/api/server_fn/get_question_detail", &(question, choices));

	assert_server_fn_not_called("/api/server_fn/get_question_detail");

	clear_mocks();
}

/// Test mock infrastructure for get_question_results endpoint
#[wasm_bindgen_test]
fn test_mock_get_question_results_endpoint() {
	clear_mocks();

	let question = mock_question();
	let choices = mock_choices();
	let total_votes = 97; // Sum of votes in mock_choices
	mock_server_fn(
		"/api/server_fn/get_question_results",
		&(question, choices, total_votes),
	);

	assert_server_fn_not_called("/api/server_fn/get_question_results");

	clear_mocks();
}

/// Test mock infrastructure for vote endpoint
#[wasm_bindgen_test]
fn test_mock_vote_endpoint() {
	clear_mocks();

	// Vote returns unit type on success
	mock_server_fn("/api/server_fn/vote", &());

	assert_server_fn_not_called("/api/server_fn/vote");

	clear_mocks();
}

/// Test mock error for polls endpoints
#[wasm_bindgen_test]
fn test_mock_polls_error_endpoints() {
	clear_mocks();

	// Mock various error scenarios
	mock_server_fn_error("/api/server_fn/get_questions", 500, "Internal server error");
	mock_server_fn_error(
		"/api/server_fn/get_question_detail",
		404,
		"Question not found",
	);
	mock_server_fn_error("/api/server_fn/vote", 400, "Invalid choice");

	// Verify none were called
	assert_server_fn_not_called("/api/server_fn/get_questions");
	assert_server_fn_not_called("/api/server_fn/get_question_detail");
	assert_server_fn_not_called("/api/server_fn/vote");

	clear_mocks();
}

// ============================================================================
// Shared Types Serialization Tests
// ============================================================================

/// Test QuestionInfo serialization
#[wasm_bindgen_test]
fn test_question_info_serialization() {
	let question = mock_question();

	let json = serde_json::to_string(&question).expect("Should serialize QuestionInfo");
	assert!(json.contains("favorite programming language"));
	assert!(json.contains("question_text"));
	assert!(json.contains("pub_date"));
}

/// Test QuestionInfo deserialization
#[wasm_bindgen_test]
fn test_question_info_deserialization() {
	let json = r#"{
        "id": 1,
        "question_text": "What is Rust?",
        "pub_date": "2025-01-01T12:00:00Z"
    }"#;

	let question: QuestionInfo =
		serde_json::from_str(json).expect("Should deserialize QuestionInfo");
	assert_eq!(question.id, 1);
	assert_eq!(question.question_text, "What is Rust?");
}

/// Test ChoiceInfo serialization
#[wasm_bindgen_test]
fn test_choice_info_serialization() {
	let choice = ChoiceInfo {
		id: 1,
		question_id: 1,
		choice_text: "Rust".to_string(),
		votes: 42,
	};

	let json = serde_json::to_string(&choice).expect("Should serialize ChoiceInfo");
	assert!(json.contains("Rust"));
	assert!(json.contains("42"));
	assert!(json.contains("question_id"));
}

/// Test ChoiceInfo deserialization
#[wasm_bindgen_test]
fn test_choice_info_deserialization() {
	let json = r#"{
        "id": 1,
        "question_id": 1,
        "choice_text": "Python",
        "votes": 30
    }"#;

	let choice: ChoiceInfo = serde_json::from_str(json).expect("Should deserialize ChoiceInfo");
	assert_eq!(choice.id, 1);
	assert_eq!(choice.choice_text, "Python");
	assert_eq!(choice.votes, 30);
}

/// Test VoteRequest serialization
#[wasm_bindgen_test]
fn test_vote_request_serialization() {
	let request = VoteRequest {
		question_id: 1,
		choice_id: 2,
	};

	let json = serde_json::to_string(&request).expect("Should serialize VoteRequest");
	assert!(json.contains("question_id"));
	assert!(json.contains("choice_id"));
}

/// Test QuestionInfo roundtrip serialization
#[wasm_bindgen_test]
fn test_question_info_roundtrip() {
	let original = mock_question();
	let json = serde_json::to_string(&original).expect("Should serialize");
	let deserialized: QuestionInfo = serde_json::from_str(&json).expect("Should deserialize");

	assert_eq!(original.id, deserialized.id);
	assert_eq!(original.question_text, deserialized.question_text);
}

/// Test ChoiceInfo roundtrip serialization
#[wasm_bindgen_test]
fn test_choice_info_roundtrip() {
	let original = mock_choices()[0].clone();
	let json = serde_json::to_string(&original).expect("Should serialize");
	let deserialized: ChoiceInfo = serde_json::from_str(&json).expect("Should deserialize");

	assert_eq!(original.id, deserialized.id);
	assert_eq!(original.choice_text, deserialized.choice_text);
	assert_eq!(original.votes, deserialized.votes);
}

/// Test questions list serialization
#[wasm_bindgen_test]
fn test_questions_list_serialization() {
	let questions = mock_questions_list();

	let json = serde_json::to_string(&questions).expect("Should serialize questions list");
	let deserialized: Vec<QuestionInfo> =
		serde_json::from_str(&json).expect("Should deserialize questions list");

	assert_eq!(questions.len(), deserialized.len());
	assert_eq!(questions[0].id, deserialized[0].id);
	assert_eq!(questions[1].question_text, deserialized[1].question_text);
}

/// Test choices list serialization
#[wasm_bindgen_test]
fn test_choices_list_serialization() {
	let choices = mock_choices();

	let json = serde_json::to_string(&choices).expect("Should serialize choices list");
	let deserialized: Vec<ChoiceInfo> =
		serde_json::from_str(&json).expect("Should deserialize choices list");

	assert_eq!(choices.len(), deserialized.len());
	assert_eq!(choices[0].choice_text, deserialized[0].choice_text);
}

// ============================================================================
// VoteRequest Tests
// ============================================================================

/// Test VoteRequest with various values
#[wasm_bindgen_test]
fn test_vote_request_various_values() {
	let requests = vec![
		VoteRequest {
			question_id: 1,
			choice_id: 1,
		},
		VoteRequest {
			question_id: 100,
			choice_id: 500,
		},
		VoteRequest {
			question_id: 0,
			choice_id: 0,
		},
	];

	for request in requests {
		let json = serde_json::to_string(&request).expect("Should serialize");
		let deserialized: VoteRequest = serde_json::from_str(&json).expect("Should deserialize");
		assert_eq!(request.question_id, deserialized.question_id);
		assert_eq!(request.choice_id, deserialized.choice_id);
	}
}

// ============================================================================
// ChoiceInfo Vote Count Tests
// ============================================================================

/// Test ChoiceInfo with zero votes
#[wasm_bindgen_test]
fn test_choice_info_zero_votes() {
	let choice = ChoiceInfo {
		id: 1,
		question_id: 1,
		choice_text: "New Choice".to_string(),
		votes: 0,
	};

	assert_eq!(choice.votes, 0);

	let json = serde_json::to_string(&choice).expect("Should serialize");
	let deserialized: ChoiceInfo = serde_json::from_str(&json).expect("Should deserialize");
	assert_eq!(deserialized.votes, 0);
}

/// Test ChoiceInfo with high vote count
#[wasm_bindgen_test]
fn test_choice_info_high_votes() {
	let choice = ChoiceInfo {
		id: 1,
		question_id: 1,
		choice_text: "Popular Choice".to_string(),
		votes: 1_000_000,
	};

	let json = serde_json::to_string(&choice).expect("Should serialize");
	let deserialized: ChoiceInfo = serde_json::from_str(&json).expect("Should deserialize");
	assert_eq!(deserialized.votes, 1_000_000);
}
