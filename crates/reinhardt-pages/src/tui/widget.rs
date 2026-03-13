//! TUI widget representation for rendered Page elements.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table};

/// A terminal widget produced by mapping a `Page` element.
///
/// Each variant corresponds to a ratatui widget type that can be rendered
/// to a terminal frame.
#[derive(Debug, Clone)]
pub enum TuiWidget {
	/// A block container (mapped from `div`).
	Container {
		/// Optional title derived from element attributes.
		title: Option<String>,
		/// Style applied to the block border and background.
		style: Style,
		/// Child widgets rendered inside the block.
		children: Vec<TuiWidget>,
	},
	/// A text paragraph (mapped from `p`, `h1`-`h6`, `span`, `a`, `button`).
	TextBlock {
		/// The text content.
		content: Text<'static>,
		/// Style applied to the paragraph.
		style: Style,
	},
	/// A list widget (mapped from `ul`, `ol`).
	ListWidget {
		/// List items as styled text.
		items: Vec<Text<'static>>,
		/// Optional title for the list block.
		title: Option<String>,
		/// Style for the list container.
		style: Style,
	},
	/// A table widget (mapped from `table`).
	TableWidget {
		/// Header row (from `thead > tr > th`).
		header: Vec<String>,
		/// Body rows (from `tbody > tr > td`).
		rows: Vec<Vec<String>>,
		/// Style for the table container.
		style: Style,
	},
	/// An input field widget (mapped from `input`).
	InputField {
		/// Placeholder text.
		placeholder: String,
		/// Current value.
		value: String,
		/// Style for the input.
		style: Style,
	},
	/// Raw text node (no wrapper element).
	RawText(String),
	/// A group of widgets rendered sequentially (for fragments).
	Group(Vec<TuiWidget>),
	/// An empty widget (renders nothing).
	Empty,
}

impl TuiWidget {
	/// Renders this widget tree into a ratatui frame at the given area.
	pub fn render_to_frame(&self, frame: &mut Frame, area: Rect) {
		match self {
			TuiWidget::Container {
				title,
				style,
				children,
			} => {
				let mut block = Block::default().borders(Borders::ALL).style(*style);
				if let Some(t) = title {
					block = block.title(t.as_str());
				}
				let inner = block.inner(area);
				frame.render_widget(block, area);
				render_children(frame, inner, children);
			}
			TuiWidget::TextBlock { content, style } => {
				let paragraph = Paragraph::new(content.clone()).style(*style);
				frame.render_widget(paragraph, area);
			}
			TuiWidget::ListWidget {
				items,
				title,
				style,
			} => {
				let list_items: Vec<ListItem> =
					items.iter().map(|t| ListItem::new(t.clone())).collect();
				let mut block = Block::default().borders(Borders::ALL).style(*style);
				if let Some(t) = title {
					block = block.title(t.as_str());
				}
				let list = List::new(list_items).block(block);
				frame.render_widget(list, area);
			}
			TuiWidget::TableWidget {
				header,
				rows,
				style,
			} => {
				let header_row = Row::new(header.iter().map(|h| h.as_str()).collect::<Vec<&str>>());
				let body_rows: Vec<Row> = rows
					.iter()
					.map(|row| {
						Row::new(row.iter().map(|cell| cell.as_str()).collect::<Vec<&str>>())
					})
					.collect();
				let col_count = header.len().max(1);
				let widths: Vec<Constraint> = (0..col_count).map(|_| Constraint::Fill(1)).collect();
				let table = Table::new(body_rows, widths)
					.header(header_row)
					.block(Block::default().borders(Borders::ALL).style(*style));
				frame.render_widget(table, area);
			}
			TuiWidget::InputField {
				placeholder,
				value,
				style,
			} => {
				let display = if value.is_empty() {
					placeholder.as_str()
				} else {
					value.as_str()
				};
				let paragraph = Paragraph::new(display)
					.style(*style)
					.block(Block::default().borders(Borders::ALL));
				frame.render_widget(paragraph, area);
			}
			TuiWidget::RawText(text) => {
				let paragraph = Paragraph::new(text.as_str());
				frame.render_widget(paragraph, area);
			}
			TuiWidget::Group(widgets) => {
				render_children(frame, area, widgets);
			}
			TuiWidget::Empty => {}
		}
	}

	/// Estimates the minimum height needed for this widget.
	pub fn estimated_height(&self) -> u16 {
		match self {
			TuiWidget::Container { children, .. } => {
				// Border top + border bottom + children
				let child_height: u16 = children.iter().map(|c| c.estimated_height()).sum();
				child_height + 2
			}
			TuiWidget::TextBlock { content, .. } => content.lines.len().max(1) as u16,
			TuiWidget::ListWidget { items, .. } => {
				// Border (2) + items
				items.len() as u16 + 2
			}
			TuiWidget::TableWidget { rows, .. } => {
				// Border (2) + header (1) + rows
				rows.len() as u16 + 3
			}
			TuiWidget::InputField { .. } => 3, // Border (2) + content (1)
			TuiWidget::RawText(_) => 1,
			TuiWidget::Group(widgets) => widgets.iter().map(|w| w.estimated_height()).sum(),
			TuiWidget::Empty => 0,
		}
	}
}

/// Renders a list of child widgets vertically within the given area.
fn render_children(frame: &mut Frame, area: Rect, children: &[TuiWidget]) {
	if children.is_empty() {
		return;
	}

	let constraints: Vec<Constraint> = children
		.iter()
		.map(|child| {
			let h = child.estimated_height();
			if h == 0 {
				Constraint::Min(0)
			} else {
				Constraint::Min(h)
			}
		})
		.collect();

	let chunks = Layout::default()
		.direction(Direction::Vertical)
		.constraints(constraints)
		.split(area);

	for (widget, chunk) in children.iter().zip(chunks.iter()) {
		widget.render_to_frame(frame, *chunk);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use ratatui::text::Line;
	use rstest::rstest;

	#[rstest]
	fn test_empty_widget_height() {
		// Arrange
		let widget = TuiWidget::Empty;

		// Act
		let height = widget.estimated_height();

		// Assert
		assert_eq!(height, 0);
	}

	#[rstest]
	fn test_raw_text_height() {
		// Arrange
		let widget = TuiWidget::RawText("Hello".to_string());

		// Act
		let height = widget.estimated_height();

		// Assert
		assert_eq!(height, 1);
	}

	#[rstest]
	fn test_text_block_height() {
		// Arrange
		let content = Text::from(vec![Line::from("Line 1"), Line::from("Line 2")]);
		let widget = TuiWidget::TextBlock {
			content,
			style: Style::default(),
		};

		// Act
		let height = widget.estimated_height();

		// Assert
		assert_eq!(height, 2);
	}

	#[rstest]
	fn test_container_height_includes_borders() {
		// Arrange
		let widget = TuiWidget::Container {
			title: None,
			style: Style::default(),
			children: vec![TuiWidget::RawText("inner".to_string())],
		};

		// Act
		let height = widget.estimated_height();

		// Assert
		assert_eq!(height, 3); // 1 (child) + 2 (borders)
	}

	#[rstest]
	fn test_list_height_includes_borders() {
		// Arrange
		let widget = TuiWidget::ListWidget {
			items: vec![
				Text::from("Item 1"),
				Text::from("Item 2"),
				Text::from("Item 3"),
			],
			title: None,
			style: Style::default(),
		};

		// Act
		let height = widget.estimated_height();

		// Assert
		assert_eq!(height, 5); // 3 items + 2 borders
	}

	#[rstest]
	fn test_table_height_includes_header_and_borders() {
		// Arrange
		let widget = TuiWidget::TableWidget {
			header: vec!["Col A".to_string(), "Col B".to_string()],
			rows: vec![
				vec!["1".to_string(), "2".to_string()],
				vec!["3".to_string(), "4".to_string()],
			],
			style: Style::default(),
		};

		// Act
		let height = widget.estimated_height();

		// Assert
		assert_eq!(height, 5); // 2 rows + 1 header + 2 borders
	}

	#[rstest]
	fn test_input_field_height() {
		// Arrange
		let widget = TuiWidget::InputField {
			placeholder: "Enter text...".to_string(),
			value: String::new(),
			style: Style::default(),
		};

		// Act
		let height = widget.estimated_height();

		// Assert
		assert_eq!(height, 3); // 1 content + 2 borders
	}

	#[rstest]
	fn test_group_height_sums_children() {
		// Arrange
		let widget = TuiWidget::Group(vec![
			TuiWidget::RawText("A".to_string()),
			TuiWidget::RawText("B".to_string()),
			TuiWidget::RawText("C".to_string()),
		]);

		// Act
		let height = widget.estimated_height();

		// Assert
		assert_eq!(height, 3);
	}
}
