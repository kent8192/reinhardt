//! Model and migration lifecycle events

use super::core::SignalName;
use super::registry::get_signal;
use super::signal::Signal;
use std::fmt;

// ========================================
// Model lifecycle events
// ========================================

/// M2M changed signal - sent when many-to-many relationships change
#[derive(Debug, Clone)]
pub struct M2MChangeEvent<T, R> {
	/// The model instance whose M2M relationship changed.
	pub instance: T,
	/// The type of M2M action that occurred.
	pub action: M2MAction,
	/// The related objects involved in the change.
	pub related: Vec<R>,
	/// Whether this is the reverse side of the relationship.
	pub reverse: bool,
	/// Name of the model class.
	pub model_name: String,
}

impl<T, R> M2MChangeEvent<T, R> {
	/// Creates a new M2M change event.
	pub fn new(instance: T, action: M2MAction, related: Vec<R>) -> Self {
		Self {
			instance,
			action,
			related,
			reverse: false,
			model_name: String::new(),
		}
	}

	/// Sets whether this is the reverse side of the relationship.
	pub fn with_reverse(mut self, reverse: bool) -> Self {
		self.reverse = reverse;
		self
	}

	/// Sets the model name for this event.
	pub fn with_model_name(mut self, model_name: impl Into<String>) -> Self {
		self.model_name = model_name.into();
		self
	}
}

/// Actions that can occur on a many-to-many relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum M2MAction {
	/// Before objects are added to the relationship.
	PreAdd,
	/// After objects are added to the relationship.
	PostAdd,
	/// Before objects are removed from the relationship.
	PreRemove,
	/// After objects are removed from the relationship.
	PostRemove,
	/// Before all objects are cleared from the relationship.
	PreClear,
	/// After all objects are cleared from the relationship.
	PostClear,
}

impl fmt::Display for M2MAction {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			M2MAction::PreAdd => write!(f, "pre_add"),
			M2MAction::PostAdd => write!(f, "post_add"),
			M2MAction::PreRemove => write!(f, "pre_remove"),
			M2MAction::PostRemove => write!(f, "post_remove"),
			M2MAction::PreClear => write!(f, "pre_clear"),
			M2MAction::PostClear => write!(f, "post_clear"),
		}
	}
}

/// Returns the M2M changed signal for the given model types.
pub fn m2m_changed<T: Send + Sync + 'static, R: Send + Sync + 'static>()
-> Signal<M2MChangeEvent<T, R>> {
	get_signal::<M2MChangeEvent<T, R>>(SignalName::M2M_CHANGED)
}

/// Pre-init signal - sent at the beginning of a model's __init__ method
#[derive(Debug, Clone)]
pub struct PreInitEvent<T> {
	/// Name of the model type being initialized.
	pub model_type: String,
	/// Arguments passed to the model constructor.
	pub args: Vec<String>,
	_phantom: std::marker::PhantomData<T>,
}

impl<T> PreInitEvent<T> {
	/// Creates a new pre-init event for the given model type.
	pub fn new(model_type: impl Into<String>) -> Self {
		Self {
			model_type: model_type.into(),
			args: Vec::new(),
			_phantom: std::marker::PhantomData,
		}
	}

	/// Sets the constructor arguments for this event.
	pub fn with_args(mut self, args: Vec<String>) -> Self {
		self.args = args;
		self
	}
}

/// Returns the pre-init signal for the given model type.
pub fn pre_init<T: Send + Sync + 'static>() -> Signal<PreInitEvent<T>> {
	get_signal::<PreInitEvent<T>>(SignalName::PRE_INIT)
}

/// Post-init signal - sent at the end of a model's __init__ method
#[derive(Debug, Clone)]
pub struct PostInitEvent<T> {
	/// The fully initialized model instance.
	pub instance: T,
}

impl<T> PostInitEvent<T> {
	/// Creates a new post-init event with the initialized instance.
	pub fn new(instance: T) -> Self {
		Self { instance }
	}
}

/// Returns the post-init signal for the given model type.
pub fn post_init<T: Send + Sync + 'static>() -> Signal<PostInitEvent<T>> {
	get_signal::<PostInitEvent<T>>(SignalName::POST_INIT)
}

// ========================================
// Migration lifecycle events
// ========================================

/// Pre-migrate signal - sent before running migrations
#[derive(Debug, Clone)]
pub struct MigrationEvent {
	/// Name of the application being migrated.
	pub app_name: String,
	/// Name of the migration being applied.
	pub migration_name: String,
	/// Ordered list of migration steps in the plan.
	pub plan: Vec<String>,
}

impl MigrationEvent {
	/// Creates a new migration event for the given app and migration.
	pub fn new(app_name: impl Into<String>, migration_name: impl Into<String>) -> Self {
		Self {
			app_name: app_name.into(),
			migration_name: migration_name.into(),
			plan: Vec::new(),
		}
	}

	/// Sets the migration plan steps.
	pub fn with_plan(mut self, plan: Vec<String>) -> Self {
		self.plan = plan;
		self
	}
}

/// Returns the pre-migrate signal.
pub fn pre_migrate() -> Signal<MigrationEvent> {
	get_signal::<MigrationEvent>(SignalName::PRE_MIGRATE)
}

/// Post-migrate signal - sent after running migrations
pub fn post_migrate() -> Signal<MigrationEvent> {
	get_signal::<MigrationEvent>(SignalName::POST_MIGRATE)
}

/// Class prepared signal - sent when a model class is prepared
#[derive(Debug, Clone)]
pub struct ClassPreparedEvent {
	/// Name of the model class that was prepared.
	pub model_name: String,
	/// Application label the model belongs to.
	pub app_label: String,
}

impl ClassPreparedEvent {
	/// Creates a new class-prepared event.
	pub fn new(model_name: impl Into<String>, app_label: impl Into<String>) -> Self {
		Self {
			model_name: model_name.into(),
			app_label: app_label.into(),
		}
	}
}

/// Returns the class-prepared signal.
pub fn class_prepared() -> Signal<ClassPreparedEvent> {
	get_signal::<ClassPreparedEvent>(SignalName::CLASS_PREPARED)
}
