use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use proc_macro2::{LineColumn, Span, TokenStream, TokenTree};
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::visit_mut::{self, VisitMut};
use syn::{
	Attribute, Expr, ExprMethodCall, ExprPath, File, Item, ItemFn, ItemUse, Macro, Path, Stmt,
	UseTree, parse_quote,
};

use super::discovery::{ModulePath, ServerFnIndex, ServerFnKey, TargetKey};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ReportKind {
	WouldRewrite,
	Rewrote,
	MixedRegistration,
	UnresolvedMarker(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Report {
	pub(crate) path: PathBuf,
	pub(crate) line: usize,
	pub(crate) kind: ReportKind,
}

impl Ord for Report {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.path
			.cmp(&other.path)
			.then_with(|| self.line.cmp(&other.line))
			.then_with(|| self.kind.cmp(&other.kind))
	}
}

impl PartialOrd for Report {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl fmt::Display for Report {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match &self.kind {
			ReportKind::WouldRewrite => {
				write!(formatter, "would rewrite: {}", self.path.display())
			}
			ReportKind::Rewrote => write!(formatter, "rewrote: {}", self.path.display()),
			ReportKind::MixedRegistration => write!(
				formatter,
				"skipped mixed registration: {}:{}",
				self.path.display(),
				self.line
			),
			ReportKind::UnresolvedMarker(name) => write!(
				formatter,
				"skipped unresolved marker `{name}`: {}:{}",
				self.path.display(),
				self.line
			),
		}
	}
}

pub(crate) struct Skipped {
	pub(crate) line: usize,
	pub(crate) kind: ReportKind,
}

pub(crate) struct RewriteOutcome {
	pub(crate) rewritten: Option<File>,
	pub(crate) skipped: Vec<Skipped>,
	pub(crate) edits: Vec<TextEdit>,
}

pub(crate) fn rewrite(
	mut file: File,
	target: &TargetKey,
	module: &ModulePath,
	server_fns: &ServerFnIndex,
) -> RewriteOutcome {
	let original = file.clone();
	let mut skipped = Vec::new();
	let mut edits = Vec::new();
	rewrite_module_items(
		&mut file.items,
		target,
		module,
		server_fns,
		&mut skipped,
		&mut edits,
	);
	RewriteOutcome {
		rewritten: (file != original).then_some(file),
		skipped,
		edits,
	}
}

fn rewrite_module_items(
	items: &mut Vec<Item>,
	target: &TargetKey,
	module: &ModulePath,
	server_fns: &ServerFnIndex,
	skipped: &mut Vec<Skipped>,
	edits: &mut Vec<TextEdit>,
) {
	let imports = ImportIndex::from_items(items, module);
	let mut removable_bindings = BTreeSet::new();

	for item in items.iter_mut() {
		match item {
			Item::Fn(function) if function.sig.ident == "server_url_patterns" => {
				match rewrite_router_function(function, target, module, server_fns, &imports) {
					FunctionOutcome::Rewritten {
						bindings,
						edits: function_edits,
					} => {
						removable_bindings.extend(bindings);
						edits.extend(function_edits);
					}
					FunctionOutcome::Skipped(skip) => skipped.push(skip),
					FunctionOutcome::Unchanged => {}
				}
			}
			Item::Mod(item_mod) => {
				if let Some((_, child_items)) = &mut item_mod.content {
					let mut child_module = module.clone();
					child_module.push(item_mod.ident.to_string());
					rewrite_module_items(
						child_items,
						target,
						&child_module,
						server_fns,
						skipped,
						edits,
					);
				}
			}
			_ => {}
		}
	}

	if removable_bindings.is_empty() {
		return;
	}
	let used = used_bindings(items, &removable_bindings);
	let removable: BTreeSet<_> = removable_bindings.difference(&used).cloned().collect();
	if removable.is_empty() {
		return;
	}
	let mut retained = Vec::with_capacity(items.len());
	for mut item in std::mem::take(items) {
		let Item::Use(item_use) = &mut item else {
			retained.push(item);
			continue;
		};
		let original = item_use.clone();
		if prune_use_tree(&mut item_use.tree, &mut Vec::new(), &removable).is_none() {
			edits.push(TextEdit::remove_item(&original));
			continue;
		}
		if item_use.tree != original.tree {
			edits.push(TextEdit::replace_item(&original, item_use));
		}
		retained.push(item);
	}
	*items = retained;
}

enum FunctionOutcome {
	Unchanged,
	Rewritten {
		bindings: BTreeSet<String>,
		edits: Vec<TextEdit>,
	},
	Skipped(Skipped),
}

fn rewrite_router_function(
	function: &mut ItemFn,
	target: &TargetKey,
	module: &ModulePath,
	server_fns: &ServerFnIndex,
	imports: &ImportIndex,
) -> FunctionOutcome {
	let mut analyzer = FunctionAnalyzer {
		target,
		module,
		server_fns,
		imports,
		chains: Vec::new(),
	};
	analyzer.visit_block(&function.block);
	if analyzer.chains.is_empty() {
		return FunctionOutcome::Unchanged;
	}

	if analyzer.chains.len() != 1 {
		let line = analyzer.chains.get(1).unwrap_or(&analyzer.chains[0]).line;
		return FunctionOutcome::Skipped(Skipped {
			line,
			kind: ReportKind::MixedRegistration,
		});
	}

	let analyzed = analyzer.chains.pop().expect("one chain was analyzed");
	if tail_registration_chain(&function.block) != Some(analyzed.location) {
		return FunctionOutcome::Skipped(Skipped {
			line: analyzed.line,
			kind: ReportKind::MixedRegistration,
		});
	}
	let (removable_bindings, edits) = match analyzed.outcome {
		ChainOutcome::AlreadyAutomatic => return FunctionOutcome::Unchanged,
		ChainOutcome::Mixed(line) => {
			return FunctionOutcome::Skipped(Skipped {
				line,
				kind: ReportKind::MixedRegistration,
			});
		}
		ChainOutcome::Unresolved { name, line } => {
			return FunctionOutcome::Skipped(Skipped {
				line,
				kind: ReportKind::UnresolvedMarker(name),
			});
		}
		ChainOutcome::Safe { bindings, edits } => (bindings, edits),
	};

	let mut transformer = ChainTransformer;
	let Stmt::Expr(tail, None) = function
		.block
		.stmts
		.last_mut()
		.expect("the analyzed tail chain exists")
	else {
		unreachable!("the analyzed chain was verified as the tail expression");
	};
	transformer.visit_expr_mut(tail);
	FunctionOutcome::Rewritten {
		bindings: removable_bindings,
		edits,
	}
}

fn tail_registration_chain(block: &syn::Block) -> Option<LineColumn> {
	let Stmt::Expr(Expr::MethodCall(method), None) = block.stmts.last()? else {
		return None;
	};
	is_registration_chain(method).then(|| method.method.span().start())
}

enum ChainOutcome {
	Safe {
		bindings: BTreeSet<String>,
		edits: Vec<TextEdit>,
	},
	Mixed(usize),
	Unresolved {
		name: String,
		line: usize,
	},
	AlreadyAutomatic,
}

struct FunctionAnalyzer<'a> {
	target: &'a TargetKey,
	module: &'a ModulePath,
	server_fns: &'a ServerFnIndex,
	imports: &'a ImportIndex,
	chains: Vec<AnalyzedChain>,
}

struct AnalyzedChain {
	outcome: ChainOutcome,
	line: usize,
	location: LineColumn,
}

impl<'ast> Visit<'ast> for FunctionAnalyzer<'_> {
	fn visit_expr(&mut self, expression: &'ast Expr) {
		if let Expr::MethodCall(method) = expression {
			let methods = receiver_chain(method);
			if is_registration_chain(method) {
				self.chains.push(AnalyzedChain {
					outcome: analyze_chain(
						&methods,
						self.target,
						self.module,
						self.server_fns,
						self.imports,
					),
					line: span_line(method.method.span()),
					location: method.method.span().start(),
				});
				for method in &methods {
					for argument in &method.args {
						self.visit_expr(argument);
					}
				}
				self.visit_expr(methods[0].receiver.as_ref());
				return;
			}
		}
		visit::visit_expr(self, expression);
	}
}

fn is_registration_chain(method: &ExprMethodCall) -> bool {
	receiver_chain(method).iter().any(|call| {
		call.method == "server_fn"
			|| call.method == "server_fnset"
			|| call.method == "auto_server_fns"
	})
}

fn receiver_chain(method: &ExprMethodCall) -> Vec<&ExprMethodCall> {
	let mut methods = vec![method];
	let mut receiver = method.receiver.as_ref();
	while let Expr::MethodCall(next) = receiver {
		methods.push(next);
		receiver = next.receiver.as_ref();
	}
	methods.reverse();
	methods
}

fn analyze_chain(
	methods: &[&ExprMethodCall],
	target: &TargetKey,
	module: &ModulePath,
	server_fns: &ServerFnIndex,
	imports: &ImportIndex,
) -> ChainOutcome {
	if methods
		.iter()
		.any(|method| method.method == "auto_server_fns")
	{
		return ChainOutcome::AlreadyAutomatic;
	}
	if let Some(method) = methods
		.iter()
		.find(|method| method.method == "server_fnset")
	{
		return ChainOutcome::Mixed(span_line(method.method.span()));
	}

	let mut bindings = BTreeSet::new();
	let server_methods: Vec<_> = methods
		.iter()
		.copied()
		.filter(|method| method.method == "server_fn")
		.collect();
	for method in &server_methods {
		let Some(argument) = single_marker_argument(method) else {
			return ChainOutcome::Unresolved {
				name: marker_name(method.args.first()),
				line: span_line(method.method.span()),
			};
		};
		let Some(resolved) = resolve_marker(argument, target, module, server_fns, imports) else {
			return ChainOutcome::Unresolved {
				name: marker_name(method.args.first()),
				line: span_line(method.method.span()),
			};
		};
		if !resolved.auto_register {
			return ChainOutcome::Mixed(span_line(method.method.span()));
		}
		if let Some(binding) = resolved.import_binding {
			bindings.insert(binding);
		}
	}
	let outer = methods.last().expect("method chains are non-empty");
	let mut edits = Vec::with_capacity(server_methods.len() + 1);
	for method in server_methods {
		let is_outer = std::ptr::eq(method, *outer);
		edits.push(TextEdit::method_suffix(
			method,
			if is_outer {
				Some(".auto_server_fns(module_path!())")
			} else {
				None
			},
		));
	}
	if outer.method != "server_fn" {
		edits.push(TextEdit::insert_after_call(
			outer,
			".auto_server_fns(module_path!())",
		));
	}
	ChainOutcome::Safe { bindings, edits }
}

fn single_marker_argument(method: &ExprMethodCall) -> Option<&ExprPath> {
	if method.args.len() != 1 {
		return None;
	}
	let Expr::Path(path) = method.args.first()? else {
		return None;
	};
	path.path
		.segments
		.last()
		.is_some_and(|segment| segment.ident == "marker")
		.then_some(path)
}

fn marker_name(argument: Option<&Expr>) -> String {
	let Some(Expr::Path(path)) = argument else {
		return argument
			.map(ToTokens::to_token_stream)
			.map(|tokens| tokens.to_string())
			.unwrap_or_else(|| "<missing>".to_owned());
	};
	let mut segments = path.path.segments.iter().rev();
	let last = segments.next();
	if last.is_some_and(|segment| segment.ident == "marker") {
		return segments
			.next()
			.map(|segment| segment.ident.to_string())
			.unwrap_or_else(|| "marker".to_owned());
	}
	last.map(|segment| segment.ident.to_string())
		.unwrap_or_else(|| "<missing>".to_owned())
}

struct ResolvedMarker {
	auto_register: bool,
	import_binding: Option<String>,
}

fn resolve_marker(
	marker: &ExprPath,
	target: &TargetKey,
	module: &ModulePath,
	server_fns: &ServerFnIndex,
	imports: &ImportIndex,
) -> Option<ResolvedMarker> {
	let mut components: Vec<String> = marker
		.path
		.segments
		.iter()
		.map(|segment| segment.ident.to_string())
		.collect();
	components.pop();
	if components.is_empty() || imports.has_glob {
		return None;
	}

	let first = components.first()?.clone();
	let (candidates, import_binding) = if matches!(first.as_str(), "crate" | "self" | "super") {
		(
			vec![normalize_components(&components, module, false)?],
			None,
		)
	} else if let Some(imported) = imports.bindings.get(&first) {
		if imported.len() != 1 {
			return None;
		}
		let mut candidate = imported[0].clone();
		candidate.extend(components.into_iter().skip(1));
		(vec![candidate], Some(first))
	} else {
		let mut candidate = module.clone();
		candidate.extend(components);
		(vec![candidate], None)
	};

	let mut resolved = Vec::new();
	for mut candidate in candidates {
		let function = candidate.pop()?;
		let key = ServerFnKey {
			target: target.clone(),
			module: candidate,
			function,
		};
		if let Some(entries) = server_fns.get(&key) {
			resolved.extend(entries);
		}
	}
	if resolved.len() != 1 {
		return None;
	}
	Some(ResolvedMarker {
		auto_register: resolved[0].auto_register,
		import_binding,
	})
}

struct ChainTransformer;

impl VisitMut for ChainTransformer {
	fn visit_expr_mut(&mut self, expression: &mut Expr) {
		let Expr::MethodCall(method) = expression else {
			visit_mut::visit_expr_mut(self, expression);
			return;
		};
		let methods = receiver_chain(method);
		if !methods.iter().any(|method| method.method == "server_fn") {
			visit_mut::visit_expr_mut(self, expression);
			return;
		}

		visit_chain_children(method, self);
		let original = std::mem::replace(expression, parse_quote!(()));
		let cleaned = remove_server_fn_calls(original);
		let mut automatic: ExprMethodCall =
			parse_quote!(__reinhardt_router.auto_server_fns(module_path!()));
		automatic.receiver = Box::new(cleaned);
		*expression = Expr::MethodCall(automatic);
	}
}

fn visit_chain_children(method: &mut ExprMethodCall, transformer: &mut ChainTransformer) {
	for argument in &mut method.args {
		transformer.visit_expr_mut(argument);
	}
	match method.receiver.as_mut() {
		Expr::MethodCall(receiver) => visit_chain_children(receiver, transformer),
		receiver => transformer.visit_expr_mut(receiver),
	}
}

fn remove_server_fn_calls(expression: Expr) -> Expr {
	let Expr::MethodCall(mut method) = expression else {
		return expression;
	};
	let receiver = remove_server_fn_calls(*method.receiver);
	if method.method == "server_fn" {
		return receiver;
	}
	method.receiver = Box::new(receiver);
	Expr::MethodCall(method)
}

#[derive(Default)]
struct ImportIndex {
	bindings: BTreeMap<String, Vec<ModulePath>>,
	has_glob: bool,
}

impl ImportIndex {
	fn from_items(items: &[Item], module: &ModulePath) -> Self {
		let mut index = Self::default();
		for item in items {
			let Item::Use(item_use) = item else {
				continue;
			};
			let mut leaves = Vec::new();
			flatten_use_tree(
				&item_use.tree,
				&mut Vec::new(),
				&mut leaves,
				&mut index.has_glob,
			);
			for leaf in leaves {
				let Some(canonical) = normalize_components(&leaf.path, module, true) else {
					continue;
				};
				index
					.bindings
					.entry(leaf.binding)
					.or_default()
					.push(canonical);
			}
		}
		index
	}
}

struct ImportLeaf {
	binding: String,
	path: Vec<String>,
}

fn flatten_use_tree(
	tree: &UseTree,
	prefix: &mut Vec<String>,
	leaves: &mut Vec<ImportLeaf>,
	has_glob: &mut bool,
) {
	match tree {
		UseTree::Path(path) => {
			prefix.push(path.ident.to_string());
			flatten_use_tree(&path.tree, prefix, leaves, has_glob);
			prefix.pop();
		}
		UseTree::Name(name) => {
			let ident = name.ident.to_string();
			if ident == "self" {
				if let Some(binding) = prefix.last() {
					leaves.push(ImportLeaf {
						binding: binding.clone(),
						path: prefix.clone(),
					});
				}
			} else {
				let mut path = prefix.clone();
				path.push(ident.clone());
				leaves.push(ImportLeaf {
					binding: ident,
					path,
				});
			}
		}
		UseTree::Rename(rename) => {
			let mut path = prefix.clone();
			if rename.ident != "self" {
				path.push(rename.ident.to_string());
			}
			leaves.push(ImportLeaf {
				binding: rename.rename.to_string(),
				path,
			});
		}
		UseTree::Glob(_) => *has_glob = true,
		UseTree::Group(group) => {
			for item in &group.items {
				flatten_use_tree(item, prefix, leaves, has_glob);
			}
		}
	}
}

fn normalize_components(
	components: &[String],
	current: &ModulePath,
	use_path: bool,
) -> Option<ModulePath> {
	let mut canonical;
	let mut offset = 0;
	match components.first().map(String::as_str) {
		Some("crate") => {
			canonical = Vec::new();
			offset = 1;
		}
		Some("self") => {
			canonical = current.clone();
			offset = 1;
		}
		Some("super") => {
			canonical = current.clone();
			while components.get(offset).is_some_and(|part| part == "super") {
				canonical.pop()?;
				offset += 1;
			}
		}
		Some(_) if use_path => canonical = Vec::new(),
		Some(_) => canonical = current.clone(),
		None => return None,
	}
	canonical.extend(components.iter().skip(offset).cloned());
	Some(canonical)
}

fn used_bindings(items: &[Item], candidates: &BTreeSet<String>) -> BTreeSet<String> {
	let mut visitor = BindingUseVisitor {
		candidates,
		used: BTreeSet::new(),
	};
	for item in items {
		if let Item::Use(item_use) = item {
			collect_use_tree_binding_uses(&item_use.tree, candidates, &mut visitor.used);
		} else {
			visitor.visit_item(item);
		}
	}
	visitor.used
}

fn collect_use_tree_binding_uses(
	tree: &UseTree,
	candidates: &BTreeSet<String>,
	used: &mut BTreeSet<String>,
) {
	let mut leaves = Vec::new();
	let mut has_glob = false;
	flatten_use_tree(tree, &mut Vec::new(), &mut leaves, &mut has_glob);
	for leaf in leaves {
		let Some(referenced) = use_tree_local_reference(&leaf.path) else {
			continue;
		};
		let defines_same_binding =
			leaf.binding == *referenced && leaf.path.last().is_some_and(|last| last == referenced);
		if candidates.contains(referenced) && !defines_same_binding {
			used.insert(referenced.clone());
		}
	}
}

fn use_tree_local_reference(path: &[String]) -> Option<&String> {
	match path.first().map(String::as_str) {
		Some("crate" | "super") | None => None,
		Some("self") => path.get(1),
		Some(_) => path.first(),
	}
}

struct BindingUseVisitor<'a> {
	candidates: &'a BTreeSet<String>,
	used: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for BindingUseVisitor<'_> {
	fn visit_path(&mut self, path: &'ast Path) {
		if let Some(first) = path.segments.first() {
			let binding = first.ident.to_string();
			if self.candidates.contains(&binding) {
				self.used.insert(binding);
			}
		}
		visit::visit_path(self, path);
	}

	fn visit_macro(&mut self, mac: &'ast Macro) {
		collect_token_binding_uses(mac.tokens.clone(), self.candidates, &mut self.used);
		visit::visit_macro(self, mac);
	}

	fn visit_attribute(&mut self, attribute: &'ast Attribute) {
		collect_token_binding_uses(
			attribute.meta.to_token_stream(),
			self.candidates,
			&mut self.used,
		);
		visit::visit_attribute(self, attribute);
	}

	fn visit_item_use(&mut self, _item_use: &'ast ItemUse) {}
}

fn collect_token_binding_uses(
	tokens: TokenStream,
	candidates: &BTreeSet<String>,
	used: &mut BTreeSet<String>,
) {
	for token in tokens {
		match token {
			TokenTree::Group(group) => {
				collect_token_binding_uses(group.stream(), candidates, used);
			}
			TokenTree::Ident(ident) => {
				let binding = ident.to_string();
				if candidates.contains(&binding) {
					used.insert(binding);
				}
			}
			TokenTree::Literal(_) | TokenTree::Punct(_) => {}
		}
	}
}

fn prune_use_tree(
	tree: &mut UseTree,
	prefix: &mut Vec<String>,
	removable: &BTreeSet<String>,
) -> Option<()> {
	match tree {
		UseTree::Path(path) => {
			prefix.push(path.ident.to_string());
			let retained = prune_use_tree(&mut path.tree, prefix, removable);
			prefix.pop();
			retained
		}
		UseTree::Name(name) => {
			let name = name.ident.to_string();
			let binding = if name == "self" {
				prefix.last().unwrap_or(&name)
			} else {
				&name
			};
			(!removable.contains(binding)).then_some(())
		}
		UseTree::Rename(rename) => (!removable.contains(&rename.rename.to_string())).then_some(()),
		UseTree::Glob(_) => Some(()),
		UseTree::Group(group) => {
			let mut retained = syn::punctuated::Punctuated::new();
			for mut item in std::mem::take(&mut group.items) {
				if prune_use_tree(&mut item, prefix, removable).is_some() {
					retained.push(item);
				}
			}
			group.items = retained;
			(!group.items.is_empty()).then_some(())
		}
	}
}

fn span_line(span: Span) -> usize {
	span.start().line
}

#[derive(Clone)]
pub(crate) struct TextEdit {
	start: LineColumn,
	end: LineColumn,
	replacement: String,
	kind: TextEditKind,
}

#[derive(Clone, Copy)]
enum TextEditKind {
	Exact,
	MethodSuffix,
	WholeLine,
}

impl TextEdit {
	fn method_suffix(method: &ExprMethodCall, replacement: Option<&str>) -> Self {
		Self {
			start: method.method.span().start(),
			end: method.paren_token.span.close().end(),
			replacement: replacement.unwrap_or_default().to_owned(),
			kind: TextEditKind::MethodSuffix,
		}
	}

	fn insert_after_call(method: &ExprMethodCall, replacement: &str) -> Self {
		let position = method.paren_token.span.close().end();
		Self {
			start: position,
			end: position,
			replacement: replacement.to_owned(),
			kind: TextEditKind::Exact,
		}
	}

	fn remove_item(item: &ItemUse) -> Self {
		Self {
			start: item_start(item),
			end: item.span().end(),
			replacement: String::new(),
			kind: TextEditKind::WholeLine,
		}
	}

	fn replace_item(original: &ItemUse, replacement: &ItemUse) -> Self {
		let formatted = prettyplease::unparse(&File {
			shebang: None,
			attrs: Vec::new(),
			items: vec![Item::Use(replacement.clone())],
		});
		Self {
			start: item_start(original),
			end: original.span().end(),
			replacement: formatted.trim_end().to_owned(),
			kind: TextEditKind::Exact,
		}
	}
}

fn item_start(item: &ItemUse) -> LineColumn {
	item.attrs
		.first()
		.map_or_else(|| item.span().start(), |attribute| attribute.span().start())
}

pub(crate) fn apply_text_edits(source: &str, edits: &[TextEdit]) -> Option<String> {
	if edits.is_empty() {
		return None;
	}
	let line_starts = line_starts(source);
	let mut byte_edits = Vec::with_capacity(edits.len());
	for edit in edits {
		let mut start = byte_offset(source, &line_starts, edit.start)?;
		let mut end = byte_offset(source, &line_starts, edit.end)?;
		match edit.kind {
			TextEditKind::Exact => {}
			TextEditKind::MethodSuffix => {
				start = find_method_dot(source, start)?;
				if edit.replacement.is_empty() {
					start = include_line_prefix(source, start);
				}
			}
			TextEditKind::WholeLine => {
				start = line_start_byte(source, start);
				if source.as_bytes().get(end) == Some(&b'\n') {
					end += 1;
				}
			}
		}
		byte_edits.push((start, end, edit.replacement.as_str()));
	}
	byte_edits.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
	let mut previous_start = source.len();
	let mut rewritten = source.to_owned();
	for (start, end, replacement) in byte_edits {
		if start > end || end > previous_start {
			return None;
		}
		rewritten.replace_range(start..end, replacement);
		previous_start = start;
	}
	Some(rewritten)
}

fn line_starts(source: &str) -> Vec<usize> {
	let mut starts = vec![0];
	for (index, byte) in source.bytes().enumerate() {
		if byte == b'\n' {
			starts.push(index + 1);
		}
	}
	starts
}

fn byte_offset(source: &str, line_starts: &[usize], location: LineColumn) -> Option<usize> {
	let line = location.line.checked_sub(1)?;
	let offset = line_starts.get(line)?.checked_add(location.column)?;
	(offset <= source.len()).then_some(offset)
}

fn find_method_dot(source: &str, method_start: usize) -> Option<usize> {
	let bytes = source.as_bytes();
	let mut position = method_start;
	while position > 0 && bytes[position - 1].is_ascii_whitespace() {
		position -= 1;
	}
	(position > 0 && bytes[position - 1] == b'.').then_some(position - 1)
}

fn include_line_prefix(source: &str, dot: usize) -> usize {
	let line_start = line_start_byte(source, dot);
	if source[line_start..dot].trim().is_empty() && line_start > 0 {
		line_start - 1
	} else {
		dot
	}
}

fn line_start_byte(source: &str, offset: usize) -> usize {
	source[..offset]
		.rfind('\n')
		.map_or(0, |newline| newline + 1)
}
