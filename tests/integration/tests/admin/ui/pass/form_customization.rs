// The macros emit Reinhardt-specific configuration names in this standalone fixture.
#![allow(unexpected_cfgs)]

use reinhardt::admin::AdminForm;
use reinhardt::{admin, model};
use serde::{Deserialize, Serialize};

#[model(app_label = "admin_ui", table_name = "customized_articles")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Article {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	text: String,
	#[field(max_length = 255)]
	email: String,
	#[field(max_length = 255)]
	number: i64,
	#[field(max_length = 255)]
	flag: bool,
	#[field(max_length = 255)]
	date: String,
	#[field(max_length = 255)]
	datetime: String,
	#[field(max_length = 255)]
	body: String,
	#[field(max_length = 255)]
	status: String,
	#[field(max_length = 255)]
	tags: String,
	#[field(max_length = 255)]
	owner: String,
	#[field(max_length = 255)]
	raw: String,
	#[field(max_length = 255)]
	many: String,
	#[field(max_length = 255)]
	file: String,
	#[field(max_length = 255)]
	hidden: String,
}

#[derive(Debug, Default)]
struct ArticleAdminForm;

impl AdminForm for ArticleAdminForm {}

#[admin(
	model,
	for = Article,
	name = "Article",
	form = ArticleAdminForm,
	formfield_overrides = [
		(text, widget = text_input),
		(email, widget = email_input),
		(number, widget = number_input),
		(flag, widget = checkbox),
		(date, widget = date_input),
		(datetime, widget = datetime_input),
		(body, widget = textarea, rows = 8),
		(status, widget = select, choices = [("draft", "Draft")]),
		(tags, widget = multiselect, choices = [("rust", "Rust")]),
		(owner, widget = autocomplete),
		(raw, widget = raw_id),
		(many, widget = many_to_many),
		(file, widget = file_input),
		(hidden, widget = hidden_input),
	],
	prepopulated_fields = [(hidden, sources = [text, email])]
)]
struct ArticleAdmin;

fn main() {}
