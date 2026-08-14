//! Validation-ready application contract resolution.

use reinhardt_conf::MigrationSettings;
use reinhardt_conf::settings::{ComposedSettings, PendingSettings, SettingsContractState};
use reinhardt_core::endpoint::ResolvedEndpoint;
use reinhardt_db::migrations::{
	DependencyResolutionContext, FilesystemSource, MigrationCatalog, MigrationKey, ProjectState,
	verification::SchemaContractState,
};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::PathBuf;

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

/// Resolve one validation-ready application contract aggregate.
pub async fn resolve_contract_state<S: ComposedSettings>(
	settings: &PendingSettings<S>,
	applied_migrations: Option<BTreeSet<MigrationKey>>,
) -> Result<ResolvedContractState, ContractResolutionError> {
	let migration_settings = settings
		.deserialize_section::<MigrationSettings>("migrations")
		.map_err(|_| ContractResolutionError {
			kind: ContractResolutionErrorKind::SettingsSection,
			safe_target: Some(SafeContractTarget::SettingsSection("migrations")),
		})?;
	resolve_contract_state_with_inputs(
		settings.contract_state(),
		migration_settings,
		applied_migrations,
	)
	.await
}

/// Resolve contract inputs that are already typed but retain their merged
/// settings state for callers that started from `ResolvedSettings`.
pub(crate) async fn resolve_contract_state_with_inputs(
	contract_settings: SettingsContractState,
	migration_settings: MigrationSettings,
	applied_migrations: Option<BTreeSet<MigrationKey>>,
) -> Result<ResolvedContractState, ContractResolutionError> {
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
	let base_dir = contract_settings
		.merged
		.get("core")
		.and_then(|value| value.get("base_dir"))
		.and_then(serde_json::Value::as_str)
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from("."));
	let mut dependency_context = DependencyResolutionContext::new();
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
	if let Some(installed_apps) = contract_settings
		.merged
		.get("core")
		.and_then(|value| value.get("installed_apps"))
		.and_then(serde_json::Value::as_array)
	{
		dependency_context = dependency_context.with_apps(
			installed_apps
				.iter()
				.filter_map(serde_json::Value::as_str)
				.map(str::to_string),
		);
	}
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
