//! Mock Panel implementation for testing
//!
//! This module provides a configurable mock implementation of the Panel trait
//! for testing panel registry, middleware, and UI rendering.

use async_trait::async_trait;
use reinhardt_debug_toolbar::{
	context::ToolbarContext,
	error::{ToolbarError, ToolbarResult},
	panels::{Panel, PanelStats},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Mock Panel implementation for testing
///
/// This panel supports configurable behavior and tracks method calls.
///
/// # Example
///
/// ```rust
/// use reinhardt_debug_toolbar_tests::mock_panel::MockPanel;
///
/// let panel = MockPanel::new("test", "Test Panel")
///     .with_priority(50)
///     .with_success_response();
/// ```
#[derive(Debug, Clone)]
pub struct MockPanel {
	/// Panel ID
	id: &'static str,
	/// Panel display name
	name: &'static str,
	/// Panel priority
	priority: i32,
	/// Call counters
	enable_count: Arc<AtomicUsize>,
	disable_count: Arc<AtomicUsize>,
	generate_stats_count: Arc<AtomicUsize>,
	render_count: Arc<AtomicUsize>,
	/// Whether generate_stats should return an error
	should_fail_generate_stats: bool,
	/// Whether render should return an error
	should_fail_render: bool,
	/// Custom stats to return (if any)
	custom_stats: Option<PanelStats>,
	/// Custom HTML to return (if any)
	custom_html: Option<String>,
}

impl MockPanel {
	/// Create a new MockPanel with the given ID and name
	pub fn new(id: &'static str, name: &'static str) -> Self {
		Self {
			id,
			name,
			priority: 0,
			enable_count: Arc::new(AtomicUsize::new(0)),
			disable_count: Arc::new(AtomicUsize::new(0)),
			generate_stats_count: Arc::new(AtomicUsize::new(0)),
			render_count: Arc::new(AtomicUsize::new(0)),
			should_fail_generate_stats: false,
			should_fail_render: false,
			custom_stats: None,
			custom_html: None,
		}
	}

	/// Set panel priority
	pub fn with_priority(mut self, priority: i32) -> Self {
		self.priority = priority;
		self
	}

	/// Configure panel to fail on generate_stats
	pub fn with_generate_stats_failure(mut self) -> Self {
		self.should_fail_generate_stats = true;
		self
	}

	/// Configure panel to fail on render
	pub fn with_render_failure(mut self) -> Self {
		self.should_fail_render = true;
		self
	}

	/// Set custom stats to return from generate_stats
	pub fn with_custom_stats(mut self, stats: PanelStats) -> Self {
		self.custom_stats = Some(stats);
		self
	}

	/// Set custom HTML to return from render
	pub fn with_custom_html(mut self, html: impl Into<String>) -> Self {
		self.custom_html = Some(html.into());
		self
	}

	/// Get the number of times enable_instrumentation was called
	pub fn enable_count(&self) -> usize {
		self.enable_count.load(Ordering::SeqCst)
	}

	/// Get the number of times disable_instrumentation was called
	pub fn disable_count(&self) -> usize {
		self.disable_count.load(Ordering::SeqCst)
	}

	/// Get the number of times generate_stats was called
	pub fn generate_stats_count(&self) -> usize {
		self.generate_stats_count.load(Ordering::SeqCst)
	}

	/// Get the number of times render was called
	pub fn render_count(&self) -> usize {
		self.render_count.load(Ordering::SeqCst)
	}

	/// Reset all call counters
	pub fn reset_counters(&self) {
		self.enable_count.store(0, Ordering::SeqCst);
		self.disable_count.store(0, Ordering::SeqCst);
		self.generate_stats_count.store(0, Ordering::SeqCst);
		self.render_count.store(0, Ordering::SeqCst);
	}

	/// Create default stats for this panel
	fn create_default_stats(&self) -> PanelStats {
		PanelStats {
			panel_id: self.id.to_string(),
			panel_name: self.name.to_string(),
			data: serde_json::json!({
				"test_field": "test_value",
				"panel_type": "mock"
			}),
			summary: format!("{}: Mock Summary", self.name),
			rendered_html: None,
		}
	}

	/// Create default HTML for this panel
	fn create_default_html(&self) -> String {
		format!(
			r#"<div class="mock-panel" id="mock-panel-{}">
				<h3>{}</h3>
				<p>Mock panel content</p>
			</div>"#,
			self.id, self.name
		)
	}
}

impl Default for MockPanel {
	fn default() -> Self {
		Self::new("mock", "Mock Panel")
	}
}

#[async_trait]
impl Panel for MockPanel {
	fn id(&self) -> &'static str {
		self.id
	}

	fn name(&self) -> &'static str {
		self.name
	}

	fn priority(&self) -> i32 {
		self.priority
	}

	async fn enable_instrumentation(&self) -> ToolbarResult<()> {
		self.enable_count.fetch_add(1, Ordering::SeqCst);
		Ok(())
	}

	async fn disable_instrumentation(&self) -> ToolbarResult<()> {
		self.disable_count.fetch_add(1, Ordering::SeqCst);
		Ok(())
	}

	async fn generate_stats(&self, _ctx: &ToolbarContext) -> ToolbarResult<PanelStats> {
		self.generate_stats_count.fetch_add(1, Ordering::SeqCst);

		if self.should_fail_generate_stats {
			return Err(ToolbarError::RenderError(format!(
				"MockPanel '{}' failed to generate stats",
				self.id
			)));
		}

		Ok(self
			.custom_stats
			.clone()
			.unwrap_or_else(|| self.create_default_stats()))
	}

	fn render(&self, _stats: &PanelStats) -> ToolbarResult<String> {
		self.render_count.fetch_add(1, Ordering::SeqCst);

		if self.should_fail_render {
			return Err(ToolbarError::RenderError(format!(
				"MockPanel '{}' failed to render",
				self.id
			)));
		}

		Ok(self
			.custom_html
			.clone()
			.unwrap_or_else(|| self.create_default_html()))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::common::fixtures::test_context;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_mock_panel_basic() {
		let panel = MockPanel::new("test", "Test Panel");
		assert_eq!(panel.id(), "test");
		assert_eq!(panel.name(), "Test Panel");
		assert_eq!(panel.priority(), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_mock_panel_with_priority() {
		let panel = MockPanel::new("test", "Test Panel").with_priority(100);
		assert_eq!(panel.priority(), 100);
	}

	#[rstest]
	#[tokio::test]
	async fn test_mock_panel_call_tracking() {
		let panel = MockPanel::new("test", "Test Panel");
		let ctx = test_context();

		// Enable instrumentation
		panel.enable_instrumentation().await.unwrap();
		assert_eq!(panel.enable_count(), 1);

		// Generate stats
		panel.generate_stats(&ctx).await.unwrap();
		assert_eq!(panel.generate_stats_count(), 1);

		// Disable instrumentation
		panel.disable_instrumentation().await.unwrap();
		assert_eq!(panel.disable_count(), 1);

		// Render
		let stats = panel.generate_stats(&ctx).await.unwrap();
		panel.render(&stats).unwrap();
		assert_eq!(panel.render_count(), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_mock_panel_failure() {
		let panel = MockPanel::new("test", "Test Panel")
			.with_generate_stats_failure()
			.with_render_failure();

		let ctx = test_context();

		// Test generate_stats failure
		assert!(panel.generate_stats(&ctx).await.is_err());

		// Test render failure
		let stats = panel.create_default_stats();
		assert!(panel.render(&stats).is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_mock_panel_custom_output() {
		let custom_stats = PanelStats {
			panel_id: "custom".to_string(),
			panel_name: "Custom".to_string(),
			data: serde_json::json!({"custom": "data"}),
			summary: "Custom Summary".to_string(),
			rendered_html: None,
		};

		let custom_html = r#"<div class="custom">Custom HTML</div>"#;

		let panel = MockPanel::new("test", "Test Panel")
			.with_custom_stats(custom_stats.clone())
			.with_custom_html(custom_html.to_string());

		let ctx = test_context();

		// Test custom stats
		let stats = panel.generate_stats(&ctx).await.unwrap();
		assert_eq!(stats.panel_id, "custom");
		assert_eq!(stats.summary, "Custom Summary");

		// Test custom HTML
		let html = panel.render(&stats).unwrap();
		assert!(html.contains("Custom HTML"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_mock_panel_reset_counters() {
		let panel = MockPanel::new("test", "Test Panel");
		let ctx = test_context();

		// Make some calls
		panel.enable_instrumentation().await.unwrap();
		panel.generate_stats(&ctx).await.unwrap();
		panel.disable_instrumentation().await.unwrap();

		assert_eq!(panel.enable_count(), 1);
		assert_eq!(panel.generate_stats_count(), 1);
		assert_eq!(panel.disable_count(), 1);

		// Reset counters
		panel.reset_counters();

		assert_eq!(panel.enable_count(), 0);
		assert_eq!(panel.generate_stats_count(), 0);
		assert_eq!(panel.disable_count(), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_mock_panel_default_impl() {
		let panel = MockPanel::default();
		let ctx = test_context();

		assert_eq!(panel.id(), "mock");
		assert_eq!(panel.name(), "Mock Panel");

		let stats = panel.generate_stats(&ctx).await.unwrap();
		let html = panel.render(&stats).unwrap();

		assert!(html.contains("Mock Panel"));
		assert!(html.contains("mock-panel"));
	}
}
