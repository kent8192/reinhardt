//! Fail case: `#[model(manager = X)]` where `X::Model` is not the model itself
//! triggers a compile error from the trait bound on `HasCustomManager::Manager`.
//! Issue #3980.

use reinhardt::db::orm::custom_manager::CustomManager;
use reinhardt::model;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(app_label = "test", table_name = "other")]
pub(crate) struct Other {
	#[field(primary_key = true)]
	pub id: i64,
}

#[derive(Default)]
struct OtherManager;

impl CustomManager for OtherManager {
	type Model = Other;

	fn new() -> Self {
		Self
	}
}

// `OtherManager::Model = Other`, but it is attached to `User`, so the
// `HasCustomManager::Manager: CustomManager<Model = Self>` bound fails.
#[derive(Serialize, Deserialize)]
#[model(app_label = "test", table_name = "users", manager = OtherManager)]
pub(crate) struct User {
	#[field(primary_key = true)]
	pub id: i64,
}

fn main() {}
