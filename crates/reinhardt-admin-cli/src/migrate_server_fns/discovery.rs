use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use cargo_metadata::{MetadataCommand, PackageId};
use syn::{Attribute, Expr, ExprLit, Item, ItemMod, Lit, Meta, Token, UseTree};

use super::{MigrateServerFnsError, Result};

pub(crate) type ModulePath = Vec<String>;
pub(crate) type TargetKey = String;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ServerFnKey {
	pub(crate) target: TargetKey,
	pub(crate) module: ModulePath,
	pub(crate) function: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ServerFn {
	pub(crate) auto_register: bool,
}

pub(crate) type ServerFnIndex = BTreeMap<ServerFnKey, Vec<ServerFn>>;
pub(crate) type AppModuleIndex = BTreeMap<TargetKey, Vec<ModulePath>>;

#[derive(Clone, Debug)]
pub(crate) struct SourceModule {
	pub(crate) target: TargetKey,
	pub(crate) module: ModulePath,
	pub(crate) path: PathBuf,
	pub(crate) relative_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct ProjectIndex {
	pub(crate) app_modules: AppModuleIndex,
	pub(crate) server_fns: ServerFnIndex,
	pub(crate) source_modules: Vec<SourceModule>,
}

impl ProjectIndex {
	pub(crate) fn discover(path: &Path) -> Result<Self> {
		let metadata = MetadataCommand::new().no_deps().current_dir(path).exec()?;
		let workspace_root = metadata
			.workspace_root
			.as_std_path()
			.canonicalize()
			.map_err(|source| MigrateServerFnsError::Io {
				path: metadata.workspace_root.as_std_path().to_path_buf(),
				source,
			})?;
		let workspace_members: BTreeSet<&PackageId> = metadata.workspace_members.iter().collect();
		let mut scanner = Scanner {
			workspace_root,
			app_modules: BTreeMap::new(),
			server_fns: BTreeMap::new(),
			source_modules: Vec::new(),
			visited: BTreeSet::new(),
		};

		for package in metadata
			.packages
			.iter()
			.filter(|package| workspace_members.contains(&package.id))
		{
			for target in &package.targets {
				let source = target.src_path.as_std_path();
				if !source.is_file() {
					continue;
				}
				let target_key = format!("{}::{}::{}", package.id, target.name, source.display());
				scanner.scan_external_module(
					target_key,
					Vec::new(),
					source.to_path_buf(),
					true,
					&BTreeMap::new(),
				)?;
			}
		}

		scanner
			.source_modules
			.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
		scanner.source_modules.dedup_by(|left, right| {
			left.path == right.path && left.target == right.target && left.module == right.module
		});
		Ok(Self {
			app_modules: scanner.app_modules,
			server_fns: scanner.server_fns,
			source_modules: scanner.source_modules,
		})
	}
}

struct Scanner {
	workspace_root: PathBuf,
	app_modules: AppModuleIndex,
	server_fns: ServerFnIndex,
	source_modules: Vec<SourceModule>,
	visited: BTreeSet<(TargetKey, ModulePath, PathBuf)>,
}

impl Scanner {
	fn scan_external_module(
		&mut self,
		target: TargetKey,
		module: ModulePath,
		path: PathBuf,
		is_target_root: bool,
		inherited_attribute_aliases: &BTreeMap<String, String>,
	) -> Result<()> {
		let path = path
			.canonicalize()
			.map_err(|source| MigrateServerFnsError::Io {
				path: path.clone(),
				source,
			})?;
		if !self
			.visited
			.insert((target.clone(), module.clone(), path.clone()))
		{
			return Ok(());
		}
		let relative_path = path
			.strip_prefix(&self.workspace_root)
			.map_err(|_| MigrateServerFnsError::TargetOutsideWorkspace {
				path: path.clone(),
				root: self.workspace_root.clone(),
			})?
			.to_path_buf();
		let source = fs::read_to_string(&path).map_err(|source| MigrateServerFnsError::Io {
			path: path.clone(),
			source,
		})?;
		let parsed = syn::parse_file(&source).map_err(|source| MigrateServerFnsError::Parse {
			path: path.clone(),
			source,
		})?;
		self.source_modules.push(SourceModule {
			target: target.clone(),
			module: module.clone(),
			path: path.clone(),
			relative_path,
		});
		let declaring_directory = path.parent().unwrap_or(Path::new(""));
		let module_directory = child_module_directory(&path, is_target_root);
		self.scan_items(
			&target,
			&module,
			&module_directory,
			declaring_directory,
			&parsed.items,
			inherited_attribute_aliases,
		)
	}

	fn scan_items(
		&mut self,
		target: &TargetKey,
		module: &ModulePath,
		module_directory: &Path,
		declaring_directory: &Path,
		items: &[Item],
		inherited_attribute_aliases: &BTreeMap<String, String>,
	) -> Result<()> {
		let attribute_aliases = reinhardt_attribute_aliases(items, inherited_attribute_aliases);
		let mut visible_attribute_aliases = inherited_attribute_aliases.clone();
		visible_attribute_aliases.extend(attribute_aliases.clone());
		for item in items {
			if has_unresolved_item_attribute(item_attributes(item), &attribute_aliases) {
				// An attribute macro may generate an automatically registered server
				// function that source-only discovery cannot inspect.
				self.record_incomplete_server_fn_coverage(target, module);
				continue;
			}
			match item {
				Item::Macro(item_macro) if !item_macro.mac.path.is_ident("macro_rules") => {
					// An item macro may expand to an automatically registered server function.
					// Keep the migration conservative until expansion can be inspected.
					let key = ServerFnKey {
						target: target.clone(),
						module: module.clone(),
						function: "__reinhardt_unexpanded_item_macro__".to_owned(),
					};
					self.server_fns.entry(key).or_default().push(ServerFn {
						auto_register: true,
					});
				}
				Item::Struct(item_struct)
					if is_app_config(&item_struct.attrs, &attribute_aliases)
						&& !is_conditionally_compiled(&item_struct.attrs) =>
				{
					self.app_modules
						.entry(target.clone())
						.or_default()
						.push(module.clone());
				}
				Item::Struct(item_struct)
					if is_app_config(&item_struct.attrs, &attribute_aliases)
						&& is_conditionally_compiled(&item_struct.attrs) =>
				{
					self.record_incomplete_server_fn_coverage(target, module);
				}
				Item::Fn(function) if is_server_fn(&function.attrs, &attribute_aliases) => {
					if is_conditionally_compiled(&function.attrs) {
						self.record_incomplete_server_fn_coverage(target, module);
					} else {
						let key = ServerFnKey {
							target: target.clone(),
							module: module.clone(),
							function: function.sig.ident.to_string(),
						};
						self.server_fns.entry(key).or_default().push(ServerFn {
							auto_register: server_fn_auto_registers(
								&function.attrs,
								&attribute_aliases,
							),
						});
					}
				}
				Item::Fn(function)
					if has_cfg_attr_server_fn(&function.attrs, &attribute_aliases)
						|| has_unknown_server_fn_attribute(&function.attrs, &attribute_aliases) =>
				{
					self.record_incomplete_server_fn_coverage(target, module);
				}
				Item::Mod(item_mod)
					if is_conditionally_compiled(&item_mod.attrs)
						|| has_unresolved_module_attribute(&item_mod.attrs) =>
				{
					// A conditionally compiled module can contribute server functions in a
					// configuration the migration cannot inspect.
					self.record_incomplete_server_fn_coverage(target, module);
				}
				Item::Mod(item_mod) if !is_conditionally_compiled(&item_mod.attrs) => {
					self.scan_module(
						target,
						module,
						module_directory,
						declaring_directory,
						item_mod,
						&visible_attribute_aliases,
					)?;
				}
				_ => {}
			}
		}
		Ok(())
	}

	fn record_incomplete_server_fn_coverage(&mut self, target: &TargetKey, module: &ModulePath) {
		let key = ServerFnKey {
			target: target.clone(),
			module: module.clone(),
			function: "__reinhardt_incomplete_server_fn_coverage__".to_owned(),
		};
		self.server_fns.entry(key).or_default().push(ServerFn {
			auto_register: true,
		});
	}

	fn scan_module(
		&mut self,
		target: &TargetKey,
		module: &ModulePath,
		module_directory: &Path,
		declaring_directory: &Path,
		item_mod: &ItemMod,
		inherited_attribute_aliases: &BTreeMap<String, String>,
	) -> Result<()> {
		let mut child_module = module.clone();
		child_module.push(item_mod.ident.to_string());
		if let Some((_, items)) = &item_mod.content {
			let child_directory = path_attribute(&item_mod.attrs).map_or_else(
				|| module_directory.join(item_mod.ident.to_string()),
				|path| declaring_directory.join(path),
			);
			return self.scan_items(
				target,
				&child_module,
				&child_directory,
				&child_directory,
				items,
				inherited_attribute_aliases,
			);
		}

		let Some(path) = find_external_module(module_directory, declaring_directory, item_mod)
		else {
			return Ok(());
		};
		self.scan_external_module(
			target.clone(),
			child_module,
			path,
			false,
			inherited_attribute_aliases,
		)
	}
}

fn child_module_directory(path: &Path, is_target_root: bool) -> PathBuf {
	let parent = path.parent().unwrap_or(Path::new(""));
	if is_target_root {
		return parent.to_path_buf();
	}
	match path.file_name().and_then(|name| name.to_str()) {
		Some("mod.rs") => parent.to_path_buf(),
		_ => parent.join(path.file_stem().unwrap_or_default()),
	}
}

fn find_external_module(
	module_directory: &Path,
	declaring_directory: &Path,
	item_mod: &ItemMod,
) -> Option<PathBuf> {
	if let Some(explicit) = path_attribute(&item_mod.attrs) {
		let candidate = declaring_directory.join(explicit);
		return candidate.is_file().then_some(candidate);
	}

	let name = item_mod.ident.to_string();
	let flat = module_directory.join(format!("{name}.rs"));
	if flat.is_file() {
		return Some(flat);
	}
	let legacy = module_directory.join(name).join("mod.rs");
	legacy.is_file().then_some(legacy)
}

fn path_attribute(attributes: &[Attribute]) -> Option<PathBuf> {
	attributes.iter().find_map(|attribute| {
		if !attribute.path().is_ident("path") {
			return None;
		}
		let Meta::NameValue(name_value) = &attribute.meta else {
			return None;
		};
		let Expr::Lit(ExprLit {
			lit: Lit::Str(path),
			..
		}) = &name_value.value
		else {
			return None;
		};
		Some(PathBuf::from(path.value()))
	})
}

fn is_server_fn(attributes: &[Attribute], aliases: &BTreeMap<String, String>) -> bool {
	attributes
		.iter()
		.any(|attribute| is_reinhardt_attribute(attribute, "server_fn", aliases))
}

fn is_app_config(attributes: &[Attribute], aliases: &BTreeMap<String, String>) -> bool {
	attributes
		.iter()
		.any(|attribute| is_reinhardt_attribute(attribute, "app_config", aliases))
}

fn has_unknown_server_fn_attribute(
	attributes: &[Attribute],
	aliases: &BTreeMap<String, String>,
) -> bool {
	attributes.iter().any(|attribute| {
		let segments = &attribute.path().segments;
		let Some(last) = segments.last() else {
			return false;
		};
		if segments.len() == 1 {
			return last.ident == "server_fn"
				&& !is_reinhardt_attribute(attribute, "server_fn", aliases)
				|| aliases
					.get(&last.ident.to_string())
					.is_some_and(|source| source == "__reinhardt_unknown_server_fn__");
		}
		last.ident == "server_fn" && !is_reinhardt_attribute(attribute, "server_fn", aliases)
	})
}

fn is_reinhardt_attribute(
	attribute: &Attribute,
	expected: &str,
	aliases: &BTreeMap<String, String>,
) -> bool {
	let segments = &attribute.path().segments;
	let Some(last) = segments.last() else {
		return false;
	};
	if segments.len() == 1 {
		if expected == "app_config" {
			return aliases
				.get(&last.ident.to_string())
				.map_or(last.ident == expected, |resolved| resolved == expected);
		}
		return aliases
			.get(&last.ident.to_string())
			.is_some_and(|resolved| resolved == expected);
	}
	last.ident == expected
		&& segments.first().is_some_and(|segment| {
			matches!(
				segment.ident.to_string().as_str(),
				"reinhardt" | "reinhardt_pages"
			)
		})
}

fn reinhardt_attribute_aliases(
	items: &[Item],
	inherited: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
	let mut aliases = BTreeMap::new();
	for item in items {
		let Item::Use(item_use) = item else {
			continue;
		};
		if is_conditionally_compiled(&item_use.attrs) {
			collect_conditional_attribute_aliases(&item_use.tree, &mut aliases);
			continue;
		}
		collect_reinhardt_attribute_aliases(&item_use.tree, &mut Vec::new(), &mut aliases);
		collect_inherited_attribute_aliases(
			&item_use.tree,
			&mut Vec::new(),
			inherited,
			&mut aliases,
		);
	}
	aliases
}

fn collect_conditional_attribute_aliases(tree: &UseTree, aliases: &mut BTreeMap<String, String>) {
	match tree {
		UseTree::Path(path) => collect_conditional_attribute_aliases(&path.tree, aliases),
		UseTree::Name(name) => {
			if name.ident != "self" {
				aliases.insert(
					name.ident.to_string(),
					"__reinhardt_unknown_attribute__".to_owned(),
				);
			}
		}
		UseTree::Rename(rename) => {
			aliases.insert(
				rename.rename.to_string(),
				"__reinhardt_unknown_attribute__".to_owned(),
			);
		}
		UseTree::Group(group) => {
			for item in &group.items {
				collect_conditional_attribute_aliases(item, aliases);
			}
		}
		UseTree::Glob(_) => {}
	}
}

fn collect_inherited_attribute_aliases(
	tree: &UseTree,
	prefix: &mut Vec<String>,
	inherited: &BTreeMap<String, String>,
	aliases: &mut BTreeMap<String, String>,
) {
	match tree {
		UseTree::Path(path) => {
			prefix.push(path.ident.to_string());
			collect_inherited_attribute_aliases(&path.tree, prefix, inherited, aliases);
			prefix.pop();
		}
		UseTree::Name(name) => {
			let mut path = prefix.clone();
			if name.ident != "self" {
				path.push(name.ident.to_string());
			}
			let binding = if name.ident == "self" {
				prefix.last().cloned()
			} else {
				Some(name.ident.to_string())
			};
			if let Some(binding) = binding {
				record_inherited_attribute_alias(&path, binding, inherited, aliases);
			}
		}
		UseTree::Rename(rename) => {
			let mut path = prefix.clone();
			if rename.ident != "self" {
				path.push(rename.ident.to_string());
			}
			record_inherited_attribute_alias(&path, rename.rename.to_string(), inherited, aliases);
		}
		UseTree::Glob(_) => {
			if prefix.iter().all(|segment| segment == "super") {
				aliases.extend(inherited.clone());
			}
		}
		UseTree::Group(group) => {
			for item in &group.items {
				collect_inherited_attribute_aliases(item, prefix, inherited, aliases);
			}
		}
	}
}

fn record_inherited_attribute_alias(
	path: &[String],
	binding: String,
	inherited: &BTreeMap<String, String>,
	aliases: &mut BTreeMap<String, String>,
) {
	let Some(source) = path.last() else {
		return;
	};
	let parent = &path[..path.len() - 1];
	if parent == ["super"] {
		aliases.insert(
			binding,
			inherited
				.get(source)
				.cloned()
				.unwrap_or_else(|| "__reinhardt_unknown_server_fn__".to_owned()),
		);
		return;
	}
	if parent.iter().all(|segment| segment == "super")
		|| parent.first().is_some_and(|part| part == "crate")
	{
		aliases.insert(binding, "__reinhardt_unknown_server_fn__".to_owned());
	}
}

fn collect_reinhardt_attribute_aliases(
	tree: &UseTree,
	prefix: &mut Vec<String>,
	aliases: &mut BTreeMap<String, String>,
) {
	match tree {
		UseTree::Path(path) => {
			prefix.push(path.ident.to_string());
			collect_reinhardt_attribute_aliases(&path.tree, prefix, aliases);
			prefix.pop();
		}
		UseTree::Name(name) => {
			prefix.push(name.ident.to_string());
			record_attribute_alias(prefix, &name.ident.to_string(), aliases);
			prefix.pop();
		}
		UseTree::Rename(rename) => {
			prefix.push(rename.ident.to_string());
			record_attribute_alias(prefix, &rename.rename.to_string(), aliases);
			prefix.pop();
		}
		UseTree::Group(group) => {
			for item in &group.items {
				collect_reinhardt_attribute_aliases(item, prefix, aliases);
			}
		}
		UseTree::Glob(_) => {}
	}
}

fn record_attribute_alias(path: &[String], binding: &str, aliases: &mut BTreeMap<String, String>) {
	if let Some(attribute) = reinhardt_attribute_from_path(path) {
		aliases.insert(binding.to_owned(), attribute.to_owned());
	} else if path.last().is_some_and(|segment| segment == "server_fn") {
		aliases.insert(
			binding.to_owned(),
			"__reinhardt_unknown_server_fn__".to_owned(),
		);
	} else if path.last().is_some_and(|segment| segment == "app_config") {
		aliases.insert(
			binding.to_owned(),
			"__reinhardt_unknown_app_config__".to_owned(),
		);
	}
}

fn reinhardt_attribute_from_path(path: &[String]) -> Option<&'static str> {
	let (first, last) = (path.first()?, path.last()?);
	if !matches!(first.as_str(), "reinhardt" | "reinhardt_pages") {
		return None;
	}
	match last.as_str() {
		"server_fn" => Some("server_fn"),
		"app_config" => Some("app_config"),
		_ => None,
	}
}

/// Returns whether an item is subject to conditional compilation.
///
/// The migration command cannot evaluate the target application's feature and
/// target configuration. Treating these items as unavailable is conservative:
/// it prevents a rewrite that would only be valid in one configuration.
fn is_conditionally_compiled(attributes: &[Attribute]) -> bool {
	attributes
		.iter()
		.any(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}

fn has_unresolved_module_attribute(attributes: &[Attribute]) -> bool {
	attributes.iter().any(|attribute| {
		!matches!(
			attribute
				.path()
				.get_ident()
				.map(|ident| ident.to_string())
				.as_deref(),
			Some("cfg" | "cfg_attr" | "path" | "doc" | "allow" | "warn" | "deny" | "forbid")
		)
	})
}

fn has_unresolved_item_attribute(
	attributes: &[Attribute],
	aliases: &BTreeMap<String, String>,
) -> bool {
	attributes.iter().any(|attribute| {
		if is_reinhardt_attribute(attribute, "server_fn", aliases)
			|| is_reinhardt_attribute(attribute, "app_config", aliases)
		{
			return false;
		}
		!matches!(
			attribute
				.path()
				.get_ident()
				.map(|ident| ident.to_string())
				.as_deref(),
			Some(
				"allow"
					| "automatically_derived"
					| "cfg" | "cfg_attr"
					| "cold" | "deny"
					| "deprecated" | "derive"
					| "doc" | "export_name"
					| "forbid" | "inline"
					| "link" | "link_name"
					| "link_section"
					| "macro_export"
					| "must_use" | "no_mangle"
					| "non_exhaustive"
					| "path" | "repr"
					| "should_panic"
					| "test" | "track_caller"
					| "unsafe" | "used"
					| "warn"
			)
		)
	})
}

fn item_attributes(item: &Item) -> &[Attribute] {
	match item {
		Item::Const(item) => &item.attrs,
		Item::Enum(item) => &item.attrs,
		Item::ExternCrate(item) => &item.attrs,
		Item::Fn(item) => &item.attrs,
		Item::ForeignMod(item) => &item.attrs,
		Item::Impl(item) => &item.attrs,
		Item::Macro(item) => &item.attrs,
		Item::Mod(item) => &item.attrs,
		Item::Static(item) => &item.attrs,
		Item::Struct(item) => &item.attrs,
		Item::Trait(item) => &item.attrs,
		Item::TraitAlias(item) => &item.attrs,
		Item::Type(item) => &item.attrs,
		Item::Union(item) => &item.attrs,
		Item::Use(item) => &item.attrs,
		Item::Verbatim(_) => &[],
		_ => &[],
	}
}

fn has_cfg_attr_server_fn(attributes: &[Attribute], aliases: &BTreeMap<String, String>) -> bool {
	attributes.iter().any(|attribute| {
		attribute.path().is_ident("cfg_attr")
			&& cfg_attr_contains_server_fn(&attribute.meta, aliases)
	})
}

fn cfg_attr_contains_server_fn(meta: &Meta, aliases: &BTreeMap<String, String>) -> bool {
	let Meta::List(list) = meta else {
		return false;
	};
	let Ok(arguments) =
		list.parse_args_with(syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated)
	else {
		return true;
	};
	arguments.iter().skip(1).any(|argument| {
		let path = argument.path();
		let last = path
			.segments
			.last()
			.map(|segment| segment.ident.to_string());
		((path.segments.len() == 1
			&& (last.as_deref() == Some("server_fn")
				|| aliases
					.get(last.as_deref().unwrap_or_default())
					.is_some_and(|resolved| resolved == "server_fn")))
			|| (last.as_deref() == Some("server_fn")
				&& path.segments.first().is_some_and(|segment| {
					matches!(
						segment.ident.to_string().as_str(),
						"reinhardt" | "reinhardt_pages"
					)
				}))) || (path.is_ident("cfg_attr") && cfg_attr_contains_server_fn(argument, aliases))
	})
}

fn server_fn_auto_registers(attributes: &[Attribute], aliases: &BTreeMap<String, String>) -> bool {
	let Some(attribute) = attributes
		.iter()
		.find(|attribute| is_reinhardt_attribute(attribute, "server_fn", aliases))
	else {
		return true;
	};
	let Meta::List(list) = &attribute.meta else {
		return true;
	};
	let Ok(arguments) =
		list.parse_args_with(syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated)
	else {
		return true;
	};
	!arguments.iter().any(|argument| {
		let Meta::NameValue(name_value) = argument else {
			return false;
		};
		if !name_value.path.is_ident("auto_register") {
			return false;
		}
		matches!(
			&name_value.value,
			Expr::Lit(ExprLit {
				lit: Lit::Bool(value),
				..
			}) if !value.value
		)
	})
}
