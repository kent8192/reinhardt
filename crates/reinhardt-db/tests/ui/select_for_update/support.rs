#![allow(dead_code)] // Shared fixtures are referenced by separate trybuild crates.

use std::borrow::Cow;

use reinhardt_db::orm::{
	FieldSelector, Manager, Model, RelationJoinKind, RelationMultiplicity, RelationPath,
	RelationStep,
};
use serde::{Deserialize, Serialize};

macro_rules! model {
	($model:ident, $fields:ident, $table:literal) => {
		#[derive(Clone, Debug, Deserialize, Serialize)]
		pub(crate) struct $model {
			pub(crate) id: Option<i64>,
		}

		#[derive(Clone)]
		pub(crate) struct $fields;

		impl FieldSelector for $fields {
			fn with_alias(self, _alias: &str) -> Self {
				self
			}
		}

		impl Model for $model {
			type PrimaryKey = i64;
			type Fields = $fields;
			type Objects = Manager<Self>;

			fn table_name() -> &'static str {
				$table
			}

			fn new_fields() -> Self::Fields {
				$fields
			}

			fn primary_key(&self) -> Option<Self::PrimaryKey> {
				self.id
			}

			fn set_primary_key(&mut self, value: Self::PrimaryKey) {
				self.id = Some(value);
			}
		}
	};
}

model!(Article, ArticleFields, "articles");
model!(Author, AuthorFields, "authors");
model!(Comment, CommentFields, "comments");

pub(crate) fn article_author() -> RelationPath<Article, Author> {
	RelationPath::from_owned_steps(vec![RelationStep {
		name: Cow::Borrowed("author"),
		source_table: Cow::Borrowed("articles"),
		target_table: Cow::Borrowed("authors"),
		source_column: Cow::Borrowed("author_id"),
		target_column: Cow::Borrowed("id"),
		default_join_kind: RelationJoinKind::Inner,
		multiplicity: RelationMultiplicity::Single,
	}])
}

pub(crate) fn comment_author() -> RelationPath<Comment, Author> {
	RelationPath::from_owned_steps(vec![RelationStep {
		name: Cow::Borrowed("author"),
		source_table: Cow::Borrowed("comments"),
		target_table: Cow::Borrowed("authors"),
		source_column: Cow::Borrowed("author_id"),
		target_column: Cow::Borrowed("id"),
		default_join_kind: RelationJoinKind::Inner,
		multiplicity: RelationMultiplicity::Single,
	}])
}
