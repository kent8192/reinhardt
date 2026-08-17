// The macros emit Reinhardt-specific configuration names in this standalone fixture.
#![allow(unexpected_cfgs)]

use reinhardt::{admin, model};
use serde::{Deserialize, Serialize};

#[model(app_label = "admin_ui", table_name = "unknown_customization_articles")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Article {
	#[field(primary_key = true)]
	id: i64,
}

#[admin(
	model,
	for = Article,
	name = "Article",
	formfield_overrides = [(missing, widget = text_input)],
	prepopulated_fields = [(id, sources = [missing_source])]
)]
struct ArticleAdmin;

fn main() {}
