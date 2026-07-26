//! Native inventory-backed registration for server functions.

use reinhardt_apps::{
	AppModuleRegistration, AppModuleResolutionError, iter_app_module_registrations,
	resolve_app_module_owner,
};
use reinhardt_urls::routers::ServerRouter;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

/// Registers one server function on a native router.
pub type ServerFnRegister = fn(ServerRouter) -> ServerRouter;

/// Compile-time inventory metadata for a generated server function.
#[derive(Debug, Clone, Copy)]
pub struct ServerFnInventoryEntry {
	/// Rust module defining this server function.
	pub module_path: &'static str,
	/// Crate instance defining this server function.
	pub crate_id: &'static str,
	/// Compiled Cargo target identity for this entry.
	pub target_id: Option<&'static str>,
	/// HTTP endpoint path.
	pub path: &'static str,
	/// Route name.
	pub name: &'static str,
	/// Native router registration factory.
	pub register: ServerFnRegister,
}

impl ServerFnInventoryEntry {
	/// Creates a server function inventory entry.
	pub const fn new(
		module_path: &'static str,
		path: &'static str,
		name: &'static str,
		register: ServerFnRegister,
	) -> Self {
		Self::new_in_crate(module_path, "", path, name, register)
	}

	/// Creates a server function inventory entry with a crate-instance identity.
	pub const fn new_in_crate(
		module_path: &'static str,
		crate_id: &'static str,
		path: &'static str,
		name: &'static str,
		register: ServerFnRegister,
	) -> Self {
		Self::new_in_target(module_path, crate_id, None, path, name, register)
	}

	/// Creates a server function inventory entry for one compiled target instance.
	pub const fn new_in_target(
		module_path: &'static str,
		crate_id: &'static str,
		target_id: Option<&'static str>,
		path: &'static str,
		name: &'static str,
		register: ServerFnRegister,
	) -> Self {
		Self {
			module_path,
			crate_id,
			target_id,
			path,
			name,
			register,
		}
	}
}

inventory::collect!(ServerFnInventoryEntry);

/// Deterministic configuration errors discovered while reading server function inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerFnInventoryError {
	/// The router construction module is not owned by an application.
	OrphanCaller {
		/// Unowned router construction module.
		module_path: String,
	},
	/// An inventory entry is not owned by an application.
	OrphanFunction {
		/// Unowned server function module.
		module_path: String,
		/// HTTP endpoint exposed by the unowned server function.
		path: String,
	},
	/// More than one application owns a module at the same specificity.
	AmbiguousOwner {
		/// Module with equally specific owning applications.
		module_path: String,
		/// Sorted labels of equally specific owning applications.
		labels: Vec<String>,
	},
	/// An application contains more than one server function for one path.
	DuplicatePath {
		/// Application containing the conflicting entries.
		app_label: String,
		/// Duplicate endpoint path.
		path: String,
		/// Sorted modules declaring the duplicate path.
		modules: Vec<String>,
	},
	/// An application contains more than one server function with one route name.
	DuplicateName {
		/// Application containing the conflicting entries.
		app_label: String,
		/// Duplicate endpoint name.
		name: String,
		/// Sorted modules declaring the duplicate name.
		modules: Vec<String>,
	},
}

impl Display for ServerFnInventoryError {
	fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::OrphanCaller { module_path } => write!(
				formatter,
				"pages.server_fn.E001: no application owns caller module `{module_path}`"
			),
			Self::OrphanFunction { module_path, path } => write!(
				formatter,
				"pages.server_fn.E002: no application owns server function `{module_path}` at `{path}`"
			),
			Self::AmbiguousOwner {
				module_path,
				labels,
			} => write!(
				formatter,
				"pages.server_fn.E003: multiple applications own module `{module_path}`: {}",
				labels.join(", ")
			),
			Self::DuplicatePath {
				app_label,
				path,
				modules,
			} => write!(
				formatter,
				"pages.server_fn.E004: application `{app_label}` has duplicate server function path `{path}`: {}",
				modules.join(", ")
			),
			Self::DuplicateName {
				app_label,
				name,
				modules,
			} => write!(
				formatter,
				"pages.server_fn.E005: application `{app_label}` has duplicate server function name `{name}`: {}",
				modules.join(", ")
			),
		}
	}
}

/// Returns every native server function inventory error linked into the binary.
pub fn validate_server_fn_inventory() -> Vec<ServerFnInventoryError> {
	let apps = iter_app_module_registrations().copied().collect::<Vec<_>>();
	let entries = inventory::iter::<ServerFnInventoryEntry>()
		.copied()
		.collect::<Vec<_>>();
	validate_entries(&apps, &entries).err().unwrap_or_default()
}

/// Registers native inventory entries owned by the application containing `caller_module`.
pub(crate) fn collect_auto_server_fns(router: ServerRouter, caller_module: &str) -> ServerRouter {
	let apps = iter_app_module_registrations().copied().collect::<Vec<_>>();
	let entries = inventory::iter::<ServerFnInventoryEntry>()
		.copied()
		.collect::<Vec<_>>();
	collect_auto_server_fns_from_entries(router, &apps, &entries, caller_module)
}

/// Registers inventory entries owned by one crate instance.
pub(crate) fn collect_auto_server_fns_in_crate(
	router: ServerRouter,
	caller_module: &str,
	caller_crate: &str,
	caller_target: Option<&str>,
) -> ServerRouter {
	let apps = iter_app_module_registrations().copied().collect::<Vec<_>>();
	let entries = inventory::iter::<ServerFnInventoryEntry>()
		.copied()
		.collect::<Vec<_>>();
	let selected = match select_entries_for_app_in_crate(
		&apps,
		&entries,
		caller_module,
		caller_crate,
		caller_target,
	) {
		Ok(entries) => entries,
		Err(errors) => {
			return errors.into_iter().fold(router, |router, error| {
				router.with_configuration_error(error.to_string())
			});
		}
	};
	selected
		.into_iter()
		.fold(router, |router, entry| (entry.register)(router))
}

fn select_entries_for_app_in_crate<'a>(
	apps: &[AppModuleRegistration],
	entries: &'a [ServerFnInventoryEntry],
	caller_module: &str,
	caller_crate: &str,
	caller_target: Option<&str>,
) -> Result<Vec<&'a ServerFnInventoryEntry>, Vec<ServerFnInventoryError>> {
	let caller =
		resolve_app_module_owner_in_crate_compat(apps, caller_module, caller_crate, caller_target)
			.map_err(|error| vec![resolution_error(caller_module, None, error, true)])?;
	let mut selected = Vec::new();
	for entry in entries {
		match resolve_entry_owner(apps, entry) {
			Ok(owner)
				if owner.module_path == caller.module_path
					&& compatible_owner_identity(owner, caller) =>
			{
				selected.push(entry)
			}
			Ok(_) => {}
			Err(error) => {
				return Err(vec![resolution_error(
					entry.module_path,
					Some(entry.path),
					error,
					false,
				)]);
			}
		}
	}
	sort_entries(&mut selected);
	Ok(selected)
}

fn collect_auto_server_fns_from_entries(
	router: ServerRouter,
	apps: &[AppModuleRegistration],
	entries: &[ServerFnInventoryEntry],
	caller_module: &str,
) -> ServerRouter {
	let selected = match select_entries_for_app(apps, entries, caller_module) {
		Ok(entries) => entries,
		Err(errors) => {
			return errors.into_iter().fold(router, |router, error| {
				router.with_configuration_error(error.to_string())
			});
		}
	};

	selected.into_iter().fold(router, |router, entry| {
		if let Some(existing) = router
			.registered_endpoints()
			.into_iter()
			.find(|endpoint| endpoint.method == hyper::Method::POST && endpoint.path == entry.path)
		{
			router.with_configuration_error(duplicate_server_fn_path_error(
				entry.path,
				[
					entry.module_path,
					existing
						.origin
						.map_or("<manual-server-fn>", |origin| origin.module_path),
				],
			))
		} else {
			(entry.register)(router)
		}
	})
}

pub(crate) fn duplicate_server_fn_path_error(
	path: &str,
	_modules: impl IntoIterator<Item = &'static str>,
) -> String {
	format!(
		"Failed to compile route '{path}' (POST): Insertion failed due to conflict with previously registered route: {path}"
	)
}

fn select_entries_for_app<'a>(
	apps: &[AppModuleRegistration],
	entries: &'a [ServerFnInventoryEntry],
	caller_module: &str,
) -> Result<Vec<&'a ServerFnInventoryEntry>, Vec<ServerFnInventoryError>> {
	let mut errors = match resolve_app_module_owner(apps.iter(), caller_module) {
		Ok(_) => Vec::new(),
		Err(error) => vec![resolution_error(caller_module, None, error, true)],
	};

	let mut owned_entries = Vec::new();
	for entry in entries {
		match resolve_entry_owner(apps, entry) {
			Ok(owner) => owned_entries.push((entry, owner)),
			Err(error) => errors.push(resolution_error(
				entry.module_path,
				Some(entry.path),
				error,
				false,
			)),
		}
	}

	errors.extend(duplicate_errors(&owned_entries));
	sort_errors(&mut errors);
	if !errors.is_empty() {
		return Err(errors);
	}

	let caller_owner = resolve_app_module_owner(apps.iter(), caller_module)
		.expect("caller ownership was validated before selection");
	let mut selected = owned_entries
		.into_iter()
		.filter_map(|(entry, owner)| {
			(owner.module_path == caller_owner.module_path).then_some(entry)
		})
		.collect::<Vec<_>>();
	sort_entries(&mut selected);
	Ok(selected)
}

fn validate_entries(
	apps: &[AppModuleRegistration],
	entries: &[ServerFnInventoryEntry],
) -> Result<(), Vec<ServerFnInventoryError>> {
	let mut errors = Vec::new();
	let mut owned_entries = Vec::new();
	for entry in entries {
		match resolve_entry_owner(apps, entry) {
			Ok(owner) => owned_entries.push((entry, owner)),
			Err(error) => errors.push(resolution_error(
				entry.module_path,
				Some(entry.path),
				error,
				false,
			)),
		}
	}
	errors.extend(duplicate_errors(&owned_entries));
	sort_errors(&mut errors);
	if errors.is_empty() {
		Ok(())
	} else {
		Err(errors)
	}
}

fn resolve_entry_owner<'a>(
	apps: &'a [AppModuleRegistration],
	entry: &ServerFnInventoryEntry,
) -> Result<&'a AppModuleRegistration, AppModuleResolutionError> {
	if entry.crate_id.is_empty() {
		resolve_app_module_owner(apps.iter(), entry.module_path)
	} else {
		resolve_app_module_owner_in_crate_compat(
			apps,
			entry.module_path,
			entry.crate_id,
			entry.target_id,
		)
	}
}

fn resolve_app_module_owner_in_crate_compat<'a>(
	apps: &'a [AppModuleRegistration],
	module_path: &str,
	crate_id: &str,
	target_id: Option<&str>,
) -> Result<&'a AppModuleRegistration, AppModuleResolutionError> {
	match reinhardt_apps::resolve_app_module_owner_in_target(
		apps.iter(),
		module_path,
		crate_id,
		target_id,
	) {
		Ok(owner) => Ok(owner),
		Err(AppModuleResolutionError::Orphan) => resolve_app_module_owner(
			apps.iter()
				.filter(|app| app.crate_id.is_empty() && app.target_id.is_none()),
			module_path,
		),
		Err(error) => Err(error),
	}
}

fn compatible_owner_identity(
	owner: &AppModuleRegistration,
	caller: &AppModuleRegistration,
) -> bool {
	owner.crate_id.is_empty()
		|| (owner.crate_id == caller.crate_id && owner.target_id == caller.target_id)
}

fn resolution_error(
	module_path: &str,
	path: Option<&str>,
	error: AppModuleResolutionError,
	is_caller: bool,
) -> ServerFnInventoryError {
	match error {
		AppModuleResolutionError::Orphan if is_caller => ServerFnInventoryError::OrphanCaller {
			module_path: module_path.to_string(),
		},
		AppModuleResolutionError::Orphan => ServerFnInventoryError::OrphanFunction {
			module_path: module_path.to_string(),
			path: path
				.expect("function errors always include a path")
				.to_string(),
		},
		AppModuleResolutionError::Ambiguous(labels) => {
			let mut labels = labels.into_iter().map(str::to_string).collect::<Vec<_>>();
			labels.sort_unstable();
			ServerFnInventoryError::AmbiguousOwner {
				module_path: module_path.to_string(),
				labels,
			}
		}
	}
}

fn duplicate_errors(
	owned_entries: &[(&ServerFnInventoryEntry, &AppModuleRegistration)],
) -> Vec<ServerFnInventoryError> {
	let mut paths = BTreeMap::<(&str, &str, Option<&str>, &str, &str), Vec<&str>>::new();
	let mut names = BTreeMap::<(&str, &str, Option<&str>, &str, &str), Vec<&str>>::new();
	for (entry, owner) in owned_entries {
		paths
			.entry((
				owner.module_path,
				owner.crate_id,
				owner.target_id,
				owner.app_label,
				entry.path,
			))
			.or_default()
			.push(entry.module_path);
		names
			.entry((
				owner.module_path,
				owner.crate_id,
				owner.target_id,
				owner.app_label,
				entry.name,
			))
			.or_default()
			.push(entry.module_path);
	}

	let mut errors = Vec::new();
	for ((_module_path, _crate_id, _target_id, app_label, path), mut modules) in paths {
		if modules.len() > 1 {
			modules.sort_unstable();
			errors.push(ServerFnInventoryError::DuplicatePath {
				app_label: app_label.to_string(),
				path: path.to_string(),
				modules: modules.into_iter().map(str::to_string).collect(),
			});
		}
	}
	for ((_module_path, _crate_id, _target_id, app_label, name), mut modules) in names {
		if modules.len() > 1 {
			modules.sort_unstable();
			errors.push(ServerFnInventoryError::DuplicateName {
				app_label: app_label.to_string(),
				name: name.to_string(),
				modules: modules.into_iter().map(str::to_string).collect(),
			});
		}
	}
	errors
}

fn sort_errors(errors: &mut [ServerFnInventoryError]) {
	errors.sort_unstable_by_key(ToString::to_string);
}

fn sort_entries(entries: &mut Vec<&ServerFnInventoryEntry>) {
	entries.sort_unstable_by_key(|entry| (entry.path, entry.name, entry.module_path));
}

#[cfg(test)]
mod tests {
	use super::{
		ServerFnInventoryEntry, ServerFnInventoryError, collect_auto_server_fns_from_entries,
		select_entries_for_app, select_entries_for_app_in_crate, sort_entries, validate_entries,
	};
	use bytes::Bytes;
	use reinhardt_apps::AppModuleRegistration;
	use reinhardt_http::Request;
	use reinhardt_urls::routers::ServerRouter;
	use std::future::Future;
	use std::pin::Pin;

	use crate::server_fn::{
		ServerFnHandler, ServerFnMetadata, ServerFnRegistration, ServerFnRouterExt,
	};

	fn test_entry(
		module_path: &'static str,
		path: &'static str,
		name: &'static str,
	) -> ServerFnInventoryEntry {
		ServerFnInventoryEntry::new(module_path, path, name, passthrough)
	}

	fn target_entry(
		module_path: &'static str,
		target_id: Option<&'static str>,
		path: &'static str,
		name: &'static str,
	) -> ServerFnInventoryEntry {
		ServerFnInventoryEntry::new_in_target(
			module_path,
			"demo-crate",
			target_id,
			path,
			name,
			passthrough,
		)
	}

	fn passthrough(
		router: reinhardt_urls::routers::ServerRouter,
	) -> reinhardt_urls::routers::ServerRouter {
		router
	}

	struct RuntimeMarker;

	impl ServerFnMetadata for RuntimeMarker {
		const MODULE_PATH: &'static str = "demo::apps::polls::server_fn";
		const PATH: &'static str = "/api/polls/runtime";
		const NAME: &'static str = "runtime";
		const IS_JSON_CODEC: bool = true;
	}

	impl ServerFnRegistration for RuntimeMarker {
		fn handler() -> ServerFnHandler {
			runtime_handler
		}
	}

	fn runtime_handler(
		_request: Request,
	) -> Pin<Box<dyn Future<Output = Result<Bytes, Bytes>> + Send>> {
		Box::pin(async { Ok(Bytes::new()) })
	}

	fn register_runtime_marker(router: ServerRouter) -> ServerRouter {
		router.server_fn(RuntimeMarker)
	}

	#[test]
	fn selects_only_entries_owned_by_the_caller_app() {
		let apps = [
			AppModuleRegistration::new("polls", "demo::apps::polls"),
			AppModuleRegistration::new("users", "demo::apps::users"),
		];
		let entries = [
			test_entry("demo::apps::users::server_fn", "/api/users", "users"),
			test_entry("demo::apps::polls::server_fn", "/api/polls", "polls"),
		];

		let selected =
			select_entries_for_app(&apps, &entries, "demo::apps::polls::urls::server_router")
				.expect("polls caller should resolve");

		assert_eq!(
			selected.iter().map(|entry| entry.path).collect::<Vec<_>>(),
			["/api/polls"]
		);
	}

	#[test]
	fn does_not_select_entries_from_another_owner_with_the_same_label() {
		let apps = [
			AppModuleRegistration::new("shared", "demo::apps::first"),
			AppModuleRegistration::new("shared", "demo::apps::second"),
		];
		let entries = [
			test_entry("demo::apps::first::server_fn", "/api/first", "first"),
			test_entry("demo::apps::second::server_fn", "/api/second", "second"),
		];

		let selected =
			select_entries_for_app(&apps, &entries, "demo::apps::first::urls::server_router")
				.expect("first caller should resolve");

		assert_eq!(
			selected.iter().map(|entry| entry.path).collect::<Vec<_>>(),
			["/api/first"]
		);
	}

	#[test]
	fn selects_entries_only_from_the_callers_compiled_target() {
		let apps = [
			AppModuleRegistration::new_in_target("polls", "demo::apps::polls", "demo-crate", None),
			AppModuleRegistration::new_in_target(
				"polls",
				"demo::apps::polls",
				"demo-crate",
				Some("demo"),
			),
		];
		let entries = [
			target_entry(
				"demo::apps::polls::server_fn",
				None,
				"/api/library",
				"library",
			),
			target_entry(
				"demo::apps::polls::server_fn",
				Some("demo"),
				"/api/binary",
				"binary",
			),
		];

		let selected = select_entries_for_app_in_crate(
			&apps,
			&entries,
			"demo::apps::polls::urls::server_router",
			"demo-crate",
			Some("demo"),
		)
		.expect("the binary target should resolve independently");

		assert_eq!(
			selected.iter().map(|entry| entry.path).collect::<Vec<_>>(),
			["/api/binary"]
		);
	}

	#[test]
	fn accepts_legacy_crate_agnostic_inventory_entries() {
		let apps = [AppModuleRegistration::new_in_target(
			"polls",
			"demo::apps::polls",
			"demo-crate",
			None,
		)];
		let entries = [test_entry(
			"demo::apps::polls::server_fn",
			"/api/legacy",
			"legacy",
		)];

		assert_eq!(validate_entries(&apps, &entries), Ok(()));
	}

	#[test]
	fn reports_an_orphan_caller() {
		let error = select_entries_for_app(
			&[AppModuleRegistration::new("polls", "demo::apps::polls")],
			&[],
			"demo::outside::urls::server_router",
		)
		.expect_err("unowned caller must fail");

		assert_eq!(
			error,
			vec![ServerFnInventoryError::OrphanCaller {
				module_path: "demo::outside::urls::server_router".to_string(),
			}]
		);
	}

	#[test]
	fn reports_an_orphan_function() {
		let error = select_entries_for_app(
			&[AppModuleRegistration::new("polls", "demo::apps::polls")],
			&[test_entry(
				"demo::outside::server_fn",
				"/api/outside",
				"outside",
			)],
			"demo::apps::polls::urls::server_router",
		)
		.expect_err("unowned function must fail");

		assert_eq!(
			error,
			vec![ServerFnInventoryError::OrphanFunction {
				module_path: "demo::outside::server_fn".to_string(),
				path: "/api/outside".to_string(),
			}]
		);
	}

	#[test]
	fn selects_entries_for_the_most_specific_nested_owner() {
		let apps = [
			AppModuleRegistration::new("polls", "demo::apps::polls"),
			AppModuleRegistration::new("admin", "demo::apps::polls::admin"),
		];
		let entries = [
			test_entry("demo::apps::polls::server_fn", "/api/polls", "polls"),
			test_entry("demo::apps::polls::admin::server_fn", "/api/admin", "admin"),
		];

		let selected = select_entries_for_app(
			&apps,
			&entries,
			"demo::apps::polls::admin::urls::server_router",
		)
		.expect("nested caller should resolve");

		assert_eq!(
			selected.iter().map(|entry| entry.path).collect::<Vec<_>>(),
			["/api/admin"]
		);
	}

	#[test]
	fn does_not_treat_partial_module_components_as_ownership() {
		let error = select_entries_for_app(
			&[AppModuleRegistration::new("bar", "demo::apps::bar")],
			&[test_entry(
				"demo::apps::barista::server_fn",
				"/api/barista",
				"barista",
			)],
			"demo::apps::bar::urls::server_router",
		)
		.expect_err("bar must not own barista");

		assert_eq!(
			error,
			vec![ServerFnInventoryError::OrphanFunction {
				module_path: "demo::apps::barista::server_fn".to_string(),
				path: "/api/barista".to_string(),
			}]
		);
	}

	#[test]
	fn sorts_selected_entries_by_path_name_and_module_path() {
		let entries = [
			test_entry("demo::apps::polls::z", "/api/b", "z"),
			test_entry("demo::apps::polls::y", "/api/a", "a"),
			test_entry("demo::apps::polls::x", "/api/a", "a"),
		];
		let mut selected = entries.iter().collect::<Vec<_>>();

		sort_entries(&mut selected);

		assert_eq!(
			selected
				.iter()
				.map(|entry| (entry.path, entry.name, entry.module_path))
				.collect::<Vec<_>>(),
			[
				("/api/a", "a", "demo::apps::polls::x"),
				("/api/a", "a", "demo::apps::polls::y"),
				("/api/b", "z", "demo::apps::polls::z"),
			]
		);
	}

	#[test]
	fn reports_duplicate_path_with_sorted_modules() {
		let apps = [AppModuleRegistration::new("polls", "demo::apps::polls")];
		let entries = [
			test_entry("demo::apps::polls::z", "/api/shared", "z"),
			test_entry("demo::apps::polls::a", "/api/shared", "a"),
		];

		let error =
			select_entries_for_app(&apps, &entries, "demo::apps::polls::urls::server_router")
				.expect_err("duplicate paths must fail");

		assert_eq!(
			error,
			vec![ServerFnInventoryError::DuplicatePath {
				app_label: "polls".to_string(),
				path: "/api/shared".to_string(),
				modules: vec![
					"demo::apps::polls::a".to_string(),
					"demo::apps::polls::z".to_string(),
				],
			}]
		);
	}

	#[test]
	fn reports_duplicate_name_with_sorted_modules() {
		let apps = [AppModuleRegistration::new("polls", "demo::apps::polls")];
		let entries = [
			ServerFnInventoryEntry::new(
				"demo::apps::polls::z",
				"/api/first",
				"shared",
				passthrough,
			),
			ServerFnInventoryEntry::new(
				"demo::apps::polls::a",
				"/api/second",
				"shared",
				passthrough,
			),
		];

		let error =
			select_entries_for_app(&apps, &entries, "demo::apps::polls::urls::server_router")
				.expect_err("duplicate names must fail");

		assert_eq!(
			error,
			vec![ServerFnInventoryError::DuplicateName {
				app_label: "polls".to_string(),
				name: "shared".to_string(),
				modules: vec![
					"demo::apps::polls::a".to_string(),
					"demo::apps::polls::z".to_string(),
				],
			}]
		);
	}

	#[test]
	fn runtime_conflict_permutations_keep_one_endpoint_and_one_error() {
		let apps = [AppModuleRegistration::new("polls", "demo::apps::polls")];
		let entries = [ServerFnInventoryEntry::new(
			RuntimeMarker::MODULE_PATH,
			RuntimeMarker::PATH,
			RuntimeMarker::NAME,
			register_runtime_marker,
		)];
		let caller_module = "demo::apps::polls::urls::server_router";

		let explicit_then_auto = collect_auto_server_fns_from_entries(
			ServerRouter::new().server_fn(RuntimeMarker),
			&apps,
			&entries,
			caller_module,
		);
		let auto_then_explicit = collect_auto_server_fns_from_entries(
			ServerRouter::new(),
			&apps,
			&entries,
			caller_module,
		)
		.server_fn(RuntimeMarker);
		let auto_then_auto = collect_auto_server_fns_from_entries(
			collect_auto_server_fns_from_entries(
				ServerRouter::new(),
				&apps,
				&entries,
				caller_module,
			),
			&apps,
			&entries,
			caller_module,
		);

		let explicit_then_auto_errors = runtime_conflict_errors(explicit_then_auto);
		let auto_then_explicit_errors = runtime_conflict_errors(auto_then_explicit);
		let auto_then_auto_errors = runtime_conflict_errors(auto_then_auto);

		assert_eq!(explicit_then_auto_errors, auto_then_explicit_errors);
		assert_eq!(explicit_then_auto_errors, auto_then_auto_errors);
	}

	fn runtime_conflict_errors(router: ServerRouter) -> Vec<String> {
		assert_eq!(router.registered_endpoints().len(), 1);
		assert_eq!(router.registered_endpoints()[0].path, RuntimeMarker::PATH,);
		let errors = router
			.validate_routes()
			.expect_err("each duplicate registration sequence must fail at startup");
		assert_eq!(errors.len(), 1);
		errors
	}
}
