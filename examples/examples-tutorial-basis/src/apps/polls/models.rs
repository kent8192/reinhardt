use chrono::{DateTime, Utc};
use reinhardt::db::associations::ForeignKeyField;
use reinhardt::prelude::*;
use serde::{Deserialize, Serialize};

/// Question model representing a poll question
#[model(app_label = "polls", table_name = "questions")]
#[derive(Serialize, Deserialize)]
pub struct Question {
	#[field(primary_key = true)]
	id: i64,

	#[field(max_length = 200)]
	question_text: String,

	#[field(auto_now_add = true)]
	pub_date: DateTime<Utc>,
}

impl Question {
	/// Check if the question was published recently (within last day)
	pub fn was_published_recently(&self) -> bool {
		let now = Utc::now();
		let one_day_ago = now - chrono::Duration::days(1);
		self.pub_date >= one_day_ago && self.pub_date <= now
	}
}

/// Choice model representing an answer option for a question
#[model(app_label = "polls", table_name = "choices")]
#[derive(Serialize, Deserialize)]
pub struct Choice {
	#[field(primary_key = true)]
	id: i64,

	// ⚠️ IMPORTANT: related_name is REQUIRED for #[rel(foreign_key)]
	#[rel(foreign_key, related_name = "choices")]
	question: ForeignKeyField<Question>,

	#[field(max_length = 200)]
	choice_text: String,

	#[field(default = 0)]
	votes: i32,
}

impl Choice {
	/// Increment the vote count
	pub fn vote(&mut self) {
		self.votes += 1;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_choice_vote() {
		let mut choice = Choice::new(
			"Choice 1".to_string(), // choice_text
			0,                      // votes
			1,                      // question_id (ForeignKeyField is last)
		);
		assert_eq!(choice.votes(), 0);

		choice.vote();
		assert_eq!(choice.votes(), 1);

		choice.vote();
		assert_eq!(choice.votes(), 2);
	}
}
