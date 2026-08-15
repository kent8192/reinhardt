// The model macro emits native-only cfgs evaluated in this standalone trybuild crate.
#![allow(unexpected_cfgs)]

use reinhardt::admin::ModelAdmin;
use reinhardt::db::associations::ForeignKeyField;
use reinhardt::{admin, model};
use serde::{Deserialize, Serialize};

#[model(app_label = "admin_ui", table_name = "authors")]
#[derive(Serialize, Deserialize)]
struct Author {
	#[field(primary_key = true)]
	id: i64,
}

#[model(app_label = "admin_ui", table_name = "articles")]
#[derive(Serialize, Deserialize)]
struct Article {
	#[field(primary_key = true)]
	id: i64,
	#[rel(foreign_key)]
	author: ForeignKeyField<Author>,
}

#[admin(
	model,
	for = Article,
	name = "Article",
	list_select_related = [author]
)]
struct ArticleAdmin;

fn main() {
	let admin = ArticleAdmin;
	assert_eq!(admin.list_select_related(), vec!["author"]);
}
