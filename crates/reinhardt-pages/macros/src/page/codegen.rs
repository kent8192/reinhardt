//! Code generation for the page! macro.
//!
//! This module converts typed AST nodes into Rust code that uses the ElementView API.
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
//!     |initial: i32| -> View {
//!         ElementView::new("div")
//!             .child("hello")
//!             .into_view()
//!     }
//! }
//! ```

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::LitStr;

// Import AST types from reinhardt-manouche
use crate::crate_paths::get_reinhardt_pages_crate_info;
use reinhardt_manouche::core::types::AttrValue;
use reinhardt_manouche::core::{
	PageEvent, PageExpression, PageParam, PageText, TypedPageAttr, TypedPageBody,
	TypedPageComponent, TypedPageElement, TypedPageElse, TypedPageFor, TypedPageIf, TypedPageMacro,
	TypedPageNode, TypedPageWatch,
};

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

	// Generate closure parameters
	let params = generate_params(&macro_ast.params);

	// Generate body
	let body = generate_body(&macro_ast.body, pages_crate);

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

	// Wrap in a closure with conditional use statement if needed
	quote! {
		{
			#use_statement
			#params -> #pages_crate::component::Page {
				#body_with_head
			}
		}
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
fn generate_body(body: &TypedPageBody, pages_crate: &TokenStream) -> TokenStream {
	let nodes = generate_nodes(&body.nodes, pages_crate);

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
fn generate_nodes(nodes: &[TypedPageNode], pages_crate: &TokenStream) -> TokenStream {
	let node_tokens: Vec<TokenStream> = nodes
		.iter()
		.map(|n| generate_node(n, pages_crate))
		.collect();

	if node_tokens.len() == 1 {
		node_tokens.into_iter().next().unwrap()
	} else {
		quote!(#(#node_tokens),*)
	}
}

/// Generates code for a single node.
fn generate_node(node: &TypedPageNode, pages_crate: &TokenStream) -> TokenStream {
	match node {
		TypedPageNode::Element(elem) => generate_element(elem, pages_crate),
		TypedPageNode::Text(text) => generate_text(text, pages_crate),
		TypedPageNode::Expression(expr) => generate_expression(expr, pages_crate),
		TypedPageNode::If(if_node) => generate_if(if_node, pages_crate),
		TypedPageNode::For(for_node) => generate_for(for_node, pages_crate),
		TypedPageNode::Component(comp) => generate_component(comp, pages_crate),
		TypedPageNode::Watch(watch_node) => generate_watch(watch_node, pages_crate),
	}
}

/// Generates code for an element node.
///
/// When the element has event handlers, this function generates conditional compilation
/// code that:
/// - On WASM targets: Binds event handlers to DOM events
/// - On native targets: Suppresses unused variable warnings for captured variables
///
/// This allows users to write event handlers once without manual `#[cfg]` annotations.
fn generate_element(elem: &TypedPageElement, pages_crate: &TokenStream) -> TokenStream {
	let tag = elem.tag.to_string();

	// Generate attributes
	let attrs: Vec<TokenStream> = elem.attrs.iter().map(generate_attr).collect();

	// Generate children
	let children: Vec<TokenStream> = elem
		.children
		.iter()
		.map(|child| generate_child(child, pages_crate))
		.collect();

	// Build the base element (attributes and children, without events)
	let mut base_builder = quote! {
		#pages_crate::component::PageElement::new(#tag)
	};

	// Add attributes
	for attr in &attrs {
		base_builder = quote! {
			#base_builder
			#attr
		};
	}

	// Add children
	for child in &children {
		base_builder = quote! {
			#base_builder
			.child(#child)
		};
	}

	// Fast path: no events - simple generation (preserves current behavior)
	if elem.events.is_empty() {
		return quote! {
			#pages_crate::component::IntoPage::into_page(#base_builder)
		};
	}

	// Has events - generate conditional compilation code
	// This eliminates the need for users to write #[cfg(target_arch = "wasm32")] blocks

	// Generate event bindings for WASM target
	let event_bindings: Vec<TokenStream> = elem
		.events
		.iter()
		.map(|event| generate_event(event, pages_crate))
		.collect();

	// Generate typed wrappers for non-WASM to enable closure type inference.
	// We wrap each handler in a typed closure that calls it, which forces Rust to
	// infer the closure parameter type from the wrapper's explicit type annotation.
	//
	// For Callback types and other non-closure handlers, we use into_event_handler
	// to convert them first, since they can't be called directly.
	let handler_exprs: Vec<&syn::Expr> = elem.events.iter().map(|event| &event.handler).collect();
	let typed_handler_refs: Vec<TokenStream> = handler_exprs
		.iter()
		.map(|handler| {
			// Check if the handler is a closure expression
			if matches!(handler, syn::Expr::Closure(_)) {
				// For closures, wrap in a typed closure to enable type inference
				quote! {
					{
						let __typed_wrapper = |__e: #pages_crate::component::DummyEvent| {
							(#handler)(__e)
						};
						let _ = &__typed_wrapper;
					}
				}
			} else {
				// For non-closure handlers (Callback, variables, etc.),
				// convert to ViewEventHandler first then reference it
				quote! {
					{
						let __vh = #pages_crate::callback::into_event_handler(#handler);
						let _ = &__vh;
					}
				}
			}
		})
		.collect();

	quote! {
		{
			let __elem_base = #base_builder;

			#[cfg(target_arch = "wasm32")]
			let __elem_with_events = __elem_base #(#event_bindings)*;

			#[cfg(not(target_arch = "wasm32"))]
			let __elem_with_events = {
				// Create typed wrappers to enable closure parameter type inference.
				// The wrapper calls the user's handler with a typed argument, which forces
				// Rust to infer the closure parameter type.
				#(#typed_handler_refs)*
				__elem_base
			};

			#pages_crate::component::IntoPage::into_page(__elem_with_events)
		}
	}
}

/// Boolean attributes that should use `.bool_attr()` method.
/// These attributes are either present or absent in HTML, not string-valued.
const BOOLEAN_ATTRS: &[&str] = &[
	"disabled",
	"required",
	"readonly",
	"checked",
	"selected",
	"autofocus",
	"autoplay",
	"controls",
	"loop",
	"muted",
	"default",
	"defer",
	"formnovalidate",
	"hidden",
	"ismap",
	"multiple",
	"novalidate",
	"open",
	"reversed",
];

/// Generates code for an attribute.
fn generate_attr(attr: &TypedPageAttr) -> TokenStream {
	let name_str = attr.html_name();

	// Check if this is a boolean attribute
	if BOOLEAN_ATTRS.contains(&name_str.as_str()) {
		// Boolean attributes use .bool_attr() which handles true/false values
		let value_expr = attr.value.to_expr();
		return quote! {
			.bool_attr(#name_str, #value_expr)
		};
	}

	// Use name_str for regular attributes as well
	let name = &name_str;

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
		_ => {
			// For StringLit, BoolLit, Dynamic: use as-is
			let expr = attr.value.to_expr();
			quote! { #expr }
		}
	};

	quote! {
		.attr(#name, #value_expr)
	}
}

/// Checks if an expression is an async closure.
fn is_async_closure(expr: &syn::Expr) -> bool {
	match expr {
		syn::Expr::Closure(closure) => closure.asyncness.is_some(),
		_ => false,
	}
}

/// Generates code for an event handler.
///
/// This function generates platform-aware code that handles event handler type inference.
/// The key challenge is that Rust cannot infer closure parameter types from `impl Fn(Event)`
/// bounds or type annotations on Box.
///
/// The solution is to wrap the handler in a typed closure that explicitly calls the handler.
/// This works because calling `(#handler)(__event)` where `__event` is typed forces Rust
/// to infer that `#handler` implements `Fn(EventType)`, thereby typing the closure parameter.
fn generate_event(event: &PageEvent, pages_crate: &TokenStream) -> TokenStream {
	let event_type = event.dom_event_type();
	let handler = &event.handler;

	// Convert event type string to EventType enum variant
	// NOTE: Variant names must match exactly with dom::EventType definition
	let event_type_ident = match event_type.as_str() {
		// Mouse events
		"click" => quote!(Click),
		"dblclick" => quote!(DblClick),
		"mousedown" => quote!(MouseDown),
		"mouseup" => quote!(MouseUp),
		"mouseenter" => quote!(MouseEnter),
		"mouseleave" => quote!(MouseLeave),
		"mousemove" => quote!(MouseMove),
		"mouseover" => quote!(MouseOver),
		"mouseout" => quote!(MouseOut),
		// Keyboard events
		"keydown" => quote!(KeyDown),
		"keyup" => quote!(KeyUp),
		"keypress" => quote!(KeyPress),
		// Form events
		"input" => quote!(Input),
		"change" => quote!(Change),
		"submit" => quote!(Submit),
		"focus" => quote!(Focus),
		"blur" => quote!(Blur),
		// Touch events
		"touchstart" => quote!(TouchStart),
		"touchend" => quote!(TouchEnd),
		"touchmove" => quote!(TouchMove),
		"touchcancel" => quote!(TouchCancel),
		// Drag events
		"dragstart" => quote!(DragStart),
		"drag" => quote!(Drag),
		"drop" => quote!(Drop),
		"dragenter" => quote!(DragEnter),
		"dragleave" => quote!(DragLeave),
		"dragover" => quote!(DragOver),
		"dragend" => quote!(DragEnd),
		// Other events
		"load" => quote!(Load),
		"error" => quote!(Error),
		"scroll" => quote!(Scroll),
		"resize" => quote!(Resize),
		other => {
			// Unsupported event type - emit compile error
			let error_msg = format!("unsupported event type: '{}'", other);
			return quote! {
				compile_error!(#error_msg)
			};
		}
	};

	// ✅ NEW: Async closure detection
	if is_async_closure(handler) {
		// Automatically wrap async closures in async_handler
		return quote! {
			.on(
				#pages_crate::dom::EventType::#event_type_ident,
				#pages_crate::callback::async_handler(#handler)
			)
		};
	}

	// Generate event handler code.
	// For closure expressions, we use a typed wrapper to enable type inference.
	// For non-closure handlers (Callback, variables), we use into_event_handler.
	if matches!(handler, syn::Expr::Closure(_)) {
		// For closures, wrap in a typed closure to enable type inference.
		// The wrapper calls the user's closure with a typed argument, which forces
		// Rust to infer the closure parameter type.
		quote! {
			.on(
				#pages_crate::dom::EventType::#event_type_ident,
				{
					#[cfg(target_arch = "wasm32")]
					let __typed_wrapper = |__event: ::web_sys::Event| {
						(#handler)(__event)
					};

					#[cfg(not(target_arch = "wasm32"))]
					let __typed_wrapper = |__event: #pages_crate::component::DummyEvent| {
						(#handler)(__event)
					};

					::std::sync::Arc::new(__typed_wrapper)
				}
			)
		}
	} else {
		// For non-closure handlers (Callback, variables, etc.),
		// use into_event_handler which handles all handler types correctly.
		quote! {
			.on(
				#pages_crate::dom::EventType::#event_type_ident,
				#pages_crate::callback::into_event_handler(#handler)
			)
		}
	}
}

/// Generates code for a child node (used in .child() calls).
fn generate_child(node: &TypedPageNode, pages_crate: &TokenStream) -> TokenStream {
	match node {
		TypedPageNode::Text(text) => {
			// Create a proper string literal token
			let lit = LitStr::new(&text.content, Span::call_site());
			quote!(#lit)
		}
		TypedPageNode::Expression(expr) => {
			let e = &expr.expr;
			quote!(#e)
		}
		_ => generate_node(node, pages_crate),
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
fn generate_expression(expr: &PageExpression, pages_crate: &TokenStream) -> TokenStream {
	let e = &expr.expr;
	quote! {
		#pages_crate::component::IntoPage::into_page(#e)
	}
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
fn generate_if(if_node: &TypedPageIf, pages_crate: &TokenStream) -> TokenStream {
	let condition = &if_node.condition;
	let then_branch = generate_if_branch(&if_node.then_branch, pages_crate);

	let else_branch = match &if_node.else_branch {
		Some(TypedPageElse::Block(nodes)) => {
			// else { ... } block - generate view directly
			generate_if_branch(nodes, pages_crate)
		}
		Some(TypedPageElse::If(nested_if)) => {
			// else if { ... } - recursively generate another if
			generate_if(nested_if, pages_crate)
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
fn generate_if_branch(nodes: &[TypedPageNode], pages_crate: &TokenStream) -> TokenStream {
	if nodes.is_empty() {
		quote! { #pages_crate::component::Page::Empty }
	} else if nodes.len() == 1 {
		generate_node(&nodes[0], pages_crate)
	} else {
		let node_tokens: Vec<TokenStream> = nodes
			.iter()
			.map(|n| generate_node(n, pages_crate))
			.collect();
		quote! {
			#pages_crate::component::Page::fragment([#(#node_tokens),*])
		}
	}
}

/// Generates code for a for node.
fn generate_for(for_node: &TypedPageFor, pages_crate: &TokenStream) -> TokenStream {
	let pat = &for_node.pat;
	let iter = &for_node.iter;
	let body = generate_if_branch(&for_node.body, pages_crate);

	quote! {
		#pages_crate::component::Page::fragment(
			(#iter).into_iter().map(|#pat| {
				#body
			}).collect::<::std::vec::Vec<_>>()
		)
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
///         ElementView::new("div").child("Positive").into_view()
///     } else {
///         ElementView::new("div").child("Non-positive").into_view()
///     }
/// })
/// ```
fn generate_watch(watch_node: &TypedPageWatch, pages_crate: &TokenStream) -> TokenStream {
	let inner_expr = generate_node(&watch_node.expr, pages_crate);

	quote! {
		#pages_crate::component::Page::reactive(move || {
			#inner_expr
		})
	}
}

/// Generates code for a component call.
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
fn generate_component(comp: &TypedPageComponent, pages_crate: &TokenStream) -> TokenStream {
	let name = &comp.name;

	// Generate argument values (names are discarded in generated code)
	let args: Vec<TokenStream> = comp
		.args
		.iter()
		.map(|arg| {
			let value = &arg.value;
			quote! { #value }
		})
		.collect();

	// Generate the component call
	if let Some(children) = &comp.children {
		// With children: add children as last argument
		let children_view = generate_if_branch(children, pages_crate);

		if args.is_empty() {
			quote! { #name(#children_view) }
		} else {
			quote! { #name(#(#args),*, #children_view) }
		}
	} else {
		// Without children: simple function call
		if args.is_empty() {
			quote! { #name() }
		} else {
			quote! { #name(#(#args),*) }
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

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
		assert!(output_str.contains(". attr"));
		assert!(output_str.contains("\"class\""));
		assert!(output_str.contains("\"container\""));
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
}
