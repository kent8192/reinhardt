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
		let workspace_root = metadata.workspace_root.as_std_path().to_path_buf();
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
				scanner.scan_external_module(target_key, Vec::new(), source.to_path_buf(), true)?;
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
		)
	}

	fn scan_items(
		&mut self,
		target: &TargetKey,
		module: &ModulePath,
		module_directory: &Path,
		declaring_directory: &Path,
		items: &[Item],
	) -> Result<()> {
		let attribute_aliases = reinhardt_attribute_aliases(items);
		for item in items {
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
				Item::Fn(function)
					if is_server_fn(&function.attrs, &attribute_aliases)
						&& !is_conditionally_compiled(&function.attrs) =>
				{
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
				Item::Mod(item_mod) if !is_conditionally_compiled(&item_mod.attrs) => {
					self.scan_module(
						target,
						module,
						module_directory,
						declaring_directory,
						item_mod,
					)?;
				}
				_ => {}
			}
		}
		Ok(())
	}

	fn scan_module(
		&mut self,
		target: &TargetKey,
		module: &ModulePath,
		module_directory: &Path,
		declaring_directory: &Path,
		item_mod: &ItemMod,
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
			);
		}

		let Some(path) = find_external_module(module_directory, declaring_directory, item_mod)
		else {
			return Ok(());
		};
		self.scan_external_module(target.clone(), child_module, path, false)
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
		return last.ident == expected
			|| aliases
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

fn reinhardt_attribute_aliases(items: &[Item]) -> BTreeMap<String, String> {
	let mut aliases = BTreeMap::new();
	for item in items {
		let Item::Use(item_use) = item else {
			continue;
		};
		collect_reinhardt_attribute_aliases(&item_use.tree, &mut Vec::new(), &mut aliases);
	}
	aliases
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
			prefix.pop();
		}
		UseTree::Rename(rename) => {
			prefix.push(rename.ident.to_string());
			if let Some(attribute) = reinhardt_attribute_from_path(prefix) {
				aliases.insert(rename.rename.to_string(), attribute.to_owned());
			}
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
