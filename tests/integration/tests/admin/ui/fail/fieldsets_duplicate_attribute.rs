// The macros emit Reinhardt-specific configuration names in this standalone fixture.
#![allow(unexpected_cfgs)]

use reinhardt::{admin, model};
use serde::{Deserialize, Serialize};

#[model(app_label = "admin_ui", table_name = "duplicate_attribute_articles")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Article {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
}

#[admin(model,
	for = Article,
	name = "Article",
	fieldsets = [(fields = [name], fields = [name])]
)]
struct ArticleAdmin;

fn main() {}
