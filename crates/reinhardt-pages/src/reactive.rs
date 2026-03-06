//! Reactive System
//!
//! This module provides a fine-grained reactivity system inspired by Leptos and Solid.js.
//! The core primitives are:
//!
//! - [`Signal<T>`]: A reactive value that can change over time
//! - [`Effect`]: A side effect that automatically reruns when dependencies change
//! - [`Memo<T>`]: A cached computation that automatically updates when dependencies change
//!
//! ## Architecture
//!
//! The reactivity system is built on a pull-based model:
//!
//! 1. **Dependency Tracking**: When a Signal is read (`.get()`), it automatically registers
//!    the current observer (Effect or Memo) as a dependent.
//! 2. **Change Notification**: When a Signal changes (`.set()` or `.update()`), all dependent
//!    observers are notified and scheduled for re-execution.
//! 3. **Batching**: Multiple Signal changes are batched into a single update cycle using
//!    micro-tasks for efficiency.
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::reactive::{Signal, Effect, Memo};
//!
//! // Create signals
//! let count = Signal::new(0);
//! let name = Signal::new("Alice".to_string());
//!
//! // Create a memo (derived value)
//! let doubled = Memo::new(move || count.get() * 2);
//!
//! // Create an effect (side effect)
//! Effect::new(move || {
//!     println!("{}: count = {}, doubled = {}",
//!         name.get(), count.get(), doubled.get());
//! });
//! // Prints: "Alice: count = 0, doubled = 0"
//!
//! // Update signals - effect automatically reruns
//! count.set(5);
//! // Prints: "Alice: count = 5, doubled = 10"
//!
//! name.set("Bob".to_string());
//! // Prints: "Bob: count = 5, doubled = 10"
//! ```
//!
//! ## Fine-grained Reactivity
//!
//! Unlike Virtual DOM frameworks that re-render entire component trees, this system
//! uses **fine-grained reactivity** - only the specific DOM nodes that depend on
//! changed Signals are updated. This provides excellent performance for complex UIs.
//!
//! ## Layout Effects
//!
//! For DOM measurements and synchronous updates before paint, use `use_layout_effect`:
//!
//! ```ignore
//! use reinhardt_pages::reactive::{Signal, hooks::{use_layout_effect, use_ref}};
//!
//! let element_ref = use_ref(None::<Element>);
//! let width = Signal::new(0);
//!
//! use_layout_effect({
//!     let element_ref = element_ref.clone();
//!     let width = width.clone();
//!     move || {
//!         if let Some(el) = element_ref.current().as_ref() {
//!             // Runs synchronously before browser paint
//!             width.set(el.offset_width());
//!         }
//!     }
//! });
//! ```
//!
//! **When to use `use_layout_effect`**:
//!
//! - Measuring DOM elements (e.g., offsetWidth, getBoundingClientRect)
//! - Synchronously applying visual updates to prevent flicker
//! - Reading layout information that must be accurate before next paint
//!
//! **When to use `use_effect`** (preferred):
//!
//! - Data fetching
//! - Subscriptions
//! - Logging and analytics
//! - Any side effects that don't require synchronous execution
//!
//! ## Memory Management
//!
//! All reactive nodes (Signals, Effects, Memos) automatically clean up their dependencies
//! when dropped, preventing memory leaks. However, Effects that capture references to
//! Signals will keep those Signals alive - be mindful of circular dependencies.

// Re-export core reactive primitives from reinhardt-reactive
pub use reinhardt_core::reactive::{
	Context, ContextGuard, Effect, EffectTiming, Memo, NodeId, NodeType, Observer, Runtime, Signal,
	context, create_context, effect, get_context, memo, provide_context, remove_context, runtime,
	signal, with_runtime,
};

// WASM-specific modules (kept in reinhardt-pages)
pub mod hooks;
pub mod resource;

// Re-export resource types
pub use resource::{Resource, ResourceState};
#[cfg(target_arch = "wasm32")]
pub use resource::{create_resource, create_resource_with_deps};

// Re-export hooks
pub use hooks::{
	ActionState, Dispatch, OptimisticState, Ref, SetState, SharedSetState, SharedSignal,
	TransitionState, use_action_state, use_callback, use_context, use_debug_value,
	use_deferred_value, use_effect, use_effect_event, use_id, use_layout_effect, use_memo,
	use_optimistic, use_reducer, use_ref, use_shared_state, use_state, use_sync_external_store,
	use_transition,
};
