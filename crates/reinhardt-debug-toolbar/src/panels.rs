//! Panel system
//!
//! This module provides the Panel trait and panel registry for the debug toolbar.

pub mod registry;
pub mod request;

#[cfg(feature = "sql-panel")]
pub mod sql;

// Note: These panels will be implemented in later tasks
// #[cfg(feature = "template-panel")]
// pub mod templates;

// #[cfg(feature = "cache-panel")]
// pub mod cache;

// #[cfg(feature = "performance-panel")]
// pub mod performance;

use crate::context::ToolbarContext;
use crate::error::ToolbarResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use registry::PanelRegistry;

/// Panel trait for debug toolbar panels
///
/// All panels must implement this trait to be registered with the toolbar.
#[async_trait]
pub trait Panel: Send + Sync {
	/// Panel unique identifier (used for HTML element IDs, feature flags)
	fn id(&self) -> &'static str;

	/// Panel display name (shown in toolbar handle)
	fn name(&self) -> &'static str;

	/// Panel priority (for ordering, higher = shown first)
	///
	/// Default priority is 0. Recommended priorities:
	/// - 100: Critical panels (SQL, Request)
	/// - 50: Important panels (Templates, Cache)
	/// - 0: Standard panels (Performance)
	/// - -50: Low priority panels (Settings)
	fn priority(&self) -> i32 {
		0
	}

	/// Enable instrumentation hooks
	///
	/// Called when toolbar is initialized for a request.
	/// Use this to set up any necessary hooks or listeners.
	async fn enable_instrumentation(&self) -> ToolbarResult<()> {
		Ok(())
	}

	/// Disable instrumentation hooks
	///
	/// Called when toolbar is cleaned up after response.
	/// Use this to tear down hooks or release resources.
	async fn disable_instrumentation(&self) -> ToolbarResult<()> {
		Ok(())
	}

	/// Generate statistics from toolbar context
	///
	/// Called after request processing completes.
	/// Extract data from context and compute summary statistics.
	async fn generate_stats(&self, ctx: &ToolbarContext) -> ToolbarResult<PanelStats>;

	/// Render panel HTML
	///
	/// Called when toolbar is injected into response.
	/// Generate HTML content for the panel.
	fn render(&self, stats: &PanelStats) -> ToolbarResult<String>;
}

/// Panel statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelStats {
	/// Panel ID
	pub panel_id: String,

	/// Panel display name
	pub panel_name: String,

	/// Panel data (arbitrary JSON)
	pub data: serde_json::Value,

	/// Summary text (shown in toolbar handle)
	pub summary: String,

	/// Rendered HTML (optional, for lazy rendering)
	#[serde(skip_serializing)]
	pub rendered_html: Option<String>,
}
