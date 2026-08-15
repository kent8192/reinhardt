// The macros emit Reinhardt-specific configuration names in this standalone fixture.
#![allow(unexpected_cfgs)]

use reinhardt::{admin, model};
use serde::{Deserialize, Serialize};

#[model(app_label = "admin_ui", table_name = "duplicate_customization_articles")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Article {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	title: String,
	#[field(max_length = 255)]
	slug: String,
}

#[admin(
	model,
	for = Article,
	name = "Article",
	prepopulated_fields = [(slug, sources = [title]), (slug, sources = [title])]
)]
struct ArticleAdmin;

fn main() {}
