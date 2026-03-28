//! Debug toolbar Tower layer

use crate::middleware::{DebugToolbarService, ToolbarConfig};
use crate::panels::{PanelRegistry, request::RequestPanel};
use std::sync::Arc;
use tower::Layer;

/// Tower layer for debug toolbar middleware
#[derive(Clone)]
pub struct DebugToolbarLayer {
	config: Arc<ToolbarConfig>,
	registry: Arc<PanelRegistry>,
}

impl DebugToolbarLayer {
	/// Create new toolbar layer with configuration and panel registry
	pub fn new(config: ToolbarConfig, registry: PanelRegistry) -> Self {
		Self {
			config: Arc::new(config),
			registry: Arc::new(registry),
		}
	}

	/// Create toolbar layer with default configuration
	pub fn with_default() -> Self {
		let config = ToolbarConfig::default();
		let registry = Self::create_default_registry_with_config(&config);
		Self::new(config, registry)
	}

	fn create_default_registry_with_config(config: &ToolbarConfig) -> PanelRegistry {
		let mut registry = PanelRegistry::new();

		// Request panel (always enabled)
		registry.register(Box::new(RequestPanel::new()));

		// SQL panel (feature-gated)
		#[cfg(feature = "sql-panel")]
		{
			use crate::panels::sql::SqlPanel;
			registry.register(Box::new(SqlPanel::with_threshold(
				config.sql_warning_threshold_ms,
			)));
		}

		// Note: Other panels will be registered once implemented
		// Template panel (feature-gated)
		// #[cfg(feature = "template-panel")]
		// {
		//     use crate::panels::templates::TemplatesPanel;
		//     registry.register(Box::new(TemplatesPanel::new()));
		// }

		// Cache panel (feature-gated)
		// #[cfg(feature = "cache-panel")]
		// {
		//     use crate::panels::cache::CachePanel;
		//     registry.register(Box::new(CachePanel::new()));
		// }

		// Performance panel (feature-gated)
		// #[cfg(feature = "performance-panel")]
		// {
		//     use crate::panels::performance::PerformancePanel;
		//     registry.register(Box::new(PerformancePanel::new()));
		// }

		registry
	}
}

impl Default for DebugToolbarLayer {
	fn default() -> Self {
		Self::with_default()
	}
}

impl<S> Layer<S> for DebugToolbarLayer {
	type Service = DebugToolbarService<S>;

	fn layer(&self, inner: S) -> Self::Service {
		DebugToolbarService {
			inner,
			config: self.config.clone(),
			registry: self.registry.clone(),
		}
	}
}
