//! Code generation for the page! macro.
//!
//! This module converts typed AST nodes into Rust code that uses the `PageElement` API.
//!
//! ## Generated Code Structure
//!
//! ```text
//! page!(|initial: i32| {
//!     div {
//!         "hello"
//!     }
//! })
//! ```
//!
//! Generates:
//!
//! ```text
//! {
//!     |initial: i32| -> Page {
//!         PageElement::new("div")
//!             .child("hello")
//!             .into_page()
//!     }
//! }
//! ```

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
#[cfg(feature = "hmr")]
use std::cell::Cell;
use std::collections::HashSet;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{LitStr, Token};

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

// Import AST types from reinhardt-manouche
use crate::crate_paths::get_reinhardt_pages_crate_info;
use reinhardt_event_catalog::KnownEvent;
use reinhardt_manouche::core::types::AttrValue;
use reinhardt_manouche::core::{
	ComponentInvocationForm, ImplicitPageCapture, IntrinsicEvent, PageExpression, PageParam,
	PageText, TypedControlBinding, TypedControlBindingExpr, TypedControlBindingKind, TypedPageAttr,
	TypedPageBody, TypedPageComponent, TypedPageElement, TypedPageElse, TypedPageFor, TypedPageIf,
	TypedPageMacro, TypedPageMacroForm, TypedPageNode, TypedPageWatch,
};

use super::scope_utils::collect_pat_idents;

/// Generates code for the entire page! macro.
///
/// This function generates conditional code when both `reinhardt` and `reinhardt-pages`
/// are dependencies, allowing the macro to work correctly for both WASM and server builds.
///
/// # Arguments
///
/// * `macro_ast` - The validated and typed AST from the validator
pub(super) fn generate(macro_ast: &TypedPageMacro) -> TokenStream {
	let crate_info = get_reinhardt_pages_crate_info();
	let use_statement = &crate_info.use_statement;
	let pages_crate = &crate_info.ident;
	let ctx = CodegenContext::new(macro_ast.implicit_captures());

	let body = generate_body(macro_ast.body(), pages_crate, &ctx);

	// If head is provided, wrap the view with .with_head()
	let body_with_head = if let Some(head_expr) = &macro_ast.head {
		quote! {
			{
				let __view = #body;
				let __head = #head_expr;
				__view.with_head(__head)
			}
		}
	} else {
		body
	};

	#[cfg(feature = "hmr")]
	let body_with_head = {
		// Head metadata participates in the ABI but has no DOM placement in the
		// body template. Keep it out of the mounted-range registry so static body
		// edits conservatively fall back instead of retaining an invalid range.
		if macro_ast.head.is_some() {
			let _ = ctx.allocate_slot();
		}
		body_with_head
	};

	#[cfg(feature = "hmr")]
	let body_with_descriptor = {
		let descriptor = super::hot_reload::generate_template_descriptor(macro_ast, pages_crate);
		quote! {
			{
				let __view = #body_with_head;
				__view.with_dev_template_metadata(#descriptor)
			}
		}
	};
	#[cfg(not(feature = "hmr"))]
	let body_with_descriptor = body_with_head;

	match &macro_ast.form {
		TypedPageMacroForm::StrictClosure { params, .. } => {
			let params = generate_params(params);
			// Wrap in a closure with conditional use statement if needed.
			// Some generated paths consume parameters only after later macro expansion.
			quote! {
				{
					#use_statement
					#[allow(unused_variables)]
					#params -> #pages_crate::component::Page {
						#body_with_descriptor
					}
				}
			}
		}
		TypedPageMacroForm::ImplicitBody { .. } => {
			quote! {
				{
					#use_statement
				#body_with_descriptor
				}
			}
		}
	}
}

struct CodegenContext {
	capture_names: HashSet<String>,
	#[cfg(feature = "hmr")]
	next_slot_id: Cell<u32>,
}

impl CodegenContext {
	fn new(captures: &[ImplicitPageCapture]) -> Self {
		Self {
			capture_names: captures.iter().map(|c| c.ident.to_string()).collect(),
			#[cfg(feature = "hmr")]
			next_slot_id: Cell::new(0),
		}
	}

	#[cfg(feature = "hmr")]
	fn allocate_slot(&self) -> u32 {
		let slot_id = self.next_slot_id.get();
		self.next_slot_id.set(
			slot_id
				.checked_add(1)
				.expect("page template dynamic slot id overflow"),
		);
		slot_id
	}

	fn captures_in_expr(&self, expr: &syn::Expr) -> Vec<syn::Ident> {
		let mut collector = ExprCaptureCollector {
			capture_names: &self.capture_names,
			locals_stack: Vec::new(),
			seen: HashSet::new(),
			captures: Vec::new(),
		};
		collector.visit_expr(expr);
		collector.captures
	}

	fn captures_in_node(&self, node: &TypedPageNode) -> Vec<syn::Ident> {
		let mut collector = NodeCaptureCollector {
			expr_collector: ExprCaptureCollector {
				capture_names: &self.capture_names,
				locals_stack: Vec::new(),
				seen: HashSet::new(),
				captures: Vec::new(),
			},
		};
		collector.visit_node(node);
		collector.expr_collector.captures
	}

	fn captures_in_for_iteration(&self, for_node: &TypedPageFor) -> Vec<syn::Ident> {
		let mut collector = NodeCaptureCollector {
			expr_collector: ExprCaptureCollector {
				capture_names: &self.capture_names,
				locals_stack: Vec::new(),
				seen: HashSet::new(),
				captures: Vec::new(),
			},
		};

		let mut locals = HashSet::new();
		collect_pat_idents(&for_node.pat, &mut locals);
		collector.expr_collector.locals_stack.push(locals);
		if let Some(key) = &for_node.key {
			collector.expr_collector.visit_expr(key);
		}
		for node in &for_node.body {
			collector.visit_node(node);
		}
		collector.expr_collector.locals_stack.pop();

		collector.expr_collector.captures
	}
}

struct ExprCaptureCollector<'a> {
	capture_names: &'a HashSet<String>,
	locals_stack: Vec<HashSet<String>>,
	seen: HashSet<String>,
	captures: Vec<syn::Ident>,
}

impl ExprCaptureCollector<'_> {
	fn is_local(&self, name: &str) -> bool {
		self.locals_stack.iter().any(|s| s.contains(name))
	}

	fn record(&mut self, ident: &syn::Ident) {
		let name = ident.to_string();
		if self.capture_names.contains(&name) && !self.is_local(&name) && self.seen.insert(name) {
			self.captures.push(ident.clone());
		}
	}
}

impl<'ast> Visit<'ast> for ExprCaptureCollector<'_> {
	fn visit_expr_path(&mut self, ep: &'ast syn::ExprPath) {
		if ep.qself.is_none() && ep.path.segments.len() == 1 {
			self.record(&ep.path.segments[0].ident);
		}
		visit::visit_expr_path(self, ep);
	}

	fn visit_expr_closure(&mut self, c: &'ast syn::ExprClosure) {
		let mut locals = HashSet::new();
		for input in &c.inputs {
			collect_pat_idents(input, &mut locals);
		}
		self.locals_stack.push(locals);
		visit::visit_expr_closure(self, c);
		self.locals_stack.pop();
	}

	fn visit_expr_let(&mut self, l: &'ast syn::ExprLet) {
		let mut locals = HashSet::new();
		collect_pat_idents(&l.pat, &mut locals);
		self.locals_stack.push(locals);
		visit::visit_expr_let(self, l);
		self.locals_stack.pop();
	}

	fn visit_expr_if(&mut self, i: &'ast syn::ExprIf) {
		if let syn::Expr::Let(let_expr) = &*i.cond {
			let mut locals = HashSet::new();
			collect_pat_idents(&let_expr.pat, &mut locals);
			self.visit_expr(&let_expr.expr);
			self.locals_stack.push(locals);
			self.visit_block(&i.then_branch);
			self.locals_stack.pop();
			if let Some((_, else_branch)) = &i.else_branch {
				self.visit_expr(else_branch);
			}
		} else {
			visit::visit_expr_if(self, i);
		}
	}

	fn visit_arm(&mut self, a: &'ast syn::Arm) {
		let mut locals = HashSet::new();
		collect_pat_idents(&a.pat, &mut locals);
		self.locals_stack.push(locals);
		if let Some((_, guard)) = &a.guard {
			self.visit_expr(guard);
		}
		self.visit_expr(&a.body);
		self.locals_stack.pop();
	}

	fn visit_expr_for_loop(&mut self, f: &'ast syn::ExprForLoop) {
		self.visit_expr(&f.expr);
		let mut locals = HashSet::new();
		collect_pat_idents(&f.pat, &mut locals);
		self.locals_stack.push(locals);
		self.visit_block(&f.body);
		self.locals_stack.pop();
	}

	fn visit_expr_macro(&mut self, expr_macro: &'ast syn::ExprMacro) {
		let has_named_arguments = macro_supports_named_arguments(&expr_macro.mac.path);
		if let Ok(args) = expr_macro
			.mac
			.parse_body_with(Punctuated::<syn::Expr, Token![,]>::parse_terminated)
		{
			for arg in args {
				if has_named_arguments && let syn::Expr::Assign(assign) = arg {
					self.visit_expr(&assign.right);
				} else {
					self.visit_expr(&arg);
				}
			}
		}
	}

	fn visit_block(&mut self, b: &'ast syn::Block) {
		let mut pushed = 0_usize;
		for stmt in &b.stmts {
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
					self.locals_stack.push(locals);
					pushed += 1;
				}
				syn::Stmt::Item(_) => {}
				syn::Stmt::Expr(e, _) => self.visit_expr(e),
				syn::Stmt::Macro(m) => visit::visit_stmt_macro(self, m),
			}
		}
		for _ in 0..pushed {
			self.locals_stack.pop();
		}
	}
}

struct NodeCaptureCollector<'a> {
	expr_collector: ExprCaptureCollector<'a>,
}

impl NodeCaptureCollector<'_> {
	fn visit_node(&mut self, node: &TypedPageNode) {
		match node {
			TypedPageNode::Element(elem) => {
				for attr in &elem.attrs {
					self.expr_collector.visit_expr(&attr.value.to_expr());
				}
				if let Some(binding) = &elem.control_binding {
					match &binding.expression {
						TypedControlBindingExpr::Direct(value) => {
							self.expr_collector.visit_expr(value);
						}
						TypedControlBindingExpr::NumberWithError { value, error } => {
							self.expr_collector.visit_expr(value);
							self.expr_collector.visit_expr(error);
						}
					}
					if let Some(value) = &binding.radio_value {
						self.expr_collector.visit_expr(value);
					}
				}
				for event in &elem.events {
					self.expr_collector.visit_expr(event.handler());
				}
				for child in &elem.children {
					self.visit_node(child);
				}
			}
			TypedPageNode::Text(_) => {}
			TypedPageNode::Expression(expr) => self.expr_collector.visit_expr(&expr.expr),
			TypedPageNode::If(if_node) => self.visit_if(if_node),
			TypedPageNode::For(for_node) => self.visit_for(for_node),
			TypedPageNode::Component(comp) => self.visit_component(comp),
			TypedPageNode::Watch(watch) => self.visit_node(&watch.expr),
		}
	}

	fn visit_if(&mut self, if_node: &TypedPageIf) {
		let mut then_locals = None;
		if let syn::Expr::Let(let_expr) = &if_node.condition {
			let mut locals = HashSet::new();
			collect_pat_idents(&let_expr.pat, &mut locals);
			self.expr_collector.visit_expr(&let_expr.expr);
			then_locals = Some(locals);
		} else {
			self.expr_collector.visit_expr(&if_node.condition);
		}
		let pushed_then_locals = then_locals.is_some();
		if let Some(locals) = then_locals {
			self.expr_collector.locals_stack.push(locals);
		}
		for node in &if_node.then_branch {
			self.visit_node(node);
		}
		if pushed_then_locals {
			self.expr_collector.locals_stack.pop();
		}
		if let Some(else_branch) = &if_node.else_branch {
			match else_branch {
				TypedPageElse::Block(nodes) => {
					for node in nodes {
						self.visit_node(node);
					}
				}
				TypedPageElse::If(inner) => self.visit_if(inner),
			}
		}
	}

	fn visit_for(&mut self, for_node: &TypedPageFor) {
		self.expr_collector.visit_expr(&for_node.iter);
		let mut locals = HashSet::new();
		collect_pat_idents(&for_node.pat, &mut locals);
		self.expr_collector.locals_stack.push(locals);
		if let Some(key) = &for_node.key {
			self.expr_collector.visit_expr(key);
		}
		for node in &for_node.body {
			self.visit_node(node);
		}
		self.expr_collector.locals_stack.pop();
	}

	fn visit_component(&mut self, comp: &TypedPageComponent) {
		for arg in &comp.args {
			self.expr_collector.visit_expr(&arg.value);
		}
		for event in &comp.events {
			self.expr_collector.visit_expr(&event.handler);
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
}

fn capture_statements(captures: &[syn::Ident], pages_crate: &TokenStream) -> Vec<TokenStream> {
	captures
		.iter()
		.map(|ident| {
			quote! {
				let #ident = #pages_crate::__private::capture(&#ident);
			}
		})
		.collect()
}

fn wrap_expr_with_captures(
	expr: &syn::Expr,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	wrap_value_expr_with_captures(quote! { #expr }, expr, pages_crate, ctx)
}

fn wrap_value_expr_with_captures(
	value: TokenStream,
	source_expr: &syn::Expr,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let captures = ctx.captures_in_expr(source_expr);
	if captures.is_empty() {
		value
	} else {
		let capture_statements = capture_statements(&captures, pages_crate);
		quote! {
			{
				#(#capture_statements)*
				#value
			}
		}
	}
}

fn closure_expr_with_move_captures(
	expr: &syn::Expr,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let captures = ctx.captures_in_expr(expr);
	if captures.is_empty() {
		return quote! { #expr };
	}

	let capture_statements = capture_statements(&captures, pages_crate);
	match expr {
		syn::Expr::Closure(closure) => {
			let mut modified = closure.clone();
			modified.capture = Some(syn::token::Move {
				span: Span::call_site(),
			});
			let modified = syn::Expr::Closure(modified);
			quote! {
				{
					#(#capture_statements)*
					#modified
				}
			}
		}
		_ => quote! {
			{
				#(#capture_statements)*
				#expr
			}
		},
	}
}

/// Generates the closure parameter list.
fn generate_params(params: &[PageParam]) -> TokenStream {
	if params.is_empty() {
		quote!(||)
	} else {
		let param_tokens: Vec<TokenStream> = params
			.iter()
			.map(|p| {
				let name = &p.name;
				let ty = &p.ty;
				quote!(#name: #ty)
			})
			.collect();

		quote!(|#(#param_tokens),*|)
	}
}

/// Generates code for the page body.
fn generate_body(
	body: &TypedPageBody,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let nodes = generate_nodes(&body.nodes, pages_crate, ctx);

	// If there's exactly one node, return it directly
	// Otherwise, wrap in a fragment
	if body.nodes.len() == 1 {
		nodes
	} else {
		quote! {
			#pages_crate::component::Page::fragment([#nodes])
		}
	}
}

/// Generates code for multiple nodes.
fn generate_nodes(
	nodes: &[TypedPageNode],
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let node_tokens: Vec<TokenStream> = nodes
		.iter()
		.map(|n| generate_node(n, pages_crate, ctx))
		.collect();

	if node_tokens.len() == 1 {
		node_tokens.into_iter().next().unwrap()
	} else {
		quote!(#(#node_tokens),*)
	}
}

/// Generates code for a single node.
///
/// Spec §4.1 unconditional auto-wrap: every `{expr}` and every `if` / `for`
/// control-flow expression is wrapped in `Page::reactive(move || ...)` at
/// codegen time. The wrap is the single point of truth so reactive reads
/// inside helper-routed Signals (#4515) "just work" without a static
/// detection step.
fn generate_node(
	node: &TypedPageNode,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	match node {
		TypedPageNode::Element(elem) => generate_element(elem, pages_crate, ctx),
		TypedPageNode::Text(text) => generate_text(text, pages_crate),
		TypedPageNode::Expression(expr) => {
			#[cfg(feature = "hmr")]
			let slot_id = ctx.allocate_slot();
			let inner = generate_expression(expr, pages_crate);
			let page = wrap_reactive(inner, pages_crate, &ctx.captures_in_node(node));
			#[cfg(feature = "hmr")]
			return super::hot_reload::wrap_dynamic_slot(slot_id, page, pages_crate);
			#[cfg(not(feature = "hmr"))]
			page
		}
		TypedPageNode::If(if_node) => {
			#[cfg(feature = "hmr")]
			let slot_id = ctx.allocate_slot();
			let inner = generate_if(if_node, pages_crate, ctx);
			let page = wrap_reactive(inner, pages_crate, &ctx.captures_in_node(node));
			#[cfg(feature = "hmr")]
			return super::hot_reload::wrap_dynamic_slot(slot_id, page, pages_crate);
			#[cfg(not(feature = "hmr"))]
			page
		}
		TypedPageNode::For(for_node) => {
			#[cfg(feature = "hmr")]
			let mut slot_ids = vec![ctx.allocate_slot()];
			#[cfg(feature = "hmr")]
			if for_node.key.is_some() {
				slot_ids.push(ctx.allocate_slot());
			}
			let inner = generate_for(for_node, pages_crate, ctx);
			let page = wrap_reactive(inner, pages_crate, &ctx.captures_in_node(node));
			#[cfg(feature = "hmr")]
			return super::hot_reload::wrap_dynamic_slots(page, &slot_ids, pages_crate);
			#[cfg(not(feature = "hmr"))]
			page
		}
		TypedPageNode::Component(comp) => {
			#[cfg(feature = "hmr")]
			let slot_id = ctx.allocate_slot();
			let page = generate_component(comp, pages_crate, ctx);
			#[cfg(feature = "hmr")]
			return super::hot_reload::wrap_dynamic_slot(slot_id, page, pages_crate);
			#[cfg(not(feature = "hmr"))]
			page
		}
		TypedPageNode::Watch(watch_node) => generate_watch(watch_node, pages_crate, ctx),
	}
}

/// Generates code for an element node.
///
/// Event handlers use the same raw storage path on native and WASM targets.
fn generate_element(
	elem: &TypedPageElement,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let tag = elem.tag.to_string();
	#[cfg(feature = "hmr")]
	let mut dynamic_slot_ids = Vec::new();
	let radio_value_ident = syn::Ident::new("__reinhardt_radio_value", Span::mixed_site());
	let radio_value = elem.control_binding.as_ref().and_then(|binding| {
		(binding.kind == TypedControlBindingKind::Radio).then(|| {
			let value = binding.radio_value.as_ref().expect("validated radio value");
			let value = wrap_expr_with_captures(value, pages_crate, ctx);
			quote! { (#value).to_string() }
		})
	});
	let radio_value_initializer = radio_value.as_ref().map(|value| {
		quote! { let #radio_value_ident = #value; }
	});
	#[cfg(feature = "hmr")]
	for attr in &elem.attrs {
		if matches!(&attr.value, AttrValue::Dynamic(_)) {
			dynamic_slot_ids.push(ctx.allocate_slot());
		}
	}

	// Generate attributes
	let regular_attrs: Vec<TokenStream> = elem
		.attrs
		.iter()
		.filter(|attr| !BOOLEAN_ATTRS.contains(&attr.html_name().as_str()))
		.map(|attr| {
			if radio_value.is_some() && attr.html_name() == "value" {
				quote! {
					(
						::std::borrow::Cow::Borrowed("value"),
						::std::borrow::Cow::Owned(#radio_value_ident.clone())
					)
				}
			} else {
				generate_regular_attr_pair(attr, pages_crate, ctx)
			}
		})
		.collect();
	let bool_attrs: Vec<TokenStream> = elem
		.attrs
		.iter()
		.filter(|attr| BOOLEAN_ATTRS.contains(&attr.html_name().as_str()))
		.map(|attr| generate_bool_attr_pair(attr, pages_crate, ctx))
		.collect();

	#[cfg(feature = "hmr")]
	if elem.control_binding.is_some() {
		dynamic_slot_ids.push(ctx.allocate_slot());
	}
	#[cfg(feature = "hmr")]
	for _event in &elem.events {
		dynamic_slot_ids.push(ctx.allocate_slot());
	}

	// Generate children
	let children: Vec<TokenStream> = elem
		.children
		.iter()
		.map(|child| generate_child(child, pages_crate, ctx))
		.collect();

	// Build the base element (attributes and children, without events)
	let mut base_builder = quote! {
		#pages_crate::component::PageElement::new(#tag)
	};

	if !regular_attrs.is_empty() {
		base_builder = quote! {
			#base_builder.with_attrs([#(#regular_attrs),*])
		};
	}

	if !bool_attrs.is_empty() {
		base_builder = quote! {
			#base_builder.with_bool_attrs([#(#bool_attrs),*])
		};
	}

	// Add children
	for child in &children {
		base_builder = quote! {
			#base_builder
			.child(#child)
		};
	}

	if let Some(binding) = &elem.control_binding {
		let control_binding = generate_control_binding(
			binding,
			pages_crate,
			ctx,
			radio_value
				.as_ref()
				.map(|_| quote! { #radio_value_ident.clone() }),
		);
		base_builder = quote! {
			#base_builder #control_binding
		};
	}

	// Fast path: no events - simple generation.
	if elem.events.is_empty() {
		let page = quote! {
			#pages_crate::component::IntoPage::into_page(#base_builder)
		};
		#[cfg(feature = "hmr")]
		let page = super::hot_reload::wrap_dynamic_slots(page, &dynamic_slot_ids, pages_crate);
		return if let Some(initializer) = radio_value_initializer {
			quote! {{ #initializer #page }}
		} else {
			page
		};
	}

	// Register every intrinsic handler on both native and WASM targets.
	let event_bindings: Vec<TokenStream> = elem
		.events
		.iter()
		.map(|event| generate_event(event, pages_crate, ctx))
		.collect();

	let page = quote! {
		#pages_crate::component::IntoPage::into_page(
			#base_builder #(#event_bindings)*
		)
	};
	#[cfg(feature = "hmr")]
	let page = super::hot_reload::wrap_dynamic_slots(page, &dynamic_slot_ids, pages_crate);
	if let Some(initializer) = radio_value_initializer {
		quote! {{ #initializer #page }}
	} else {
		page
	}
}

fn generate_control_binding(
	binding: &TypedControlBinding,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
	radio_value_override: Option<TokenStream>,
) -> TokenStream {
	let value = match &binding.expression {
		TypedControlBindingExpr::Direct(value) => value,
		TypedControlBindingExpr::NumberWithError { value, .. } => value,
	};
	let binding_span = binding.span;
	let private = quote! { #pages_crate::control_binding::__private };
	let descriptor = match (&binding.kind, &binding.expression) {
		(TypedControlBindingKind::Text, _) => {
			quote_spanned!(binding_span=> #private::into_control_binding::<#private::TextBinding, _>(#value, ()))
		}
		(TypedControlBindingKind::Checkbox, _) => {
			quote_spanned!(binding_span=> #private::into_control_binding::<#private::CheckboxBinding, _>(#value, ()))
		}
		(TypedControlBindingKind::SelectOne, _) => {
			quote_spanned!(binding_span=> #private::into_control_binding::<#private::SelectOneBinding, _>(#value, ()))
		}
		(TypedControlBindingKind::SelectMany, _) => {
			quote_spanned!(binding_span=> #private::into_control_binding::<#private::SelectManyBinding, _>(#value, ()))
		}
		(TypedControlBindingKind::Number, TypedControlBindingExpr::Direct(_)) => {
			quote_spanned!(binding_span=> #private::into_control_binding::<#private::NumberBinding, _>(#value, ()))
		}
		(
			TypedControlBindingKind::Number,
			TypedControlBindingExpr::NumberWithError { error, .. },
		) => {
			let error = wrap_value_expr_with_captures(
				quote! { #pages_crate::reactive::copy_signal_handle(#error) },
				error,
				pages_crate,
				ctx,
			);
			quote_spanned!(binding_span=> #private::into_control_binding::<#private::NumberBinding, _>((#pages_crate::reactive::copy_signal_handle(#value), #error), ()))
		}
		(TypedControlBindingKind::Radio, _) => {
			let radio_value = radio_value_override.unwrap_or_else(|| {
				let radio_value = binding.radio_value.as_ref().expect("validated radio value");
				wrap_value_expr_with_captures(
					quote! { (#radio_value).to_string() },
					radio_value,
					pages_crate,
					ctx,
				)
			});
			quote_spanned!(binding_span=> #private::into_control_binding::<#private::RadioBinding, _>(#value, #radio_value))
		}
	};
	let descriptor = wrap_value_expr_with_captures(descriptor, value, pages_crate, ctx);
	quote!(.control_binding(#descriptor))
}

/// Boolean attributes that should use `.bool_attr()` method.
/// These attributes are either present or absent in HTML, not string-valued.
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

/// Generates code for a regular attribute pair.
fn generate_regular_attr_pair(
	attr: &TypedPageAttr,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let name_str = attr.html_name();

	// Handle different attribute value types
	// IntLit and FloatLit need to be converted to strings
	let value_expr = match &attr.value {
		AttrValue::IntLit(lit) => {
			// Generate: lit.to_string()
			quote! { #lit.to_string() }
		}
		AttrValue::FloatLit(lit) => {
			// Generate: lit.to_string()
			quote! { #lit.to_string() }
		}
		AttrValue::Dynamic(expr) if is_negative_integer_literal(expr) => {
			quote! { (#expr).to_string() }
		}
		_ => {
			// For StringLit, BoolLit, Dynamic: use as-is
			let expr = attr.value.to_expr();
			quote! { #expr }
		}
	};
	let value_expr =
		wrap_value_expr_with_captures(value_expr, &attr.value.to_expr(), pages_crate, ctx);

	quote! {
		(
			::std::borrow::Cow::Borrowed(#name_str),
			::std::borrow::Cow::from(#value_expr)
		)
	}
}

fn is_negative_integer_literal(expr: &syn::Expr) -> bool {
	if let syn::Expr::Unary(unary) = expr
		&& matches!(unary.op, syn::UnOp::Neg(_))
		&& let syn::Expr::Lit(lit) = unary.expr.as_ref()
	{
		return matches!(lit.lit, syn::Lit::Int(_));
	}

	false
}

/// Generates code for a boolean attribute pair.
fn generate_bool_attr_pair(
	attr: &TypedPageAttr,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let name_str = attr.html_name();
	let value_expr = attr.value.to_expr();
	let value_expr = wrap_expr_with_captures(&value_expr, pages_crate, ctx);
	quote! {
		(::std::borrow::Cow::Borrowed(#name_str), #value_expr)
	}
}

/// Checks if an expression is an async closure.
fn is_async_closure(expr: &syn::Expr) -> bool {
	match expr {
		syn::Expr::Closure(closure) => closure.asyncness.is_some(),
		_ => false,
	}
}

macro_rules! define_known_event_variant_ident {
	(
		$(
			$kind:ident,
			$dom_name:literal,
			$payload:ident,
			$interface:ident,
			[$($fallback:ident),* $(,)?],
			[$($capability:ident),* $(,)?],
			$bubbles:literal,
			$cancelable:literal,
			$composed:literal,
			$fixture_defaults:ident;
		)*
	) => {
		fn known_event_variant_ident(event: KnownEvent, span: Span) -> syn::Ident {
			match event {
				$(KnownEvent::$kind => syn::Ident::new(stringify!($kind), span),)*
			}
		}
	};
}

reinhardt_event_catalog::__reinhardt_event_catalog!(define_known_event_variant_ident);

/// Applies the inferred payload type while preserving explicit annotations.
fn lower_intrinsic_closure(handler: &syn::Expr, payload_type: TokenStream) -> syn::Expr {
	let syn::Expr::Closure(closure) = handler else {
		return handler.clone();
	};

	let mut closure = closure.clone();
	let payload_type = syn::parse2(payload_type).expect("generated event payload type must parse");
	match closure.inputs.first_mut() {
		None => closure.inputs.push(syn::Pat::Type(syn::PatType {
			attrs: Vec::new(),
			pat: Box::new(syn::Pat::Ident(syn::PatIdent {
				attrs: Vec::new(),
				by_ref: None,
				mutability: None,
				ident: syn::Ident::new("_event", handler.span()),
				subpat: None,
			})),
			colon_token: Default::default(),
			ty: Box::new(payload_type),
		})),
		Some(syn::Pat::Type(_)) => {}
		Some(parameter) => {
			let pattern = parameter.clone();
			*parameter = syn::Pat::Type(syn::PatType {
				attrs: Vec::new(),
				pat: Box::new(pattern),
				colon_token: Default::default(),
				ty: Box::new(payload_type),
			});
		}
	}
	syn::Expr::Closure(closure)
}

/// Generates code for an intrinsic event handler.
fn generate_event(
	event: &IntrinsicEvent,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	match event {
		IntrinsicEvent::Standard { event, handler } => {
			let spec = event.spec();
			let event_ident = known_event_variant_ident(*event, handler.span());
			let payload_ident = syn::Ident::new(spec.payload_name, handler.span());
			let payload_type = quote! { #pages_crate::event::#payload_ident };
			let lowered_handler = lower_intrinsic_closure(handler, payload_type.clone());
			let lowered_handler = if matches!(handler, syn::Expr::Closure(_)) {
				closure_expr_with_move_captures(&lowered_handler, pages_crate, ctx)
			} else {
				wrap_expr_with_captures(handler, pages_crate, ctx)
			};

			if is_async_closure(handler) {
				quote! {
					.on(
						#pages_crate::event::KnownEvent::#event_ident,
						#pages_crate::callback::typed_async_event_handler::<#payload_type, _, _>(#lowered_handler)
					)
				}
			} else {
				quote! {
					.on(
						#pages_crate::event::KnownEvent::#event_ident,
						#pages_crate::callback::typed_event_handler::<#payload_type, _>(#lowered_handler)
					)
				}
			}
		}
		IntrinsicEvent::RawCustom { name, handler } => {
			let raw_type = quote! { #pages_crate::platform::Event };
			let lowered_handler = lower_intrinsic_closure(handler, raw_type);
			let lowered_handler = if matches!(handler, syn::Expr::Closure(_)) {
				closure_expr_with_move_captures(&lowered_handler, pages_crate, ctx)
			} else {
				wrap_expr_with_captures(handler, pages_crate, ctx)
			};
			let adapter = if is_async_closure(handler) {
				quote! { #pages_crate::callback::raw_async_event_handler(#lowered_handler) }
			} else {
				quote! { #pages_crate::callback::raw_event_handler(#lowered_handler) }
			};

			quote! {
				.on(
					#pages_crate::event::EventName::Custom(::std::borrow::Cow::Borrowed(#name)),
					#adapter
				)
			}
		}
		IntrinsicEvent::TypedCustom {
			name,
			payload_type,
			handler,
		} => {
			let event_type = quote! {
				#pages_crate::event::CustomEvent<#payload_type>
			};
			let lowered_handler = lower_intrinsic_closure(handler, event_type.clone());
			let lowered_handler = if matches!(handler, syn::Expr::Closure(_)) {
				closure_expr_with_move_captures(&lowered_handler, pages_crate, ctx)
			} else {
				wrap_expr_with_captures(handler, pages_crate, ctx)
			};
			let adapter = if is_async_closure(handler) {
				quote_spanned! {handler.span()=>
					#pages_crate::callback::typed_async_custom_event_handler::<#payload_type, _, _>(#lowered_handler)
				}
			} else {
				quote_spanned! {handler.span()=>
					#pages_crate::callback::typed_custom_event_handler::<#payload_type, _>(#lowered_handler)
				}
			};

			quote! {
				.on(
					#pages_crate::event::EventName::Custom(::std::borrow::Cow::Borrowed(#name)),
					#adapter
				)
			}
		}
	}
}

/// Generates code for a child node (used in .child() calls).
///
/// Spec §4.1 unconditional auto-wrap: child `{expr}` / control-flow nodes
/// are routed through `generate_node` so they pick up the
/// `Page::reactive(move || ...)` wrap. Text and bare literals stay
/// uninstrumented for performance.
///
/// Note: passing pre-built non-`Clone` `Page` values as `page!` parameters
/// and then re-emitting them via `{ value }` will fail to compile under
/// this rule because the inner `Page::reactive` closure must be `Fn` and
/// `IntoPage::into_page` consumes the value. Compose such values inline
/// from their underlying data (or pass them in via a `Clone`able wrapper
/// once Page-Clone lands) instead.
fn generate_child(
	node: &TypedPageNode,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	match node {
		TypedPageNode::Text(text) => {
			// Create a proper string literal token
			let lit = LitStr::new(&text.content, Span::call_site());
			quote!(#lit)
		}
		_ => generate_node(node, pages_crate, ctx),
	}
}

/// Generates code for a text node.
fn generate_text(text: &PageText, pages_crate: &TokenStream) -> TokenStream {
	// Create a proper string literal token
	let lit = LitStr::new(&text.content, Span::call_site());
	quote! {
		#pages_crate::component::Page::text(#lit)
	}
}

/// Generates code for an expression node.
///
/// The expression is cloned before conversion so the enclosing
/// `Page::reactive(move || ...)` (spec §4.1 auto-wrap) remains `Fn` even
/// when the captured value would otherwise be consumed by
/// `IntoPage::into_page`. `Page` is `Clone` (cheap, Arc-backed), so this
/// is a constant-time clone for the common case. For values that already
/// borrow (e.g. `count.get()` returning `i32`), the redundant `.clone()`
/// on the owned result is a no-op.
fn generate_expression(expr: &PageExpression, pages_crate: &TokenStream) -> TokenStream {
	let e = &expr.expr;
	if is_i18n_t_macro_expr(e) {
		return quote! {
			#pages_crate::component::Page::text((#e).render_string())
		};
	}
	quote! {
		#pages_crate::component::IntoPage::into_page((#e).clone())
	}
}

fn is_i18n_t_macro_expr(expr: &syn::Expr) -> bool {
	let syn::Expr::Macro(expr_macro) = expr else {
		return false;
	};
	let segments: Vec<_> = expr_macro
		.mac
		.path
		.segments
		.iter()
		.map(|segment| segment.ident.to_string())
		.collect();
	matches!(
		segments.as_slice(),
		[crate_name, macro_name] if crate_name == "reinhardt_pages" && macro_name == "t"
	) || matches!(
		segments.as_slice(),
		[crate_name, module_name, macro_name]
			if crate_name == "reinhardt_pages" && module_name == "prelude" && macro_name == "t"
	)
}

/// Generates code for an if node.
///
/// Currently generates regular Rust if/else expressions for all conditions.
/// This approach avoids ownership issues with captured variables in closures.
///
/// For reactive conditional rendering with Signals, users should either:
/// 1. Use `Page::reactive_if()` directly in their code
/// 2. Restructure their code to use Signal-based state management
///
/// Future enhancements may include automatic Signal detection or explicit
/// reactive syntax (e.g., `@if condition { ... }`).
fn generate_if(
	if_node: &TypedPageIf,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let condition = &if_node.condition;
	let then_branch = generate_if_branch(&if_node.then_branch, pages_crate, ctx);

	let else_branch = match &if_node.else_branch {
		Some(TypedPageElse::Block(nodes)) => {
			// else { ... } block - generate view directly
			generate_if_branch(nodes, pages_crate, ctx)
		}
		Some(TypedPageElse::If(nested_if)) => {
			// else if { ... } - recursively generate another if
			generate_if(nested_if, pages_crate, ctx)
		}
		None => {
			// No else branch - use Empty view
			quote! { #pages_crate::component::Page::Empty }
		}
	};

	// Generate regular Rust if/else expression
	// This avoids ownership issues with captured variables in Fn closures
	quote! {
		if #condition {
			#then_branch
		} else {
			#else_branch
		}
	}
}

/// Generates code for an if branch (then or else block).
fn generate_if_branch(
	nodes: &[TypedPageNode],
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	if nodes.is_empty() {
		quote! { #pages_crate::component::Page::Empty }
	} else if nodes.len() == 1 {
		generate_node(&nodes[0], pages_crate, ctx)
	} else {
		let node_tokens: Vec<TokenStream> = nodes
			.iter()
			.map(|n| generate_node(n, pages_crate, ctx))
			.collect();
		quote! {
			#pages_crate::component::Page::fragment([#(#node_tokens),*])
		}
	}
}

/// Generates code for a for node.
///
/// The iterator expression is cloned before iteration because the enclosing
/// `Page::reactive(move || ...)` (spec §4.1 auto-wrap) re-runs on every
/// tracked change and must not consume the captured iterator. The iterator
/// expression is therefore required to implement `Clone` (e.g. `Vec<T>`,
/// `&[T]`, or any `Clone` collection); a non-`Clone` iterator is rejected at
/// compile time. The keyed branch yields `(#key, #body)` tuples that align
/// with `Page::keyed_fragment<K: Into<String>, V: IntoPage>`, and the unkeyed
/// branch yields `#body` (`Page`, which implements `IntoPage`) for
/// `Page::fragment`.
fn generate_for(
	for_node: &TypedPageFor,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let pat = &for_node.pat;
	let iter = &for_node.iter;
	let body = generate_if_branch(&for_node.body, pages_crate, ctx);
	let iteration_captures = ctx.captures_in_for_iteration(for_node);
	let iteration_capture_statements = capture_statements(&iteration_captures, pages_crate);

	if let Some(key) = &for_node.key {
		quote! {
			{
				#(#iteration_capture_statements)*
				#pages_crate::component::Page::keyed_fragment(
					(#iter).clone().into_iter().map(move |#pat| {
						(#key, #body)
					}).collect::<::std::vec::Vec<_>>()
				)
			}
		}
	} else {
		quote! {
			{
				#(#iteration_capture_statements)*
				#pages_crate::component::Page::fragment(
					(#iter).clone().into_iter().map(move |#pat| {
						#body
					}).collect::<::std::vec::Vec<_>>()
				)
			}
		}
	}
}

/// Generates code for a watch node.
///
/// The watch block wraps its inner expression in a reactive context,
/// allowing Signal dependencies to be automatically tracked and the view
/// to be re-rendered when they change.
///
/// # Example
///
/// ```text
/// watch {
///     if signal.get() > 0 {
///         div { "Positive" }
///     } else {
///         div { "Non-positive" }
///     }
/// }
/// ```
///
/// Generates:
///
/// ```text
/// Page::reactive(move || {
///     if signal.get() > 0 {
///         PageElement::new("div").child("Positive").into_page()
///     } else {
///         PageElement::new("div").child("Non-positive").into_page()
///     }
/// })
/// ```
fn generate_watch(
	watch_node: &TypedPageWatch,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let inner_expr = generate_node(&watch_node.expr, pages_crate, ctx);
	wrap_reactive(
		inner_expr,
		pages_crate,
		&ctx.captures_in_node(&watch_node.expr),
	)
}

/// Wraps a generated TokenStream in `Page::reactive(move || ...)`.
///
/// This is the single point of truth for spec §4.1 auto-wrap. Used by
/// `generate_expression`, `generate_if`, `generate_for`, and (kept for
/// backward compat) `generate_watch`.
fn wrap_reactive(
	inner: TokenStream,
	pages_crate: &TokenStream,
	captures: &[syn::Ident],
) -> TokenStream {
	let capture_statements = capture_statements(captures, pages_crate);
	quote! {
		{
			#(#capture_statements)*
			#pages_crate::component::Page::reactive(move || {
				#inner
			})
		}
	}
}

/// Generates code for a component call.
///
/// Branches on [`ComponentInvocationForm`]: the legacy positional form is
/// emitted as a direct function call (spec §3.5 backward-compat), while the
/// brace form is emitted as a `bon::Builder` chain per spec §3.5.3.
fn generate_component(
	comp: &TypedPageComponent,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	match comp.invocation_form {
		ComponentInvocationForm::Paren => generate_component_paren(comp, pages_crate, ctx),
		ComponentInvocationForm::Brace => generate_component_brace(comp, pages_crate, ctx),
	}
}

/// Generates code for the legacy positional component-call form.
///
/// # Example
///
/// ```text
/// // Input DSL
/// MyButton(label: "Click", disabled: false)
///
/// // Generated code
/// MyButton("Click", false)
/// ```
fn generate_component_paren(
	comp: &TypedPageComponent,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let name = &comp.name;

	let args: Vec<TokenStream> = comp
		.args
		.iter()
		.map(|arg| wrap_expr_with_captures(&arg.value, pages_crate, ctx))
		.collect();

	// Build the base function call: Component(args...)
	let base_call = if args.is_empty() {
		quote! { #name () }
	} else {
		quote! { #name (#(#args),*) }
	};

	// Generate children setter if present
	let children_setter = comp.children.as_ref().map(|children| {
		let children_view = generate_if_branch(children, pages_crate, ctx);
		quote! { .children (#children_view) }
	});

	// Generate named slot setters
	let slot_setters: Vec<TokenStream> = comp
		.named_slots
		.iter()
		.map(|slot| {
			let setter_name = slot_name_to_snake_case(&slot.name.to_string());
			let setter_ident = syn::Ident::new(&setter_name, slot.name.span());
			let slot_view = generate_if_branch(&slot.children, pages_crate, ctx);
			quote! { .#setter_ident (#slot_view) }
		})
		.collect();

	// If we have no children and no named slots, emit a simple function call
	if children_setter.is_none() && slot_setters.is_empty() {
		return base_call;
	}

	// Emit builder chain: Component(args...).children(c).slot1(v1).build()
	quote! {
		#base_call
		#children_setter
		#(#slot_setters)*
		.build ()
	}
}

/// Converts a DSL slot name to snake_case for the builder setter method name.
///
/// The DSL slot name is already in camelCase or PascalCase (it comes from a Rust
/// identifier), so this converts to snake_case: "bodyContent" → "body_content".
fn slot_name_to_snake_case(name: &str) -> String {
	let mut result = String::with_capacity(name.len() + 4);
	for (i, ch) in name.chars().enumerate() {
		if ch.is_uppercase() {
			if i > 0 {
				result.push('_');
			}
			result.push(ch.to_ascii_lowercase());
		} else {
			result.push(ch);
		}
	}
	result
}

/// Generates the `bon::Builder` chain for a brace-form component invocation.
///
/// Spec §3.5.1 convention: a component named `Card` resolves to a function
/// `card(props: CardProps) -> Page` where `CardProps` derives `bon::Builder`.
///
/// Spec §3.5.3 lowering rules:
///
/// - `prop: value` → `.prop(value)` on the builder
/// - `@event: handler` → `.on_event(handler)` on the builder
/// - children arity (the `children:` field on the props struct must be
///   `Option<Page>`, which `bon::Builder` exposes as a setter taking
///   `Page` directly — bon wraps it in `Some` internally):
///   - 0 children → omit the `.children(...)` setter (`bon` defaults
///     the `Option` field to `None`)
///   - 1 child    → `.children(<child_view>)`
///   - 2+ children → `.children(Page::fragment(vec![ ... ]))`
///
/// # Example
///
/// ```text
/// // Input DSL
/// Card { item: x, @click: h, p { "child" } }
///
/// // Generated code (simplified)
/// card(CardProps::builder()
///     .item(x)
///     .on_click(h)
///     .children(<p_element_view>)
///     .build())
/// ```
fn generate_component_brace(
	comp: &TypedPageComponent,
	pages_crate: &TokenStream,
	ctx: &CodegenContext,
) -> TokenStream {
	let props_ty = props_struct_name(&comp.name);
	let fn_name = component_fn_name(&comp.name);

	// `.field(value)` per named prop.
	let prop_setters: Vec<TokenStream> = comp
		.args
		.iter()
		.map(|arg| {
			let n = &arg.name;
			let v = wrap_expr_with_captures(&arg.value, pages_crate, ctx);
			quote! { .#n(#v) }
		})
		.collect();

	// `.on_<event>(handler)` per event prop.
	let event_setters: Vec<TokenStream> = comp
		.events
		.iter()
		.map(|ev| {
			let on_name = syn::Ident::new(&format!("on_{}", ev.name), ev.name.span());
			let h = closure_expr_with_move_captures(&ev.handler, pages_crate, ctx);
			quote! { .#on_name(#h) }
		})
		.collect();

	// children: per §3.5.3 table.
	//
	// `bon::Builder` synthesises an `.children(value: Page)` setter for
	// `children: Option<Page>` (the Option is wrapped internally), so we
	// pass the child view directly without an explicit `Some(...)`.
	let children_setter = match &comp.children {
		None => quote! {},
		Some(cs) if cs.len() == 1 => {
			let one = generate_node(&cs[0], pages_crate, ctx);
			quote! { .children(#one) }
		}
		Some(cs) => {
			let many: Vec<TokenStream> = cs
				.iter()
				.map(|c| generate_node(c, pages_crate, ctx))
				.collect();
			quote! {
				.children(
					#pages_crate::component::Page::fragment(::std::vec![ #(#many),* ])
				)
			}
		}
	};

	// Named slot setters (e.g., `$header { ... }` → `.header(view)`).
	let slot_setters: Vec<TokenStream> = comp
		.named_slots
		.iter()
		.map(|slot| {
			let setter_name = slot_name_to_snake_case(&slot.name.to_string());
			let setter_ident = syn::Ident::new(&setter_name, slot.name.span());
			let slot_view = generate_if_branch(&slot.children, pages_crate, ctx);
			quote! { .#setter_ident (#slot_view) }
		})
		.collect();

	quote! {
		#fn_name(
			#props_ty::builder()
				#(#prop_setters)*
				#(#event_setters)*
				#children_setter
				#(#slot_setters)*
				.build()
		)
	}
}

/// Spec §3.5.1 convention: `Card` → struct `CardProps`.
fn props_struct_name(comp: &syn::Ident) -> syn::Ident {
	syn::Ident::new(&format!("{comp}Props"), comp.span())
}

/// Spec §3.5.1 convention: `Card` → fn `card` (snake_case of the component name).
///
/// Conversion lowercases the name and inserts `_` at word boundaries, while
/// keeping consecutive uppercase runs together as a single acronym word
/// (e.g. `URLCard` → `url_card`). Already-snake-case names round-trip unchanged.
fn component_fn_name(comp: &syn::Ident) -> syn::Ident {
	let s = comp.to_string();
	let snake = pascal_to_snake(&s);
	syn::Ident::new(&snake, comp.span())
}

/// PascalCase → snake_case conversion. Treats consecutive uppercase runs as
/// acronyms per Rust naming conventions (e.g. `URLCard` → `url_card`).
fn pascal_to_snake(s: &str) -> String {
	let mut out = String::with_capacity(s.len() + 4);
	let mut chars = s.chars().peekable();
	let mut prev_is_lower_or_digit = false;

	while let Some(c) = chars.next() {
		if c.is_ascii_uppercase() {
			let next_is_lower = chars.peek().is_some_and(|next| next.is_ascii_lowercase());
			if !out.is_empty() && (prev_is_lower_or_digit || next_is_lower) {
				out.push('_');
			}
			out.push(c.to_ascii_lowercase());
			prev_is_lower_or_digit = false;
		} else {
			out.push(c);
			prev_is_lower_or_digit = c.is_ascii_lowercase() || c.is_ascii_digit();
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	fn assert_generated_expression(input: TokenStream, expected: TokenStream) {
		let actual: syn::Expr =
			syn::parse2(parse_and_generate(input)).expect("generated page expression should parse");
		let expected: syn::Expr =
			syn::parse2(expected).expect("expected page expression should parse");

		let normalize = |expression: &syn::Expr| {
			quote!(#expression)
				.to_string()
				.chars()
				.filter(|character| !character.is_whitespace())
				.collect::<String>()
				.replace(",)", ")")
				.replace(",>", ">")
		};
		let actual = normalize(&actual);
		let expected = normalize(&expected);

		#[cfg(not(feature = "hmr"))]
		assert_eq!(actual, expected);

		#[cfg(feature = "hmr")]
		{
			let closure_prefix =
				"{#[allow(unused_variables)]||->::reinhardt_pages::component::Page{";
			let instrumented_prefix = [closure_prefix, "{let__view=("].concat();
			let instrumented_suffix = ").with_dev_slot(0u32);__view.with_dev_template_metadata(";
			let actual_body = actual
				.strip_prefix(&instrumented_prefix)
				.expect("HMR output should wrap the generated page expression");
			let (actual_body, metadata) = actual_body
				.split_once(instrumented_suffix)
				.expect("HMR output should attach slot and template metadata");
			assert!(metadata.ends_with(")}}}"));

			let expected_body = expected
				.strip_prefix(closure_prefix)
				.and_then(|body| body.strip_suffix("}}"))
				.expect("expected output should contain the generated closure body");
			assert_eq!(actual_body, expected_body);
		}
	}

	fn parse_and_generate(input: TokenStream) -> TokenStream {
		use reinhardt_manouche::core::PageMacro;

		let untyped_ast: PageMacro = syn::parse2(input).unwrap();
		// Transform to typed AST
		let typed_ast = crate::page::validator::validate(&untyped_ast).unwrap();
		generate(&typed_ast)
	}

	#[test]
	fn test_generate_simple_element() {
		let input = quote::quote!(|| { div { "hello" } });
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		// TokenStream stringification adds spaces between tokens
		// e.g., "crate :: component :: ElementView :: new"
		assert!(output_str.contains("PageElement"));
		assert!(output_str.contains("new"));
		assert!(output_str.contains("\"div\""));
		assert!(output_str.contains("\"hello\""));
	}

	#[test]
	fn test_generate_element_with_attr() {
		let input = quote::quote!(|| {
			div {
				class: "container",
				"hello"
			}
		});
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		// TokenStream stringification adds spaces between tokens
		assert!(output_str.contains(". with_attrs"));
		assert!(output_str.contains("\"class\""));
		assert!(output_str.contains("\"container\""));
	}

	#[test]
	fn test_generate_standard_event_uses_catalog_typed_adapter() {
		let input = quote::quote!(|| {
			button { @click: |event| { let _ = event; }, "Click" }
		});

		let output = parse_and_generate(input).to_string();

		assert!(output.contains(". on"));
		assert!(output.contains("event :: KnownEvent :: Click"));
		assert!(output.contains("event :: ClickEvent"));
		assert!(output.contains("callback :: typed_event_handler"));
		assert!(!output.contains("cfg"));
	}

	macro_rules! assert_catalog_lowering_parity {
		(
			$(
				$kind:ident,
				$dom_name:literal,
				$payload:ident,
				$interface:ident,
				[$($fallback:ident),* $(,)?],
				[$($capability:ident),* $(,)?],
				$bubbles:literal,
				$cancelable:literal,
				$composed:literal,
				$fixture_defaults:ident;
			)*
		) => {
			#[test]
			fn every_catalog_event_has_a_macro_lowering_variant() {
				$(
					assert_eq!(
						known_event_variant_ident(KnownEvent::$kind, Span::call_site()).to_string(),
						stringify!($kind),
					);
				)*
			}
		};
	}

	reinhardt_event_catalog::__reinhardt_event_catalog!(assert_catalog_lowering_parity);

	#[test]
	fn test_generate_async_standard_event_uses_typed_async_adapter() {
		let input = quote::quote!(|| {
			button { @click: async |event| { let _ = event; }, "Click" }
		});

		let output = parse_and_generate(input).to_string();

		assert!(output.contains("callback :: typed_async_event_handler"));
		assert!(output.contains("event :: ClickEvent"));
	}

	#[test]
	fn test_generate_zero_argument_standard_event_adds_typed_parameter() {
		let input = quote::quote!(|| {
			button { @click: || {}, "Click" }
		});

		let output = parse_and_generate(input).to_string();

		assert!(output.contains("_event"));
		assert!(output.contains("event :: ClickEvent"));
		assert!(output.contains("callback :: typed_event_handler"));
	}

	#[test]
	fn test_generate_custom_event_uses_raw_adapter() {
		let input = quote::quote!(|| {
			div { @custom("item-selected"): |event| { let _ = event; }, }
		});

		let output = parse_and_generate(input).to_string();

		assert!(output.contains("event :: EventName :: Custom"));
		assert!(output.contains("\"item-selected\""));
		assert!(output.contains("callback :: raw_event_handler"));
	}

	#[rstest]
	fn test_generate_typed_custom_event_uses_custom_adapter() {
		assert_generated_expression(
			quote::quote!(|| {
				div {
					@custom::<crate::Selected>("item-selected"): |event| {
						let _ = event;
					},
				}
			}),
			quote::quote!({
				#[allow(unused_variables)]
				|| -> ::reinhardt_pages::component::Page {
					::reinhardt_pages::component::IntoPage::into_page(
						::reinhardt_pages::component::PageElement::new("div").on(
							::reinhardt_pages::event::EventName::Custom(
								::std::borrow::Cow::Borrowed("item-selected"),
							),
							::reinhardt_pages::callback::typed_custom_event_handler::<
								crate::Selected,
								_,
							>(
								|event: ::reinhardt_pages::event::CustomEvent<crate::Selected>| {
									let _ = event;
								},
							),
						),
					)
				}
			}),
		);
	}

	#[rstest]
	fn test_generate_async_typed_custom_event_uses_custom_adapter() {
		assert_generated_expression(
			quote::quote!(|| {
				div {
					@custom::<crate::Selected>("item-loaded"): async |event| {
						let _ = event;
					},
				}
			}),
			quote::quote!({
				#[allow(unused_variables)]
				|| -> ::reinhardt_pages::component::Page {
					::reinhardt_pages::component::IntoPage::into_page(
						::reinhardt_pages::component::PageElement::new("div").on(
							::reinhardt_pages::event::EventName::Custom(
								::std::borrow::Cow::Borrowed("item-loaded"),
							),
							::reinhardt_pages::callback::typed_async_custom_event_handler::<
								crate::Selected,
								_,
								_,
							>(
								async |event: ::reinhardt_pages::event::CustomEvent<
									crate::Selected,
								>| {
									let _ = event;
								},
							),
						),
					)
				}
			}),
		);
	}

	#[rstest]
	fn test_generate_zero_argument_typed_custom_event_adds_typed_parameter() {
		assert_generated_expression(
			quote::quote!(|| {
				div { @custom::<crate::Selected>("item-focused"): || {}, }
			}),
			quote::quote!({
				#[allow(unused_variables)]
				|| -> ::reinhardt_pages::component::Page {
					::reinhardt_pages::component::IntoPage::into_page(
						::reinhardt_pages::component::PageElement::new("div").on(
							::reinhardt_pages::event::EventName::Custom(
								::std::borrow::Cow::Borrowed("item-focused"),
							),
							::reinhardt_pages::callback::typed_custom_event_handler::<
								crate::Selected,
								_,
							>(
								|_event: ::reinhardt_pages::event::CustomEvent<crate::Selected>| {}
							),
						),
					)
				}
			}),
		);
	}

	#[test]
	fn test_generate_with_params() {
		let input = quote::quote!(|name: String| {
			div { "hello" }
		});
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("name : String"));
	}

	#[test]
	fn test_generate_data_attr_conversion() {
		let input = quote::quote!(|| {
			div {
				data_testid: "test",
				"hello"
			}
		});
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		// data_testid should become data-testid
		assert!(output_str.contains("\"data-testid\""));
	}

	#[test]
	fn test_generate_component_basic() {
		let input = quote::quote!(|| {
			MyButton(label: "Click")
		});
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		// Component call should be generated as a function call
		assert!(output_str.contains("MyButton"));
		assert!(output_str.contains("\"Click\""));
	}

	#[test]
	fn test_generate_component_multiple_args() {
		let input = quote::quote!(|| {
			MyButton(label: "Click", disabled: true, count: 42)
		});
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("MyButton"));
		assert!(output_str.contains("\"Click\""));
		assert!(output_str.contains("true"));
		assert!(output_str.contains("42"));
	}

	#[test]
	fn test_generate_component_empty_args() {
		let input = quote::quote!(|| { MyComponent() });
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		// Should generate MyComponent()
		assert!(output_str.contains("MyComponent"));
		assert!(output_str.contains("()"));
	}

	#[test]
	fn test_generate_component_with_children() {
		let input = quote::quote!(|| {
			MyWrapper(class: "container") {
				div { "content" }
			}
		});
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		// Should include component name and the children view
		assert!(output_str.contains("MyWrapper"));
		assert!(output_str.contains("\"container\""));
		assert!(output_str.contains("PageElement"));
	}

	#[test]
	fn test_generate_component_with_named_slots() {
		let input = quote::quote!(|| {
			Table(args: 1) {
				$header { div { "Header" } }
				$body { div { "Body" } }
			}
		});
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("Table"));
		// Should contain builder chain with named slot setters
		assert!(output_str.contains(". header"));
		assert!(output_str.contains(". body"));
		assert!(output_str.contains(". build ()"));
	}

	#[test]
	fn test_generate_component_mixed_default_and_named() {
		let input = quote::quote!(|| {
			Layout(args: 1) {
				div { "default" }
				$sidebar { div { "Sidebar" } }
			}
		});
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		assert!(output_str.contains("Layout"));
		assert!(output_str.contains(". children"));
		assert!(output_str.contains(". sidebar"));
		assert!(output_str.contains(". build ()"));
	}

	#[test]
	fn test_generate_component_slot_name_snake_case() {
		let input = quote::quote!(|| {
			Container(args: 1) {
				$bodyContent { div { "Body" } }
			}
		});
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		// bodyContent → body_content
		assert!(output_str.contains(". body_content"));
	}

	#[test]
	fn test_generate_component_no_slots_unchanged() {
		// Components without named slots should keep existing behavior
		let input = quote::quote!(|| {
			MyButton(label: "Click")
		});
		let output = parse_and_generate(input);
		let output_str = output.to_string();

		// No builder chain for simple component
		assert!(output_str.contains("MyButton"));
		assert!(!output_str.contains(". build ()"));
	}

	#[test]
	fn test_for_key_capture_scope_uses_loop_local() {
		let input = quote::quote!({
			div {
				{ item.clone() }
				for item in items @key(item.clone()) {
					li { { item.clone() } }
				}
			}
		});
		let untyped_ast: reinhardt_manouche::core::PageMacro = syn::parse2(input).unwrap();
		let typed_ast = crate::page::validator::validate(&untyped_ast).unwrap();
		let for_node = match &typed_ast.body().nodes[0] {
			TypedPageNode::Element(element) => &element.children[1],
			_ => panic!("expected root element"),
		};
		let ctx = CodegenContext::new(typed_ast.implicit_captures());

		let captures: Vec<String> = ctx
			.captures_in_node(for_node)
			.into_iter()
			.map(|ident| ident.to_string())
			.collect();

		assert_eq!(captures, vec!["items"]);
	}

	#[test]
	fn test_if_capture_includes_control_binding_expression() {
		let input = quote::quote!({
			if visible.get() {
				input {
					a11y: off,
					bind: selected,
				}
			}
		});
		let untyped_ast: reinhardt_manouche::core::PageMacro = syn::parse2(input).unwrap();
		let typed_ast = crate::page::validator::validate(&untyped_ast).unwrap();
		let if_node = &typed_ast.body().nodes[0];
		let ctx = CodegenContext::new(typed_ast.implicit_captures());

		let captures: Vec<String> = ctx
			.captures_in_node(if_node)
			.into_iter()
			.map(|ident| ident.to_string())
			.collect();

		assert_eq!(captures, vec!["visible", "selected"]);
	}

	#[test]
	fn test_control_binding_codegen_uses_typed_source_adapters() {
		let input = quote::quote!(|
			text: Signal<String>,
			checked: Signal<bool>,
			radio: Signal<String>,
			amount: Signal<i32>,
			parse_error: Signal<Option<NumberParseError>>,
			selected: Signal<String>,
			selected_many: Signal<Vec<String>>,
		| {
			div {
				input { a11y: off, bind: text }
				input { a11y: off, type: "checkbox", bind: checked }
				input { a11y: off, type: "radio", value: "choice", bind: radio }
				input { a11y: off, type: "number", bind: number(amount, parse_error) }
				select { a11y: off, bind: selected, option { value: "one", "One" } }
				select { a11y: off, multiple: true, bind: selected_many, option { value: "one", "One" } }
			}
		});
		let untyped_ast: reinhardt_manouche::core::PageMacro = syn::parse2(input).unwrap();
		let typed_ast = crate::page::validator::validate(&untyped_ast).unwrap();
		let TypedPageNode::Element(root) = &typed_ast.body().nodes[0] else {
			panic!("expected root element");
		};
		let ctx = CodegenContext::new(typed_ast.implicit_captures());
		let pages_crate = quote::quote!(reinhardt_pages);

		let expected_markers = [
			"TextBinding",
			"CheckboxBinding",
			"RadioBinding",
			"NumberBinding",
			"SelectOneBinding",
			"SelectManyBinding",
		];
		for (node, marker) in root.children.iter().zip(expected_markers) {
			let TypedPageNode::Element(element) = node else {
				panic!("expected bound control");
			};
			let binding = element.control_binding.as_ref().expect("validated binding");
			let output = generate_control_binding(
				binding,
				&pages_crate,
				&ctx,
				(binding.kind == TypedControlBindingKind::Radio)
					.then(|| quote::quote!("choice".to_string())),
			)
			.to_string()
			.replace(' ', "");
			assert!(output.contains("control_binding::__private::into_control_binding"));
			assert!(output.contains(marker));
			assert!(!output.contains("ControlBinding::text"));
		}
	}

	#[test]
	fn test_for_key_iteration_captures_macro_arguments() {
		let input = quote::quote!({
			ul {
				for todo in todos @key(format!("{}:{}", selected.as_str(), todo.id)) {
					li { { todo.title.clone() } }
				}
			}
		});
		let untyped_ast: reinhardt_manouche::core::PageMacro = syn::parse2(input).unwrap();
		let typed_ast = crate::page::validator::validate(&untyped_ast).unwrap();
		let for_node = match &typed_ast.body().nodes[0] {
			TypedPageNode::Element(element) => &element.children[0],
			_ => panic!("expected root element"),
		};
		let TypedPageNode::For(for_node) = for_node else {
			panic!("expected for node");
		};
		let ctx = CodegenContext::new(typed_ast.implicit_captures());

		let captures: Vec<String> = ctx
			.captures_in_for_iteration(for_node)
			.into_iter()
			.map(|ident| ident.to_string())
			.collect();

		assert_eq!(captures, vec!["selected"]);
	}

	#[test]
	fn test_named_macro_argument_key_is_not_a_capture() {
		let input = quote::quote!({
			p { { ::reinhardt::pages::t!("Project {id}", id = project_id) } }
		});
		let untyped_ast: reinhardt_manouche::core::PageMacro = syn::parse2(input).unwrap();
		let typed_ast = crate::page::validator::validate(&untyped_ast).unwrap();
		let ctx = CodegenContext::new(typed_ast.implicit_captures());

		let captures: Vec<String> = ctx
			.captures_in_node(&typed_ast.body().nodes[0])
			.into_iter()
			.map(|ident| ident.to_string())
			.collect();

		assert_eq!(captures, vec!["project_id"]);
	}

	#[test]
	fn test_relative_framework_macro_assignment_lhs_remains_a_capture() {
		let input = quote::quote!({
			p { { reinhardt_pages::t!("Project {id}", id = project_id) } }
		});
		let untyped_ast: reinhardt_manouche::core::PageMacro = syn::parse2(input).unwrap();
		let typed_ast = crate::page::validator::validate(&untyped_ast).unwrap();
		let ctx = CodegenContext::new(typed_ast.implicit_captures());

		let captures: Vec<String> = ctx
			.captures_in_node(&typed_ast.body().nodes[0])
			.into_iter()
			.map(|ident| ident.to_string())
			.collect();

		assert_eq!(captures, vec!["id", "project_id"]);
	}

	#[test]
	fn test_custom_macro_assignment_lhs_remains_a_capture() {
		let input = quote::quote!({
			p { { custom!(label = fallback) } }
		});
		let untyped_ast: reinhardt_manouche::core::PageMacro = syn::parse2(input).unwrap();
		let typed_ast = crate::page::validator::validate(&untyped_ast).unwrap();
		let ctx = CodegenContext::new(typed_ast.implicit_captures());

		let captures: Vec<String> = ctx
			.captures_in_node(&typed_ast.body().nodes[0])
			.into_iter()
			.map(|ident| ident.to_string())
			.collect();

		assert_eq!(captures, vec!["label", "fallback"]);
	}

	#[test]
	fn test_bare_t_macro_assignment_lhs_remains_a_capture() {
		let input = quote::quote!({
			p { { t!(label = fallback) } }
		});
		let untyped_ast: reinhardt_manouche::core::PageMacro = syn::parse2(input).unwrap();
		let typed_ast = crate::page::validator::validate(&untyped_ast).unwrap();
		let ctx = CodegenContext::new(typed_ast.implicit_captures());

		let captures: Vec<String> = ctx
			.captures_in_node(&typed_ast.body().nodes[0])
			.into_iter()
			.map(|ident| ident.to_string())
			.collect();

		assert_eq!(captures, vec!["label", "fallback"]);
	}
}
