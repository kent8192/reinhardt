//! TUI application runtime with terminal management and event loop.

use std::io;
use std::time::Duration;

use super::renderer::TuiRenderer;
use super::widget::TuiWidget;
use crate::component::Page;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
	EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// Builder for configuring and creating a `TuiApp`.
pub struct TuiAppBuilder {
	title: Option<String>,
	tick_rate: Duration,
	renderer: TuiRenderer,
}

impl Default for TuiAppBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl TuiAppBuilder {
	/// Creates a new builder with default settings.
	pub fn new() -> Self {
		Self {
			title: None,
			tick_rate: Duration::from_millis(250),
			renderer: TuiRenderer::new(),
		}
	}

	/// Sets the application title displayed in the terminal.
	pub fn title(mut self, title: impl Into<String>) -> Self {
		self.title = Some(title.into());
		self
	}

	/// Sets the tick rate for the event loop.
	///
	/// Lower values increase responsiveness but use more CPU.
	/// Default: 250ms.
	pub fn tick_rate(mut self, duration: Duration) -> Self {
		self.tick_rate = duration;
		self
	}

	/// Sets a custom renderer.
	pub fn renderer(mut self, renderer: TuiRenderer) -> Self {
		self.renderer = renderer;
		self
	}

	/// Builds the `TuiApp` and initializes the terminal.
	///
	/// # Errors
	///
	/// Returns an error if terminal initialization fails (e.g., not a TTY).
	pub fn build(self) -> io::Result<TuiApp> {
		TuiApp::new_with_config(self.title, self.tick_rate, self.renderer)
	}
}

/// TUI application runtime that manages terminal lifecycle and rendering.
///
/// Handles terminal initialization, the event loop for keyboard input,
/// and rendering `Page` trees to the terminal. The terminal is automatically
/// restored when the `TuiApp` is dropped.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::tui::TuiApp;
/// use reinhardt_pages::component::{Page, PageElement, IntoPage};
///
/// let page = PageElement::new("div")
///     .child(PageElement::new("h1").child("My TUI App"))
///     .into_page();
///
/// let mut app = TuiApp::builder()
///     .title("My App")
///     .build()?;
///
/// app.run(page)?;
/// ```
pub struct TuiApp {
	terminal: Terminal<CrosstermBackend<io::Stdout>>,
	#[allow(dead_code)] // Used for display purposes in future title bar rendering
	title: Option<String>,
	tick_rate: Duration,
	renderer: TuiRenderer,
}

impl TuiApp {
	/// Creates a new TUI app builder.
	pub fn builder() -> TuiAppBuilder {
		TuiAppBuilder::new()
	}

	/// Creates a new TUI app with default settings.
	///
	/// # Errors
	///
	/// Returns an error if terminal initialization fails.
	pub fn new() -> io::Result<Self> {
		Self::new_with_config(None, Duration::from_millis(250), TuiRenderer::new())
	}

	fn new_with_config(
		title: Option<String>,
		tick_rate: Duration,
		renderer: TuiRenderer,
	) -> io::Result<Self> {
		// Initialize terminal
		enable_raw_mode()?;
		let mut stdout = io::stdout();
		execute!(stdout, EnterAlternateScreen)?;
		let backend = CrosstermBackend::new(stdout);
		let terminal = Terminal::new(backend)?;

		Ok(Self {
			terminal,
			title,
			tick_rate,
			renderer,
		})
	}

	/// Runs the TUI application with the given page.
	///
	/// Enters the event loop, rendering the page and handling input events.
	/// The loop exits when the user presses `q` or `Ctrl+C`.
	///
	/// # Errors
	///
	/// Returns an error if terminal drawing or event polling fails.
	pub fn run(&mut self, page: Page) -> io::Result<()> {
		let widget = self.renderer.render(&page);

		loop {
			self.draw(&widget)?;

			if self.handle_events()? {
				break;
			}
		}

		Ok(())
	}

	/// Renders a single frame with the given widget tree.
	///
	/// This method can be used for one-shot rendering without an event loop.
	///
	/// # Errors
	///
	/// Returns an error if terminal drawing fails.
	pub fn render_once(&mut self, page: &Page) -> io::Result<()> {
		let widget = self.renderer.render(page);
		self.draw(&widget)?;
		Ok(())
	}

	/// Draws the widget tree to the terminal.
	fn draw(&mut self, widget: &TuiWidget) -> io::Result<()> {
		self.terminal.draw(|frame| {
			let area = frame.area();
			widget.render_to_frame(frame, area);
		})?;
		Ok(())
	}

	/// Handles terminal events (keyboard, resize).
	///
	/// Returns `true` if the application should quit.
	fn handle_events(&mut self) -> io::Result<bool> {
		if event::poll(self.tick_rate)?
			&& let Event::Key(key_event) = event::read()?
			&& key_event.kind == KeyEventKind::Press
		{
			match key_event.code {
				KeyCode::Char('q') => return Ok(true),
				KeyCode::Char('c')
					if key_event
						.modifiers
						.contains(crossterm::event::KeyModifiers::CONTROL) =>
				{
					return Ok(true);
				}
				KeyCode::Esc => return Ok(true),
				_ => {}
			}
		}
		Ok(false)
	}
}

impl Drop for TuiApp {
	fn drop(&mut self) {
		// Restore terminal state on drop
		let _ = disable_raw_mode();
		let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
		let _ = self.terminal.show_cursor();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_builder_default() {
		// Arrange & Act
		let builder = TuiAppBuilder::new();

		// Assert
		assert!(builder.title.is_none());
		assert_eq!(builder.tick_rate, Duration::from_millis(250));
	}

	#[rstest]
	fn test_builder_with_title() {
		// Arrange & Act
		let builder = TuiAppBuilder::new().title("My App");

		// Assert
		assert_eq!(builder.title, Some("My App".to_string()));
	}

	#[rstest]
	fn test_builder_with_tick_rate() {
		// Arrange & Act
		let builder = TuiAppBuilder::new().tick_rate(Duration::from_millis(100));

		// Assert
		assert_eq!(builder.tick_rate, Duration::from_millis(100));
	}

	#[rstest]
	fn test_builder_default_trait() {
		// Arrange & Act
		let builder = TuiAppBuilder::default();

		// Assert
		assert!(builder.title.is_none());
	}

	// Note: Tests for TuiApp::new() and TuiApp::run() require a real terminal
	// and cannot be run in CI. They are tested manually.
}
