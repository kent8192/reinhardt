//! Panel registry

use crate::panels::Panel;
use std::collections::HashMap;
use std::sync::Arc;

/// Global panel registry
pub struct PanelRegistry {
	panels: HashMap<String, Arc<Box<dyn Panel>>>,
}

impl PanelRegistry {
	/// Create new empty registry
	pub fn new() -> Self {
		Self {
			panels: HashMap::new(),
		}
	}

	/// Register a panel
	pub fn register(&mut self, panel: Box<dyn Panel>) {
		let id = panel.id().to_string();
		self.panels.insert(id, Arc::new(panel));
	}

	/// Get panel by ID
	pub fn get(&self, id: &str) -> Option<&Arc<Box<dyn Panel>>> {
		self.panels.get(id)
	}

	/// Get all panels sorted by priority
	pub fn all(&self) -> Vec<&Arc<Box<dyn Panel>>> {
		let mut panels: Vec<_> = self.panels.values().collect();
		panels.sort_by_key(|p| -p.priority());
		panels
	}

	/// Get panel count
	pub fn len(&self) -> usize {
		self.panels.len()
	}

	/// Check if registry is empty
	pub fn is_empty(&self) -> bool {
		self.panels.is_empty()
	}
}

impl Default for PanelRegistry {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::context::ToolbarContext;
	use crate::error::ToolbarResult;
	use crate::panels::PanelStats;
	use async_trait::async_trait;

	struct MockPanel {
		id: &'static str,
		name: &'static str,
		priority: i32,
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

		async fn generate_stats(&self, _ctx: &ToolbarContext) -> ToolbarResult<PanelStats> {
			Ok(PanelStats {
				panel_id: self.id.to_string(),
				panel_name: self.name.to_string(),
				data: serde_json::json!({}),
				summary: "Test".to_string(),
				rendered_html: None,
			})
		}

		fn render(&self, _stats: &PanelStats) -> ToolbarResult<String> {
			Ok("<div>Test</div>".to_string())
		}
	}

	#[test]
	fn test_registry_creation() {
		let registry = PanelRegistry::new();
		assert_eq!(registry.len(), 0);
		assert!(registry.is_empty());
	}

	#[test]
	fn test_panel_registration() {
		let mut registry = PanelRegistry::new();

		let panel = Box::new(MockPanel {
			id: "test",
			name: "Test Panel",
			priority: 0,
		});

		registry.register(panel);
		assert_eq!(registry.len(), 1);
		assert!(!registry.is_empty());
	}

	#[test]
	fn test_panel_retrieval() {
		let mut registry = PanelRegistry::new();

		let panel = Box::new(MockPanel {
			id: "test",
			name: "Test Panel",
			priority: 0,
		});

		registry.register(panel);

		let retrieved = registry.get("test");
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().id(), "test");
	}

	#[test]
	fn test_panel_sorting_by_priority() {
		let mut registry = PanelRegistry::new();

		registry.register(Box::new(MockPanel {
			id: "low",
			name: "Low Priority",
			priority: 0,
		}));

		registry.register(Box::new(MockPanel {
			id: "high",
			name: "High Priority",
			priority: 100,
		}));

		registry.register(Box::new(MockPanel {
			id: "medium",
			name: "Medium Priority",
			priority: 50,
		}));

		let panels = registry.all();
		assert_eq!(panels.len(), 3);
		assert_eq!(panels[0].id(), "high");
		assert_eq!(panels[1].id(), "medium");
		assert_eq!(panels[2].id(), "low");
	}
}
