//! TUI renderer that converts Page trees to terminal widget trees.

use super::mapper::{DefaultElementMapper, TuiElementMapper};
use super::widget::TuiWidget;
use crate::component::Page;

/// TUI renderer that converts `Page` trees into `TuiWidget` trees.
///
/// The renderer uses a [`TuiElementMapper`] to perform the actual
/// element-to-widget conversion. By default, it uses the
/// [`DefaultElementMapper`].
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::tui::TuiRenderer;
/// use reinhardt_pages::component::{Page, PageElement, IntoPage};
///
/// let page = PageElement::new("div")
///     .child(PageElement::new("h1").child("Dashboard"))
///     .child(PageElement::new("p").child("Welcome!"))
///     .into_page();
///
/// let renderer = TuiRenderer::new();
/// let widget = renderer.render(&page);
/// ```
pub struct TuiRenderer {
	mapper: Box<dyn TuiElementMapper>,
}

impl Default for TuiRenderer {
	fn default() -> Self {
		Self::new()
	}
}

impl TuiRenderer {
	/// Creates a new renderer with the default element mapper.
	pub fn new() -> Self {
		Self {
			mapper: Box::new(DefaultElementMapper::new()),
		}
	}

	/// Creates a new renderer with a custom element mapper.
	pub fn with_mapper(mapper: impl TuiElementMapper + 'static) -> Self {
		Self {
			mapper: Box::new(mapper),
		}
	}

	/// Renders a `Page` tree into a `TuiWidget` tree.
	pub fn render(&self, page: &Page) -> TuiWidget {
		self.mapper.map_page(page)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::component::{IntoPage, PageElement};
	use rstest::rstest;

	#[rstest]
	fn test_renderer_default_creates_successfully() {
		// Arrange & Act
		let renderer = TuiRenderer::new();

		// Assert
		let page = Page::text("test");
		let widget = renderer.render(&page);
		assert!(matches!(widget, TuiWidget::RawText(_)));
	}

	#[rstest]
	fn test_renderer_renders_element_tree() {
		// Arrange
		let renderer = TuiRenderer::new();
		let page = PageElement::new("div")
			.child(PageElement::new("h1").child("Title"))
			.child(PageElement::new("p").child("Body text"))
			.into_page();

		// Act
		let widget = renderer.render(&page);

		// Assert
		if let TuiWidget::Container { children, .. } = &widget {
			assert_eq!(children.len(), 2);
		} else {
			panic!("Expected Container, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_renderer_with_custom_mapper() {
		// Arrange
		struct CustomMapper;
		impl TuiElementMapper for CustomMapper {
			fn map_element(&self, _element: &crate::component::PageElement) -> TuiWidget {
				TuiWidget::RawText("custom".to_string())
			}
			fn map_page(&self, page: &Page) -> TuiWidget {
				match page {
					Page::Element(el) => self.map_element(el),
					_ => TuiWidget::Empty,
				}
			}
		}
		let renderer = TuiRenderer::with_mapper(CustomMapper);
		let page = PageElement::new("div").child("test").into_page();

		// Act
		let widget = renderer.render(&page);

		// Assert
		if let TuiWidget::RawText(text) = &widget {
			assert_eq!(text, "custom");
		} else {
			panic!("Expected custom RawText, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_renderer_default_trait() {
		// Arrange & Act
		let renderer = TuiRenderer::default();

		// Assert
		let widget = renderer.render(&Page::Empty);
		assert!(matches!(widget, TuiWidget::Empty));
	}
}
