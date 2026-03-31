use json::json;
use reinhardt::Model;
use reinhardt::StatusCode;
use reinhardt::core::serde::json;
use reinhardt::db::orm::{FilterOperator, FilterValue};
use reinhardt::http::ViewResult;
use reinhardt::{Json, Path};
use reinhardt::{Response, get, post};
use serde::Deserialize;

use super::models::{Choice, Question};

/// Request body for voting
#[derive(Debug, Deserialize)]
pub struct VoteRequest {
	pub choice_id: i64,
}

/// Index view - List all polls
#[get("/", name = "polls_index")]
pub async fn index() -> ViewResult<Response> {
	let manager = Question::objects();
	let questions = manager.all().all().await?;
	let latest_questions: Vec<_> = questions.into_iter().take(5).collect();

	let response_data = json!({
		"message": "Polls index",
		"polls": latest_questions
	});

	let json = json::to_string(&response_data)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Detail view - Show a specific poll
///
/// GET /polls/{question_id}/
#[get("/{question_id}/", name = "polls_detail")]
pub async fn detail(Path(question_id): Path<i64>) -> ViewResult<Response> {
	let question_manager = Question::objects();
	let question = question_manager
		.get(question_id)
		.first()
		.await?
		.ok_or("Question not found")?;

	let choice_manager = Choice::objects();
	let choices = choice_manager
		.filter(
			Choice::field_question_id(),
			FilterOperator::Eq,
			FilterValue::Int(question_id),
		)
		.all()
		.await?;

	let response_data = json!({
		"question": question,
		"choices": choices
	});

	let json = json::to_string(&response_data)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Results view - Show poll results
///
/// GET /polls/{question_id}/results/
#[get("/{question_id}/results/", name = "polls_results")]
pub async fn results(Path(question_id): Path<i64>) -> ViewResult<Response> {
	let question_manager = Question::objects();
	let question = question_manager
		.get(question_id)
		.first()
		.await?
		.ok_or("Question not found")?;

	let choice_manager = Choice::objects();
	let choices = choice_manager
		.filter(
			Choice::field_question_id(),
			FilterOperator::Eq,
			FilterValue::Int(question_id),
		)
		.all()
		.await?;

	let total_votes: i32 = choices.iter().map(|c| c.votes()).sum();

	let response_data = json!({
		"question": question,
		"choices": choices,
		"total_votes": total_votes
	});

	let json = json::to_string(&response_data)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Vote view - Handle voting
///
/// POST /polls/{question_id}/vote/
#[post("/{question_id}/vote/", name = "polls_vote")]
pub async fn vote(
	Path(question_id): Path<i64>,
	Json(vote_req): Json<VoteRequest>,
) -> ViewResult<Response> {
	let choice_id = vote_req.choice_id;

	let choice_manager = Choice::objects();
	let mut choice = choice_manager
		.get(choice_id)
		.first()
		.await?
		.ok_or("Choice not found")?;

	if *choice.question_id() != question_id {
		return Err("Choice does not belong to this question".into());
	}

	choice.vote();
	let updated_choice = choice_manager.update(&choice).await?;

	let response_data = json!({
		"message": "Vote recorded successfully",
		"question_id": question_id,
		"choice_id": choice_id,
		"choice_text": updated_choice.choice_text(),
		"new_vote_count": updated_choice.votes()
	});

	let json = json::to_string(&response_data)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}
