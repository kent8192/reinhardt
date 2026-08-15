// The macros emit Reinhardt-specific configuration names in this standalone fixture.
#![allow(unexpected_cfgs)]

use reinhardt::{admin, model};
use serde::{Deserialize, Serialize};

#[model(app_label = "admin_ui", table_name = "form_bound_articles")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Article {
	#[field(primary_key = true)]
	id: i64,
}

#[derive(Debug)]
struct ArticleAdminForm;

#[admin(model, for = Article, name = "Article", form = ArticleAdminForm)]
struct ArticleAdmin;

fn main() {}
