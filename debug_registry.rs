use reinhardt_apps::registry::get_registered_models;
use reinhardt_apps::validation::{check_duplicate_model_names, check_duplicate_table_names};

fn main() {
	let models = get_registered_models();
	eprintln!("Total models registered: {}", models.len());
	for model in models {
		eprintln!(
			"  - {}.{} (table: {})",
			model.app_label, model.model_name, model.table_name
		);
	}

	let errors = check_duplicate_model_names();
	eprintln!("\nDuplicate model name errors: {}", errors.len());
	for error in &errors {
		eprintln!("  - {}", error);
	}

	let errors = check_duplicate_table_names();
	eprintln!("\nDuplicate table name errors: {}", errors.len());
	for error in &errors {
		eprintln!("  - {}", error);
	}
}
