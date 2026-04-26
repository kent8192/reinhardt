//! Fail case: `#[model(manager = ...)]` with an integer literal in place of a
//! type path is rejected at parse time. Issue #3980.

use reinhardt::model;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(app_label = "test", table_name = "users", manager = 123)]
pub(crate) struct User {
	#[field(primary_key = true)]
	pub id: i64,
}

fn main() {}
