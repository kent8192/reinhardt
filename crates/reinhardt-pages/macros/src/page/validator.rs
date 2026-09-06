//! Validation and transformation logic for page! macro AST.
//!
//! This module transforms the untyped AST from the parser into a typed AST,
//! while performing semantic validation and type checking.
//!
//! ## Validation Rules
//!
//! 1. **Event Handlers**: Must be closure expressions with 0 or 1 arguments
//! 2. **Attributes**: data-* and aria-* attributes must follow naming conventions
//! 3. **Element Nesting**: Void elements cannot have children, interactive elements cannot nest
//! 4. **Required Attributes**: img elements must have required media attributes
//! 5. **Attribute Types**: Certain attributes must be specific types (e.g., img src must be string literal)
//! 6. **Accessibility**: Controls, interactive elements, roles, tabindex, and iframes are validated
//!
//! ## Component invocation (spec §3.5.1)
//!
//! A component is any Rust function matching
//! `fn <name>(props: <NameProps>) -> Page` where `<NameProps>` derives
//! `bon::Builder`. The validator does **not** type-check this signature —
//! instead, codegen emits a builder chain that relies on standard Rust
//! type inference / dispatch to surface mismatches at compile time
//! (DP #4 Fail early via E0061 / E0277).
//!
//! Both invocation forms reach this validator:
//!
//! - Paren form `Component(arg: val)` (legacy, codegen → positional call)
//! - Brace form `Component { prop: val, @event: h, child { ... } }`
//!   (spec §3.5, codegen → `bon::Builder` chain on `<Name>Props`)
//!
//! The parser sets `PageComponent::invocation_form` to record which form
//! was used; codegen branches on that field.

use proc_macro2::Span;
use std::collections::HashSet;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, Result, Token};

fn macro_supports_named_arguments(path: &syn::Path) -> bool {
	if path.leading_colon.is_none() {
		return false;
	}
	let segments: Vec<_> = path
		.segments
		.iter()
		.map(|segment| segment.ident.to_string())
		.collect();
	matches!(segments.as_slice(), [prefix, name]
			if (prefix == "std" || prefix == "alloc") && matches!(name.as_str(), "format" | "format_args"))
		|| matches!(segments.as_slice(), [prefix, name]
			if prefix == "reinhardt_pages" && name == "t")
		|| matches!(segments.as_slice(), [prefix, module, name]
			if prefix == "reinhardt_pages" && module == "prelude" && name == "t")
		|| matches!(segments.as_slice(), [facade, module, name]
			if facade == "reinhardt" && module == "pages" && name == "t")
}

use reinhardt_manouche::core::{
	ComponentEventProp, IntrinsicEvent, PageAttr, PageBody, PageComponent, PageElement, PageElse,
	PageExpression, PageFor, PageIf, PageMacro, PageMacroForm, PageNode, PageParam, PageWatch,
	TypedControlBinding, TypedControlBindingExpr, TypedControlBindingKind, TypedNamedSlot,
	TypedPageAttr, TypedPageBody, TypedPageComponent, TypedPageElement, TypedPageElse,
	TypedPageFor, TypedPageIf, TypedPageMacro, TypedPageMacroForm, TypedPageNode, TypedPageWatch,
	types::AttrValue,
};
use reinhardt_manouche::validator::classify_input_binding;

use super::scope_utils::collect_pat_idents;

#[derive(Clone, Copy, Default)]
struct ValidationContext {
	inside_bound_select: bool,
}

/// Check if a URL is safe (no dangerous schemes like javascript:).
///
/// Inlined from `reinhardt_core::security::xss::is_safe_url` to avoid
/// pulling the full reinhardt-core dependency chain (hyper/tokio/mio)
/// into this proc-macro crate, which breaks WASM builds. (Fixes #3226)
fn is_safe_url(url: &str) -> bool {
	let url_lower = url.to_lowercase();

	// Allow relative URLs and anchor links (but NOT parent traversal)
	if url.starts_with('/') || url.starts_with("./") || url.starts_with('#') {
		return true;
	}

	// Allow only safe protocols
	let safe_protocols = ["http://", "https://", "mailto:", "ftp://", "ftps://"];

	safe_protocols
		.iter()
		.any(|protocol| url_lower.starts_with(protocol))
}

/// Validates and transforms the entire PageMacro AST into a typed AST.
///
/// This is the main entry point for validation. It performs semantic checks
/// and type transformations, returning a fully validated and typed AST.
///
/// # Errors
///
/// Returns a compilation error if any validation rule is violated.
///
/// # Returns
///
/// A `TypedPageMacro` with validated and type-safe attribute values.
pub(super) fn validate(ast: &PageMacro) -> Result<TypedPageMacro> {
	let form = match &ast.form {
		PageMacroForm::StrictClosure { params, body } => {
			enforce_strict_captures(ast.head.as_ref(), body, params)?;
			let typed_body = transform_body(body, &[], ValidationContext::default())?;
			reinhardt_manouche::validator::validate_page_accessibility(&typed_body)?;
			TypedPageMacroForm::StrictClosure {
				params: params.clone(),
				body: typed_body,
			}
		}
		PageMacroForm::ImplicitBody { body } => {
			let typed_body = transform_body(body, &[], ValidationContext::default())?;
			reinhardt_manouche::validator::validate_page_accessibility(&typed_body)?;
			TypedPageMacroForm::ImplicitBody {
				captures: collect_free_idents(ast.head.as_ref(), body, &[]),
				body: typed_body,
			}
		}
	};

	Ok(TypedPageMacro {
		head: ast.head.clone(),
		form,
		span: ast.span,
	})
}

/// Collects value identifiers used in `body` that are not params or locals.
fn collect_free_idents(
	head: Option<&Expr>,
	body: &PageBody,
	params: &[PageParam],
) -> Vec<reinhardt_manouche::core::ImplicitPageCapture> {
	let allowed: HashSet<String> = params.iter().map(|p| p.name.to_string()).collect();

	let mut checker = CaptureChecker {
		allowed,
		locals_stack: Vec::new(),
		seen: HashSet::new(),
		captures: Vec::new(),
	};
	if let Some(head_expr) = head {
		checker.visit_expr(head_expr);
	}
	checker.visit_body(body);
	checker.captures
}

/// Verifies that no body identifier is an implicit capture.
///
/// Per spec §3.7, every value identifier inside the body must appear in the
/// `params` list. Item paths (`crate::util::fmt`), type identifiers
/// (`Vec`, `Option`), and constants (`MAX_LEN`) are exempt. Macro invocation
/// names (`format!`) are exempt, but macro arguments are scanned for free
/// identifiers when they parse as Rust expressions. Named argument keys such
/// as `name` in `::reinhardt_pages::t!("Hello {name}", name = source)` are not
/// value captures when the macro path is absolute and resolution-safe.
fn enforce_strict_captures(
	head: Option<&Expr>,
	body: &PageBody,
	params: &[PageParam],
) -> Result<()> {
	if let Some(capture) = collect_free_idents(head, body, params).into_iter().next() {
		return Err(missing_param_error(&capture.ident));
	}
	Ok(())
}

/// Walks a `PageBody` AST and records identifiers that are used but not
/// declared as `page!` parameters or local bindings.
struct CaptureChecker {
	allowed: HashSet<String>,
	locals_stack: Vec<HashSet<String>>,
	seen: HashSet<String>,
	captures: Vec<reinhardt_manouche::core::ImplicitPageCapture>,
}

impl CaptureChecker {
	fn is_known(&self, name: &str) -> bool {
		self.allowed.contains(name) || self.locals_stack.iter().any(|s| s.contains(name))
	}

	fn record_capture(&mut self, ident: &syn::Ident) {
		let name = ident.to_string();
		if self.seen.insert(name) {
			self.captures
				.push(reinhardt_manouche::core::ImplicitPageCapture {
					ident: ident.clone(),
					span: ident.span(),
				});
		}
	}

	fn visit_body(&mut self, body: &PageBody) {
		for node in &body.nodes {
			self.visit_node(node);
		}
	}

	fn visit_node(&mut self, node: &PageNode) {
		match node {
			PageNode::Element(el) => self.visit_element(el),
			PageNode::Text(_) => {}
			PageNode::Expression(e) => self.visit_expression(e),
			PageNode::If(p) => self.visit_if(p),
			PageNode::For(p) => self.visit_for(p),
			PageNode::Component(c) => self.visit_component(c),
			PageNode::Watch(w) => self.visit_watch(w),
		}
	}

	fn visit_element(&mut self, el: &PageElement) {
		for a in &el.attrs {
			if a.html_name() == "a11y" {
				continue;
			}
			if a.html_name() == "bind"
				&& let Expr::Call(call) = &a.value
				&& let Expr::Path(path) = call.func.as_ref()
				&& path.qself.is_none()
				&& path.path.is_ident("number")
			{
				for argument in &call.args {
					self.visit_expr(argument);
				}
				continue;
			}
			self.visit_expr(&a.value);
		}
		for e in &el.events {
			self.visit_intrinsic_event(e);
		}
		for c in &el.children {
			self.visit_node(c);
		}
	}

	fn visit_intrinsic_event(&mut self, event: &IntrinsicEvent) {
		self.visit_expr(event.handler());
	}

	fn visit_expression(&mut self, e: &PageExpression) {
		self.visit_expr(&e.expr);
	}

	fn visit_if(&mut self, p: &PageIf) {
		let mut then_locals = None;
		if let Expr::Let(let_expr) = &p.condition {
			let mut locals = HashSet::new();
			collect_pat_idents(&let_expr.pat, &mut locals);
			self.visit_expr(&let_expr.expr);
			then_locals = Some(locals);
		} else {
			self.visit_expr(&p.condition);
		}
		let pushed_then_locals = then_locals.is_some();
		if let Some(locals) = then_locals {
			self.locals_stack.push(locals);
		}
		for n in &p.then_branch {
			self.visit_node(n);
		}
		if pushed_then_locals {
			self.locals_stack.pop();
		}
		if let Some(els) = &p.else_branch {
			match els {
				PageElse::Block(nodes) => {
					for n in nodes {
						self.visit_node(n);
					}
				}
				PageElse::If(inner) => self.visit_if(inner),
			}
		}
	}

	fn visit_for(&mut self, p: &PageFor) {
		self.visit_expr(&p.iter);
		let mut locals = HashSet::new();
		collect_pat_idents(&p.pat, &mut locals);
		self.locals_stack.push(locals);
		if let Some(key) = &p.key {
			self.visit_expr(key);
		}
		for n in &p.body {
			self.visit_node(n);
		}
		self.locals_stack.pop();
	}

	fn visit_component(&mut self, c: &PageComponent) {
		// PascalCase head is allowed (type-path classification).
		for a in &c.args {
			self.visit_expr(&a.value);
		}
		if let Some(children) = &c.children {
			for n in children {
				self.visit_node(n);
			}
		}
		for slot in &c.named_slots {
			for n in &slot.children {
				self.visit_node(n);
			}
		}
		for e in &c.events {
			self.visit_expr(&e.handler);
		}
	}

	fn visit_watch(&mut self, w: &PageWatch) {
		self.visit_node(&w.expr);
	}

	fn visit_expr(&mut self, e: &Expr) {
		let mut v = ExprIdentVisitor { checker: self };
		v.visit_expr(e);
	}
}

/// `syn::visit::Visit` adapter that delegates identifier lookups back to the
/// owning `CaptureChecker` while tracking closure / `let` locals.
struct ExprIdentVisitor<'a> {
	checker: &'a mut CaptureChecker,
}

impl<'ast> Visit<'ast> for ExprIdentVisitor<'_> {
	fn visit_expr_path(&mut self, ep: &'ast syn::ExprPath) {
		if ep.qself.is_none() && ep.path.segments.len() == 1 {
			let seg = &ep.path.segments[0];
			let name = seg.ident.to_string();
			if is_value_ident(&name) && !self.checker.is_known(&name) {
				self.checker.record_capture(&seg.ident);
			}
		}
		visit::visit_expr_path(self, ep);
	}

	fn visit_expr_closure(&mut self, c: &'ast syn::ExprClosure) {
		let mut locals = HashSet::new();
		for input in &c.inputs {
			collect_pat_idents(input, &mut locals);
		}
		self.checker.locals_stack.push(locals);
		visit::visit_expr_closure(self, c);
		self.checker.locals_stack.pop();
	}

	fn visit_expr_let(&mut self, l: &'ast syn::ExprLet) {
		// `let x = ...` body introduces `x` to subsequent statements; the
		// page! body is expression-position, so this is rare but possible
		// inside embedded `{ ... }` blocks. Treat conservatively as a local.
		let mut locals = HashSet::new();
		collect_pat_idents(&l.pat, &mut locals);
		self.checker.locals_stack.push(locals);
		visit::visit_expr_let(self, l);
		self.checker.locals_stack.pop();
	}

	fn visit_expr_if(&mut self, i: &'ast syn::ExprIf) {
		// Handle `if let pat = expr { body }`: pattern bindings introduced by
		// the let-condition are in scope for the then-branch (and any
		// `else if let` chain).
		if let syn::Expr::Let(let_expr) = &*i.cond {
			let mut locals = HashSet::new();
			collect_pat_idents(&let_expr.pat, &mut locals);
			// First visit the RHS expr without the new locals — they are
			// not in scope for the matched expression.
			self.visit_expr(&let_expr.expr);
			// Now push locals and visit the then-branch in their scope.
			self.checker.locals_stack.push(locals);
			self.visit_block(&i.then_branch);
			self.checker.locals_stack.pop();
			// Else branch: locals are NOT in scope.
			if let Some((_, else_branch)) = &i.else_branch {
				self.visit_expr(else_branch);
			}
		} else {
			// Plain `if cond { ... }`
			visit::visit_expr_if(self, i);
		}
	}

	fn visit_arm(&mut self, a: &'ast syn::Arm) {
		// `match expr { pat => body }`: pat bindings are in scope for body.
		let mut locals = HashSet::new();
		collect_pat_idents(&a.pat, &mut locals);
		self.checker.locals_stack.push(locals);
		if let Some((_, guard)) = &a.guard {
			self.visit_expr(guard);
		}
		self.visit_expr(&a.body);
		self.checker.locals_stack.pop();
	}

	fn visit_expr_for_loop(&mut self, f: &'ast syn::ExprForLoop) {
		// `for pat in iter { body }`: pat bindings are in scope for body.
		self.visit_expr(&f.expr);
		let mut locals = HashSet::new();
		collect_pat_idents(&f.pat, &mut locals);
		self.checker.locals_stack.push(locals);
		self.visit_block(&f.body);
		self.checker.locals_stack.pop();
	}

	fn visit_expr_macro(&mut self, expr_macro: &'ast syn::ExprMacro) {
		let has_named_arguments = macro_supports_named_arguments(&expr_macro.mac.path);
		if let Ok(args) = expr_macro
			.mac
			.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
		{
			for arg in args {
				if has_named_arguments && let Expr::Assign(assign) = arg {
					self.visit_expr(&assign.right);
				} else {
					self.visit_expr(&arg);
				}
			}
		}
	}

	fn visit_block(&mut self, b: &'ast syn::Block) {
		// Walk statements in order so each `let pat = ...;` extends scope
		// from the next statement onward. Track how many locals scopes we
		// pushed so we can pop them all at block end.
		let mut pushed = 0_usize;
		for stmt in &b.stmts {
			match stmt {
				syn::Stmt::Local(local) => {
					// First visit the RHS (locals not yet in scope).
					if let Some(init) = &local.init {
						self.visit_expr(&init.expr);
						if let Some((_, diverge)) = &init.diverge {
							self.visit_expr(diverge);
						}
					}
					// Then push the new bindings for subsequent stmts.
					let mut locals = HashSet::new();
					collect_pat_idents(&local.pat, &mut locals);
					self.checker.locals_stack.push(locals);
					pushed += 1;
				}
				syn::Stmt::Item(_) => {
					// Items (fn, use, mod, etc.) don't introduce value
					// bindings that we track.
				}
				syn::Stmt::Expr(e, _) => {
					self.visit_expr(e);
				}
				syn::Stmt::Macro(m) => {
					visit::visit_stmt_macro(self, m);
				}
			}
		}
		for _ in 0..pushed {
			self.checker.locals_stack.pop();
		}
	}
}

/// Classifies an identifier as a value binding per spec §3.7.
///
/// Returns true for lowercase-leading names that are not SCREAMING_SNAKE
/// constants. SCREAMING_SNAKE and PascalCase paths are treated as item /
/// type paths and exempted from the capture-discipline check.
fn is_value_ident(name: &str) -> bool {
	// Lowercase first char ⇒ value binding.
	// SCREAMING_SNAKE (e.g. MAX_LEN) and PascalCase (e.g. Vec) are exempt.
	let starts_lowercase = name
		.chars()
		.next()
		.map(|c| c.is_ascii_lowercase())
		.unwrap_or(false);
	let all_screaming = name
		.chars()
		.all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit());
	starts_lowercase && !all_screaming
}

/// Builds the canonical §3.7 diagnostic for an undeclared identifier.
fn missing_param_error(ident: &syn::Ident) -> syn::Error {
	syn::Error::new(
		ident.span(),
		format!(
			"identifier `{ident}` used inside `page!` is not declared as a parameter.\n\n\
			 help: add it to the closure signature:\n\
			         page!(|{ident}: Signal<...>| {{ ... }})\n\
			 note: `page!` forbids implicit captures so reactive dependencies are\n\
			       explicit (React-style props-down data flow). Item paths\n\
			       (`module::func`), types (`Vec`, `Option`), and constants\n\
			       (`MAX_LEN`) are unaffected."
		),
	)
}

/// Transforms a PageBody into a TypedPageBody.
///
/// # Arguments
///
/// * `body` - The untyped body to transform
/// * `parent_tags` - Stack of parent element tag names (for nesting validation)
fn transform_body(
	body: &PageBody,
	parent_tags: &[String],
	context: ValidationContext,
) -> Result<TypedPageBody> {
	let nodes = transform_nodes(&body.nodes, parent_tags, context)?;
	Ok(TypedPageBody {
		nodes,
		span: body.span,
	})
}

/// Transforms a slice of page nodes into typed nodes recursively.
///
/// # Arguments
///
/// * `nodes` - The nodes to transform
/// * `parent_tags` - Stack of parent element tag names (for nesting validation)
fn transform_nodes(
	nodes: &[PageNode],
	parent_tags: &[String],
	context: ValidationContext,
) -> Result<Vec<TypedPageNode>> {
	let mut typed_nodes = Vec::new();

	for node in nodes {
		typed_nodes.push(transform_node(node, parent_tags, context)?);
	}

	Ok(typed_nodes)
}

/// Transforms a single PageNode into a TypedPageNode.
///
/// Dispatches to the appropriate transformation function based on node type.
fn transform_node(
	node: &PageNode,
	parent_tags: &[String],
	context: ValidationContext,
) -> Result<TypedPageNode> {
	match node {
		PageNode::Element(elem) => Ok(TypedPageNode::Element(transform_element(
			elem,
			parent_tags,
			context,
		)?)),
		PageNode::Text(text) => Ok(TypedPageNode::Text(text.clone())),
		PageNode::Expression(expr) => {
			if !expr.braced {
				// Recover the source ident for the fix-it hint where possible.
				// A bare path like `name` becomes `{name}`; anything else falls
				// back to a generic `{expr}` placeholder.
				let suggestion = match &expr.expr {
					Expr::Path(ep) if ep.qself.is_none() && ep.path.segments.len() == 1 => {
						format!("{{{}}}", ep.path.segments[0].ident)
					}
					_ => "{expr}".to_string(),
				};
				return Err(syn::Error::new(
					expr.span,
					format!(
						"bare identifier shorthand is removed in v2 — \
						 wrap the expression in braces: `{suggestion}`.\n\n\
						 note: spec §3.6 — `div {{ foo }}` is no longer parsed as \
						       an expression. Use `div {{ {{foo}} }}` to render the \
						       value, or `div {{ foo {{ ... }} }}` if `foo` was \
						       intended as an HTML tag.",
					),
				));
			}
			Ok(TypedPageNode::Expression(expr.clone()))
		}
		PageNode::If(if_node) => Ok(TypedPageNode::If(transform_if(
			if_node,
			parent_tags,
			context,
		)?)),
		PageNode::For(for_node) => Ok(TypedPageNode::For(Box::new(transform_for(
			for_node,
			parent_tags,
			context,
		)?))),
		PageNode::Component(comp) => Ok(TypedPageNode::Component(transform_component(
			comp,
			parent_tags,
			context,
		)?)),
		PageNode::Watch(watch_node) => Err(syn::Error::new(
			watch_node.span,
			"`watch { ... }` is removed in v2 — every `{expr}` and \
			 control-flow block is now auto-wrapped. Unwrap the watch \
			 braces: the body of `watch` can be moved out as-is \
			 (codemod available: `cargo make migrate-manouche-v2`).",
		)),
	}
}

/// Transforms a PageIf node (if/else if/else).
///
/// Recursively validates all branches.
fn transform_if(
	if_node: &reinhardt_manouche::core::PageIf,
	parent_tags: &[String],
	context: ValidationContext,
) -> Result<TypedPageIf> {
	// Transform then branch
	let then_branch = transform_nodes(&if_node.then_branch, parent_tags, context)?;

	// Transform else branch if present
	let else_branch = if let Some(else_br) = &if_node.else_branch {
		Some(transform_else(else_br, parent_tags, context)?)
	} else {
		None
	};

	Ok(TypedPageIf {
		condition: if_node.condition.clone(),
		then_branch,
		else_branch,
		span: if_node.span,
	})
}

/// Transforms a PageElse branch.
fn transform_else(
	else_branch: &PageElse,
	parent_tags: &[String],
	context: ValidationContext,
) -> Result<TypedPageElse> {
	match else_branch {
		PageElse::Block(nodes) => {
			let typed_nodes = transform_nodes(nodes, parent_tags, context)?;
			Ok(TypedPageElse::Block(typed_nodes))
		}
		PageElse::If(nested_if) => {
			// Recursively transform nested if
			let typed_if = transform_if(nested_if, parent_tags, context)?;
			Ok(TypedPageElse::If(Box::new(typed_if)))
		}
	}
}

/// Transforms a PageFor node.
fn transform_for(
	for_node: &reinhardt_manouche::core::PageFor,
	parent_tags: &[String],
	context: ValidationContext,
) -> Result<TypedPageFor> {
	let body = transform_nodes(&for_node.body, parent_tags, context)?;

	Ok(TypedPageFor {
		pat: for_node.pat.clone(),
		iter: for_node.iter.clone(),
		key: for_node.key.clone(),
		body,
		span: for_node.span,
	})
}

/// Transforms a PageWatch node.
///
/// Currently unused: the validator's `transform_node` rejects watch blocks
/// outright (Task 11 / spec §4.1 removal). Retained as a thin scaffold so
/// the future PR3 codemod can re-use it if a deprecation window is added.
#[allow(dead_code)] // Task 11: watch is rejected before this is reached.
fn transform_watch(
	watch_node: &PageWatch,
	parent_tags: &[String],
	context: ValidationContext,
) -> Result<TypedPageWatch> {
	let inner = transform_node(&watch_node.expr, parent_tags, context)?;

	Ok(TypedPageWatch {
		expr: Box::new(inner),
		span: watch_node.span,
	})
}

/// Transforms a PageComponent node.
///
/// Recursively transforms the component's children (if any) and named slots.
fn transform_component(
	comp: &PageComponent,
	parent_tags: &[String],
	context: ValidationContext,
) -> Result<TypedPageComponent> {
	// Validate component event handlers (same as element events)
	for event in &comp.events {
		validate_component_event_handler(event)?;
	}

	// Transform children if present
	let typed_children = if let Some(children) = &comp.children {
		Some(transform_nodes(children, parent_tags, context)?)
	} else {
		None
	};

	let typed_named_slots: Vec<TypedNamedSlot> = comp
		.named_slots
		.iter()
		.map(|slot| {
			Ok(TypedNamedSlot {
				name: slot.name.clone(),
				children: transform_nodes(&slot.children, parent_tags, context)?,
				span: slot.span,
			})
		})
		.collect::<Result<Vec<_>>>()?;

	Ok(TypedPageComponent {
		name: comp.name.clone(),
		invocation_form: comp.invocation_form,
		args: comp.args.clone(),
		events: comp.events.clone(),
		children: typed_children,
		named_slots: typed_named_slots,
		span: comp.span,
	})
}

/// Transforms and validates an element and its children.
///
/// Performs the following checks:
/// - Event handler validation
/// - Attribute transformation and validation
/// - Element nesting rules
/// - Required attributes
/// - HTML specification compliance (Phase 2)
fn transform_element(
	elem: &PageElement,
	parent_tags: &[String],
	context: ValidationContext,
) -> Result<TypedPageElement> {
	let tag = elem.tag.to_string();

	// 1. Validate events (unchanged from untyped version)
	for event in &elem.events {
		validate_intrinsic_event_handler(event)?;
	}

	// 2. Extract the binding before transforming ordinary attributes
	let (mut ordinary_attrs, binding_attr) = split_binding_attr(&elem.attrs)?;
	let control_binding = binding_attr
		.as_ref()
		.map(|attr| classify_control_binding(&tag, &ordinary_attrs, attr))
		.transpose()?
		.map(Box::new);
	if control_binding
		.as_deref()
		.is_some_and(|binding| binding.kind == TypedControlBindingKind::SelectOne)
	{
		ordinary_attrs.retain(|attr| {
			!(attr.html_name() == "multiple"
				&& matches!(&attr.value, Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Bool(value) if !value.value())))
		});
	}
	if control_binding.as_deref().is_some_and(|binding| {
		matches!(
			binding.kind,
			TypedControlBindingKind::Checkbox | TypedControlBindingKind::Radio
		)
	}) {
		ordinary_attrs.retain(|attr| {
			!(attr.html_name() == "checked"
				&& matches!(&attr.value, Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Bool(value) if !value.value())))
		});
	}
	if tag == "option" && context.inside_bound_select {
		ordinary_attrs.retain(|attr| {
			!(attr.html_name() == "selected"
				&& matches!(&attr.value, Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Bool(value) if !value.value())))
		});
	}
	let transformed_attrs = transform_attrs(&ordinary_attrs, &tag)?;
	let typed_attrs = transformed_attrs.attrs;

	// 3. Validate element nesting
	validate_element_nesting(elem, parent_tags)?;

	// 4. Recursively transform children
	let mut child_tags = parent_tags.to_vec();
	child_tags.push(tag.clone());
	let child_context = ValidationContext {
		inside_bound_select: if tag == "select" {
			control_binding.as_deref().is_some_and(|binding| {
				matches!(
					binding.kind,
					TypedControlBindingKind::SelectOne | TypedControlBindingKind::SelectMany
				)
			})
		} else {
			context.inside_bound_select
		},
	};
	let typed_children = transform_nodes(&elem.children, &child_tags, child_context)?;
	validate_control_binding_structure(
		&tag,
		control_binding.as_deref(),
		&typed_attrs,
		&typed_children,
	)?;

	// Create typed element
	let typed_element = TypedPageElement {
		tag: elem.tag.clone(),
		attrs: typed_attrs,
		control_binding,
		events: elem.events.clone(),
		children: typed_children,
		a11y_disabled: transformed_attrs.a11y_disabled,
		span: elem.span,
	};

	// 6. Validate against HTML specification (Phase 2)
	if tag == "option" && context.inside_bound_select {
		super::html_spec::validate_bound_select_element(&typed_element)?;
	} else {
		super::html_spec::validate_against_spec(&typed_element)?;
	}

	Ok(typed_element)
}

fn split_binding_attr(attrs: &[PageAttr]) -> Result<(Vec<PageAttr>, Option<PageAttr>)> {
	let mut ordinary_attrs = Vec::with_capacity(attrs.len());
	let mut binding_attr = None;

	for attr in attrs {
		if attr.html_name() == "bind" {
			if binding_attr.is_some() {
				return Err(syn::Error::new_spanned(
					&attr.value,
					"`bind:` may only be specified once per control",
				));
			}
			binding_attr = Some(attr.clone());
		} else {
			ordinary_attrs.push(attr.clone());
		}
	}

	Ok((ordinary_attrs, binding_attr))
}

fn parse_binding_expression(expr: &Expr) -> Result<TypedControlBindingExpr> {
	if let Expr::Call(call) = expr
		&& let Expr::Path(path) = call.func.as_ref()
		&& path.qself.is_none()
		&& path.path.is_ident("number")
	{
		if call.args.len() != 2 {
			return Err(syn::Error::new_spanned(
				call,
				"`number(value, error)` requires exactly two arguments",
			));
		}
		return Ok(TypedControlBindingExpr::NumberWithError {
			value: Box::new(call.args[0].clone()),
			error: Box::new(call.args[1].clone()),
		});
	}

	Ok(TypedControlBindingExpr::Direct(Box::new(expr.clone())))
}

fn classify_control_binding(
	element_tag: &str,
	attrs: &[PageAttr],
	binding_attr: &PageAttr,
) -> Result<TypedControlBinding> {
	let (kind, radio_value) = match element_tag {
		"input" => classify_input_binding(attrs, binding_attr)?,
		"textarea" => (TypedControlBindingKind::Text, None),
		"select" => (classify_select_binding(attrs, binding_attr)?, None),
		_ => {
			return Err(syn::Error::new_spanned(
				&binding_attr.value,
				"`bind:` is only valid on `input`, `textarea`, and `select`",
			));
		}
	};
	let expression = parse_binding_expression(&binding_attr.value)?;

	if matches!(expression, TypedControlBindingExpr::NumberWithError { .. })
		&& kind != TypedControlBindingKind::Number
	{
		return Err(syn::Error::new_spanned(
			&binding_attr.value,
			"`number(value, error)` is only valid on a numeric input",
		));
	}

	Ok(TypedControlBinding {
		kind,
		expression,
		radio_value,
		span: binding_attr.value.span(),
	})
}

fn classify_select_binding(
	attrs: &[PageAttr],
	binding_attr: &PageAttr,
) -> Result<TypedControlBindingKind> {
	match unique_untyped_attr(
		attrs,
		"multiple",
		binding_attr,
		"a bound select requires a static `multiple`",
	)? {
		None => Ok(TypedControlBindingKind::SelectOne),
		Some(attr) => match &attr.value {
			Expr::Lit(lit) => match &lit.lit {
				syn::Lit::Bool(value) if value.value() => Ok(TypedControlBindingKind::SelectMany),
				syn::Lit::Bool(_) => Ok(TypedControlBindingKind::SelectOne),
				_ => Err(syn::Error::new_spanned(
					&binding_attr.value,
					"a bound select requires a static `multiple`",
				)),
			},
			_ => Err(syn::Error::new_spanned(
				&binding_attr.value,
				"a bound select requires a static `multiple`",
			)),
		},
	}
}

fn unique_untyped_attr<'a>(
	attrs: &'a [PageAttr],
	name: &str,
	binding_attr: &PageAttr,
	diagnostic: &str,
) -> Result<Option<&'a PageAttr>> {
	let mut matching = attrs.iter().filter(|attr| attr.html_name() == name);
	let attr = matching.next();
	if matching.next().is_some() {
		return Err(syn::Error::new_spanned(&binding_attr.value, diagnostic));
	}
	Ok(attr)
}

fn validate_control_binding_structure(
	element_tag: &str,
	binding: Option<&TypedControlBinding>,
	attrs: &[TypedPageAttr],
	children: &[TypedPageNode],
) -> Result<()> {
	let Some(binding) = binding else {
		return Ok(());
	};

	let conflict = match binding.kind {
		TypedControlBindingKind::Text | TypedControlBindingKind::Number
			if element_tag == "input" && find_typed_attr(attrs, "value").is_some() =>
		{
			Some("a bound text or number input cannot specify a `value` attribute")
		}
		TypedControlBindingKind::Text
			if element_tag == "textarea" && find_typed_attr(attrs, "value").is_some() =>
		{
			Some("a bound textarea cannot specify a `value` attribute")
		}
		TypedControlBindingKind::Checkbox | TypedControlBindingKind::Radio
			if find_typed_attr(attrs, "checked").is_some() =>
		{
			Some("a bound checkbox or radio input cannot specify a `checked` attribute")
		}
		TypedControlBindingKind::Text if element_tag == "textarea" && !children.is_empty() => {
			Some("a bound textarea cannot contain initial child content")
		}
		TypedControlBindingKind::SelectOne | TypedControlBindingKind::SelectMany
			if contains_selected_option(children) =>
		{
			Some("a bound select cannot contain an option with a `selected` attribute")
		}
		TypedControlBindingKind::SelectOne | TypedControlBindingKind::SelectMany
			if contains_option_with_duplicate_value(children) =>
		{
			Some("a bound select cannot contain an option with duplicate `value` attributes")
		}
		TypedControlBindingKind::SelectOne | TypedControlBindingKind::SelectMany
			if contains_dynamic_option_without_value(children) =>
		{
			Some(
				"an option with dynamic content inside a bound select requires an explicit `value` attribute",
			)
		}
		_ => None,
	};

	match conflict {
		Some(message) => Err(syn::Error::new(binding.span, message)),
		None => Ok(()),
	}
}

fn find_typed_attr<'a>(attrs: &'a [TypedPageAttr], name: &str) -> Option<&'a TypedPageAttr> {
	attrs.iter().find(|attr| attr.html_name() == name)
}

fn is_false_selected_attr(attr: &TypedPageAttr) -> bool {
	attr.html_name() == "selected"
		&& matches!(&attr.value, AttrValue::BoolLit(value) if !value.value())
}

fn contains_selected_option(nodes: &[TypedPageNode]) -> bool {
	nodes.iter().any(|node| match node {
		TypedPageNode::Element(element) => {
			(element.tag == "option"
				&& find_typed_attr(&element.attrs, "selected")
					.is_some_and(|attr| !is_false_selected_attr(attr)))
				|| contains_selected_option(&element.children)
		}
		TypedPageNode::If(page_if) => page_if_contains_selected_option(page_if),
		TypedPageNode::For(page_for) => contains_selected_option(&page_for.body),
		TypedPageNode::Watch(watch) => {
			contains_selected_option(std::slice::from_ref(watch.expr.as_ref()))
		}
		TypedPageNode::Component(component) => {
			component
				.children
				.as_deref()
				.is_some_and(contains_selected_option)
				|| component
					.named_slots
					.iter()
					.any(|slot| contains_selected_option(&slot.children))
		}
		TypedPageNode::Text(_) | TypedPageNode::Expression(_) => false,
	})
}

fn contains_option_with_duplicate_value(nodes: &[TypedPageNode]) -> bool {
	nodes.iter().any(|node| match node {
		TypedPageNode::Element(element) => {
			(element.tag == "option"
				&& element
					.attrs
					.iter()
					.filter(|attr| attr.html_name() == "value")
					.nth(1)
					.is_some()) || contains_option_with_duplicate_value(&element.children)
		}
		TypedPageNode::If(page_if) => page_if_contains_option_with_duplicate_value(page_if),
		TypedPageNode::For(page_for) => contains_option_with_duplicate_value(&page_for.body),
		TypedPageNode::Watch(watch) => {
			contains_option_with_duplicate_value(std::slice::from_ref(watch.expr.as_ref()))
		}
		TypedPageNode::Component(component) => {
			component
				.children
				.as_deref()
				.is_some_and(contains_option_with_duplicate_value)
				|| component
					.named_slots
					.iter()
					.any(|slot| contains_option_with_duplicate_value(&slot.children))
		}
		TypedPageNode::Text(_) | TypedPageNode::Expression(_) => false,
	})
}

fn page_if_contains_option_with_duplicate_value(page_if: &TypedPageIf) -> bool {
	contains_option_with_duplicate_value(&page_if.then_branch)
		|| page_if
			.else_branch
			.as_ref()
			.is_some_and(|else_branch| match else_branch {
				TypedPageElse::Block(nodes) => contains_option_with_duplicate_value(nodes),
				TypedPageElse::If(page_if) => page_if_contains_option_with_duplicate_value(page_if),
			})
}

fn contains_dynamic_option_without_value(nodes: &[TypedPageNode]) -> bool {
	nodes.iter().any(|node| match node {
		TypedPageNode::Element(element) => {
			(element.tag == "option"
				&& find_typed_attr(&element.attrs, "value").is_none()
				&& contains_dynamic_option_content(&element.children))
				|| contains_dynamic_option_without_value(&element.children)
		}
		TypedPageNode::If(page_if) => page_if_contains_dynamic_option_without_value(page_if),
		TypedPageNode::For(page_for) => contains_dynamic_option_without_value(&page_for.body),
		TypedPageNode::Component(component) => {
			component
				.children
				.as_deref()
				.is_some_and(contains_dynamic_option_without_value)
				|| component
					.named_slots
					.iter()
					.any(|slot| contains_dynamic_option_without_value(&slot.children))
		}
		TypedPageNode::Watch(watch) => {
			contains_dynamic_option_without_value(std::slice::from_ref(watch.expr.as_ref()))
		}
		TypedPageNode::Text(_) | TypedPageNode::Expression(_) => false,
	})
}

fn page_if_contains_dynamic_option_without_value(page_if: &TypedPageIf) -> bool {
	contains_dynamic_option_without_value(&page_if.then_branch)
		|| page_if
			.else_branch
			.as_ref()
			.is_some_and(|branch| match branch {
				TypedPageElse::Block(nodes) => contains_dynamic_option_without_value(nodes),
				TypedPageElse::If(page_if) => {
					page_if_contains_dynamic_option_without_value(page_if)
				}
			})
}

fn contains_dynamic_option_content(nodes: &[TypedPageNode]) -> bool {
	nodes.iter().any(|node| match node {
		TypedPageNode::Text(_) => false,
		TypedPageNode::Element(element) => contains_dynamic_option_content(&element.children),
		TypedPageNode::Expression(_)
		| TypedPageNode::If(_)
		| TypedPageNode::For(_)
		| TypedPageNode::Component(_)
		| TypedPageNode::Watch(_) => true,
	})
}

fn page_if_contains_selected_option(page_if: &TypedPageIf) -> bool {
	contains_selected_option(&page_if.then_branch)
		|| page_if
			.else_branch
			.as_ref()
			.is_some_and(|else_branch| match else_branch {
				TypedPageElse::Block(nodes) => contains_selected_option(nodes),
				TypedPageElse::If(page_if) => page_if_contains_selected_option(page_if),
			})
}

struct TransformedPageAttrs {
	attrs: Vec<TypedPageAttr>,
	a11y_disabled: bool,
}

/// Transforms attributes from untyped to typed, with validation.
///
/// This function converts `Expr` attribute values into `AttrValue`,
/// enabling type-specific validation.
fn transform_attrs(attrs: &[PageAttr], element_tag: &str) -> Result<TransformedPageAttrs> {
	let mut typed_attrs = Vec::new();
	let mut a11y_disabled = false;

	for attr in attrs {
		if attr.html_name() == "a11y" {
			validate_a11y_opt_out_attr(attr)?;
			a11y_disabled = true;
			continue;
		}

		// Validate attribute naming conventions (data-*, aria-*)
		validate_attribute(attr, element_tag)?;

		// Transform to typed value
		let typed_value = AttrValue::from_expr(attr.value.clone());

		// Validate attribute type for specific elements/attributes
		validate_attr_type(&attr.name.to_string(), &typed_value, element_tag, attr.span)?;

		typed_attrs.push(TypedPageAttr {
			name: attr.name.clone(),
			value: typed_value,
			span: attr.span,
		});
	}

	Ok(TransformedPageAttrs {
		attrs: typed_attrs,
		a11y_disabled,
	})
}

fn validate_a11y_opt_out_attr(attr: &PageAttr) -> Result<()> {
	if let Expr::Path(path) = &attr.value
		&& path.qself.is_none()
		&& path.path.is_ident("off")
	{
		return Ok(());
	}

	Err(syn::Error::new_spanned(
		&attr.value,
		"`a11y` accepts only `off` as an opt-out marker: `a11y: off`",
	))
}

/// Checks if an attribute is a URL attribute for the given element.
///
/// Note: img element's src attribute is excluded as it has separate validation rules.
fn is_url_attribute(attr_name: &str, element_tag: &str, url_attrs: &[(&str, &str)]) -> bool {
	// Exclude img src - it has separate validation rules
	if element_tag == "img" && attr_name == "src" {
		return false;
	}

	for (url_attr, applicable_tags) in url_attrs {
		if attr_name == *url_attr {
			for tag in applicable_tags.split(',').map(|s| s.trim()) {
				if tag == element_tag {
					return true;
				}
			}
		}
	}
	false
}

/// Validates enumerated attribute values.
///
/// Checks if a string literal attribute value is one of the allowed values
/// for enumerated attributes (like `input[type]`, `button[type]`, etc.).
///
/// # Parameters
///
/// * `attr_name` - The attribute name
/// * `value` - The attribute value
/// * `element_tag` - The element tag name
/// * `span` - The span for error reporting
///
/// # Returns
///
/// Ok if validation passes, Err with descriptive error message otherwise.
fn validate_enum_attr(
	attr_name: &str,
	value: &AttrValue,
	element_tag: &str,
	span: Span,
) -> Result<()> {
	let Some(enum_spec) = super::html_spec::get_enum_attr_spec(element_tag, attr_name) else {
		return Ok(());
	};

	let Some(str_value) = value.as_string() else {
		return Ok(()); // Dynamic expressions are OK
	};

	if !enum_spec.valid_values.contains(&str_value.as_str()) {
		// Use .first() for safe access instead of direct indexing
		let example_value = enum_spec.valid_values.first().copied().unwrap_or("...");
		return Err(syn::Error::new(
			span,
			format!(
				"Invalid value '{}' for attribute '{}' on element <{}>.\n\
				Valid values are: {}\n\n\
				Examples:\n\
				  Correct:   {}=\"{}\"\n\
				  Incorrect: {}=\"{}\"",
				str_value,
				attr_name,
				element_tag,
				enum_spec.valid_values.join("\", \""),
				attr_name,
				example_value,
				attr_name,
				str_value
			),
		));
	}

	Ok(())
}

/// Checks if children nodes contain meaningful content.
///
/// Returns true if any child contains non-whitespace text or is a dynamic expression.
#[cfg(test)]
fn has_meaningful_content(children: &[TypedPageNode]) -> bool {
	for child in children {
		match child {
			TypedPageNode::Text(text) => {
				if !text.content.trim().is_empty() {
					return true;
				}
			}
			TypedPageNode::Element(elem) => {
				if has_meaningful_content(&elem.children) {
					return true;
				}
			}
			TypedPageNode::Expression(_)
			| TypedPageNode::Component(_)
			| TypedPageNode::If(_)
			| TypedPageNode::For(_)
			| TypedPageNode::Watch(_) => {
				// Dynamic content - assume it will have meaningful content at runtime
				return true;
			}
		}
	}
	false
}

/// Validates button element accessibility requirements.
///
/// Button elements must have either:
/// - Text content (direct or nested)
/// - aria-label attribute
/// - aria-labelledby attribute
#[cfg(test)]
fn validate_button_accessibility(
	attrs: &[TypedPageAttr],
	children: &[TypedPageNode],
	span: Span,
) -> Result<()> {
	// Check for aria-label or aria-labelledby attributes
	let has_aria_label = attrs.iter().any(|attr| {
		let name = attr.name.to_string();
		name == "aria_label" || name == "aria_labelledby"
	});

	if has_aria_label {
		return Ok(());
	}

	// Check for text content
	if !has_meaningful_content(children) {
		return Err(syn::Error::new(
			span,
			"Element <button> requires accessible text.\n\
			Either provide text content or use 'aria_label' attribute.\n\n\
			Examples:\n\
			  Correct:   button { \"Click me\" }\n\
			  Correct:   button { aria_label: \"Close\" }\n\
			  Correct:   button { span { \"Submit\" } }\n\
			  Incorrect: button {}",
		));
	}

	Ok(())
}

/// Validates attribute type for specific elements and attributes.
///
/// # Validation Rules
///
/// - Boolean attributes must have dynamic expressions only (no literals)
/// - Numeric attributes must have integer literals or dynamic expressions (no strings/floats/booleans)
/// - URL attributes are checked for dangerous schemes (javascript:, data:, vbscript:) for XSS prevention
/// - Enumerated attributes are validated against allowed values (`input[type]`, `button[type]`, etc.)
/// - `img` element `src` attribute: when given as a string literal it must be non-empty
///   and use a safe URL scheme; dynamic expressions (e.g. `resolve_static(...)`) are
///   accepted and deferred to runtime
///
fn validate_attr_type(
	attr_name: &str,
	value: &AttrValue,
	element_tag: &str,
	span: Span,
) -> Result<()> {
	// Boolean attributes validation - must use dynamic expressions only
	const BOOLEAN_ATTRS: &[&str] = &[
		"allowfullscreen",
		"async",
		"autofocus",
		"autoplay",
		"checked",
		"controls",
		"default",
		"defer",
		"disabled",
		"formnovalidate",
		"hidden",
		"inert",
		"ismap",
		"itemscope",
		"loop",
		"multiple",
		"muted",
		"nomodule",
		"novalidate",
		"open",
		"playsinline",
		"readonly",
		"required",
		"reversed",
		"selected",
		"truespeed",
	];

	// Numeric attributes that must have integer literal or dynamic values
	const NUMERIC_ATTRS: &[&str] = &[
		"maxlength",
		"minlength",
		"size",
		"min",
		"max",
		"step",
		"rows",
		"cols",
		"colspan",
		"rowspan",
		"tabindex",
	];

	// URL attributes that should be validated for dangerous schemes
	// Each entry is (attribute_name, applicable_element_tags)
	const URL_ATTRS: &[(&str, &str)] = &[
		("href", "a, area, link"),
		("action", "form"),
		("formaction", "button, input"),
		("src", "iframe, video, audio, source, script, embed"),
	];

	if BOOLEAN_ATTRS.contains(&attr_name) {
		// 1. String literals are prohibited
		if value.is_string_literal() {
			return Err(syn::Error::new(
				span,
				format!(
					"Boolean attribute '{}' cannot have a string literal value.\n\
					HTML boolean attributes represent true/false by their presence/absence:\n\
					  - Attribute present = true\n\
					  - Attribute absent = false\n\n\
					Use a variable or expression for dynamic boolean values:\n\
					  Correct:   {}: is_disabled\n\
					  Correct:   {}: state.is_active()\n\
					  Incorrect: {}: \"true\"\n\
					  Incorrect: {}: \"disabled\"",
					attr_name, attr_name, attr_name, attr_name, attr_name
				),
			));
		}

		// 2. Boolean literal `false` is prohibited (omit the attribute instead).
		//    `true` is allowed to support standalone syntax (e.g., `required`
		//    which the parser desugars to `required: true`).
		if let AttrValue::BoolLit(lit) = value
			&& !lit.value()
		{
			return Err(syn::Error::new(
				span,
				format!(
					"Boolean attribute '{}' cannot be set to `false`.\n\
						To disable a boolean attribute, omit it entirely:\n\
						  - Attribute present = true (e.g., `{0}` or `{0}: true`)\n\
						  - Attribute absent = false (just remove `{0}`)\n\n\
						Use a variable or expression for dynamic boolean values:\n\
						  Correct:   {0}: is_disabled\n\
						  Correct:   {0}: state.is_active()",
					attr_name
				),
			));
		}

		// 3. Numeric literals are prohibited
		if value.is_int_literal() || value.is_float_literal() {
			return Err(syn::Error::new(
				span,
				format!(
					"Boolean attribute '{}' cannot have a numeric literal value.\n\
					HTML boolean attributes represent true/false by their presence/absence:\n\
					  - Attribute present = true\n\
					  - Attribute absent = false\n\n\
					Use a variable or expression for dynamic boolean values:\n\
					  Correct:   {}: is_disabled\n\
					  Correct:   {}: state.is_active()\n\
					  Incorrect: {}: 1\n\
					  Incorrect: {}: 0",
					attr_name, attr_name, attr_name, attr_name, attr_name
				),
			));
		}

		// 4. Dynamic expressions are OK (no check needed)
	}

	// Numeric attributes validation - must be integer literal or dynamic
	if NUMERIC_ATTRS.contains(&attr_name) {
		match value {
			AttrValue::StringLit(_) => {
				return Err(syn::Error::new(
					span,
					format!(
						"Attribute '{}' must be an integer literal or dynamic expression, not a string literal.\n\n\
						Examples:\n\
						  Incorrect: {}=\"100\"  // String literal is not allowed\n\
						  Correct:   {}=100      // Integer literal\n\
						  Correct:   {}=max_len  // Dynamic expression",
						attr_name, attr_name, attr_name, attr_name
					),
				));
			}
			AttrValue::FloatLit(_) => {
				return Err(syn::Error::new(
					span,
					format!(
						"Attribute '{}' must be an integer, not a floating-point number.\n\n\
						Examples:\n\
						  Incorrect: {}=100.0   // Float literal is not allowed\n\
						  Correct:   {}=100     // Integer literal",
						attr_name, attr_name, attr_name
					),
				));
			}
			AttrValue::BoolLit(_) => {
				return Err(syn::Error::new(
					span,
					format!(
						"Attribute '{}' must be an integer, not a boolean.\n\n\
						Use an integer literal or dynamic expression instead.",
						attr_name
					),
				));
			}
			AttrValue::IntLit(_) | AttrValue::Dynamic(_) => {
				// OK: Integer literal or dynamic expression
			}
		}
	}

	// URL attributes safety check - prevent XSS attacks
	if is_url_attribute(attr_name, element_tag, URL_ATTRS)
		&& let Some(url_str) = value.as_string()
	{
		// Check for empty strings first
		if url_str.trim().is_empty() {
			return Err(syn::Error::new(
				span,
				format!(
					"URL attribute '{}' cannot be empty.\n\n\
						Provide a valid URL or use a dynamic expression.",
					attr_name
				),
			));
		}

		// Check for dangerous schemes (case-insensitive) using is_safe_url from reinhardt-core
		// Fixes #849
		if !is_safe_url(&url_str) {
			return Err(syn::Error::new(
				span,
				format!(
					"Dangerous URL scheme detected in attribute '{}'.\n\
						The URL scheme can be used for XSS (Cross-Site Scripting) attacks.\n\n\
						Security risk: This URL could execute arbitrary JavaScript code.\n\n\
						Use safe URL schemes instead:\n\
						  - https://example.com\n\
						  - /path/to/resource\n\
						  - #anchor\n\
						  - mailto:user@example.com",
					attr_name
				),
			));
		}
	}
	// Dynamic expressions are OK (runtime validation recommended)

	// Enumerated attributes validation - check if value is in allowed list
	validate_enum_attr(attr_name, value, element_tag, span)?;

	// img element src attribute validation
	// Fixes #849
	//
	// String literals are checked here for emptiness and dangerous URL schemes.
	// Dynamic expressions (e.g. `resolve_static(...)`, variable references) are
	// accepted and deferred to runtime validation.
	if element_tag == "img" && attr_name == "src" {
		// Must not be empty (only checkable on string literals)
		if let Some(src_value) = value.as_string() {
			if src_value.trim().is_empty() {
				return Err(syn::Error::new(
					span,
					"Element <img> 'src' attribute must not be empty",
				));
			}

			// Check for dangerous URL schemes (XSS prevention) using is_safe_url from reinhardt-core
			// Fixes #849
			if !is_safe_url(&src_value) {
				return Err(syn::Error::new(
					span,
					"Dangerous URL scheme detected in <img> 'src' attribute.\n\
					The URL scheme can be used for XSS (Cross-Site Scripting) attacks.\n\n\
					Security risk: This URL could execute arbitrary JavaScript code.\n\n\
					Use safe URL schemes instead:\n\
					  - https://example.com/image.png\n\
					  - /path/to/image.png\n\
					  - image.png",
				));
			}
		}
	}

	Ok(())
}

/// Validates event handlers.
///
/// # Rules
///
/// - If the handler is a closure, it must have 0 or 1 arguments
/// - Other expressions (variable references, Callback::new(), etc.) are allowed
///   and will be type-checked by the Rust compiler
///
/// # Errors
///
/// Returns a compilation error if the handler is a closure with more than 1 argument.
fn validate_intrinsic_event_handler(event: &IntrinsicEvent) -> Result<()> {
	let handler = match event {
		IntrinsicEvent::Standard { event, handler } => {
			let _spec = event.spec();
			handler
		}
		IntrinsicEvent::RawCustom { handler, .. } | IntrinsicEvent::TypedCustom { handler, .. } => {
			handler
		}
	};
	validate_event_handler_expr(handler)
}

fn validate_component_event_handler(event: &ComponentEventProp) -> Result<()> {
	validate_event_handler_expr(&event.handler)
}

fn validate_event_handler_expr(handler: &Expr) -> Result<()> {
	// Only validate argument count for closure expressions
	// Other expressions (variables, method calls, etc.) are allowed
	// and will be type-checked by the Rust compiler
	if let Expr::Closure(closure) = handler {
		let arg_count = closure.inputs.len();
		if arg_count > 1 {
			return Err(syn::Error::new_spanned(
				handler,
				format!(
					"Event handler closure must have 0 or 1 arguments, but this closure has {} arguments",
					arg_count
				),
			));
		}
	}

	// All other expression types are allowed:
	// - Variable references: let handler = |_| {}; @click: handler
	// - Callback::new(): @click: Callback::new(|_| {})
	// - Method calls: @click: handler.clone()
	// Type checking for these will be done by the Rust compiler

	Ok(())
}

/// Validates attribute naming and values.
///
/// # Rules
///
/// - data-* attributes must match pattern: `data-[a-z][a-z0-9-]*`
/// - aria-* attributes must match pattern: `aria-[a-z-]+`
///
/// # Errors
///
/// Returns an error if naming conventions are violated.
fn validate_attribute(attr: &PageAttr, _element_tag: &str) -> Result<()> {
	let attr_name = attr.name.to_string();

	// Validate data-* attributes
	if attr_name.starts_with("data_") {
		let html_name = attr.html_name();
		// Check if all characters after "data-" are lowercase letters, digits, or hyphens
		let suffix = &html_name[5..]; // Skip "data-"
		if suffix.is_empty()
			|| !suffix
				.chars()
				.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
		{
			return Err(syn::Error::new_spanned(
				&attr.name,
				format!(
					"Invalid data attribute name '{}'. Must match pattern: data-[a-z][a-z0-9-]*",
					html_name
				),
			));
		}
		// Additionally check first character is lowercase letter
		if !suffix.chars().next().unwrap().is_ascii_lowercase() {
			return Err(syn::Error::new_spanned(
				&attr.name,
				format!(
					"Invalid data attribute name '{}'. Must start with a lowercase letter after 'data-'",
					html_name
				),
			));
		}
	}

	// Validate aria-* attributes
	if attr_name.starts_with("aria_") {
		let html_name = attr.html_name();
		// Check if all characters after "aria-" are lowercase letters or hyphens
		let suffix = &html_name[5..]; // Skip "aria-"
		if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
			return Err(syn::Error::new_spanned(
				&attr.name,
				format!(
					"Invalid aria attribute name '{}'. Must match pattern: aria-[a-z-]+",
					html_name
				),
			));
		}
	}

	Ok(())
}

/// Validates element nesting rules.
///
/// # Rules
///
/// - Void elements (br, hr, img, input, etc.) cannot have children
/// - Interactive elements (button, a, label, etc.) cannot be nested inside other interactive elements
///
/// # Errors
///
/// Returns an error if nesting rules are violated.
fn validate_element_nesting(elem: &PageElement, parent_tags: &[String]) -> Result<()> {
	let tag = elem.tag.to_string();

	// Check if void element has children
	if elem.is_void() && !elem.children.is_empty() {
		return Err(syn::Error::new_spanned(
			&elem.tag,
			format!("Void element <{}> cannot have children", tag),
		));
	}

	// Check interactive element nesting
	let interactive_elements = ["button", "a", "label", "select", "textarea"];
	if interactive_elements.contains(&tag.as_str()) {
		for parent_tag in parent_tags {
			if interactive_elements.contains(&parent_tag.as_str()) {
				return Err(syn::Error::new_spanned(
					&elem.tag,
					format!(
						"Interactive element <{}> cannot be nested inside another interactive element <{}>",
						tag, parent_tag
					),
				));
			}
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use syn::parse_quote;

	fn controlled_binding_invalid_cases() -> Vec<(proc_macro2::TokenStream, &'static str)> {
		vec![
			(
				quote::quote!({ div { bind: value } }),
				"`bind:` is only valid on `input`, `textarea`, and `select`",
			),
			(
				quote::quote!({ input { type: dynamic_type, bind: value } }),
				"a bound input requires a static `type`",
			),
			(
				quote::quote!({ input { type: "radio", bind: value } }),
				"a bound radio input requires a `value` attribute",
			),
			(
				quote::quote!({ input { a11y: off, type: "radio", value: "one", value: "two", bind: value } }),
				"a bound radio input cannot specify duplicate `value` attributes",
			),
			(
				quote::quote!({
					select {
						a11y: off,
						bind: value,
						if condition {
							optgroup { option { value: "one", value: "two", "Choice" } }
						}
					}
				}),
				"a bound select cannot contain an option with duplicate `value` attributes",
			),
			(
				quote::quote!({
					input {
						bind: first,
						bind: second,
					}
				}),
				"`bind:` may only be specified once per control",
			),
			(
				quote::quote!({
					select {
						multiple: dynamic_multiple,
						bind: value,
					}
				}),
				"a bound select requires a static `multiple`",
			),
			(
				quote::quote!({ input { a11y: off, type: "text", type: dynamic_type, bind: value } }),
				"a bound input requires a static `type`",
			),
			(
				quote::quote!({
					select {
						a11y: off,
						multiple: false,
						multiple: dynamic_multiple,
						bind: value,
					}
				}),
				"a bound select requires a static `multiple`",
			),
			(
				quote::quote!({ input { type: "file", bind: value } }),
				"`bind:` does not support input type `file`",
			),
			(
				quote::quote!({
					textarea {
						bind: number(value, error),
					}
				}),
				"`number(value, error)` is only valid on a numeric input",
			),
			(
				quote::quote!({ input { type: "number", bind: number(value) } }),
				"`number(value, error)` requires exactly two arguments",
			),
			(
				quote::quote!({
					input {
						value: "initial",
						bind: value,
					}
				}),
				"a bound text or number input cannot specify a `value` attribute",
			),
			(
				quote::quote!({ input { type: "checkbox", checked: true, bind: value } }),
				"a bound checkbox or radio input cannot specify a `checked` attribute",
			),
			(
				quote::quote!({ textarea { bind: value, "initial" } }),
				"a bound textarea cannot contain initial child content",
			),
			(
				quote::quote!({
					select {
						a11y: off,
						bind: value,
						option { { dynamic_label } }
					}
				}),
				"an option with dynamic content inside a bound select requires an explicit `value` attribute",
			),
			(
				quote::quote!({
					select {
						a11y: off,
						bind: value,
						option { script { "ignored" } }
					}
				}),
				"Element <option> in a bound select only supports non-interactive phrasing content",
			),
			(
				quote::quote!({
					select {
						a11y: off,
						bind: value,
						option {
							value: "explicit",
							span { strong { tabindex: dynamic_tabindex, "Label" } }
						}
					}
				}),
				"Element <option> in a bound select cannot contain a descendant with a `tabindex` attribute",
			),
			(
				quote::quote!({
					select {
						bind: value,
						optgroup { option { value: "one", selected: true, "One" } }
					}
				}),
				"a bound select cannot contain an option with a `selected` attribute",
			),
			(
				quote::quote!({
					select {
						a11y: off,
						bind: value,
						if first {
							option { value: "one", "One" }
						} else if second {
							option { value: "two", "Two" }
						} else {
							option { value: "three", selected: true, "Three" }
						}
					}
				}),
				"a bound select cannot contain an option with a `selected` attribute",
			),
		]
	}

	#[rstest]
	#[case(
		quote::quote!({ div { bind: value } }),
		"`bind:` is only valid on `input`, `textarea`, and `select`"
	)]
	#[case(
		quote::quote!({ input { type: dynamic_type, bind: value } }),
		"a bound input requires a static `type`"
	)]
	#[case(
		quote::quote!({ input { type: "radio", bind: value } }),
		"a bound radio input requires a `value` attribute"
	)]
	#[case(
		quote::quote!({ input { bind: first, bind: second } }),
		"`bind:` may only be specified once per control"
	)]
	#[case(
		quote::quote!({ select { multiple: dynamic_multiple, bind: value } }),
		"a bound select requires a static `multiple`"
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "text", type: dynamic_type, bind: value } }),
		"a bound input requires a static `type`"
	)]
	#[case(
		quote::quote!({ select { a11y: off, multiple: false, multiple: dynamic_multiple, bind: value } }),
		"a bound select requires a static `multiple`"
	)]
	#[case(
		quote::quote!({ input { type: "file", bind: value } }),
		"`bind:` does not support input type `file`"
	)]
	#[case(
		quote::quote!({ textarea { bind: number(value, error) } }),
		"`number(value, error)` is only valid on a numeric input"
	)]
	#[case(
		quote::quote!({ input { type: "number", bind: number(value) } }),
		"`number(value, error)` requires exactly two arguments"
	)]
	#[case(
		quote::quote!({ input { value: "initial", bind: value } }),
		"a bound text or number input cannot specify a `value` attribute"
	)]
	#[case(
		quote::quote!({ input { type: "checkbox", checked: true, bind: value } }),
		"a bound checkbox or radio input cannot specify a `checked` attribute"
	)]
	#[case(
		quote::quote!({ textarea { bind: value, "initial" } }),
		"a bound textarea cannot contain initial child content"
	)]
	#[case(
		quote::quote!({
			select {
				bind: value,
				optgroup { option { value: "one", selected: true, "One" } }
			}
		}),
		"a bound select cannot contain an option with a `selected` attribute"
	)]
	#[case(
		quote::quote!({
			select {
				a11y: off,
				bind: value,
				if first {
					option { value: "one", "One" }
				} else if second {
					option { value: "two", "Two" }
				} else {
					option { value: "three", selected: true, "Three" }
				}
			}
		}),
		"a bound select cannot contain an option with a `selected` attribute"
	)]
	fn controlled_binding_rejects_invalid_structure(
		#[case] input: proc_macro2::TokenStream,
		#[case] expected: &str,
	) {
		// Arrange
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Act
		let error = validate(&ast).unwrap_err();

		// Assert
		assert_eq!(error.to_string(), expected);
	}

	#[test]
	fn controlled_binding_accepts_false_selected_option() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote!({
			select { a11y: off, bind: value, option { selected: false, value: "one", "One" } }
		}))
		.unwrap();

		// Act
		let result = validate(&ast);

		// Assert
		assert!(result.is_ok());
	}

	#[test]
	fn controlled_binding_rejects_textarea_value_attribute() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote!({
			textarea {
				bind: value,
				value: "stale",
			}
		}))
		.unwrap();

		// Act
		let error = validate(&ast).unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"a bound textarea cannot specify a `value` attribute"
		);
	}

	#[test]
	fn literal_bound_select_marker_tag_does_not_enable_option_phrasing() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote!({
			__reinhardt_bound_select { option { span { "Label" } } }
		}))
		.unwrap();

		// Act
		let error = validate(&ast).unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Element <option> can only contain text, not child elements"
		);
	}

	#[rstest]
	#[case(false, false)]
	#[case(true, true)]
	fn select_resets_inherited_bound_context(
		#[case] nested_is_bound: bool,
		#[case] should_accept_phrasing_option: bool,
	) {
		// Arrange
		let input = if nested_is_bound {
			quote::quote!({ select { a11y: off, bind: nested, option { span { "Nested" } } } })
		} else {
			quote::quote!({ select { a11y: off, option { span { "Nested" } } } })
		};
		let ast: PageMacro = syn::parse2(input).unwrap();
		let PageNode::Element(element) = &ast.body().nodes[0] else {
			panic!("expected an element");
		};

		// Act
		let result = transform_element(
			element,
			&[],
			ValidationContext {
				inside_bound_select: true,
			},
		);

		// Assert
		assert_eq!(result.is_ok(), should_accept_phrasing_option);
		if !should_accept_phrasing_option {
			assert_eq!(
				result.unwrap_err().to_string(),
				"Element <option> can only contain text, not child elements"
			);
		}
	}

	#[test]
	fn bound_select_context_does_not_leak_to_outside_sibling() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote!({
			div {
				select { a11y: off, bind: outer, option { value: "outer", "Outer" } }
				select { a11y: off, option { span { "Sibling" } } }
			}
		}))
		.unwrap();

		// Act
		let error = validate(&ast).unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Element <option> can only contain text, not child elements"
		);
	}

	#[rstest]
	#[case(quote::quote!({
		select {
			a11y: off,
			bind: selected,
			if show { option { span { "Flow" } } }
		}
	}))]
	#[case(quote::quote!({
		select {
			a11y: off,
			bind: selected,
			ChoiceList() { option { span { "Component" } } }
		}
	}))]
	#[case(quote::quote!({
		select {
			a11y: off,
			bind: selected,
			ChoiceList() { $choices { option { span { "Slot" } } } }
		}
	}))]
	fn bound_select_context_propagates_through_non_select_nodes(
		#[case] input: proc_macro2::TokenStream,
	) {
		// Arrange
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Act
		let result = validate(&ast);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[case(quote::quote!({ select { a11y: off, bind: selected, option { span { tabindex: 0, "Zero" } } } }))]
	#[case(quote::quote!({ select { a11y: off, bind: selected, option { span { tabindex: -1, "Negative" } } } }))]
	#[case(quote::quote!({ select { a11y: off, bind: selected, option { span { tabindex: dynamic_tabindex, "Dynamic" } } } }))]
	#[case(quote::quote!({ select { a11y: off, bind: selected, option { value: "explicit", span { strong { tabindex: 0, "Nested" } } } } }))]
	fn bound_option_rejects_descendant_tabindex(#[case] input: proc_macro2::TokenStream) {
		// Arrange
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Act
		let error = validate(&ast).unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Element <option> in a bound select cannot contain a descendant with a `tabindex` attribute"
		);
	}

	#[test]
	fn bound_option_rejects_non_phrasing_control_flow_content() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote!({
			select {
				a11y: off,
				bind: value,
				option {
					value: "choice",
					if condition { div { "Block" } }
				}
			}
		}))
		.unwrap();

		// Act
		let error = validate(&ast).unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Element <option> in a bound select only supports non-interactive phrasing content"
		);
	}

	#[rstest]
	#[case(quote::quote!({ input { a11y: off, bind: value } }), TypedControlBindingKind::Text, false)]
	#[case(
		quote::quote!({ input { a11y: off, type: "checkbox", bind: value } }),
		TypedControlBindingKind::Checkbox,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "checkbox", checked: false, bind: value } }),
		TypedControlBindingKind::Checkbox,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "radio", value: choice, bind: value } }),
		TypedControlBindingKind::Radio,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "radio", value: choice, checked: false, bind: value } }),
		TypedControlBindingKind::Radio,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "number", bind: value } }),
		TypedControlBindingKind::Number,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "search", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "tel", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "url", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "email", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "password", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "color", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "date", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "datetime-local", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "month", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "week", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "time", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "range", bind: value } }),
		TypedControlBindingKind::Number,
		false
	)]
	#[case(
		quote::quote!({ input { a11y: off, type: "number", bind: number(value, error) } }),
		TypedControlBindingKind::Number,
		true
	)]
	#[case(quote::quote!({ textarea { a11y: off, bind: value } }), TypedControlBindingKind::Text, false)]
	#[case(quote::quote!({ select { a11y: off, bind: value } }), TypedControlBindingKind::SelectOne, false)]
	#[case(
		quote::quote!({
			select {
				a11y: off,
				bind: value,
				option { span { "Static" } }
			}
		}),
		TypedControlBindingKind::SelectOne,
		false
	)]
	#[case(
		quote::quote!({
			select {
				a11y: off,
				bind: value,
				option { value: "dynamic", { dynamic_label } }
			}
		}),
		TypedControlBindingKind::SelectOne,
		false
	)]
	#[case(
		quote::quote!({ select { a11y: off, multiple: true, bind: value } }),
		TypedControlBindingKind::SelectMany,
		false
	)]
	#[case(
		quote::quote!({ select { a11y: off, multiple: false, bind: value } }),
		TypedControlBindingKind::SelectOne,
		false
	)]
	fn controlled_binding_accepts_supported_structure(
		#[case] input: proc_macro2::TokenStream,
		#[case] expected_kind: TypedControlBindingKind,
		#[case] expects_number_error: bool,
	) {
		// Arrange
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Act
		let typed = validate(&ast).unwrap();
		let TypedPageNode::Element(element) = &typed.body().nodes[0] else {
			panic!("expected a typed element");
		};
		let binding = element.control_binding.as_ref().unwrap();

		// Assert
		assert!(element.attrs.iter().all(|attr| attr.html_name() != "bind"));
		assert_eq!(binding.kind, expected_kind);
		assert_eq!(
			matches!(
				binding.expression,
				TypedControlBindingExpr::NumberWithError { .. }
			),
			expects_number_error
		);
		if expected_kind == TypedControlBindingKind::SelectOne {
			assert!(
				element
					.attrs
					.iter()
					.all(|attr| attr.html_name() != "multiple")
			);
		}
		assert_eq!(
			binding.radio_value.is_some(),
			expected_kind == TypedControlBindingKind::Radio
		);
	}

	#[rstest]
	fn controlled_binding_diagnostics_match_shared_validator() {
		for (input, expected) in controlled_binding_invalid_cases() {
			// Arrange
			let ast: PageMacro = syn::parse2(input).unwrap();

			// Act
			let shared_error = reinhardt_manouche::validator::validate_page(&ast).unwrap_err();
			let macro_error = validate(&ast).unwrap_err();

			// Assert
			assert_eq!(shared_error.to_string(), expected);
			assert_eq!(macro_error.to_string(), shared_error.to_string());
		}
	}

	#[test]
	fn test_validate_valid_closure() {
		let event = IntrinsicEvent::Standard {
			event: reinhardt_event_catalog::KnownEvent::Click,
			handler: parse_quote!(|_| {}),
		};
		assert!(validate_intrinsic_event_handler(&event).is_ok());
	}

	#[test]
	fn test_validate_closure_with_one_arg() {
		let event = IntrinsicEvent::Standard {
			event: reinhardt_event_catalog::KnownEvent::Click,
			handler: parse_quote!(|e| {
				handle_click(e);
			}),
		};
		assert!(validate_intrinsic_event_handler(&event).is_ok());
	}

	#[test]
	fn test_validate_closure_too_many_args() {
		let event = IntrinsicEvent::Standard {
			event: reinhardt_event_catalog::KnownEvent::Click,
			handler: parse_quote!(|a, b, c| {}),
		};
		let result = validate_intrinsic_event_handler(&event);
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("0 or 1 arguments"));
	}

	#[test]
	fn test_validate_valid_data_attribute() {
		let attr = PageAttr {
			name: syn::Ident::new("data_testid", proc_macro2::Span::call_site()),
			value: parse_quote!("test"),
			span: proc_macro2::Span::call_site(),
		};
		assert!(validate_attribute(&attr, "div").is_ok());
	}

	#[test]
	fn test_validate_invalid_data_attribute() {
		let attr = PageAttr {
			name: syn::Ident::new("data_", proc_macro2::Span::call_site()),
			value: parse_quote!("test"),
			span: proc_macro2::Span::call_site(),
		};
		let result = validate_attribute(&attr, "div");
		assert!(result.is_err());
	}

	#[test]
	fn test_validate_valid_aria_attribute() {
		let attr = PageAttr {
			name: syn::Ident::new("aria_label", proc_macro2::Span::call_site()),
			value: parse_quote!("Navigation"),
			span: proc_macro2::Span::call_site(),
		};
		assert!(validate_attribute(&attr, "div").is_ok());
	}

	#[test]
	fn test_void_element_with_children() {
		let mut elem = PageElement::new(
			syn::Ident::new("input", proc_macro2::Span::call_site()),
			proc_macro2::Span::call_site(),
		);
		elem.children
			.push(PageNode::Text(reinhardt_manouche::core::PageText {
				content: "text".to_string(),
				span: proc_macro2::Span::call_site(),
			}));

		let result = validate_element_nesting(&elem, &[]);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("cannot have children")
		);
	}

	#[test]
	fn test_nested_interactive_elements() {
		let elem = PageElement::new(
			syn::Ident::new("button", proc_macro2::Span::call_site()),
			proc_macro2::Span::call_site(),
		);

		let result = validate_element_nesting(&elem, &["a".to_string()]);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("cannot be nested inside another interactive element")
		);
	}

	#[test]
	fn test_transform_attrs_with_string_lit() {
		let attrs = vec![PageAttr {
			name: syn::Ident::new("src", proc_macro2::Span::call_site()),
			value: parse_quote!("/image.png"),
			span: proc_macro2::Span::call_site(),
		}];

		let result = transform_attrs(&attrs, "img");
		assert!(result.is_ok());
		let typed_attrs = result.unwrap().attrs;
		assert_eq!(typed_attrs.len(), 1);
		assert!(typed_attrs[0].value.is_string_literal());
	}

	#[test]
	fn test_transform_attrs_with_dynamic() {
		let attrs = vec![PageAttr {
			name: syn::Ident::new("src", proc_macro2::Span::call_site()),
			value: parse_quote!(image_url),
			span: proc_macro2::Span::call_site(),
		}];

		let result = transform_attrs(&attrs, "div");
		assert!(result.is_ok());
		let typed_attrs = result.unwrap().attrs;
		assert_eq!(typed_attrs.len(), 1);
		assert!(typed_attrs[0].value.is_dynamic());
	}

	#[test]
	fn test_validate_attr_type_img_src_literal() {
		let value = AttrValue::from_expr(parse_quote!("/image.png"));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_attr_type_img_src_dynamic() {
		// Dynamic expressions (function calls, identifiers) are accepted —
		// their value can only be validated at runtime.
		let value = AttrValue::from_expr(parse_quote!(resolve_static("images/poll.svg")));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_attr_type_img_src_empty() {
		let value = AttrValue::from_expr(parse_quote!(""));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("must not be empty")
		);
	}

	// Boolean attribute tests - string literals are prohibited
	#[test]
	fn test_validate_boolean_attr_string_literal() {
		let value = AttrValue::from_expr(parse_quote!("disabled"));
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a string literal value"));
	}

	#[test]
	fn test_validate_boolean_attr_string_empty() {
		let value = AttrValue::from_expr(parse_quote!(""));
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a string literal value"));
	}

	#[test]
	fn test_validate_boolean_attr_bool_literal_true() {
		let value = AttrValue::from_expr(parse_quote!(true));
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_boolean_attr_bool_literal_false() {
		let value = AttrValue::from_expr(parse_quote!(false));
		let result = validate_attr_type("checked", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("cannot be set to `false`"));
	}

	#[test]
	fn test_validate_boolean_attr_int_literal() {
		let value = AttrValue::from_expr(parse_quote!(1));
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a numeric literal value"));
	}

	#[test]
	fn test_validate_boolean_attr_float_literal() {
		let value = AttrValue::from_expr(parse_quote!(1.0));
		let result =
			validate_attr_type("required", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a numeric literal value"));
	}

	#[test]
	fn test_validate_boolean_attr_dynamic_variable() {
		let value = AttrValue::from_expr(parse_quote!(is_disabled));
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_boolean_attr_dynamic_function() {
		let value = AttrValue::from_expr(parse_quote!(is_disabled()));
		let result = validate_attr_type("checked", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_boolean_attr_dynamic_conditional() {
		let value = AttrValue::from_expr(parse_quote!(if condition { true } else { false }));
		let result =
			validate_attr_type("readonly", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	// Numeric attribute tests - string and float literals are prohibited
	#[test]
	fn test_validate_numeric_attr_string_literal() {
		let value = AttrValue::from_expr(parse_quote!("100"));
		let result =
			validate_attr_type("maxlength", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Attribute"));
		assert!(err_msg.contains("must be an integer literal or dynamic expression"));
	}

	#[test]
	fn test_validate_numeric_attr_float_literal() {
		let value = AttrValue::from_expr(parse_quote!(100.0));
		let result = validate_attr_type("rows", &value, "textarea", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Attribute"));
		assert!(err_msg.contains("must be an integer, not a floating-point number"));
	}

	#[test]
	fn test_validate_numeric_attr_bool_literal() {
		let value = AttrValue::from_expr(parse_quote!(true));
		let result = validate_attr_type("min", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Attribute"));
		assert!(err_msg.contains("must be an integer, not a boolean"));
	}

	#[test]
	fn test_validate_numeric_attr_int_literal() {
		let value = AttrValue::from_expr(parse_quote!(100));
		let result =
			validate_attr_type("maxlength", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_numeric_attr_dynamic_variable() {
		let value = AttrValue::from_expr(parse_quote!(max_len));
		let result =
			validate_attr_type("maxlength", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_numeric_attr_dynamic_function() {
		let value = AttrValue::from_expr(parse_quote!(get_max_len()));
		let result = validate_attr_type("cols", &value, "textarea", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	// URL attribute tests - dangerous schemes and empty strings are prohibited
	#[test]
	fn test_validate_url_attr_javascript_scheme() {
		let value = AttrValue::from_expr(parse_quote!("javascript:alert('xss')"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
		assert!(err_msg.contains("XSS"));
	}

	#[test]
	fn test_validate_url_attr_data_scheme() {
		let value =
			AttrValue::from_expr(parse_quote!("data:text/html,<script>alert('xss')</script>"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_vbscript_scheme() {
		let value = AttrValue::from_expr(parse_quote!("vbscript:msgbox('xss')"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_case_insensitive() {
		let value = AttrValue::from_expr(parse_quote!("JavaScript:alert(1)"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_empty_string() {
		let value = AttrValue::from_expr(parse_quote!(""));
		let result = validate_attr_type("action", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("cannot be empty"));
	}

	#[test]
	fn test_validate_url_attr_whitespace_string() {
		let value = AttrValue::from_expr(parse_quote!("   "));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("cannot be empty"));
	}

	#[test]
	fn test_validate_url_attr_https_scheme() {
		let value = AttrValue::from_expr(parse_quote!("https://example.com"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_url_attr_relative_path() {
		let value = AttrValue::from_expr(parse_quote!("/path/to/page"));
		let result = validate_attr_type("action", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_url_attr_anchor() {
		let value = AttrValue::from_expr(parse_quote!("#section"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_url_attr_dynamic_variable() {
		let value = AttrValue::from_expr(parse_quote!(url_var));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_url_attr_img_src_dangerous_scheme() {
		// img src should be validated for dangerous URL schemes (Fixes #849)
		let value = AttrValue::from_expr(parse_quote!("javascript:alert(1)"));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_img_src_data_scheme() {
		let value =
			AttrValue::from_expr(parse_quote!("data:text/html,<script>alert('xss')</script>"));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_img_src_vbscript_scheme() {
		let value = AttrValue::from_expr(parse_quote!("vbscript:alert(1)"));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_img_src_safe_url() {
		let value = AttrValue::from_expr(parse_quote!("/images/photo.png"));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	// Enumerated attribute tests - invalid values are prohibited
	#[test]
	fn test_validate_enum_attr_input_type_invalid() {
		let value = AttrValue::from_expr(parse_quote!("invalid"));
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
		assert!(err_msg.contains("invalid"));
		assert!(err_msg.contains("input"));
	}

	#[test]
	fn test_validate_enum_attr_input_type_text() {
		let value = AttrValue::from_expr(parse_quote!("text"));
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_input_type_email() {
		let value = AttrValue::from_expr(parse_quote!("email"));
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_button_type_invalid() {
		let value = AttrValue::from_expr(parse_quote!("invalid"));
		let result = validate_attr_type("type", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
		assert!(err_msg.contains("button"));
	}

	#[test]
	fn test_validate_enum_attr_button_type_submit() {
		let value = AttrValue::from_expr(parse_quote!("submit"));
		let result = validate_attr_type("type", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_form_method_invalid() {
		let value = AttrValue::from_expr(parse_quote!("put"));
		let result = validate_attr_type("method", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
		assert!(err_msg.contains("put"));
	}

	#[test]
	fn test_validate_enum_attr_form_method_post() {
		let value = AttrValue::from_expr(parse_quote!("post"));
		let result = validate_attr_type("method", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_form_enctype_invalid() {
		let value = AttrValue::from_expr(parse_quote!("invalid"));
		let result = validate_attr_type("enctype", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
	}

	#[test]
	fn test_validate_enum_attr_form_enctype_multipart() {
		let value = AttrValue::from_expr(parse_quote!("multipart/form-data"));
		let result = validate_attr_type("enctype", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_script_type_module() {
		let value = AttrValue::from_expr(parse_quote!("module"));
		let result = validate_attr_type("type", &value, "script", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_dynamic_variable() {
		let value = AttrValue::from_expr(parse_quote!(input_type));
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	// Accessibility tests - button elements must have text or aria-label
	#[test]
	fn test_validate_button_accessibility_empty() {
		let attrs = vec![];
		let children = vec![];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("requires accessible text"));
		assert!(err_msg.contains("aria_label"));
	}

	#[test]
	fn test_validate_button_accessibility_whitespace_only() {
		use reinhardt_manouche::core::{PageText, TypedPageNode};
		let attrs = vec![];
		let children = vec![TypedPageNode::Text(PageText {
			content: "   ".to_string(),
			span: proc_macro2::Span::call_site(),
		})];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_err());
	}

	#[test]
	fn test_validate_button_accessibility_with_text() {
		use reinhardt_manouche::core::{PageText, TypedPageNode};
		let attrs = vec![];
		let children = vec![TypedPageNode::Text(PageText {
			content: "Click me".to_string(),
			span: proc_macro2::Span::call_site(),
		})];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_button_accessibility_with_nested_text() {
		use reinhardt_manouche::core::{PageText, TypedPageElement, TypedPageNode};
		let attrs = vec![];
		let children = vec![TypedPageNode::Element(TypedPageElement {
			tag: syn::Ident::new("span", proc_macro2::Span::call_site()),
			attrs: vec![],
			control_binding: None,
			events: vec![],
			children: vec![TypedPageNode::Text(PageText {
				content: "Submit".to_string(),
				span: proc_macro2::Span::call_site(),
			})],
			a11y_disabled: false,
			span: proc_macro2::Span::call_site(),
		})];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_button_accessibility_with_aria_label() {
		use reinhardt_manouche::core::TypedPageAttr;
		let attrs = vec![TypedPageAttr {
			name: syn::Ident::new("aria_label", proc_macro2::Span::call_site()),
			value: AttrValue::from_expr(parse_quote!("Close")),
			span: proc_macro2::Span::call_site(),
		}];
		let children = vec![];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_button_accessibility_with_expression() {
		use reinhardt_manouche::core::{PageExpression, TypedPageNode};
		let attrs = vec![];
		let children = vec![TypedPageNode::Expression(PageExpression {
			expr: parse_quote!(button_text),
			braced: true,
			span: proc_macro2::Span::call_site(),
		})];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[rstest]
	#[case("https://example.com", true)]
	#[case("http://example.com", true)]
	#[case("mailto:user@example.com", true)]
	#[case("ftp://files.example.com", true)]
	#[case("ftps://files.example.com", true)]
	#[case("/relative/path", true)]
	#[case("./local/path", true)]
	#[case("#anchor", true)]
	#[case("javascript:alert(1)", false)]
	#[case("data:text/html,<script>alert(1)</script>", false)]
	#[case("vbscript:msgbox", false)]
	#[case("JAVASCRIPT:alert(1)", false)]
	fn test_is_safe_url(#[case] url: &str, #[case] expected: bool) {
		assert_eq!(is_safe_url(url), expected);
	}

	/// Helper: parse a `quote!`-built TokenStream into a `PageMacro` AST.
	///
	/// Scoped to this test module for now; PR1 may promote it to a shared
	/// helper. Panics on parse failure so individual tests stay concise.
	fn parse(input: proc_macro2::TokenStream) -> PageMacro {
		syn::parse2(input).expect("test input must parse as PageMacro")
	}

	#[rstest]
	fn rejects_bare_identifier_shorthand() {
		// Arrange
		let ast = parse(quote::quote! {
			|name: String| { div { name } }
		});

		// Act
		let result = validate(&ast);

		// Assert
		let err = result.unwrap_err();
		let msg = err.to_string();
		assert!(
			msg.contains("bare identifier"),
			"expected diagnostic to mention 'bare identifier', got: {msg}"
		);
		assert!(
			msg.contains("{name}"),
			"expected fix-it hint to quote `{{name}}`, got: {msg}"
		);
	}
}

#[cfg(test)]
mod capture_tests {
	use super::*;
	use proc_macro2::TokenStream;
	use quote::quote;
	use rstest::rstest;

	fn parse(input: TokenStream) -> PageMacro {
		syn::parse2(input).expect("input must be a valid page! macro")
	}

	#[rstest]
	fn rejects_implicit_lowercase_capture() {
		// Arrange
		let ast = parse(quote! {
			|| { div { {outer_count.get()} } }
		});

		// Act
		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		// Assert
		let err = result.unwrap_err();
		assert!(err.to_string().contains("outer_count"));
	}

	#[rstest]
	fn accepts_declared_param() {
		// Arrange
		let ast = parse(quote! {
			|outer_count: reinhardt_pages::reactive::Signal<i32>| {
				div { {outer_count.get()} }
			}
		});

		// Act
		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn accepts_numeric_binding_sentinel_without_a_bogus_parameter() {
		let ast = parse(quote! {
			|amount: reinhardt_pages::reactive::Signal<i32>, parse_error: reinhardt_pages::reactive::Signal<Option<String>>| {
				input { a11y: off, type: "number", bind: number(amount, parse_error) }
			}
		});

		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		assert!(result.is_ok());
	}

	#[rstest]
	fn accepts_multi_segment_path() {
		// Arrange
		let ast = parse(quote! {
			|| { div { {crate::util::truncate("x", 5)} } }
		});

		// Act
		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn accepts_screaming_snake_constant() {
		// Arrange
		let ast = parse(quote! {
			|| { p { {format!("limit={}", MAX_LEN)} } }
		});

		// Act
		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn strict_form_rejects_macro_argument_capture() {
		// Arrange
		let ast = parse(quote! {
			|| { p { {format!("value={}", outer_value)} } }
		});

		// Act
		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		// Assert
		let err = result.expect_err("macro arguments should still be scanned");
		assert!(err.to_string().contains("outer_value"));
	}

	#[rstest]
	fn accepts_pascal_case_type_or_component() {
		// Arrange
		let ast = parse(quote! {
			|| { div { {Vec::<i32>::new().len()} } }
		});

		// Act
		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn accepts_for_loop_local_binding() {
		// Arrange
		let ast = parse(quote! {
			|items: Vec<i32>| {
				div {
					for x in items.iter() { li { {x.to_string()} } }
				}
			}
		});

		// Act
		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn accepts_page_for_key_loop_local_binding() {
		// Arrange
		let ast = parse(quote! {
			|items: Vec<String>| {
				ul {
					for item in items @key(item.clone()) {
						li { {item.clone()} }
					}
				}
			}
		});

		// Act
		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn strict_form_rejects_page_for_key_outer_capture() {
		// Arrange
		let ast = parse(quote! {
			|items: Vec<String>| {
				ul {
					for item in items @key(route_id.to_string()) {
						li { {item.clone()} }
					}
				}
			}
		});

		// Act
		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		// Assert
		let err = result.unwrap_err();
		assert!(err.to_string().contains("route_id"));
	}

	#[rstest]
	fn strict_form_records_page_for_iter_and_key_captures() {
		// Arrange
		let ast = parse(quote! {
			|| {
				ul {
					for item in items @key(route_id.to_string()) {
						li { {item.clone()} }
					}
				}
			}
		});

		// Act
		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());
		let captures: Vec<String> =
			collect_free_idents(ast.head.as_ref(), ast.body(), ast.params())
				.into_iter()
				.map(|capture| capture.ident.to_string())
				.collect();

		// Assert
		let err = result.unwrap_err();
		assert!(err.to_string().contains("items"));
		assert_eq!(captures, vec!["items", "route_id"]);
	}

	#[rstest]
	fn body_only_form_records_implicit_captures() {
		// Arrange
		let ast = parse(quote! {
			{ div { {outer_count.get()} } }
		});

		// Act
		let result = validate(&ast).unwrap();

		// Assert
		let captures = result.implicit_captures();
		assert_eq!(captures.len(), 1);
		assert_eq!(captures[0].ident.to_string(), "outer_count");
	}

	#[rstest]
	fn body_only_form_records_page_for_key_outer_capture() {
		// Arrange
		let ast = parse(quote! {
			{
				ul {
					for item in items @key(route_id.to_string()) {
						li { {item.clone()} }
					}
				}
			}
		});

		// Act
		let result = validate(&ast).unwrap();

		// Assert
		let captures: Vec<String> = result
			.implicit_captures()
			.iter()
			.map(|capture| capture.ident.to_string())
			.collect();
		assert_eq!(captures, vec!["items", "route_id"]);
		assert!(!captures.iter().any(|capture| capture == "item"));
	}

	#[rstest]
	fn body_only_form_records_macro_argument_capture() {
		// Arrange
		let ast = parse(quote! {
			{ p { {format!("value={}", outer_value)} } }
		});

		// Act
		let result = validate(&ast).expect("implicit body should validate");
		let captures: Vec<String> = result
			.implicit_captures()
			.iter()
			.map(|capture| capture.ident.to_string())
			.collect();

		// Assert
		assert_eq!(captures, vec!["outer_value"]);
	}

	#[rstest]
	fn body_only_form_ignores_named_macro_argument_key() {
		let ast = parse(quote! {
			{ p { {::reinhardt_pages::t!("Project {id}", id = project_id)} } }
		});

		let result = validate(&ast).expect("implicit body should validate");
		let captures: Vec<String> = result
			.implicit_captures()
			.iter()
			.map(|capture| capture.ident.to_string())
			.collect();

		assert_eq!(captures, vec!["project_id"]);
	}

	#[rstest]
	fn strict_form_ignores_named_macro_argument_key() {
		let ast = parse(quote! {
			|project_id: i64| { p { {::reinhardt::pages::t!("Project {id}", id = project_id)} } }
		});

		let result = enforce_strict_captures(ast.head.as_ref(), ast.body(), ast.params());

		assert!(result.is_ok());
	}

	#[rstest]
	fn relative_framework_macro_path_keeps_assignment_key_as_capture() {
		let ast = parse(quote! {
			{ p { {reinhardt_pages::t!("Project {id}", id = project_id)} } }
		});

		let result = validate(&ast).expect("implicit body should validate");
		let captures: Vec<String> = result
			.implicit_captures()
			.iter()
			.map(|capture| capture.ident.to_string())
			.collect();

		assert_eq!(captures, vec!["id", "project_id"]);
	}

	#[rstest]
	fn strict_form_still_rejects_implicit_captures() {
		// Arrange
		let ast = parse(quote! {
			|| { div { {outer_count.get()} } }
		});

		// Act
		let result = validate(&ast);

		// Assert
		let err = result.unwrap_err();
		assert!(err.to_string().contains("outer_count"));
	}
}
