//! Validation-ready application contract resolution.

use reinhardt_conf::settings::{
	ComposedSettings, PendingSettings, SettingsContractState, SettingsRootSchema,
};
use reinhardt_conf::{CoreSettings, MigrationSettings};
use reinhardt_core::endpoint::ResolvedEndpoint;
use reinhardt_db::migrations::{
	DependencyResolutionContext, FilesystemSource, MigrationCatalog, MigrationKey, ProjectState,
	verification::SchemaContractState,
};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// Validation-ready state shared by contract export and verification.
pub struct ResolvedContractState {
	/// Model and migration schema inputs.
	pub schema: SchemaContractState,
	/// Side-effect-free mounted endpoint topology.
	pub registered_endpoints: Vec<ResolvedEndpoint>,
	/// Merged settings and generated verification schema.
	pub settings: SettingsContractState,
	pub(crate) migration_dependencies: BTreeMap<MigrationKey, Vec<MigrationKey>>,
}

/// Stable category for a contract resolution failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractResolutionErrorKind {
	/// Cargo invocation context could not be replayed.
	CargoContext,
	/// A settings source could not be loaded.
	SettingsSource,
	/// One settings section could not be deserialized safely.
	SettingsSection,
	/// The composed settings type did not provide a verification schema.
	SettingsSchema,
	/// Migration sources could not be loaded or replayed.
	MigrationCatalog,
	/// Registered model metadata could not be collected.
	ModelRegistry,
	/// Mounted route topology could not be collected safely.
	RouteTopology,
}

/// Redacted target metadata that is safe to render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SafeContractTarget {
	/// A named settings section.
	SettingsSection(&'static str),
	/// Project migration sources.
	Migrations,
}

/// A safe contract resolution failure without an underlying error payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractResolutionError {
	/// Stable failure category.
	pub kind: ContractResolutionErrorKind,
	/// Optional non-secret target metadata.
	pub safe_target: Option<SafeContractTarget>,
}

impl std::fmt::Display for ContractResolutionError {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			formatter,
			"application contract resolution failed: {:?}",
			self.kind
		)
	}
}

impl std::error::Error for ContractResolutionError {}

/// Return the composition key for a fragment type, including explicit aliases.
pub(crate) fn composed_section_key<F>(root_schema: &SettingsRootSchema) -> Option<&'static str> {
	let type_name = std::any::type_name::<F>()
		.rsplit("::")
		.next()
		.unwrap_or_default();
	root_schema
		.sections
		.iter()
		.find(|section| section.node.type_name == type_name)
		.map(|section| section.canonical_key)
}

pub(crate) fn migration_settings_from_contract(
	contract_settings: &SettingsContractState,
) -> Result<MigrationSettings, ContractResolutionError> {
	let migration_key = composed_section_key::<MigrationSettings>(&contract_settings.root_schema)
		.unwrap_or("migrations");
	let uses_root_default = contract_settings
		.root_schema
		.sections
		.iter()
		.find(|section| section.canonical_key == migration_key)
		.is_some_and(|section| {
			section.has_default
				&& section
					.accepted_keys
					.iter()
					.all(|key| !contract_settings.merged.contains_key(key))
		});
	if uses_root_default {
		return Ok(MigrationSettings::default());
	}
	contract_settings
		.deserialize_section::<MigrationSettings>(migration_key)
		.map_err(|_| ContractResolutionError {
			kind: ContractResolutionErrorKind::SettingsSection,
			safe_target: Some(SafeContractTarget::SettingsSection(migration_key)),
		})
}

fn ensure_root_schema(
	contract_settings: &SettingsContractState,
) -> Result<(), ContractResolutionError> {
	if contract_settings.root_schema.sections.is_empty() {
		return Err(ContractResolutionError {
			kind: ContractResolutionErrorKind::SettingsSchema,
			safe_target: None,
		});
	}
	Ok(())
}

/// Resolve one validation-ready application contract aggregate.
pub async fn resolve_contract_state<S: ComposedSettings>(
	settings: &PendingSettings<S>,
	applied_migrations: Option<BTreeSet<MigrationKey>>,
) -> Result<ResolvedContractState, ContractResolutionError> {
	let contract_settings = settings.contract_state();
	ensure_root_schema(&contract_settings)?;
	let migration_settings = migration_settings_from_contract(&contract_settings)?;
	resolve_contract_state_with_inputs(contract_settings, migration_settings, applied_migrations)
		.await
}

/// Resolve contract inputs that are already typed but retain their merged
/// settings state for callers that started from `ResolvedSettings`.
pub(crate) async fn resolve_contract_state_with_inputs(
	contract_settings: SettingsContractState,
	migration_settings: MigrationSettings,
	applied_migrations: Option<BTreeSet<MigrationKey>>,
) -> Result<ResolvedContractState, ContractResolutionError> {
	ensure_root_schema(&contract_settings)?;
	let (schema, migration_dependencies) = resolve_contract_schema_with_inputs(
		&contract_settings,
		&migration_settings,
		applied_migrations,
	)
	.await?;
	let registered_endpoints =
		reinhardt_urls::routers::collect_resolved_endpoints().map_err(|_| {
			ContractResolutionError {
				kind: ContractResolutionErrorKind::RouteTopology,
				safe_target: None,
			}
		})?;

	Ok(ResolvedContractState {
		schema,
		registered_endpoints,
		settings: contract_settings,
		migration_dependencies,
	})
}

/// Resolve migration and model inputs independently of route topology.
pub(crate) async fn resolve_contract_schema_with_inputs(
	contract_settings: &SettingsContractState,
	migration_settings: &MigrationSettings,
	applied_migrations: Option<BTreeSet<MigrationKey>>,
) -> Result<
	(
		SchemaContractState,
		BTreeMap<MigrationKey, Vec<MigrationKey>>,
	),
	ContractResolutionError,
> {
	let core_key = composed_section_key::<CoreSettings>(&contract_settings.root_schema);
	let core_settings = if let Some(core_key) =
		core_key.filter(|key| contract_settings.merged.contains_key(*key))
	{
		contract_settings
			.deserialize_section::<CoreSettings>(core_key)
			.map_err(|_| ContractResolutionError {
				kind: ContractResolutionErrorKind::SettingsSection,
				safe_target: Some(SafeContractTarget::SettingsSection(core_key)),
			})?
	} else {
		CoreSettings::default()
	};
	let base_dir = core_settings.base_dir.clone();
	let dependency_context = migration_dependency_context(&core_settings, migration_settings);
	let source = FilesystemSource::new(base_dir.join("migrations"));
	let catalog = MigrationCatalog::load_strict_with_context(&source, &dependency_context)
		.await
		.map_err(|_| ContractResolutionError {
			kind: ContractResolutionErrorKind::MigrationCatalog,
			safe_target: Some(SafeContractTarget::Migrations),
		})?;
	let migration_state =
		catalog
			.resolved_project_state()
			.map_err(|_| ContractResolutionError {
				kind: ContractResolutionErrorKind::MigrationCatalog,
				safe_target: Some(SafeContractTarget::Migrations),
			})?;
	let ordered = catalog
		.raw_ordered_migrations()
		.map_err(|_| ContractResolutionError {
			kind: ContractResolutionErrorKind::MigrationCatalog,
			safe_target: Some(SafeContractTarget::Migrations),
		})?;
	let known_migrations = ordered
		.iter()
		.map(|(migration, _)| MigrationKey::new(&migration.app_label, &migration.name))
		.collect();
	let migration_dependencies = ordered
		.iter()
		.map(|(migration, dependencies)| {
			(
				MigrationKey::new(&migration.app_label, &migration.name),
				dependencies.to_vec(),
			)
		})
		.collect();
	let replacement_edges = ordered
		.iter()
		.flat_map(|(migration, _)| {
			let replacement = MigrationKey::new(&migration.app_label, &migration.name);
			migration
				.replaces
				.iter()
				.map(move |(app, name)| (replacement.clone(), MigrationKey::new(app, name)))
		})
		.collect();
	let model_state =
		ProjectState::try_from_global_registry().map_err(|_| ContractResolutionError {
			kind: ContractResolutionErrorKind::ModelRegistry,
			safe_target: None,
		})?;

	Ok((
		SchemaContractState {
			model_state,
			migration_state,
			known_migrations,
			applied_migrations,
			replacement_edges,
		},
		migration_dependencies,
	))
}

pub(crate) fn migration_dependency_context(
	core_settings: &CoreSettings,
	migration_settings: &MigrationSettings,
) -> DependencyResolutionContext {
	let mut dependency_context =
		DependencyResolutionContext::new().with_apps(core_settings.installed_apps.iter().cloned());
	for feature in &core_settings.migration_features {
		dependency_context = dependency_context.with_feature(feature);
	}
	for (key, value) in &core_settings.migration_swappable_settings {
		dependency_context = dependency_context.with_setting(key, value);
	}
	for feature in &migration_settings.migration_features {
		dependency_context = dependency_context.with_feature(feature);
	}
	for (key, value) in migration_settings
		.migration_settings
		.iter()
		.chain(&migration_settings.migration_swappable_settings)
	{
		dependency_context = dependency_context.with_setting(key, value);
	}
	dependency_context
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::macros::settings;

	#[settings(config: CoreSettings | migration_config: MigrationSettings)]
	struct AliasedContractSettings;

	#[settings(migrations: MigrationSettings)]
	#[derive(Default)]
	#[serde(default)]
	struct DefaultedMigrationContractSettings;

	#[test]
	fn composed_section_key_follows_explicit_alias() {
		let schema = AliasedContractSettings::root_schema();

		assert_eq!(
			composed_section_key::<CoreSettings>(&schema),
			Some("config")
		);
		assert_eq!(
			composed_section_key::<MigrationSettings>(&schema),
			Some("migration_config")
		);
	}

	#[test]
	fn migration_context_merges_legacy_core_and_migration_sections() {
		let mut core = CoreSettings::default();
		core.installed_apps = vec!["accounts".to_owned()];
		core.migration_features = vec!["legacy".to_owned()];
		core.migration_swappable_settings
			.insert("AUTH_USER_MODEL".to_owned(), "accounts.User".to_owned());
		let mut migrations = MigrationSettings::default();
		migrations.migration_features = vec!["new".to_owned()];
		migrations
			.migration_settings
			.insert("ENABLE_AUDIT".to_owned(), "true".to_owned());

		let context = migration_dependency_context(&core, &migrations);

		assert!(context.is_app_installed("accounts"));
		assert!(context.is_feature_enabled("legacy"));
		assert!(context.is_feature_enabled("new"));
		assert_eq!(
			context.get_setting("AUTH_USER_MODEL").map(String::as_str),
			Some("accounts.User")
		);
		assert_eq!(
			context.get_setting("ENABLE_AUDIT").map(String::as_str),
			Some("true")
		);
	}

	#[test]
	fn missing_defaulted_migration_section_uses_fragment_default() {
		let contract = SettingsContractState {
			root_schema: DefaultedMigrationContractSettings::root_schema(),
			merged: Default::default(),
			typed_coercion: false,
		};

		let migration = migration_settings_from_contract(&contract)
			.expect("root default should resolve the migration fragment");

		assert!(migration.migration_features.is_empty());
		assert!(migration.migration_settings.is_empty());
	}
}
