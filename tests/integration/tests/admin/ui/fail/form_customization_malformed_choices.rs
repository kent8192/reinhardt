// The macros emit Reinhardt-specific configuration names in this standalone fixture.
#![allow(unexpected_cfgs)]

use reinhardt::{admin, model};
use serde::{Deserialize, Serialize};

#[model(app_label = "admin_ui", table_name = "malformed_choices_articles")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Article {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	body: String,
}

#[admin(
	model,
	for = Article,
	name = "Article",
	formfield_overrides = [(body, widget = textarea, choices = [("x", "X")])]
)]
struct ArticleAdmin;

fn main() {}
