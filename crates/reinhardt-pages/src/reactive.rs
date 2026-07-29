//! Reactive System
//!
//! This module provides a fine-grained reactivity system inspired by Leptos and Solid.js.
//! The core primitives are:
//!
//! - [`Signal<T>`]: A reactive value that can change over time
//! - [`Effect`]: A side effect that automatically reruns when dependencies change
//! - [`Memo<T>`]: A cached computation that automatically updates when dependencies change
//!
//! Normal Pages entrypoints, including SSR rendering, hydration, and
//! [`ClientLauncher`](crate::app::ClientLauncher), install and own reactive scopes
//! automatically. Low-level code that creates nodes directly must use
//! [`ReactiveScope::run`](reinhardt_core::reactive::ReactiveScope::run).
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
//! For DOM measurements and synchronous updates before paint, use
//! `use_layout_effect`:
//!
//! ```ignore
//! use reinhardt_pages::reactive::{Signal, hooks::{use_ref, use_retained_layout_effect}};
//!
//! let element_ref = use_ref(None::<Element>);
//! let width = Signal::new(0);
//!
//! use_retained_layout_effect(
//!     {
//!         let element_ref = element_ref.clone();
//!         let width = width.clone();
//!         move || {
//!             if let Some(el) = element_ref.current().as_ref() {
//!                 // Runs synchronously before browser paint
//!                 width.set(el.offset_width());
//!             }
//!             None::<fn()>
//!         }
//!     },
//!     (element_ref,),
//! );
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
//! Reactive nodes are owned by the active scope and clean up together when that scope is
//! dropped. Reactive handles are copied keys and do not extend the scope lifetime.

// Re-export core reactive primitives from reinhardt-reactive
pub use reinhardt_core::reactive::{
	Context, ContextGuard, Effect, EffectTiming, ExplicitDeps, IntoSignalHandle, Memo, NodeId,
	NodeType, Observer, ReactiveDeps, ReactiveScope, Runtime, Signal, batch, context,
	copy_signal_handle, create_context, current_scope_id, effect, get_context, memo,
	provide_context, remove_context, runtime, signal, untracked, with_runtime,
};

// WASM-specific modules (kept in reinhardt-pages)
pub mod hooks;
pub(crate) mod pages_arena;
pub mod query;
pub mod resource;
pub mod resource_value;
pub mod trackable;

pub use trackable::Trackable;

// Re-export resource types and the unified hook (available on all targets)
pub(crate) use query::{
	QueryAcquireOptions, QueryConsumer, QueryErrorPolicy, QueryLease, acquire_query,
	seed_query_from_serialized,
};
pub use query::{
	QueryClient, QueryDefaults, QueryDescriptor, QueryFamily, QueryHandle, QueryKey, QueryOptions,
	QueryPhase, queries, use_mutation, use_query,
};
pub use resource::{Resource, ResourceState, use_resource, use_resource_with_key};
pub use resource_value::{
	LatestResourceState, LatestResourceValue, LatestResourceValueBuilder, use_latest_resource_value,
};

// Re-export hooks
pub use hooks::{
	Action, ActionPhase, ActionStateBuilder, Dispatch, EffectReturn, OptimisticState, Ref,
	SetState, SetStateExt, SharedSetState, SharedSignal, TransitionState, use_action,
	use_action_state, use_callback, use_context, use_debug_value, use_deferred_value, use_effect,
	use_head, use_id, use_layout_effect, use_memo, use_optimistic, use_page_title, use_reducer,
	use_ref, use_retained_effect, use_retained_layout_effect, use_shared_state, use_state,
	use_sync_external_store, use_transition,
};
