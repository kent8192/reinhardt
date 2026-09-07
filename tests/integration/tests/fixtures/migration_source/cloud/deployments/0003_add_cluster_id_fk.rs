use reinhardt::db::migrations::prelude::*;
pub fn migration() -> Migration {
	Migration {
		app_label: "deployments".to_string(),
		name: "0003_add_cluster_id_fk".to_string(),
		operations: vec![Operation::AddConstraint {
			table: "deployments".to_string(),
			constraint_sql: "CONSTRAINT deployments_cluster_id_fk FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE RESTRICT ON UPDATE CASCADE".to_string(),
		}],
		dependencies: vec![
			("deployments".to_string(), "0002_add_user_id".to_string()),
			("clusters".to_string(), "0001_initial".to_string()),
		],
		atomic: true,
		replaces: vec![],
		initial: Some(false),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
