//! Component trait definition.

use super::into_page::{IntoPage, Page};

/// Trait for reusable UI components.
///
/// Components are the building blocks of the UI. They encapsulate
/// state and rendering logic into reusable units.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::component::{Component, View, IntoPage};
///
/// struct Greeting {
///     name: String,
/// }
///
/// impl Component for Greeting {
///     fn render(&self) -> Page {
///         Page::element("div")
///             .attr("class", "greeting")
///             .child(format!("Hello, {}!", self.name))
///             .into_page()
///     }
///
///     fn name() -> &'static str {
///         "Greeting"
///     }
/// }
/// ```
pub trait Component: 'static {
	/// Renders the component to a Page.
	fn render(&self) -> Page;

	/// Returns the component's name for debugging and hydration.
	fn name() -> &'static str
	where
		Self: Sized;
}

// Note: Blanket implementation of IntoPage for Component was removed
// because IntoPage is now defined in reinhardt-types (external crate).
// Components can:
// 1. Manually implement IntoPage: impl IntoPage for MyComponent { fn into_page(self) -> Page { self.render() } }
// 2. Use render() directly: component.render() returns Page

/// A boxed component for dynamic dispatch.
#[allow(dead_code)]
pub(super) struct DynComponent {
	inner: Box<dyn Component>,
	name: &'static str,
}

// Allow dead_code: impl block for DynComponent reserved for future dynamic dispatch
#[allow(dead_code)]
impl DynComponent {
	/// Creates a new dynamic component.
	pub(super) fn new<T: Component>(component: T) -> Self {
		Self {
			inner: Box::new(component),
			name: T::name(),
		}
	}

	/// Returns the component's name.
	pub(super) fn name(&self) -> &'static str {
		self.name
	}

	/// Renders the component.
	pub(super) fn render(&self) -> Page {
		self.inner.render()
	}
}

impl std::fmt::Debug for DynComponent {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DynComponent")
			.field("name", &self.name)
			.finish()
	}
}

/// Helper trait for creating components from functions.
///
/// This allows simple functions to be used as components.
#[allow(dead_code)]
pub(super) trait FnComponent<Args> {
	/// The output type of the function.
	type Output: IntoPage;

	/// Calls the function to produce a view.
	fn call(&self, args: Args) -> Self::Output;
}

// Implementation for functions with no arguments
impl<F, O> FnComponent<()> for F
where
	F: Fn() -> O,
	O: IntoPage,
{
	type Output = O;

	fn call(&self, _args: ()) -> Self::Output {
		self()
	}
}

// Implementation for functions with one argument
impl<F, A, O> FnComponent<(A,)> for F
where
	F: Fn(A) -> O,
	O: IntoPage,
{
	type Output = O;

	fn call(&self, args: (A,)) -> Self::Output {
		self(args.0)
	}
}

// Implementation for functions with two arguments
impl<F, A, B, O> FnComponent<(A, B)> for F
where
	F: Fn(A, B) -> O,
	O: IntoPage,
{
	type Output = O;

	fn call(&self, args: (A, B)) -> Self::Output {
		self(args.0, args.1)
	}
}

#[cfg(test)]
#[allow(unstable_name_collisions)]
mod tests {
	use super::*;
	use crate::component::into_page::PageElement;

	struct TestComponent {
		message: String,
	}

	impl Component for TestComponent {
		fn render(&self) -> Page {
			PageElement::new("div")
				.child(self.message.clone())
				.into_page()
		}

		fn name() -> &'static str {
			"TestComponent"
		}
	}

	#[test]
	fn test_component_render() {
		let comp = TestComponent {
			message: "Hello".to_string(),
		};
		let view = comp.render();
		assert_eq!(view.render_to_string(), "<div>Hello</div>");
	}

	#[test]
	fn test_component_name() {
		assert_eq!(TestComponent::name(), "TestComponent");
	}

	#[test]
	fn test_component_render_returns_page() {
		let comp = TestComponent {
			message: "World".to_string(),
		};
		// Component::render() returns Page directly
		let view: Page = comp.render();
		assert_eq!(view.render_to_string(), "<div>World</div>");
	}

	#[test]
	fn test_dyn_component() {
		let comp = TestComponent {
			message: "Dynamic".to_string(),
		};
		let dyn_comp = DynComponent::new(comp);
		assert_eq!(dyn_comp.name(), "TestComponent");
		assert_eq!(dyn_comp.render().render_to_string(), "<div>Dynamic</div>");
	}

	#[test]
	fn test_fn_component_no_args() {
		fn greeting() -> Page {
			Page::text("Hello")
		}

		let output = greeting.call(());
		assert_eq!(output.render_to_string(), "Hello");
	}

	#[test]
	fn test_fn_component_one_arg() {
		fn greeting(name: String) -> Page {
			Page::text(format!("Hello, {}!", name))
		}

		let output = greeting.call(("World".to_string(),));
		assert_eq!(output.render_to_string(), "Hello, World!");
	}

	#[test]
	fn test_fn_component_two_args() {
		fn greeting(first: String, last: String) -> Page {
			Page::text(format!("Hello, {} {}!", first, last))
		}

		let output = greeting.call(("John".to_string(), "Doe".to_string()));
		assert_eq!(output.render_to_string(), "Hello, John Doe!");
	}
}
