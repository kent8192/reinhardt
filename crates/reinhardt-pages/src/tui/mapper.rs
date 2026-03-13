//! Element-to-widget mapping for TUI rendering.

use std::borrow::Cow;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use super::style::StyleConverter;
use super::widget::TuiWidget;
use crate::component::{Page, PageElement};

/// Trait for mapping HTML elements to TUI widgets.
///
/// Implement this trait to provide custom element-to-widget mappings.
/// The default implementation maps standard HTML elements to ratatui widgets.
pub trait TuiElementMapper {
	/// Maps a `PageElement` to a `TuiWidget`.
	fn map_element(&self, element: &PageElement) -> TuiWidget;

	/// Maps a `Page` tree to a `TuiWidget`.
	fn map_page(&self, page: &Page) -> TuiWidget;
}

/// Default element mapper with standard HTML-to-TUI widget mappings.
///
/// | HTML Element | TUI Widget |
/// |-------------|-----------|
/// | `div` | `Container` (Block with borders) |
/// | `p` | `TextBlock` (Paragraph) |
/// | `h1`-`h6` | Bold/styled `TextBlock` |
/// | `ul`, `ol` | `ListWidget` |
/// | `table` | `TableWidget` |
/// | `span` | Inline `TextBlock` |
/// | `button` | Highlighted `TextBlock` |
/// | `input` | `InputField` |
/// | `a` | Underlined colored `TextBlock` |
#[derive(Debug, Clone, Default)]
pub struct DefaultElementMapper;

impl DefaultElementMapper {
	/// Creates a new default mapper.
	pub fn new() -> Self {
		Self
	}

	/// Extracts the inline style from element attributes.
	fn extract_style(element: &PageElement) -> Style {
		let mut style = Style::default();
		for (name, value) in element.attrs() {
			if name.as_ref() == "style" {
				style = StyleConverter::from_inline_style(value);
				break;
			}
		}
		style
	}

	/// Extracts a named attribute from the element.
	fn get_attr<'a>(element: &'a PageElement, attr_name: &str) -> Option<&'a Cow<'static, str>> {
		element
			.attrs()
			.iter()
			.find(|(name, _)| name.as_ref() == attr_name)
			.map(|(_, value)| value)
	}

	/// Collects text content from a page tree recursively.
	fn collect_text(page: &Page) -> String {
		match page {
			Page::Text(text) => text.to_string(),
			Page::Element(el) => el
				.child_views()
				.iter()
				.map(Self::collect_text)
				.collect::<Vec<_>>()
				.join(""),
			Page::Fragment(children) => children
				.iter()
				.map(Self::collect_text)
				.collect::<Vec<_>>()
				.join(""),
			Page::WithHead { view, .. } => Self::collect_text(view),
			Page::Empty => String::new(),
			Page::ReactiveIf(reactive_if) => {
				// Evaluate condition once for static rendering
				let view = if reactive_if.condition() {
					reactive_if.then_view()
				} else {
					reactive_if.else_view()
				};
				Self::collect_text(&view)
			}
			Page::Reactive(reactive) => {
				let view = reactive.render();
				Self::collect_text(&view)
			}
		}
	}

	/// Maps children of an element to TUI widgets.
	fn map_children(&self, element: &PageElement) -> Vec<TuiWidget> {
		element
			.child_views()
			.iter()
			.map(|child| self.map_page(child))
			.filter(|w| !matches!(w, TuiWidget::Empty))
			.collect()
	}

	/// Maps a heading element (h1-h6) to a styled text block.
	fn map_heading(&self, element: &PageElement) -> TuiWidget {
		let mut style = Self::extract_style(element);
		style = style.add_modifier(Modifier::BOLD);

		// Larger headings get additional styling
		let tag = element.tag_name();
		if tag == "h1" {
			style = style.add_modifier(Modifier::REVERSED);
		} else if tag == "h2" {
			style = style.fg(Color::Yellow);
		}

		let text_content = element
			.child_views()
			.iter()
			.map(Self::collect_text)
			.collect::<Vec<_>>()
			.join("");

		TuiWidget::TextBlock {
			content: Text::from(Line::from(Span::styled(text_content, style))),
			style: Style::default(),
		}
	}
}

impl TuiElementMapper for DefaultElementMapper {
	fn map_element(&self, element: &PageElement) -> TuiWidget {
		let tag = element.tag_name();
		let style = Self::extract_style(element);

		match tag {
			"div" | "section" | "article" | "main" | "header" | "footer" | "nav" | "aside" => {
				let title = Self::get_attr(element, "title")
					.or_else(|| Self::get_attr(element, "id"))
					.map(|v| v.to_string());
				let children = self.map_children(element);
				TuiWidget::Container {
					title,
					style,
					children,
				}
			}
			"p" => {
				let text = element
					.child_views()
					.iter()
					.map(DefaultElementMapper::collect_text)
					.collect::<Vec<_>>()
					.join("");
				TuiWidget::TextBlock {
					content: Text::from(text),
					style,
				}
			}
			"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => self.map_heading(element),
			"ul" | "ol" => {
				let title = Self::get_attr(element, "title").map(|v| v.to_string());
				let items: Vec<Text<'static>> = element
					.child_views()
					.iter()
					.filter_map(|child| {
						if let Page::Element(li) = child
							&& li.tag_name() == "li"
						{
							let text = DefaultElementMapper::collect_text(child);
							return Some(Text::from(text));
						}
						// Fallback: render any child as text
						let text = DefaultElementMapper::collect_text(child);
						if text.is_empty() {
							None
						} else {
							Some(Text::from(text))
						}
					})
					.collect();
				TuiWidget::ListWidget {
					items,
					title,
					style,
				}
			}
			"table" => {
				let mut header = Vec::new();
				let mut rows = Vec::new();

				for child in element.child_views() {
					if let Page::Element(section) = child {
						match section.tag_name() {
							"thead" => {
								for row_page in section.child_views() {
									if let Page::Element(tr) = row_page {
										for cell_page in tr.child_views() {
											if let Page::Element(_) = cell_page {
												header.push(DefaultElementMapper::collect_text(
													cell_page,
												));
											}
										}
									}
								}
							}
							"tbody" => {
								for row_page in section.child_views() {
									if let Page::Element(tr) = row_page {
										let row: Vec<String> = tr
											.child_views()
											.iter()
											.map(DefaultElementMapper::collect_text)
											.collect();
										rows.push(row);
									}
								}
							}
							"tr" => {
								// Direct tr children (no thead/tbody wrapper)
								let row: Vec<String> = section
									.child_views()
									.iter()
									.map(DefaultElementMapper::collect_text)
									.collect();
								if header.is_empty() {
									// First tr without thead is treated as header
									header = row;
								} else {
									rows.push(row);
								}
							}
							_ => {}
						}
					}
				}

				TuiWidget::TableWidget {
					header,
					rows,
					style,
				}
			}
			"span" => {
				let text = element
					.child_views()
					.iter()
					.map(DefaultElementMapper::collect_text)
					.collect::<Vec<_>>()
					.join("");
				TuiWidget::TextBlock {
					content: Text::from(text),
					style,
				}
			}
			"a" => {
				let text = element
					.child_views()
					.iter()
					.map(DefaultElementMapper::collect_text)
					.collect::<Vec<_>>()
					.join("");
				let href = Self::get_attr(element, "href")
					.map(|v| format!(" ({})", v))
					.unwrap_or_default();
				let display = format!("{}{}", text, href);
				let link_style = style
					.fg(Color::LightBlue)
					.add_modifier(Modifier::UNDERLINED);
				TuiWidget::TextBlock {
					content: Text::from(Line::from(Span::styled(display, link_style))),
					style: Style::default(),
				}
			}
			"button" => {
				let text = element
					.child_views()
					.iter()
					.map(DefaultElementMapper::collect_text)
					.collect::<Vec<_>>()
					.join("");
				let btn_style = style.add_modifier(Modifier::REVERSED);
				TuiWidget::TextBlock {
					content: Text::from(Line::from(Span::styled(
						format!("[ {} ]", text),
						btn_style,
					))),
					style: Style::default(),
				}
			}
			"input" => {
				let placeholder = Self::get_attr(element, "placeholder")
					.map(|v| v.to_string())
					.unwrap_or_default();
				let value = Self::get_attr(element, "value")
					.map(|v| v.to_string())
					.unwrap_or_default();
				TuiWidget::InputField {
					placeholder,
					value,
					style,
				}
			}
			"textarea" => {
				let placeholder = Self::get_attr(element, "placeholder")
					.map(|v| v.to_string())
					.unwrap_or_default();
				let text = element
					.child_views()
					.iter()
					.map(DefaultElementMapper::collect_text)
					.collect::<Vec<_>>()
					.join("");
				TuiWidget::InputField {
					placeholder,
					value: text,
					style,
				}
			}
			"br" | "hr" => TuiWidget::RawText("---".to_string()),
			"img" => {
				let alt = Self::get_attr(element, "alt")
					.map(|v| v.to_string())
					.unwrap_or_else(|| "[image]".to_string());
				TuiWidget::RawText(format!("[{}]", alt))
			}
			"select" => {
				let items: Vec<Text<'static>> = element
					.child_views()
					.iter()
					.filter_map(|child| {
						let text = DefaultElementMapper::collect_text(child);
						if text.is_empty() {
							None
						} else {
							Some(Text::from(text))
						}
					})
					.collect();
				TuiWidget::ListWidget {
					items,
					title: Some("Select".to_string()),
					style,
				}
			}
			"form" => {
				let title = Self::get_attr(element, "action")
					.map(|v| format!("Form: {}", v))
					.or_else(|| Some("Form".to_string()));
				let children = self.map_children(element);
				TuiWidget::Container {
					title,
					style,
					children,
				}
			}
			"label" => {
				let text = element
					.child_views()
					.iter()
					.map(DefaultElementMapper::collect_text)
					.collect::<Vec<_>>()
					.join("");
				TuiWidget::TextBlock {
					content: Text::from(Line::from(Span::styled(
						text,
						style.add_modifier(Modifier::BOLD),
					))),
					style: Style::default(),
				}
			}
			_ => {
				// Graceful fallback: render unsupported elements as plain text
				let text = element
					.child_views()
					.iter()
					.map(DefaultElementMapper::collect_text)
					.collect::<Vec<_>>()
					.join("");
				if text.is_empty() {
					TuiWidget::Empty
				} else {
					TuiWidget::RawText(text)
				}
			}
		}
	}

	fn map_page(&self, page: &Page) -> TuiWidget {
		match page {
			Page::Element(el) => self.map_element(el),
			Page::Text(text) => TuiWidget::RawText(text.to_string()),
			Page::Fragment(children) => {
				let widgets: Vec<TuiWidget> = children
					.iter()
					.map(|child| self.map_page(child))
					.filter(|w| !matches!(w, TuiWidget::Empty))
					.collect();
				if widgets.is_empty() {
					TuiWidget::Empty
				} else if widgets.len() == 1 {
					widgets.into_iter().next().expect("checked non-empty")
				} else {
					TuiWidget::Group(widgets)
				}
			}
			Page::Empty => TuiWidget::Empty,
			Page::WithHead { view, .. } => {
				// TUI ignores head metadata; render only the view content
				self.map_page(view)
			}
			Page::ReactiveIf(reactive_if) => {
				// Evaluate condition once for static TUI rendering
				let view = if reactive_if.condition() {
					reactive_if.then_view()
				} else {
					reactive_if.else_view()
				};
				self.map_page(&view)
			}
			Page::Reactive(reactive) => {
				let view = reactive.render();
				self.map_page(&view)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::component::PageElement;
	use rstest::rstest;

	#[rstest]
	fn test_map_div_to_container() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("div").child("Hello");

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		assert!(matches!(widget, TuiWidget::Container { .. }));
	}

	#[rstest]
	fn test_map_p_to_text_block() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("p").child("Paragraph text");

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		if let TuiWidget::TextBlock { content, .. } = &widget {
			let rendered: String = content
				.lines
				.iter()
				.flat_map(|l| l.spans.iter())
				.map(|s| s.content.as_ref())
				.collect();
			assert_eq!(rendered, "Paragraph text");
		} else {
			panic!("Expected TextBlock, got {:?}", widget);
		}
	}

	#[rstest]
	#[case("h1")]
	#[case("h2")]
	#[case("h3")]
	#[case("h4")]
	#[case("h5")]
	#[case("h6")]
	fn test_map_headings_to_bold_text(#[case] tag: &str) {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new(tag.to_string()).child("Heading");

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		assert!(matches!(widget, TuiWidget::TextBlock { .. }));
	}

	#[rstest]
	fn test_map_ul_to_list() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("ul")
			.child(PageElement::new("li").child("Item 1"))
			.child(PageElement::new("li").child("Item 2"));

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		if let TuiWidget::ListWidget { items, .. } = &widget {
			assert_eq!(items.len(), 2);
		} else {
			panic!("Expected ListWidget, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_map_ol_to_list() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("ol")
			.child(PageElement::new("li").child("First"))
			.child(PageElement::new("li").child("Second"));

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		assert!(matches!(widget, TuiWidget::ListWidget { .. }));
	}

	#[rstest]
	fn test_map_table_with_thead_tbody() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("table")
			.child(
				PageElement::new("thead").child(
					PageElement::new("tr")
						.child(PageElement::new("th").child("Name"))
						.child(PageElement::new("th").child("Age")),
				),
			)
			.child(
				PageElement::new("tbody")
					.child(
						PageElement::new("tr")
							.child(PageElement::new("td").child("Alice"))
							.child(PageElement::new("td").child("30")),
					)
					.child(
						PageElement::new("tr")
							.child(PageElement::new("td").child("Bob"))
							.child(PageElement::new("td").child("25")),
					),
			);

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		if let TuiWidget::TableWidget { header, rows, .. } = &widget {
			assert_eq!(header, &["Name", "Age"]);
			assert_eq!(rows.len(), 2);
			assert_eq!(rows[0], &["Alice", "30"]);
			assert_eq!(rows[1], &["Bob", "25"]);
		} else {
			panic!("Expected TableWidget, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_map_span_to_text() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("span").child("inline text");

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		assert!(matches!(widget, TuiWidget::TextBlock { .. }));
	}

	#[rstest]
	fn test_map_button_to_styled_text() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("button").child("Click me");

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		if let TuiWidget::TextBlock { content, .. } = &widget {
			let rendered: String = content
				.lines
				.iter()
				.flat_map(|l| l.spans.iter())
				.map(|s| s.content.as_ref())
				.collect();
			assert!(rendered.contains("Click me"));
			assert!(rendered.contains("["));
		} else {
			panic!("Expected TextBlock, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_map_a_to_underlined_text() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("a")
			.attr("href", "https://example.com")
			.child("Example");

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		if let TuiWidget::TextBlock { content, .. } = &widget {
			let rendered: String = content
				.lines
				.iter()
				.flat_map(|l| l.spans.iter())
				.map(|s| s.content.as_ref())
				.collect();
			assert!(rendered.contains("Example"));
			assert!(rendered.contains("https://example.com"));
		} else {
			panic!("Expected TextBlock, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_map_input_to_input_field() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("input")
			.attr("placeholder", "Enter name")
			.attr("value", "John");

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		if let TuiWidget::InputField {
			placeholder, value, ..
		} = &widget
		{
			assert_eq!(placeholder, "Enter name");
			assert_eq!(value, "John");
		} else {
			panic!("Expected InputField, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_map_unknown_element_falls_back_to_text() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("custom-element").child("Custom content");

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		if let TuiWidget::RawText(text) = &widget {
			assert_eq!(text, "Custom content");
		} else {
			panic!("Expected RawText fallback, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_map_empty_page() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let page = Page::Empty;

		// Act
		let widget = mapper.map_page(&page);

		// Assert
		assert!(matches!(widget, TuiWidget::Empty));
	}

	#[rstest]
	fn test_map_text_page() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let page = Page::text("Hello TUI");

		// Act
		let widget = mapper.map_page(&page);

		// Assert
		if let TuiWidget::RawText(text) = &widget {
			assert_eq!(text, "Hello TUI");
		} else {
			panic!("Expected RawText, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_map_fragment_page() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let page = Page::fragment(["A", "B", "C"]);

		// Act
		let widget = mapper.map_page(&page);

		// Assert
		assert!(matches!(widget, TuiWidget::Group(_)));
	}

	#[rstest]
	fn test_map_with_head_ignores_head() {
		// Arrange
		use crate::component::Head;
		let mapper = DefaultElementMapper::new();
		let page = Page::text("Content").with_head(Head::new().title("Title"));

		// Act
		let widget = mapper.map_page(&page);

		// Assert
		if let TuiWidget::RawText(text) = &widget {
			assert_eq!(text, "Content");
		} else {
			panic!("Expected RawText, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_map_div_with_inline_style() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("div")
			.attr("style", "color: red")
			.child("Styled");

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		if let TuiWidget::Container { style, .. } = &widget {
			assert_eq!(style.fg, Some(Color::Red));
		} else {
			panic!("Expected Container, got {:?}", widget);
		}
	}

	#[rstest]
	fn test_map_img_fallback() {
		// Arrange
		let mapper = DefaultElementMapper::new();
		let element = PageElement::new("img").attr("alt", "Logo");

		// Act
		let widget = mapper.map_element(&element);

		// Assert
		if let TuiWidget::RawText(text) = &widget {
			assert_eq!(text, "[Logo]");
		} else {
			panic!("Expected RawText, got {:?}", widget);
		}
	}
}
