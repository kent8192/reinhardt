// The macros emit Reinhardt-specific configuration names in this standalone fixture.
#![allow(unexpected_cfgs)]

use reinhardt::{admin, model};
use serde::{Deserialize, Serialize};

#[model(app_label = "admin_ui", table_name = "duplicate_setting_articles")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Article {
	#[field(primary_key = true)]
	id: i64,
}

#[admin(
	model,
	for = Article,
	name = "Article",
	formfield_overrides = [],
	formfield_overrides = []
)]
struct ArticleAdmin;

fn main() {}
