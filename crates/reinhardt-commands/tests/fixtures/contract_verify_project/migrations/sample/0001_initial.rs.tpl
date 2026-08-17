use reinhardt::db::migrations::prelude::*;

pub(super) fn migration() -> Migration {
	Migration {
		app_label: "sample".to_owned(),
		name: "0001_initial".to_owned(),
		operations: vec![],
		dependencies: vec![],
		atomic: true,
		replaces: Vec::new(),
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
