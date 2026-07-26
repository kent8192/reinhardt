use std::collections::{BTreeMap, HashSet};

use reinhardt_db::orm::registry::{ModelInfo, global_model_registry};

pub(crate) struct ImportPlan {
	imports: Vec<String>,
	warnings: Vec<String>,
}

impl ImportPlan {
	pub(crate) fn from_registry(installed_apps: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
		Self::from_models(installed_apps, global_model_registry().all())
	}

	pub(crate) fn from_models(
		installed_apps: impl IntoIterator<Item = impl AsRef<str>>,
		models: impl IntoIterator<Item = ModelInfo>,
	) -> Self {
		let installed_apps: HashSet<String> = installed_apps
			.into_iter()
			.map(|label| label.as_ref().to_string())
			.collect();
		let mut paths_by_model = BTreeMap::<String, Vec<String>>::new();
		let mut warnings = Vec::new();

		for model in models {
			if installed_apps.contains(&model.app_label) {
				if syn::parse_str::<syn::Path>(&model.type_path).is_err() {
					warnings.push(format!(
						"Invalid registered model path for `{}`: {}",
						model.model_name, model.type_path
					));
					continue;
				}
				paths_by_model
					.entry(model.model_name)
					.or_default()
					.push(model.type_path);
			}
		}

		let mut imports = Vec::new();
		for (model_name, mut paths) in paths_by_model {
			paths.sort();
			if paths.len() == 1 {
				let path = &paths[0];
				imports.push(format!("use {path};"));
			} else {
				warnings.push(format!(
					"Model name collision for `{model_name}`; use a fully qualified path: {}",
					paths.join(", ")
				));
			}
		}
		imports.sort();
		warnings.sort();

		Self { imports, warnings }
	}

	#[cfg(test)]
	pub(crate) fn imports(&self) -> &[String] {
		&self.imports
	}

	pub(crate) fn prelude_source(&self) -> String {
		self.imports.join("\n")
	}

	pub(crate) fn warnings(&self) -> &[String] {
		&self.warnings
	}
}

#[cfg(test)]
mod tests {
	use reinhardt_db::orm::registry::{ModelInfo, global_model_registry};

	use super::ImportPlan;

	fn model(app_label: &str, model_name: &str, type_path: &str) -> ModelInfo {
		ModelInfo {
			app_label: app_label.to_string(),
			model_name: model_name.to_string(),
			type_path: type_path.to_string(),
			table_name: format!("{app_label}_{model_name}").to_lowercase(),
		}
	}

	#[test]
	fn unique_installed_models_produce_sorted_imports() {
		let plan = ImportPlan::from_models(
			["users", "inventory"],
			[
				model("users", "User", "project::apps::users::models::User"),
				model(
					"inventory",
					"Product",
					"project::apps::inventory::models::Product",
				),
			],
		);

		assert_eq!(
			plan.imports(),
			[
				"use project::apps::inventory::models::Product;",
				"use project::apps::users::models::User;",
			]
		);
		assert_eq!(
			plan.prelude_source(),
			"use project::apps::inventory::models::Product;\n\
use project::apps::users::models::User;"
		);
		assert_eq!(plan.warnings(), &[] as &[String]);
	}

	#[test]
	fn colliding_short_names_are_omitted_and_report_every_sorted_path() {
		let plan = ImportPlan::from_models(
			["users", "inventory"],
			[
				model("users", "User", "project::apps::users::models::User"),
				model(
					"inventory",
					"User",
					"project::apps::inventory::models::User",
				),
				model(
					"inventory",
					"Product",
					"project::apps::inventory::models::Product",
				),
			],
		);

		assert_eq!(
			plan.imports(),
			["use project::apps::inventory::models::Product;"]
		);
		assert_eq!(
			plan.warnings(),
			[
				"Model name collision for `User`; use a fully qualified path: \
project::apps::inventory::models::User, project::apps::users::models::User"
			]
		);
	}

	#[test]
	fn models_outside_installed_apps_are_ignored() {
		let plan = ImportPlan::from_models(
			["users"],
			[
				model(
					"inventory",
					"Product",
					"project::apps::inventory::models::Product",
				),
				model("users", "User", "project::apps::users::models::User"),
			],
		);

		assert_eq!(plan.imports(), ["use project::apps::users::models::User;"]);
		assert_eq!(plan.warnings(), &[] as &[String]);
	}

	#[test]
	fn invalid_model_paths_are_omitted_without_corrupting_valid_imports() {
		let plan = ImportPlan::from_models(
			["users", "inventory"],
			[
				model("users", "User", "project::apps::users::models::User"),
				model(
					"inventory",
					"Product",
					"project::apps::inventory::models::Product; compile_error!(\"bad\")",
				),
			],
		);

		assert_eq!(plan.imports(), ["use project::apps::users::models::User;"]);
		assert_eq!(
			plan.warnings(),
			["Invalid registered model path for `Product`: \
project::apps::inventory::models::Product; compile_error!(\"bad\")"]
		);
	}

	#[test]
	fn warnings_are_sorted_by_colliding_model_name() {
		let plan = ImportPlan::from_models(
			["users", "inventory"],
			[
				model("users", "Widget", "project::apps::users::models::Widget"),
				model(
					"inventory",
					"Widget",
					"project::apps::inventory::models::Widget",
				),
				model("users", "Account", "project::apps::users::models::Account"),
				model(
					"inventory",
					"Account",
					"project::apps::inventory::models::Account",
				),
			],
		);

		assert_eq!(
			plan.warnings(),
			[
				"Model name collision for `Account`; use a fully qualified path: \
project::apps::inventory::models::Account, project::apps::users::models::Account",
				"Model name collision for `Widget`; use a fully qualified path: \
project::apps::inventory::models::Widget, project::apps::users::models::Widget",
			]
		);
	}

	struct ModelRegistryGuard {
		previous: Vec<ModelInfo>,
	}

	impl ModelRegistryGuard {
		fn empty() -> Self {
			let previous = global_model_registry().all();
			global_model_registry().clear();
			Self { previous }
		}
	}

	impl Drop for ModelRegistryGuard {
		fn drop(&mut self) {
			global_model_registry().clear();
			for model in self.previous.drain(..) {
				global_model_registry().register(model);
			}
		}
	}

	#[serial_test::serial(shell_model_registry)]
	#[test]
	fn production_planning_reads_the_existing_global_model_registry() {
		let _registry = ModelRegistryGuard::empty();
		global_model_registry().register(model(
			"users",
			"User",
			"project::apps::users::models::User",
		));
		global_model_registry().register(model(
			"inventory",
			"Product",
			"project::apps::inventory::models::Product",
		));

		let plan = ImportPlan::from_registry(["users"]);

		assert_eq!(plan.imports(), ["use project::apps::users::models::User;"]);
	}
}
