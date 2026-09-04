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

use proc_macro2::Span;
use std::collections::HashSet;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, Result, Token};

use crate::core::{
	ComponentEventProp, ImplicitPageCapture, IntrinsicEvent, PageAttr, PageBody, PageComponent,
	PageElement, PageElse, PageExpression, PageFor, PageIf, PageMacro, PageMacroForm, PageNode,
	PageParam, PageWatch, TypedControlBinding, TypedControlBindingExpr, TypedControlBindingKind,
	TypedNamedSlot, TypedPageAttr, TypedPageBody, TypedPageComponent, TypedPageElement,
	TypedPageElse, TypedPageFor, TypedPageIf, TypedPageMacro, TypedPageMacroForm, TypedPageNode,
	TypedPageWatch, types::AttrValue,
};

#[derive(Clone, Copy, Default)]
struct ValidationContext {
	inside_bound_select: bool,
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
pub fn validate_page(ast: &PageMacro) -> Result<TypedPageMacro> {
	let form = match &ast.form {
		PageMacroForm::StrictClosure { params, body } => {
			enforce_strict_captures(ast.head.as_ref(), body, params)?;
			let typed_body = transform_body(body, &[], ValidationContext::default())?;
			validate_page_accessibility(&typed_body)?;
			TypedPageMacroForm::StrictClosure {
				params: params.clone(),
				body: typed_body,
			}
		}
		PageMacroForm::ImplicitBody { body } => {
			let typed_body = transform_body(body, &[], ValidationContext::default())?;
			validate_page_accessibility(&typed_body)?;
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

fn collect_free_idents(
	head: Option<&Expr>,
	body: &PageBody,
	params: &[PageParam],
) -> Vec<ImplicitPageCapture> {
	let allowed: HashSet<String> = params.iter().map(|p| p.name.to_string()).collect();

	let mut collector = CaptureCollector {
		allowed,
		locals_stack: Vec::new(),
		seen: HashSet::new(),
		captures: Vec::new(),
	};
	if let Some(head_expr) = head {
		collector.visit_expr(head_expr);
	}
	collector.visit_body(body);
	collector.captures
}

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

struct CaptureCollector {
	allowed: HashSet<String>,
	locals_stack: Vec<HashSet<String>>,
	seen: HashSet<String>,
	captures: Vec<ImplicitPageCapture>,
}

impl CaptureCollector {
	fn is_known(&self, name: &str) -> bool {
		self.allowed.contains(name) || self.locals_stack.iter().any(|s| s.contains(name))
	}

	fn record_capture(&mut self, ident: &syn::Ident) {
		let name = ident.to_string();
		if self.seen.insert(name) {
			self.captures.push(ImplicitPageCapture {
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
			PageNode::Expression(expr) => self.visit_expression(expr),
			PageNode::If(if_node) => self.visit_if(if_node),
			PageNode::For(for_node) => self.visit_for(for_node),
			PageNode::Component(comp) => self.visit_component(comp),
			PageNode::Watch(watch) => self.visit_node(&watch.expr),
		}
	}

	fn visit_element(&mut self, el: &PageElement) {
		for attr in &el.attrs {
			if attr.html_name() == "a11y" {
				continue;
			}
			if attr.html_name() == "bind" {
				self.visit_binding_expr(&attr.value);
			} else {
				self.visit_expr(&attr.value);
			}
		}
		for event in &el.events {
			self.visit_expr(event.handler());
		}
		for child in &el.children {
			self.visit_node(child);
		}
	}

	fn visit_binding_expr(&mut self, expr: &Expr) {
		if let Expr::Call(call) = expr
			&& let Expr::Path(path) = call.func.as_ref()
			&& path.qself.is_none()
			&& path.path.is_ident("number")
			&& call.args.len() == 2
		{
			for arg in &call.args {
				self.visit_expr(arg);
			}
		} else {
			self.visit_expr(expr);
		}
	}

	fn visit_expression(&mut self, expr: &PageExpression) {
		self.visit_expr(&expr.expr);
	}

	fn visit_if(&mut self, if_node: &PageIf) {
		let mut then_locals = None;
		if let Expr::Let(let_expr) = &if_node.condition {
			let mut locals = HashSet::new();
			collect_pat_idents(&let_expr.pat, &mut locals);
			self.visit_expr(&let_expr.expr);
			then_locals = Some(locals);
		} else {
			self.visit_expr(&if_node.condition);
		}
		let pushed_then_locals = then_locals.is_some();
		if let Some(locals) = then_locals {
			self.locals_stack.push(locals);
		}
		for node in &if_node.then_branch {
			self.visit_node(node);
		}
		if pushed_then_locals {
			self.locals_stack.pop();
		}
		if let Some(else_branch) = &if_node.else_branch {
			match else_branch {
				PageElse::Block(nodes) => {
					for node in nodes {
						self.visit_node(node);
					}
				}
				PageElse::If(inner) => self.visit_if(inner),
			}
		}
	}

	fn visit_for(&mut self, for_node: &PageFor) {
		self.visit_expr(&for_node.iter);
		let mut locals = HashSet::new();
		collect_pat_idents(&for_node.pat, &mut locals);
		self.locals_stack.push(locals);
		if let Some(key) = &for_node.key {
			self.visit_expr(key);
		}
		for node in &for_node.body {
			self.visit_node(node);
		}
		self.locals_stack.pop();
	}

	fn visit_component(&mut self, comp: &PageComponent) {
		for arg in &comp.args {
			self.visit_expr(&arg.value);
		}
		for event in &comp.events {
			self.visit_expr(&event.handler);
		}
		if let Some(children) = &comp.children {
			for child in children {
				self.visit_node(child);
			}
		}
		for slot in &comp.named_slots {
			for child in &slot.children {
				self.visit_node(child);
			}
		}
	}

	fn visit_expr(&mut self, expr: &Expr) {
		let mut visitor = ExprIdentVisitor { collector: self };
		visitor.visit_expr(expr);
	}
}

struct ExprIdentVisitor<'a> {
	collector: &'a mut CaptureCollector,
}

impl<'ast> Visit<'ast> for ExprIdentVisitor<'_> {
	fn visit_expr_path(&mut self, expr_path: &'ast syn::ExprPath) {
		if expr_path.qself.is_none() && expr_path.path.segments.len() == 1 {
			let segment = &expr_path.path.segments[0];
			let name = segment.ident.to_string();
			if is_value_ident(&name) && !self.collector.is_known(&name) {
				self.collector.record_capture(&segment.ident);
			}
		}
		visit::visit_expr_path(self, expr_path);
	}

	fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
		let mut locals = HashSet::new();
		for input in &closure.inputs {
			collect_pat_idents(input, &mut locals);
		}
		self.collector.locals_stack.push(locals);
		visit::visit_expr_closure(self, closure);
		self.collector.locals_stack.pop();
	}

	fn visit_expr_let(&mut self, let_expr: &'ast syn::ExprLet) {
		let mut locals = HashSet::new();
		collect_pat_idents(&let_expr.pat, &mut locals);
		self.collector.locals_stack.push(locals);
		visit::visit_expr_let(self, let_expr);
		self.collector.locals_stack.pop();
	}

	fn visit_expr_if(&mut self, if_expr: &'ast syn::ExprIf) {
		if let syn::Expr::Let(let_expr) = &*if_expr.cond {
			let mut locals = HashSet::new();
			collect_pat_idents(&let_expr.pat, &mut locals);
			self.visit_expr(&let_expr.expr);
			self.collector.locals_stack.push(locals);
			self.visit_block(&if_expr.then_branch);
			self.collector.locals_stack.pop();
			if let Some((_, else_branch)) = &if_expr.else_branch {
				self.visit_expr(else_branch);
			}
		} else {
			visit::visit_expr_if(self, if_expr);
		}
	}

	fn visit_arm(&mut self, arm: &'ast syn::Arm) {
		let mut locals = HashSet::new();
		collect_pat_idents(&arm.pat, &mut locals);
		self.collector.locals_stack.push(locals);
		if let Some((_, guard)) = &arm.guard {
			self.visit_expr(guard);
		}
		self.visit_expr(&arm.body);
		self.collector.locals_stack.pop();
	}

	fn visit_expr_for_loop(&mut self, for_loop: &'ast syn::ExprForLoop) {
		self.visit_expr(&for_loop.expr);
		let mut locals = HashSet::new();
		collect_pat_idents(&for_loop.pat, &mut locals);
		self.collector.locals_stack.push(locals);
		self.visit_block(&for_loop.body);
		self.collector.locals_stack.pop();
	}

	fn visit_expr_macro(&mut self, expr_macro: &'ast syn::ExprMacro) {
		if let Ok(args) = expr_macro
			.mac
			.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
		{
			for arg in args {
				self.visit_expr(&arg);
			}
		}
	}

	fn visit_block(&mut self, block: &'ast syn::Block) {
		let mut pushed = 0_usize;
		for stmt in &block.stmts {
			match stmt {
				syn::Stmt::Local(local) => {
					if let Some(init) = &local.init {
						self.visit_expr(&init.expr);
						if let Some((_, diverge)) = &init.diverge {
							self.visit_expr(diverge);
						}
					}
					let mut locals = HashSet::new();
					collect_pat_idents(&local.pat, &mut locals);
					self.collector.locals_stack.push(locals);
					pushed += 1;
				}
				syn::Stmt::Item(_) => {}
				syn::Stmt::Expr(expr, _) => self.visit_expr(expr),
				syn::Stmt::Macro(mac) => visit::visit_stmt_macro(self, mac),
			}
		}
		for _ in 0..pushed {
			self.collector.locals_stack.pop();
		}
	}
}

fn collect_pat_idents(pat: &syn::Pat, out: &mut HashSet<String>) {
	match pat {
		syn::Pat::Ident(ident) => {
			out.insert(ident.ident.to_string());
			if let Some((_, subpat)) = &ident.subpat {
				collect_pat_idents(subpat, out);
			}
		}
		syn::Pat::Tuple(tuple) => {
			for elem in &tuple.elems {
				collect_pat_idents(elem, out);
			}
		}
		syn::Pat::TupleStruct(tuple_struct) => {
			for elem in &tuple_struct.elems {
				collect_pat_idents(elem, out);
			}
		}
		syn::Pat::Struct(struct_pat) => {
			for field in &struct_pat.fields {
				collect_pat_idents(&field.pat, out);
			}
		}
		syn::Pat::Reference(reference) => collect_pat_idents(&reference.pat, out),
		syn::Pat::Type(typed) => collect_pat_idents(&typed.pat, out),
		syn::Pat::Or(or_pat) => {
			for case in &or_pat.cases {
				collect_pat_idents(case, out);
			}
		}
		syn::Pat::Slice(slice) => {
			for elem in &slice.elems {
				collect_pat_idents(elem, out);
			}
		}
		syn::Pat::Paren(paren) => collect_pat_idents(&paren.pat, out),
		_ => {}
	}
}

fn is_value_ident(name: &str) -> bool {
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

fn missing_param_error(ident: &syn::Ident) -> syn::Error {
	syn::Error::new(
		ident.span(),
		format!("identifier `{ident}` used inside `page!` is not declared as a parameter"),
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
		PageNode::Expression(expr) => Ok(TypedPageNode::Expression(expr.clone())),
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
		PageNode::Watch(watch_node) => Ok(TypedPageNode::Watch(transform_watch(
			watch_node,
			parent_tags,
			context,
		)?)),
	}
}

/// Transforms a PageIf node (if/else if/else).
///
/// Recursively validates all branches.
fn transform_if(
	if_node: &PageIf,
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
	for_node: &PageFor,
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
	if tag == "select"
		&& control_binding
			.as_deref()
			.is_some_and(|binding| binding.kind == TypedControlBindingKind::SelectOne)
	{
		ordinary_attrs.retain(|attr| !is_false_multiple_attr(attr));
	}
	if tag == "option" && context.inside_bound_select {
		ordinary_attrs.retain(|attr| {
			!(attr.html_name() == "selected"
				&& matches!(&attr.value, Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Bool(value) if !value.value())))
		});
	}
	let allow_false_checked = control_binding.as_deref().is_some_and(|binding| {
		matches!(
			binding.kind,
			TypedControlBindingKind::Checkbox | TypedControlBindingKind::Radio
		)
	});
	let transformed_attrs = transform_attrs(&ordinary_attrs, &tag, allow_false_checked)?;
	let typed_attrs = transformed_attrs.attrs;

	// 3. Validate element nesting
	validate_element_nesting(elem, parent_tags)?;

	// 4. Validate required attributes (using typed attrs)
	validate_required_attributes(
		&tag,
		&typed_attrs,
		elem.span,
		transformed_attrs.a11y_disabled,
	)?;

	// 5. Recursively transform children
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

/// Classifies a statically typed `<input>` binding for downstream page macros.
///
/// String-valued controls share the `Text` binding semantics, while `number`
/// and `range` share the `Number` semantics. Checkbox and radio controls retain
/// their checked-state behavior.
#[doc(hidden)]
pub fn classify_input_binding(
	attrs: &[PageAttr],
	binding_attr: &PageAttr,
) -> Result<(TypedControlBindingKind, Option<Expr>)> {
	let input_type = match unique_untyped_attr(
		attrs,
		"type",
		binding_attr,
		"a bound input requires a static `type`",
	)? {
		Some(attr) => static_string_value(&attr.value).ok_or_else(|| {
			syn::Error::new_spanned(
				&binding_attr.value,
				"a bound input requires a static `type`",
			)
		})?,
		None => "text".to_owned(),
	};

	match input_type.to_ascii_lowercase().as_str() {
		"text" | "search" | "tel" | "url" | "email" | "password" | "color" | "date"
		| "datetime-local" | "month" | "week" | "time" => Ok((TypedControlBindingKind::Text, None)),
		"number" | "range" => Ok((TypedControlBindingKind::Number, None)),
		"checkbox" => Ok((TypedControlBindingKind::Checkbox, None)),
		"radio" => {
			let value = unique_untyped_attr(
				attrs,
				"value",
				binding_attr,
				"a bound radio input cannot specify duplicate `value` attributes",
			)?
			.ok_or_else(|| {
				syn::Error::new_spanned(
					&binding_attr.value,
					"a bound radio input requires a `value` attribute",
				)
			})?;
			Ok((TypedControlBindingKind::Radio, Some(value.value.clone())))
		}
		unsupported => Err(syn::Error::new_spanned(
			&binding_attr.value,
			format!("`bind:` does not support input type `{unsupported}`"),
		)),
	}
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

fn is_false_multiple_attr(attr: &PageAttr) -> bool {
	attr.html_name() == "multiple"
		&& matches!(
			&attr.value,
			Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Bool(value) if !value.value())
		)
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

fn static_string_value(expr: &Expr) -> Option<String> {
	let Expr::Lit(lit) = expr else {
		return None;
	};
	let syn::Lit::Str(value) = &lit.lit else {
		return None;
	};
	Some(value.value())
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
			if find_typed_attr(attrs, "checked")
				.is_some_and(|attr| !is_false_checked_attr(attr)) =>
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

fn is_false_checked_attr(attr: &TypedPageAttr) -> bool {
	attr.html_name() == "checked"
		&& matches!(&attr.value, AttrValue::BoolLit(value) if !value.value())
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

#[derive(Debug)]
struct TransformedPageAttrs {
	attrs: Vec<TypedPageAttr>,
	a11y_disabled: bool,
}

/// Transforms attributes from untyped to typed, with validation.
///
/// This function converts `Expr` attribute values into `AttrValue`,
/// enabling type-specific validation.
fn transform_attrs(
	attrs: &[PageAttr],
	element_tag: &str,
	allow_false_checked: bool,
) -> Result<TransformedPageAttrs> {
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
		if !(allow_false_checked
			&& attr.html_name() == "checked"
			&& matches!(&typed_value, AttrValue::BoolLit(value) if !value.value()))
		{
			validate_attr_type(&attr.name.to_string(), &typed_value, element_tag, attr.span)?;
		}

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
fn is_url_attribute(attr_name: &str, element_tag: &str, url_attrs: &[(&str, &str)]) -> bool {
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
				enum_spec.valid_values[0],
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
fn has_accessible_content(children: &[TypedPageNode]) -> bool {
	for child in children {
		match child {
			TypedPageNode::Text(text) => {
				if !text.content.trim().is_empty() {
					return true;
				}
			}
			TypedPageNode::Element(elem) => {
				if elem.tag == "img" && has_non_empty_or_dynamic_attr(&elem.attrs, "alt") {
					return true;
				}
				if has_accessible_content(&elem.children) {
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

/// Validates interactive element accessibility requirements.
///
/// Interactive elements must have either:
/// - Text content (direct or nested)
/// - An image child with non-empty alt text
/// - aria-label attribute
/// - aria-labelledby attribute
fn validate_interactive_accessibility(
	tag: &str,
	attrs: &[TypedPageAttr],
	children: &[TypedPageNode],
	labels: &A11yLabels,
	span: Span,
) -> Result<()> {
	if has_accessible_name_attr(attrs, labels) {
		return Ok(());
	}

	if !has_accessible_content(children) {
		return Err(syn::Error::new(
			span,
			format!(
				"Element <{}> requires an accessible name.\n\
				Either provide text content, an img child with non-empty alt text, or use 'aria_label'/'aria_labelledby'.\n\n\
				Examples:\n\
				  Correct:   {0} {{ \"Click me\" }}\n\
				  Correct:   {0} {{ aria_label: \"Close\" }}\n\
				  Correct:   {0} {{ span {{ \"Submit\" }} }}\n\
				  Incorrect: {0} {{ }}",
				tag
			),
		));
	}

	Ok(())
}

#[cfg(test)]
fn validate_button_accessibility(
	attrs: &[TypedPageAttr],
	children: &[TypedPageNode],
	span: Span,
) -> Result<()> {
	validate_interactive_accessibility("button", attrs, children, &A11yLabels::default(), span)
}

/// Validates accessibility requirements for a fully typed page tree.
///
/// This pass runs after the tree is typed so label associations can be
/// collected across sibling order within one macro invocation.
pub fn validate_page_accessibility(body: &TypedPageBody) -> Result<()> {
	validate_accessibility_nodes(&body.nodes, &A11yLabels::default(), false, false)
}

#[derive(Clone, Default)]
struct A11yLabels {
	static_for: HashSet<String>,
	has_dynamic_for: bool,
	static_labelledby: HashSet<String>,
}

fn collect_static_scope_labelledby_sources(nodes: &[TypedPageNode], labels: &mut A11yLabels) {
	for node in nodes {
		match node {
			TypedPageNode::Element(elem) => {
				collect_labelledby_source_from_element(elem, labels);
				if !element_is_statically_hidden(&elem.attrs) {
					collect_static_scope_labelledby_sources(&elem.children, labels);
				}
			}
			TypedPageNode::Component(_) => {}
			TypedPageNode::Watch(watch_node) => {
				collect_static_scope_labelledby_source_in_node(&watch_node.expr, labels);
			}
			TypedPageNode::If(_)
			| TypedPageNode::For(_)
			| TypedPageNode::Text(_)
			| TypedPageNode::Expression(_) => {}
		}
	}
}

fn collect_labelledby_source_from_element(elem: &TypedPageElement, labels: &mut A11yLabels) {
	if element_is_statically_hidden(&elem.attrs) {
		return;
	}

	if let Some(attr) = find_attr(&elem.attrs, "id")
		&& let Some(value) = attr.value.as_string()
		&& !value.trim().is_empty()
		&& element_has_accessible_content(elem)
	{
		labels.static_labelledby.insert(value);
	}
}

fn collect_static_scope_label_targets(nodes: &[TypedPageNode], labels: &mut A11yLabels) {
	for node in nodes {
		match node {
			TypedPageNode::Element(elem) => {
				collect_label_target_from_element(elem, labels);
				if !element_is_statically_hidden(&elem.attrs) {
					collect_static_scope_label_targets(&elem.children, labels);
				}
			}
			TypedPageNode::Component(_) => {}
			TypedPageNode::Watch(watch_node) => {
				collect_static_scope_label_target_in_node(&watch_node.expr, labels);
			}
			TypedPageNode::If(_)
			| TypedPageNode::For(_)
			| TypedPageNode::Text(_)
			| TypedPageNode::Expression(_) => {}
		}
	}
}

fn collect_label_target_from_element(elem: &TypedPageElement, labels: &mut A11yLabels) {
	if element_is_statically_hidden(&elem.attrs) {
		return;
	}

	if elem.tag == "label"
		&& label_has_accessible_content(elem, labels)
		&& let Some(attr) = find_attr(&elem.attrs, "for")
	{
		if let Some(value) = attr.value.as_string() {
			if !value.trim().is_empty() {
				labels.static_for.insert(value);
			}
		} else {
			labels.has_dynamic_for = true;
		}
	}
}

fn collect_static_scope_labelledby_source_in_node(node: &TypedPageNode, labels: &mut A11yLabels) {
	match node {
		TypedPageNode::Element(elem) => {
			collect_labelledby_source_from_element(elem, labels);
			if !element_is_statically_hidden(&elem.attrs) {
				collect_static_scope_labelledby_sources(&elem.children, labels);
			}
		}
		TypedPageNode::Component(_) => {}
		TypedPageNode::Watch(watch_node) => {
			collect_static_scope_labelledby_source_in_node(&watch_node.expr, labels);
		}
		TypedPageNode::If(_)
		| TypedPageNode::For(_)
		| TypedPageNode::Text(_)
		| TypedPageNode::Expression(_) => {}
	}
}

fn collect_static_scope_label_target_in_node(node: &TypedPageNode, labels: &mut A11yLabels) {
	match node {
		TypedPageNode::Element(elem) => {
			collect_label_target_from_element(elem, labels);
			if !element_is_statically_hidden(&elem.attrs) {
				collect_static_scope_label_targets(&elem.children, labels);
			}
		}
		TypedPageNode::Component(_) => {}
		TypedPageNode::Watch(watch_node) => {
			collect_static_scope_label_target_in_node(&watch_node.expr, labels);
		}
		TypedPageNode::If(_)
		| TypedPageNode::For(_)
		| TypedPageNode::Text(_)
		| TypedPageNode::Expression(_) => {}
	}
}

fn validate_accessibility_nodes(
	nodes: &[TypedPageNode],
	labels: &A11yLabels,
	inside_label: bool,
	hidden_ancestor: bool,
) -> Result<()> {
	let mut labels = labels.clone();
	if !hidden_ancestor {
		collect_static_scope_labelledby_sources(nodes, &mut labels);
		collect_static_scope_label_targets(nodes, &mut labels);
	}

	for node in nodes {
		validate_accessibility_node(node, &labels, inside_label, hidden_ancestor)?;
	}
	Ok(())
}

fn validate_accessibility_node(
	node: &TypedPageNode,
	labels: &A11yLabels,
	inside_label: bool,
	hidden_ancestor: bool,
) -> Result<()> {
	match node {
		TypedPageNode::Element(elem) => {
			validate_element_accessibility(elem, labels, inside_label, hidden_ancestor)
		}
		TypedPageNode::If(if_node) => {
			validate_accessibility_nodes(
				&if_node.then_branch,
				labels,
				inside_label,
				hidden_ancestor,
			)?;
			if let Some(else_branch) = &if_node.else_branch {
				validate_accessibility_else(else_branch, labels, inside_label, hidden_ancestor)?;
			}
			Ok(())
		}
		TypedPageNode::For(for_node) => {
			validate_accessibility_nodes(&for_node.body, labels, inside_label, hidden_ancestor)
		}
		TypedPageNode::Component(component) => {
			if let Some(children) = &component.children {
				validate_accessibility_nodes(children, labels, inside_label, hidden_ancestor)?;
			}
			for slot in &component.named_slots {
				validate_accessibility_nodes(
					&slot.children,
					labels,
					inside_label,
					hidden_ancestor,
				)?;
			}
			Ok(())
		}
		TypedPageNode::Watch(watch_node) => {
			validate_accessibility_node(&watch_node.expr, labels, inside_label, hidden_ancestor)
		}
		TypedPageNode::Text(_) | TypedPageNode::Expression(_) => Ok(()),
	}
}

fn validate_accessibility_else(
	else_branch: &TypedPageElse,
	labels: &A11yLabels,
	inside_label: bool,
	hidden_ancestor: bool,
) -> Result<()> {
	match else_branch {
		TypedPageElse::Block(nodes) => {
			validate_accessibility_nodes(nodes, labels, inside_label, hidden_ancestor)
		}
		TypedPageElse::If(if_node) => {
			validate_accessibility_nodes(
				&if_node.then_branch,
				labels,
				inside_label,
				hidden_ancestor,
			)?;
			if let Some(else_branch) = &if_node.else_branch {
				validate_accessibility_else(else_branch, labels, inside_label, hidden_ancestor)?;
			}
			Ok(())
		}
	}
}

fn validate_element_accessibility(
	elem: &TypedPageElement,
	labels: &A11yLabels,
	inside_label: bool,
	hidden_ancestor: bool,
) -> Result<()> {
	let tag = elem.tag.to_string();
	let statically_hidden = element_is_statically_hidden(&elem.attrs);
	if hidden_ancestor || statically_hidden {
		return Ok(());
	}

	let nested_inside_label =
		inside_label || (tag == "label" && label_has_accessible_content(elem, labels));

	if !elem.a11y_disabled {
		validate_role_value(&elem.attrs)?;
		validate_tabindex(&elem.attrs)?;
		validate_iframe_title(&tag, &elem.attrs, elem.span)?;

		match tag.as_str() {
			"button" => {
				validate_interactive_accessibility(
					&tag,
					&elem.attrs,
					&elem.children,
					labels,
					elem.span,
				)?;
			}
			"a" if anchor_is_interactive(elem) => {
				validate_interactive_accessibility(
					&tag,
					&elem.attrs,
					&elem.children,
					labels,
					elem.span,
				)?;
			}
			"input" if input_is_button_style(&elem.attrs) => {
				validate_input_button_accessibility(&elem.attrs, labels, elem.span)?;
			}
			"input" if !input_is_hidden_or_dynamic_type(&elem.attrs) => {
				validate_form_control_accessibility(
					&tag,
					&elem.attrs,
					labels,
					inside_label,
					elem.span,
				)?;
			}
			"select" | "textarea" => {
				validate_form_control_accessibility(
					&tag,
					&elem.attrs,
					labels,
					inside_label,
					elem.span,
				)?;
			}
			_ => {}
		}
	}

	validate_accessibility_nodes(&elem.children, labels, nested_inside_label, false)
}

fn find_attr<'a>(attrs: &'a [TypedPageAttr], html_name: &str) -> Option<&'a TypedPageAttr> {
	attrs.iter().find(|attr| attr.html_name() == html_name)
}

fn has_non_empty_or_dynamic_attr(attrs: &[TypedPageAttr], html_name: &str) -> bool {
	find_attr(attrs, html_name).is_some_and(|attr| match attr.value.as_string() {
		Some(value) => !value.trim().is_empty(),
		None => true,
	})
}

fn has_accessible_name_attr(attrs: &[TypedPageAttr], labels: &A11yLabels) -> bool {
	has_non_empty_or_dynamic_attr(attrs, "aria-label") || has_valid_aria_labelledby(attrs, labels)
}

fn has_valid_aria_labelledby(attrs: &[TypedPageAttr], labels: &A11yLabels) -> bool {
	let Some(attr) = find_attr(attrs, "aria-labelledby") else {
		return false;
	};

	let Some(value) = attr.value.as_string() else {
		return true;
	};

	let ids: Vec<&str> = value.split_whitespace().collect();
	!ids.is_empty() && ids.iter().all(|id| labels.static_labelledby.contains(*id))
}

fn element_has_accessible_content(elem: &TypedPageElement) -> bool {
	has_non_empty_or_dynamic_attr(&elem.attrs, "aria-label")
		|| has_accessible_content(&elem.children)
}

fn label_has_accessible_content(elem: &TypedPageElement, labels: &A11yLabels) -> bool {
	has_accessible_name_attr(&elem.attrs, labels) || has_accessible_content(&elem.children)
}

fn element_is_statically_hidden(attrs: &[TypedPageAttr]) -> bool {
	find_attr(attrs, "hidden").is_some_and(|attr| match &attr.value {
		AttrValue::BoolLit(value) => value.value(),
		_ => false,
	})
}

fn input_is_hidden_or_dynamic_type(attrs: &[TypedPageAttr]) -> bool {
	let Some(attr) = find_attr(attrs, "type") else {
		return false;
	};

	match attr.value.as_string() {
		Some(value) => value.eq_ignore_ascii_case("hidden"),
		None => true,
	}
}

fn input_static_type(attrs: &[TypedPageAttr]) -> Option<String> {
	find_attr(attrs, "type")
		.and_then(|attr| attr.value.as_string())
		.map(|value| value.to_ascii_lowercase())
}

fn input_is_button_style(attrs: &[TypedPageAttr]) -> bool {
	matches!(
		input_static_type(attrs).as_deref(),
		Some("submit" | "button" | "reset" | "image")
	)
}

fn validate_input_button_accessibility(
	attrs: &[TypedPageAttr],
	labels: &A11yLabels,
	span: Span,
) -> Result<()> {
	if has_accessible_name_attr(attrs, labels) {
		return Ok(());
	}

	let input_type = input_static_type(attrs);
	if matches!(input_type.as_deref(), Some("submit" | "reset")) {
		return Ok(());
	}

	let names_from_alt = matches!(input_type.as_deref(), Some("image"));
	let label_attr = if names_from_alt { "alt" } else { "value" };
	if has_non_empty_or_dynamic_attr(attrs, label_attr) {
		return Ok(());
	}

	Err(syn::Error::new(
		span,
		format!(
			"Element <input type=\"{}\"> requires an accessible name. Provide a non-empty `{}` or use `aria_label`/`aria_labelledby`.",
			input_type.as_deref().unwrap_or("button"),
			label_attr
		),
	))
}

fn anchor_is_interactive(elem: &TypedPageElement) -> bool {
	find_attr(&elem.attrs, "href").is_some() || !elem.events.is_empty()
}

fn validate_form_control_accessibility(
	tag: &str,
	attrs: &[TypedPageAttr],
	labels: &A11yLabels,
	inside_label: bool,
	span: Span,
) -> Result<()> {
	if has_accessible_name_attr(attrs, labels) || inside_label {
		return Ok(());
	}

	if let Some(id_attr) = find_attr(attrs, "id") {
		if let Some(id) = id_attr.value.as_string() {
			if labels.static_for.contains(&id) {
				return Ok(());
			}
		} else if labels.has_dynamic_for {
			return Ok(());
		}
	}

	Err(syn::Error::new(
		span,
		format!(
			"Element <{}> requires an accessible label.\n\
			Provide a wrapping label, a label with matching `for`/`id`, or use `aria_label`/`aria_labelledby`.",
			tag
		),
	))
}

fn validate_iframe_title(tag: &str, attrs: &[TypedPageAttr], span: Span) -> Result<()> {
	if tag == "iframe" && !has_non_empty_or_dynamic_attr(attrs, "title") {
		return Err(syn::Error::new(
			span,
			"Element <iframe> requires a non-empty `title` attribute for accessibility",
		));
	}

	Ok(())
}

fn validate_tabindex(attrs: &[TypedPageAttr]) -> Result<()> {
	let Some(attr) = find_attr(attrs, "tabindex") else {
		return Ok(());
	};

	if let Some(value) = static_i64_value(&attr.value)?
		&& value != 0
		&& value != -1
	{
		return Err(syn::Error::new(
			attr.span,
			"Attribute `tabindex` only allows static values `0` and `-1`; positive values hijack keyboard tab order",
		));
	}

	Ok(())
}

fn validate_role_value(attrs: &[TypedPageAttr]) -> Result<()> {
	let Some(attr) = find_attr(attrs, "role") else {
		return Ok(());
	};

	let Some(value) = attr.value.as_string() else {
		return Ok(());
	};

	let tokens: Vec<&str> = value.split_ascii_whitespace().collect();
	if tokens.is_empty() || !tokens.iter().any(|role| is_valid_aria_role(role)) {
		return Err(syn::Error::new(
			attr.span,
			format!("Invalid ARIA role value `{}`", value),
		));
	}

	Ok(())
}

fn static_i64_value(value: &AttrValue) -> Result<Option<i64>> {
	match value {
		AttrValue::IntLit(lit) => lit.base10_parse::<i64>().map(Some),
		AttrValue::Dynamic(expr) => match expr.as_ref() {
			Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => {
				if let Expr::Lit(lit) = unary.expr.as_ref()
					&& let syn::Lit::Int(int) = &lit.lit
				{
					return int.base10_parse::<i64>().map(|value| Some(-value));
				}
				Ok(None)
			}
			_ => Ok(None),
		},
		_ => Ok(None),
	}
}

fn is_valid_aria_role(role: &str) -> bool {
	VALID_ARIA_ROLES.contains(&role)
}

// Concrete author-usable roles from WAI-ARIA 1.3. Abstract roles are excluded.
const VALID_ARIA_ROLES: &[&str] = &[
	"alert",
	"alertdialog",
	"application",
	"article",
	"banner",
	"blockquote",
	"button",
	"caption",
	"cell",
	"checkbox",
	"code",
	"columnheader",
	"combobox",
	"comment",
	"complementary",
	"contentinfo",
	"definition",
	"deletion",
	"dialog",
	"directory",
	"document",
	"emphasis",
	"feed",
	"figure",
	"form",
	"generic",
	"grid",
	"gridcell",
	"group",
	"heading",
	"image",
	"img",
	"insertion",
	"link",
	"list",
	"listbox",
	"listitem",
	"log",
	"main",
	"mark",
	"marquee",
	"math",
	"menu",
	"menubar",
	"menuitem",
	"menuitemcheckbox",
	"menuitemradio",
	"meter",
	"navigation",
	"none",
	"note",
	"option",
	"paragraph",
	"presentation",
	"progressbar",
	"radio",
	"radiogroup",
	"region",
	"row",
	"rowgroup",
	"rowheader",
	"scrollbar",
	"search",
	"searchbox",
	"sectionfooter",
	"sectionheader",
	"separator",
	"slider",
	"spinbutton",
	"status",
	"strong",
	"subscript",
	"suggestion",
	"superscript",
	"switch",
	"tab",
	"table",
	"tablist",
	"tabpanel",
	"term",
	"textbox",
	"time",
	"timer",
	"toolbar",
	"tooltip",
	"tree",
	"treegrid",
	"treeitem",
];

/// Validates attribute type for specific elements and attributes.
///
/// # Validation Rules
///
/// - Boolean attributes must have dynamic expressions only (no literals)
/// - Numeric attributes must have integer literals or dynamic expressions (no strings/floats/booleans)
/// - URL attributes are checked for dangerous schemes (javascript:, data:, vbscript:) for XSS prevention
/// - Enumerated attributes are validated against allowed values (`input[type]`, `button[type]`, etc.)
/// - `img` element `src` attribute: when given as a string literal it must be non-empty;
///   dynamic expressions (e.g. `resolve_static(...)`) are allowed and deferred to runtime
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
		("src", "iframe, video, audio, source, script, embed, img"),
	];

	// Dangerous URL schemes that should be blocked for security (XSS prevention)
	const DANGEROUS_URL_SCHEMES: &[&str] = &["javascript:", "data:", "vbscript:"];

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

		// Check for dangerous schemes (case-insensitive)
		let url_lower = url_str.to_lowercase();
		for scheme in DANGEROUS_URL_SCHEMES {
			if url_lower.starts_with(scheme) {
				return Err(syn::Error::new(
					span,
					format!(
						"Dangerous URL scheme detected in attribute '{}'.\n\
							The '{}' scheme can be used for XSS (Cross-Site Scripting) attacks.\n\n\
							Security risk: This URL could execute arbitrary JavaScript code.\n\n\
							Use safe URL schemes instead:\n\
							  - https://example.com\n\
							  - /path/to/resource\n\
							  - #anchor\n\
							  - mailto:user@example.com",
						attr_name,
						scheme.trim_end_matches(':')
					),
				));
			}
		}
	}
	// Dynamic expressions are OK (runtime validation recommended)

	// Enumerated attributes validation - check if value is in allowed list
	validate_enum_attr(attr_name, value, element_tag, span)?;

	// Note: emptiness of a string-literal `<img src="">` is rejected by the
	// generic URL-attribute check above (URL_ATTRS includes ("src", img)).
	// Dynamic expressions (e.g. `resolve_static(...)`) are deferred to runtime.

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

/// Validates that a `data-*` attribute suffix is non-empty, starts with a
/// lowercase letter, and contains only lowercase letters, digits, or hyphens.
///
/// Returns an error referencing `attr_name_token` with `html_name` in the message.
fn validate_data_attr_suffix(
	attr_name_token: &syn::Ident,
	html_name: &str,
	suffix: &str,
) -> Result<()> {
	if suffix.is_empty()
		|| !suffix
			.chars()
			.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
	{
		return Err(syn::Error::new_spanned(
			attr_name_token,
			format!(
				"Invalid data attribute name '{}'. Must match pattern: data-[a-z][a-z0-9-]*",
				html_name
			),
		));
	}
	// First character must be a lowercase letter
	if !suffix.chars().next().unwrap().is_ascii_lowercase() {
		return Err(syn::Error::new_spanned(
			attr_name_token,
			format!(
				"Invalid data attribute name '{}'. Must start with a lowercase letter after 'data-'",
				html_name
			),
		));
	}
	Ok(())
}

/// Validates that an `aria-*` attribute suffix is non-empty and contains only
/// lowercase letters or hyphens.
///
/// Returns an error referencing `attr_name_token` with `html_name` in the message.
fn validate_aria_attr_suffix(
	attr_name_token: &syn::Ident,
	html_name: &str,
	suffix: &str,
) -> Result<()> {
	if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
		return Err(syn::Error::new_spanned(
			attr_name_token,
			format!(
				"Invalid aria attribute name '{}'. Must match pattern: aria-[a-z-]+",
				html_name
			),
		));
	}
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
		let suffix = &html_name[5..]; // Skip "data-"
		validate_data_attr_suffix(&attr.name, &html_name, suffix)?;
	}

	// Validate aria-* attributes
	if attr_name.starts_with("aria_") {
		let html_name = attr.html_name();
		let suffix = &html_name[5..]; // Skip "aria-"
		validate_aria_attr_suffix(&attr.name, &html_name, suffix)?;
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

/// Validates required attributes for certain elements.
///
/// # Rules
///
/// - img elements must have an alt attribute (accessibility requirement)
/// - img elements must have a src attribute with a non-empty string literal
///
/// # Errors
///
/// Returns a compilation error if required attributes are missing or invalid.
fn validate_required_attributes(
	tag: &str,
	attrs: &[TypedPageAttr],
	span: Span,
	a11y_disabled: bool,
) -> Result<()> {
	// img requires alt and src attributes
	if tag == "img" {
		// Check alt attribute (accessibility)
		let has_alt = attrs.iter().any(|attr| attr.name == "alt");
		if !has_alt && !a11y_disabled {
			return Err(syn::Error::new(
				span,
				"Element <img> requires 'alt' attribute for accessibility",
			));
		}

		// Check src attribute (required for img)
		let has_src = attrs.iter().any(|attr| attr.name == "src");
		if !has_src {
			return Err(syn::Error::new(
				span,
				"Element <img> requires 'src' attribute",
			));
		}

		// Note: src type validation (must be string literal, non-empty) is done in validate_attr_type()
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::core::{PageExpression, PageText, TypedPageElement};
	use quote::quote;
	use rstest::rstest;
	use syn::parse_quote;

	#[rstest]
	#[case(
		quote!({ div { bind: value } }),
		"`bind:` is only valid on `input`, `textarea`, and `select`"
	)]
	#[case(
		quote!({ input { type: dynamic_type, bind: value } }),
		"a bound input requires a static `type`"
	)]
	#[case(
		quote!({ input { type: "radio", bind: value } }),
		"a bound radio input requires a `value` attribute"
	)]
	#[case(
		quote!({ input { a11y: off, type: "radio", value: "one", value: "two", bind: value } }),
		"a bound radio input cannot specify duplicate `value` attributes"
	)]
	#[case(
		quote!({
			select {
				a11y: off,
				bind: value,
				if condition {
					optgroup { option { value: "one", value: "two", "Choice" } }
				}
			}
		}),
		"a bound select cannot contain an option with duplicate `value` attributes"
	)]
	#[case(
		quote!({ input { bind: first, bind: second } }),
		"`bind:` may only be specified once per control"
	)]
	#[case(
		quote!({ select { multiple: dynamic_multiple, bind: value } }),
		"a bound select requires a static `multiple`"
	)]
	#[case(
		quote!({ input { a11y: off, type: "text", type: dynamic_type, bind: value } }),
		"a bound input requires a static `type`"
	)]
	#[case(
		quote!({ select { a11y: off, multiple: false, multiple: dynamic_multiple, bind: value } }),
		"a bound select requires a static `multiple`"
	)]
	#[case(
		quote!({ input { type: "file", bind: value } }),
		"`bind:` does not support input type `file`"
	)]
	#[case(
		quote!({ textarea { bind: number(value, error) } }),
		"`number(value, error)` is only valid on a numeric input"
	)]
	#[case(
		quote!({ input { type: "number", bind: number(value) } }),
		"`number(value, error)` requires exactly two arguments"
	)]
	#[case(
		quote!({ input { value: "initial", bind: value } }),
		"a bound text or number input cannot specify a `value` attribute"
	)]
	#[case(
		quote!({ input { type: "checkbox", checked: true, bind: value } }),
		"a bound checkbox or radio input cannot specify a `checked` attribute"
	)]
	#[case(
		quote!({ textarea { bind: value, "initial" } }),
		"a bound textarea cannot contain initial child content"
	)]
	#[case(
		quote!({
			select {
				bind: value,
				optgroup { option { value: "one", selected: true, "One" } }
			}
		}),
		"a bound select cannot contain an option with a `selected` attribute"
	)]
	#[case(
		quote!({
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
		let error = validate_page(&ast).unwrap_err();

		// Assert
		assert_eq!(error.to_string(), expected);
	}

	#[test]
	fn controlled_binding_accepts_false_selected_option() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote!({
			select { a11y: off, bind: value, option { selected: false, value: "one", "One" } }
		}))
		.unwrap();

		// Act
		let result = validate_page(&ast);

		// Assert
		assert!(result.is_ok());
	}

	#[test]
	fn controlled_binding_rejects_textarea_value_attribute() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote!({
			textarea {
				bind: value,
				value: "stale",
			}
		}))
		.unwrap();

		// Act
		let error = validate_page(&ast).unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"a bound textarea cannot specify a `value` attribute"
		);
	}

	#[test]
	fn literal_bound_select_marker_tag_does_not_enable_option_phrasing() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote!({
			__reinhardt_bound_select { option { span { "Label" } } }
		}))
		.unwrap();

		// Act
		let error = validate_page(&ast).unwrap_err();

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
			quote!({ select { a11y: off, bind: nested, option { span { "Nested" } } } })
		} else {
			quote!({ select { a11y: off, option { span { "Nested" } } } })
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
		let ast: PageMacro = syn::parse2(quote!({
			div {
				select { a11y: off, bind: outer, option { value: "outer", "Outer" } }
				select { a11y: off, option { span { "Sibling" } } }
			}
		}))
		.unwrap();

		// Act
		let error = validate_page(&ast).unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Element <option> can only contain text, not child elements"
		);
	}

	#[rstest]
	#[case(quote!({
		select {
			a11y: off,
			bind: selected,
			if show { option { span { "Flow" } } }
		}
	}))]
	#[case(quote!({
		select {
			a11y: off,
			bind: selected,
			ChoiceList() { option { span { "Component" } } }
		}
	}))]
	#[case(quote!({
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
		let result = validate_page(&ast);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[case(quote!({ select { a11y: off, bind: selected, if show { option { div { "Flow" } } } } }))]
	#[case(quote!({ select { a11y: off, bind: selected, for item in items { option { div { "For" } } } } }))]
	#[case(quote!({ select { a11y: off, bind: selected, watch { option { div { "Watch" } } } } }))]
	fn bound_option_rejects_disallowed_content_inside_control_flow(
		#[case] input: proc_macro2::TokenStream,
	) {
		// Arrange
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Act
		let error = validate_page(&ast).unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Element <option> in a bound select only supports non-interactive phrasing content"
		);
	}

	#[rstest]
	#[case(quote!({ select { a11y: off, bind: selected, option { span { tabindex: 0, "Zero" } } } }))]
	#[case(quote!({ select { a11y: off, bind: selected, option { span { tabindex: -1, "Negative" } } } }))]
	#[case(quote!({ select { a11y: off, bind: selected, option { span { tabindex: dynamic_tabindex, "Dynamic" } } } }))]
	#[case(quote!({ select { a11y: off, bind: selected, option { value: "explicit", span { strong { tabindex: 0, "Nested" } } } } }))]
	fn bound_option_rejects_descendant_tabindex(#[case] input: proc_macro2::TokenStream) {
		// Arrange
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Act
		let error = validate_page(&ast).unwrap_err();

		// Assert
		assert_eq!(
			error.to_string(),
			"Element <option> in a bound select cannot contain a descendant with a `tabindex` attribute"
		);
	}

	#[rstest]
	#[case(quote!({ input { a11y: off, bind: value } }), TypedControlBindingKind::Text, false)]
	#[case(
		quote!({ input { a11y: off, type: "checkbox", bind: value } }),
		TypedControlBindingKind::Checkbox,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "checkbox", checked: false, bind: value } }),
		TypedControlBindingKind::Checkbox,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "radio", value: choice, bind: value } }),
		TypedControlBindingKind::Radio,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "radio", value: "one", checked: false, bind: value } }),
		TypedControlBindingKind::Radio,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "number", bind: value } }),
		TypedControlBindingKind::Number,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "search", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "tel", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "url", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "email", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "password", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "color", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "date", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "datetime-local", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "month", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "week", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "time", bind: value } }),
		TypedControlBindingKind::Text,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "range", bind: value } }),
		TypedControlBindingKind::Number,
		false
	)]
	#[case(
		quote!({ input { a11y: off, type: "number", bind: number(value, error) } }),
		TypedControlBindingKind::Number,
		true
	)]
	#[case(quote!({ textarea { a11y: off, bind: value } }), TypedControlBindingKind::Text, false)]
	#[case(quote!({ select { a11y: off, bind: value } }), TypedControlBindingKind::SelectOne, false)]
	#[case(quote!({ select { a11y: off, multiple: false, bind: value } }), TypedControlBindingKind::SelectOne, false)]
	#[case(
		quote!({ select { a11y: off, multiple: true, bind: value } }),
		TypedControlBindingKind::SelectMany,
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
		let typed = validate_page(&ast).unwrap();
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
		assert_eq!(
			binding.radio_value.is_some(),
			expected_kind == TypedControlBindingKind::Radio
		);
	}

	fn parse(input: proc_macro2::TokenStream) -> PageMacro {
		syn::parse2(input).expect("input must be a valid page! macro")
	}

	fn implicit_capture_names(ast: &PageMacro) -> Vec<String> {
		validate_page(ast)
			.unwrap()
			.implicit_captures()
			.iter()
			.map(|capture| capture.ident.to_string())
			.collect()
	}

	#[rstest]
	fn test_implicit_body_records_captures() {
		// Arrange
		let ast = parse(quote! {
			{ div { {outer_count.get()} } }
		});

		// Act
		let captures = implicit_capture_names(&ast);

		// Assert
		assert_eq!(captures, vec!["outer_count"]);
	}

	#[rstest]
	fn test_implicit_body_records_head_capture() {
		// Arrange
		use proc_macro2::{Punct, Spacing};
		let pound = Punct::new('#', Spacing::Alone);
		let ast = parse(quote! {
			#pound head: page_head.clone(),
			{ div { "content" } }
		});

		// Act
		let captures = implicit_capture_names(&ast);

		// Assert
		assert_eq!(captures, vec!["page_head"]);
	}

	#[rstest]
	fn test_implicit_body_records_macro_argument_captures() {
		// Arrange
		let ast = parse(quote! {
			{ p { {format!("value={}", outer_value)} } }
		});

		// Act
		let captures = implicit_capture_names(&ast);

		// Assert
		assert_eq!(captures, vec!["outer_value"]);
	}

	#[rstest]
	fn test_implicit_body_excludes_number_binding_helper_from_captures() {
		// Arrange
		let ast = parse(quote! {
			{ input { a11y: off, type: "number", bind: number(value, error) } }
		});

		// Act
		let captures = implicit_capture_names(&ast);

		// Assert
		assert_eq!(captures, vec!["value", "error"]);
	}

	#[rstest]
	fn test_implicit_body_keeps_page_if_let_binding_local() {
		// Arrange
		let ast = parse(quote! {
			{
				if let Some(item) = maybe {
					div { {item.clone()} }
				}
			}
		});

		// Act
		let captures = implicit_capture_names(&ast);

		// Assert
		assert_eq!(captures, vec!["maybe"]);
	}

	#[rstest]
	fn test_implicit_body_records_page_for_iter_and_key_captures() {
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
		let captures = implicit_capture_names(&ast);

		// Assert
		assert_eq!(captures, vec!["items", "route_id"]);
		assert!(!captures.iter().any(|capture| capture == "item"));
	}

	#[rstest]
	fn test_implicit_body_treats_page_for_key_pattern_as_local() {
		// Arrange
		let ast = parse(quote! {
			{
				ul {
					for item in items @key(item.clone()) {
						li { {item.clone()} }
					}
				}
			}
		});

		// Act
		let captures = implicit_capture_names(&ast);

		// Assert
		assert_eq!(captures, vec!["items"]);
	}

	#[rstest]
	fn test_implicit_body_treats_at_subpattern_bindings_as_local() {
		// Arrange
		let ast = parse(quote! {
			{
				ul {
					for item @ Some(alias) in items {
						li { {item.clone()} {alias.clone()} }
					}
				}
			}
		});

		// Act
		let captures = implicit_capture_names(&ast);

		// Assert
		assert_eq!(captures, vec!["items"]);
	}

	#[rstest]
	fn test_strict_closure_rejects_implicit_capture() {
		// Arrange
		let ast = parse(quote! {
			|| { div { {outer_count.get()} } }
		});

		// Act
		let result = validate_page(&ast);

		// Assert
		let err = result.expect_err("strict closure must reject implicit capture");
		assert_eq!(
			err.to_string(),
			"identifier `outer_count` used inside `page!` is not declared as a parameter"
		);
	}

	#[rstest]
	fn test_strict_closure_rejects_head_capture() {
		// Arrange
		use proc_macro2::{Punct, Spacing};
		let pound = Punct::new('#', Spacing::Alone);
		let ast = parse(quote! {
			#pound head: page_head.clone(),
			|| { div { "content" } }
		});

		// Act
		let result = validate_page(&ast);

		// Assert
		let err = result.expect_err("strict closure must reject head capture");
		assert_eq!(
			err.to_string(),
			"identifier `page_head` used inside `page!` is not declared as a parameter"
		);
	}

	#[rstest]
	fn test_validate_valid_closure() {
		// Arrange
		let event = IntrinsicEvent::Standard {
			event: reinhardt_event_catalog::KnownEvent::Click,
			handler: parse_quote!(|_| {}),
		};

		// Act
		let result = validate_intrinsic_event_handler(&event);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_closure_with_one_arg() {
		// Arrange
		let event = IntrinsicEvent::Standard {
			event: reinhardt_event_catalog::KnownEvent::Click,
			handler: parse_quote!(|e| {
				handle_click(e);
			}),
		};

		// Act
		let result = validate_intrinsic_event_handler(&event);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_closure_too_many_args() {
		// Arrange
		let event = IntrinsicEvent::Standard {
			event: reinhardt_event_catalog::KnownEvent::Click,
			handler: parse_quote!(|a, b, c| {}),
		};

		// Act
		let result = validate_intrinsic_event_handler(&event);

		// Assert
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("0 or 1 arguments"));
	}

	#[rstest]
	fn test_validate_typed_custom_event_handler() {
		// Arrange
		let event = IntrinsicEvent::TypedCustom {
			name: parse_quote!("item-selected"),
			payload_type: parse_quote!(crate::events::Envelope<Vec<u64>>),
			handler: parse_quote!(|first, second| {}),
		};

		// Act
		let result = validate_intrinsic_event_handler(&event);

		// Assert
		let error = result.expect_err("typed custom event must validate handler arity");
		assert_eq!(
			error.to_string(),
			"Event handler closure must have 0 or 1 arguments, but this closure has 2 arguments"
		);
	}

	#[rstest]
	fn test_validate_valid_data_attribute() {
		// Arrange
		let attr = PageAttr {
			name: syn::Ident::new("data_testid", proc_macro2::Span::call_site()),
			value: parse_quote!("test"),
			span: proc_macro2::Span::call_site(),
		};

		// Act
		let result = validate_attribute(&attr, "div");

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_invalid_data_attribute() {
		// Arrange
		let attr = PageAttr {
			name: syn::Ident::new("data_", proc_macro2::Span::call_site()),
			value: parse_quote!("test"),
			span: proc_macro2::Span::call_site(),
		};

		// Act
		let result = validate_attribute(&attr, "div");

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_valid_aria_attribute() {
		// Arrange
		let attr = PageAttr {
			name: syn::Ident::new("aria_label", proc_macro2::Span::call_site()),
			value: parse_quote!("Navigation"),
			span: proc_macro2::Span::call_site(),
		};

		// Act
		let result = validate_attribute(&attr, "div");

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_void_element_with_children() {
		// Arrange
		let mut elem = PageElement::new(
			syn::Ident::new("input", proc_macro2::Span::call_site()),
			proc_macro2::Span::call_site(),
		);
		elem.children.push(PageNode::Text(PageText {
			content: "text".to_string(),
			span: proc_macro2::Span::call_site(),
		}));

		// Act
		let result = validate_element_nesting(&elem, &[]);

		// Assert
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("cannot have children")
		);
	}

	#[rstest]
	fn test_nested_interactive_elements() {
		// Arrange
		let elem = PageElement::new(
			syn::Ident::new("button", proc_macro2::Span::call_site()),
			proc_macro2::Span::call_site(),
		);

		// Act
		let result = validate_element_nesting(&elem, &["a".to_string()]);

		// Assert
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("cannot be nested inside another interactive element")
		);
	}

	#[rstest]
	fn test_img_missing_alt() {
		// Arrange
		let elem = PageElement::new(
			syn::Ident::new("img", proc_macro2::Span::call_site()),
			proc_macro2::Span::call_site(),
		);

		// Act
		let result = validate_required_attributes("img", &[], elem.span, false);

		// Assert
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("requires 'alt' attribute")
		);
	}

	#[rstest]
	fn test_transform_attrs_with_string_lit() {
		// Arrange
		let attrs = vec![PageAttr {
			name: syn::Ident::new("src", proc_macro2::Span::call_site()),
			value: parse_quote!("/image.png"),
			span: proc_macro2::Span::call_site(),
		}];

		// Act
		let result = transform_attrs(&attrs, "img", false);

		// Assert
		assert!(result.is_ok());
		let typed_attrs = result.unwrap().attrs;
		assert_eq!(typed_attrs.len(), 1);
		assert!(typed_attrs[0].value.is_string_literal());
	}

	#[rstest]
	fn test_transform_attrs_with_dynamic() {
		// Arrange
		let attrs = vec![PageAttr {
			name: syn::Ident::new("src", proc_macro2::Span::call_site()),
			value: parse_quote!(image_url),
			span: proc_macro2::Span::call_site(),
		}];

		// Act
		let result = transform_attrs(&attrs, "div", false);

		// Assert
		assert!(result.is_ok());
		let typed_attrs = result.unwrap().attrs;
		assert_eq!(typed_attrs.len(), 1);
		assert!(typed_attrs[0].value.is_dynamic());
	}

	#[rstest]
	fn test_validate_attr_type_img_src_literal() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("/image.png"));

		// Act
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_attr_type_img_src_dynamic() {
		// Arrange: dynamic expressions (function calls, identifiers) are accepted
		// because their value can only be validated at runtime.
		let value = AttrValue::from_expr(parse_quote!(resolve_static("images/poll.svg")));

		// Act
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_attr_type_img_src_empty() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(""));

		// Act
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("cannot be empty"));
	}

	// Boolean attribute tests - string literals are prohibited
	#[rstest]
	fn test_validate_boolean_attr_string_literal() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("disabled"));

		// Act
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a string literal value"));
	}

	#[rstest]
	fn test_validate_boolean_attr_string_empty() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(""));

		// Act
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a string literal value"));
	}

	#[rstest]
	fn test_validate_boolean_attr_bool_literal_true() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(true));

		// Act
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_boolean_attr_bool_literal_false() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(false));

		// Act
		let result = validate_attr_type("checked", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("cannot be set to `false`"));
	}

	#[rstest]
	fn test_validate_boolean_attr_int_literal() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(1));

		// Act
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a numeric literal value"));
	}

	#[rstest]
	fn test_validate_boolean_attr_float_literal() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(1.0));

		// Act
		let result =
			validate_attr_type("required", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a numeric literal value"));
	}

	#[rstest]
	fn test_validate_boolean_attr_dynamic_variable() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(is_disabled));

		// Act
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_boolean_attr_dynamic_function() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(is_disabled()));

		// Act
		let result = validate_attr_type("checked", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_boolean_attr_dynamic_conditional() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(if condition { true } else { false }));

		// Act
		let result =
			validate_attr_type("readonly", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	// Numeric attribute tests - string and float literals are prohibited
	#[rstest]
	fn test_validate_numeric_attr_string_literal() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("100"));

		// Act
		let result =
			validate_attr_type("maxlength", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Attribute"));
		assert!(err_msg.contains("must be an integer literal or dynamic expression"));
	}

	#[rstest]
	fn test_validate_numeric_attr_float_literal() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(100.0));

		// Act
		let result = validate_attr_type("rows", &value, "textarea", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Attribute"));
		assert!(err_msg.contains("must be an integer, not a floating-point number"));
	}

	#[rstest]
	fn test_validate_numeric_attr_bool_literal() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(true));

		// Act
		let result = validate_attr_type("min", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Attribute"));
		assert!(err_msg.contains("must be an integer, not a boolean"));
	}

	#[rstest]
	fn test_validate_numeric_attr_int_literal() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(100));

		// Act
		let result =
			validate_attr_type("maxlength", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_numeric_attr_dynamic_variable() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(max_len));

		// Act
		let result =
			validate_attr_type("maxlength", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_numeric_attr_dynamic_function() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(get_max_len()));

		// Act
		let result = validate_attr_type("cols", &value, "textarea", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	// URL attribute tests - dangerous schemes and empty strings are prohibited
	#[rstest]
	fn test_validate_url_attr_javascript_scheme() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("javascript:alert('xss')"));

		// Act
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
		assert!(err_msg.contains("javascript"));
		assert!(err_msg.contains("XSS"));
	}

	#[rstest]
	fn test_validate_url_attr_data_scheme() {
		// Arrange
		let value =
			AttrValue::from_expr(parse_quote!("data:text/html,<script>alert('xss')</script>"));

		// Act
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
		assert!(err_msg.contains("data"));
	}

	#[rstest]
	fn test_validate_url_attr_vbscript_scheme() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("vbscript:msgbox('xss')"));

		// Act
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
		assert!(err_msg.contains("vbscript"));
	}

	#[rstest]
	fn test_validate_url_attr_case_insensitive() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("JavaScript:alert(1)"));

		// Act
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
		assert!(err_msg.contains("javascript"));
	}

	#[rstest]
	fn test_validate_url_attr_empty_string() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(""));

		// Act
		let result = validate_attr_type("action", &value, "form", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("cannot be empty"));
	}

	#[rstest]
	fn test_validate_url_attr_whitespace_string() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("   "));

		// Act
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("cannot be empty"));
	}

	#[rstest]
	fn test_validate_url_attr_https_scheme() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("https://example.com"));

		// Act
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_url_attr_relative_path() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("/path/to/page"));

		// Act
		let result = validate_attr_type("action", &value, "form", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_url_attr_anchor() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("#section"));

		// Act
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_url_attr_dynamic_variable() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(url_var));

		// Act
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_url_attr_img_src_javascript_blocked() {
		// Arrange
		// img src should block dangerous URL schemes like javascript:
		let value = AttrValue::from_expr(parse_quote!("javascript:alert(1)"));

		// Act
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(
			err_msg.contains("Dangerous URL scheme"),
			"Expected dangerous URL scheme error, got: {}",
			err_msg
		);
	}

	#[rstest]
	fn test_validate_url_attr_img_src_data_scheme_blocked() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("data:text/html,<script>alert(1)</script>"));

		// Act
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(
			err_msg.contains("Dangerous URL scheme"),
			"Expected dangerous URL scheme error, got: {}",
			err_msg
		);
	}

	#[rstest]
	fn test_validate_url_attr_img_src_vbscript_blocked() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("vbscript:MsgBox('XSS')"));

		// Act
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(
			err_msg.contains("Dangerous URL scheme"),
			"Expected dangerous URL scheme error, got: {}",
			err_msg
		);
	}

	#[rstest]
	fn test_validate_url_attr_img_src_safe_url_passes() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("https://example.com/image.png"));

		// Act
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_url_attr_img_src_relative_path_passes() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("/images/photo.jpg"));

		// Act
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	// Enumerated attribute tests - invalid values are prohibited
	#[rstest]
	fn test_validate_enum_attr_input_type_invalid() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("invalid"));

		// Act
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
		assert!(err_msg.contains("invalid"));
		assert!(err_msg.contains("input"));
	}

	#[rstest]
	fn test_validate_enum_attr_input_type_text() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("text"));

		// Act
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_enum_attr_input_type_email() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("email"));

		// Act
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_enum_attr_button_type_invalid() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("invalid"));

		// Act
		let result = validate_attr_type("type", &value, "button", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
		assert!(err_msg.contains("button"));
	}

	#[rstest]
	fn test_validate_enum_attr_button_type_submit() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("submit"));

		// Act
		let result = validate_attr_type("type", &value, "button", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_enum_attr_form_method_invalid() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("put"));

		// Act
		let result = validate_attr_type("method", &value, "form", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
		assert!(err_msg.contains("put"));
	}

	#[rstest]
	fn test_validate_enum_attr_form_method_post() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("post"));

		// Act
		let result = validate_attr_type("method", &value, "form", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_enum_attr_form_enctype_invalid() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("invalid"));

		// Act
		let result = validate_attr_type("enctype", &value, "form", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
	}

	#[rstest]
	fn test_validate_enum_attr_form_enctype_multipart() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("multipart/form-data"));

		// Act
		let result = validate_attr_type("enctype", &value, "form", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_enum_attr_script_type_module() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!("module"));

		// Act
		let result = validate_attr_type("type", &value, "script", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_enum_attr_dynamic_variable() {
		// Arrange
		let value = AttrValue::from_expr(parse_quote!(input_type));

		// Act
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	// Accessibility tests - button elements must have text or aria-label
	#[rstest]
	fn test_validate_button_accessibility_empty() {
		// Arrange
		let attrs = vec![];
		let children = vec![];

		// Act
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("requires an accessible name"));
		assert!(err_msg.contains("aria_label"));
	}

	#[rstest]
	fn test_validate_button_accessibility_whitespace_only() {
		// Arrange
		let attrs = vec![];
		let children = vec![TypedPageNode::Text(PageText {
			content: "   ".to_string(),
			span: proc_macro2::Span::call_site(),
		})];

		// Act
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_button_accessibility_with_text() {
		// Arrange
		let attrs = vec![];
		let children = vec![TypedPageNode::Text(PageText {
			content: "Click me".to_string(),
			span: proc_macro2::Span::call_site(),
		})];

		// Act
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_button_accessibility_with_nested_text() {
		// Arrange
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

		// Act
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_button_accessibility_with_aria_label() {
		// Arrange
		let attrs = vec![TypedPageAttr {
			name: syn::Ident::new("aria_label", proc_macro2::Span::call_site()),
			value: AttrValue::from_expr(parse_quote!("Close")),
			span: proc_macro2::Span::call_site(),
		}];
		let children = vec![];

		// Act
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_button_accessibility_with_expression() {
		// Arrange
		let attrs = vec![];
		let children = vec![TypedPageNode::Expression(PageExpression {
			expr: parse_quote!(button_text),
			braced: true,
			span: proc_macro2::Span::call_site(),
		})];

		// Act
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_transform_attrs_consumes_a11y_opt_out_marker() {
		// Arrange
		let attrs = vec![PageAttr {
			name: syn::Ident::new("a11y", proc_macro2::Span::call_site()),
			value: parse_quote!(off),
			span: proc_macro2::Span::call_site(),
		}];

		// Act
		let result = transform_attrs(&attrs, "input", false);

		// Assert
		let transformed = result.unwrap();
		assert!(transformed.attrs.is_empty());
		assert!(transformed.a11y_disabled);
	}

	#[rstest]
	fn test_transform_attrs_rejects_invalid_a11y_marker() {
		// Arrange
		let attrs = vec![PageAttr {
			name: syn::Ident::new("a11y", proc_macro2::Span::call_site()),
			value: parse_quote!(true),
			span: proc_macro2::Span::call_site(),
		}];

		// Act
		let result = transform_attrs(&attrs, "input", false);

		// Assert
		assert!(result.unwrap_err().to_string().contains("a11y"));
	}

	#[rstest]
	fn test_validate_page_accessibility_accepts_later_matching_label() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote! {
			|| {
				input { id: "email", type: "email", name: "email" }
				label { r#for: "email", "Email" }
			}
		})
		.unwrap();

		// Act
		let result = validate_page(&ast);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_page_accessibility_rejects_unlabelled_control() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote! {
			|| {
				input { type: "text", name: "q" }
			}
		})
		.unwrap();

		// Act
		let result = validate_page(&ast);

		// Assert
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("requires an accessible label")
		);
	}

	#[rstest]
	fn test_validate_page_accessibility_rejects_cross_branch_label() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote! {
			|show_input: bool| {
				if show_input {
					input { id: "query", type: "text", name: "query" }
				} else {
					label { r#for: "query", "Query" }
				}
			}
		})
		.unwrap();

		// Act
		let result = validate_page(&ast);

		// Assert
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("requires an accessible label")
		);
	}

	#[rstest]
	fn test_validate_page_accessibility_accepts_opted_out_control() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote! {
			|| {
				input { type: "range", name: "decorative", a11y: off }
			}
		})
		.unwrap();

		// Act
		let result = validate_page(&ast);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_page_accessibility_rejects_invalid_role() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote! {
			|| {
				div { role: "not-a-role", "Content" }
			}
		})
		.unwrap();

		// Act
		let result = validate_page(&ast);

		// Assert
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("Invalid ARIA role")
		);
	}

	#[rstest]
	fn test_validate_page_accessibility_rejects_positive_tabindex() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote! {
			|| {
				div { tabindex: 1, "Content" }
			}
		})
		.unwrap();

		// Act
		let result = validate_page(&ast);

		// Assert
		assert!(result.unwrap_err().to_string().contains("tabindex"));
	}

	#[rstest]
	fn test_validate_page_accessibility_requires_iframe_title() {
		// Arrange
		let ast: PageMacro = syn::parse2(quote::quote! {
			|| {
				iframe { src: "/frame" }
			}
		})
		.unwrap();

		// Act
		let result = validate_page(&ast);

		// Assert
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("requires a non-empty `title`")
		);
	}
}
