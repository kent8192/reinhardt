use serde::{Deserialize, Serialize};
use validator::Validate;

/// Serializer for creating/updating questions
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct QuestionSerializer {
	#[validate(length(
		min = 1,
		max = 200,
		message = "Question text must be between 1 and 200 characters"
	))]
	pub question_text: String,
}

/// Response model for questions
#[derive(Debug, Serialize, Deserialize)]
pub struct QuestionResponse {
	pub id: i64,
	pub question_text: String,
	pub pub_date: chrono::DateTime<chrono::Utc>,
	pub was_published_recently: bool,
}

impl QuestionResponse {
	pub fn from_model(model: &crate::apps::polls::models::Question) -> Self {
		Self {
			id: model.id(),
			question_text: model.question_text().to_string(),
			pub_date: model.pub_date(),
			was_published_recently: model.was_published_recently(),
		}
	}
}

/// Serializer for creating/updating choices
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ChoiceSerializer {
	pub question_id: i64,

	#[validate(length(
		min = 1,
		max = 200,
		message = "Choice text must be between 1 and 200 characters"
	))]
	pub choice_text: String,
}

/// Response model for choices
#[derive(Debug, Serialize, Deserialize)]
pub struct ChoiceResponse {
	pub id: i64,
	pub question_id: i64,
	pub choice_text: String,
	pub votes: i32,
}

impl ChoiceResponse {
	pub fn from_model(model: &crate::apps::polls::models::Choice) -> Self {
		Self {
			id: model.id(),
			question_id: *model.question_id(),
			choice_text: model.choice_text().to_string(),
			votes: model.votes(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_question_serializer_valid() {
		let serializer = QuestionSerializer {
			question_text: "What's your favorite color?".to_string(),
		};
		assert!(serializer.validate().is_ok());
	}

	#[rstest]
	fn test_question_serializer_empty_text() {
		let serializer = QuestionSerializer {
			question_text: String::new(),
		};
		assert!(serializer.validate().is_err());
	}

	#[rstest]
	fn test_question_serializer_too_long() {
		let serializer = QuestionSerializer {
			question_text: "a".repeat(201),
		};
		assert!(serializer.validate().is_err());
	}

	#[rstest]
	fn test_choice_serializer_valid() {
		let serializer = ChoiceSerializer {
			question_id: 1,
			choice_text: "Red".to_string(),
		};
		assert!(serializer.validate().is_ok());
	}

	#[rstest]
	fn test_choice_serializer_empty_text() {
		let serializer = ChoiceSerializer {
			question_id: 1,
			choice_text: String::new(),
		};
		assert!(serializer.validate().is_err());
	}
}
