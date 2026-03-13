/// Manual action registration support
/// This provides a simpler API for registering actions without macros
use crate::viewsets::metadata::{ActionMetadata, FunctionActionHandler};
use reinhardt_http::{Request, Response, Result};
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, RwLock};

/// Global action registry for manual registration
/// This is a temporary solution until the procedural macro is fully implemented
pub struct ManualActionRegistry {
	actions: RwLock<HashMap<String, Vec<ActionMetadata>>>,
}

impl ManualActionRegistry {
	fn new() -> Self {
		Self {
			actions: RwLock::new(HashMap::new()),
		}
	}

	/// Register an action for a ViewSet type
	pub fn register(&self, viewset_type: &str, action: ActionMetadata) {
		let mut actions = self.actions.write().unwrap_or_else(|e| e.into_inner());
		actions
			.entry(viewset_type.to_string())
			.or_default()
			.push(action);
	}

	/// Get actions for a ViewSet type
	pub fn get_actions(&self, viewset_type: &str) -> Vec<ActionMetadata> {
		let actions = self.actions.read().unwrap_or_else(|e| e.into_inner());
		actions.get(viewset_type).cloned().unwrap_or_else(Vec::new)
	}

	/// Clear all registered actions (for testing)
	pub fn clear(&self) {
		let mut actions = self.actions.write().unwrap_or_else(|e| e.into_inner());
		actions.clear();
	}
}

lazy_static::lazy_static! {
	static ref GLOBAL_REGISTRY: ManualActionRegistry = ManualActionRegistry::new();
}

/// Register an action manually
pub fn register_action(viewset_type: &str, action: ActionMetadata) {
	GLOBAL_REGISTRY.register(viewset_type, action);
}

/// Get registered actions for a ViewSet type
pub fn get_registered_actions(viewset_type: &str) -> Vec<ActionMetadata> {
	GLOBAL_REGISTRY.get_actions(viewset_type)
}

/// Clear all registered actions (primarily for testing)
pub fn clear_actions() {
	GLOBAL_REGISTRY.clear();
}

/// Helper to create an action with a closure
pub fn action<F, Fut>(name: impl Into<String>, detail: bool, handler: F) -> ActionMetadata
where
	F: Fn(Request) -> Fut + Send + Sync + 'static,
	Fut: Future<Output = Result<Response>> + Send + 'static,
{
	let handler_fn = Arc::new(handler);
	ActionMetadata::new(name)
		.with_detail(detail)
		.with_handler(FunctionActionHandler::new(move |req| {
			let h = handler_fn.clone();
			Box::pin(async move { h(req).await })
		}))
}

/// Macro to simplify action registration
#[macro_export]
macro_rules! register_viewset_actions {
    ($viewset_type:ty => {
        $(
            $action_name:ident ( $detail:expr $(, $attr:ident = $value:expr )* ) => $handler:expr
        ),* $(,)?
    }) => {
        {
            let viewset_type = std::any::type_name::<$viewset_type>();
            $(
                let mut action = $crate::viewsets::metadata::ActionMetadata::new(stringify!($action_name))
                    .with_detail($detail);

                $(
                    action = register_viewset_actions!(@attr action, $attr, $value);
                )*

                let handler = $handler;
                action = action.with_handler($crate::viewsets::metadata::FunctionActionHandler::new(
                    move |req| Box::pin(handler(req))
                ));

                $crate::viewsets::registry::register_action(viewset_type, action);
            )*
        }
    };

    (@attr $action:expr, name, $value:expr) => {
        $action.with_custom_name($value)
    };
    (@attr $action:expr, suffix, $value:expr) => {
        $action.with_suffix($value)
    };
    (@attr $action:expr, url_path, $value:expr) => {
        $action.with_url_path($value)
    };
    (@attr $action:expr, url_name, $value:expr) => {
        $action.with_url_name($value)
    };
    (@attr $action:expr, methods, $value:expr) => {
        $action.with_methods($value)
    };
}
