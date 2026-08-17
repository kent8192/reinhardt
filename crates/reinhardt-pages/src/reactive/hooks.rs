//! React-like Hooks API
//!
//! This module provides a React-like hooks API built on top of the fine-grained
//! reactivity system (Signal, Effect, Memo). It offers familiar patterns for
//! developers coming from React while leveraging Rust's type system and the
//! efficiency of fine-grained reactivity.
//!
//! ## Architecture
//!
//! The hooks are implemented as thin wrappers around the existing reactive primitives:
//!
//! ```mermaid
//! flowchart TB
//!     subgraph Hooks["React-like Hooks API"]
//!         API["use_state, use_effect, use_context, etc."]
//!     end
//!
//!     subgraph Reactivity["Fine-grained Reactivity Layer"]
//!         Primitives["Signal&lt;T&gt;, Effect, Memo&lt;T&gt;"]
//!     end
//!
//!     Hooks --> Reactivity
//! ```
//!
//! ## Available Hooks
//!
//! ### State Hooks
//! - [`use_state`] - Hold and update reactive state
//! - [`use_reducer`] - State with reducer logic
//!
//! ### Effect Hooks
//! - [`use_effect`] - Side effects with explicit or automatic dependency tracking
//! - [`use_layout_effect`] - Effects that run before paint with explicit or automatic dependencies
//! - [`use_retained_effect`] - Side effects retained by the mounted view scope
//! - [`use_retained_layout_effect`] - Layout effects retained by the mounted view scope
//!
//! ### Memoization Hooks
//! - [`use_memo`] - Memoize expensive calculations
//! - [`use_callback`] - Stabilize function references
//!
//! ### Ref Hooks
//! - [`use_ref`] - Hold non-reactive mutable values
//!
//! ### Context Hooks
//! - [`use_context`] - Access context values
//!
//! ### Transition Hooks
//! - [`use_transition`] - Non-blocking updates
//! - [`use_deferred_value`] - Defer low-priority updates
//!
//! ### Async Hooks
//! - [`use_action`] / [`use_action_state`] - Async mutations with pending/success/error tracking
//! - [`use_query`] / [`use_action`] - App-wide keyed queries and explicit mutations
//!
//! ### Other Hooks
//! - [`use_id`] - Generate unique IDs
//! - [`use_sync_external_store`] - Subscribe to external stores
//! - [`use_websocket`] - WebSocket connections (WASM only)
//! - [`use_optimistic`] - Optimistic UI updates
//! - [`use_debug_value`] - DevTools labels
//!
//! ## Dependency modes
//!
//! Every dependency-aware hook requires a second argument. Use `deps![...]`
//! for a named explicit dependency list, including `deps![]` for mount-only
//! behavior. `use_effect`, `use_layout_effect`, and `use_memo` additionally
//! accept `deps_auto!()` to infer subscriptions from tracked reads. Callbacks,
//! resources, and retained-effect helpers accept only `deps![...]`.
//!
//! The mode-typed API is the breaking change tracked by issues #5511 and #5577;
//! tuple and unit dependency arguments are no longer accepted.
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::deps;
//! use reinhardt_pages::reactive::hooks::*;
//!
//! fn counter() -> Page {
//!     let (count, set_count) = use_state(0);
//!
//!     let increment = use_callback({
//!         let set_count = set_count.clone();
//!         move |_| set_count.update(|current| current + 1)
//!     }, deps![]);
//!
//!     use_effect({
//!         let count = count.clone();
//!         move || {
//!             log!("Count changed to: {}", count.get());
//!         }
//!     }, deps![count]);
//!
//!     page!(|| {
//!         div {
//!             p { format!("Count: {}", count.get()) }
//!             button {
//!                 @click: increment,
//!                 "Increment"
//!             }
//!         }
//!     })()
//! }
//! ```

// Submodules
pub mod action;
pub mod async_action;
pub mod context;
pub mod debug;
pub mod effect;
pub mod head;
pub mod id;
pub mod memo;
pub mod refs;
pub mod router;
pub mod state;
pub mod sync;
pub mod transition;
pub mod websocket;

// Re-export all hooks
pub use reinhardt_core::reactive::batch;

pub use super::query::{QueryHandle, QueryKey, QuerySnapshot, QueryStatus, use_query};
pub use action::{OptimisticState, use_optimistic};
pub use async_action::{Action, ActionPhase, ActionStateBuilder, use_action, use_action_state};
pub use context::use_context;
pub use debug::use_debug_value;
pub use effect::{
	EffectReturn, use_effect, use_layout_effect, use_retained_effect, use_retained_layout_effect,
};
pub use head::{use_head, use_page_title};
pub use id::use_id;
pub use memo::{use_callback, use_callback_with, use_memo};
pub use refs::{Ref, use_ref};
pub use router::{NavigateError, RouterHandle, use_router};
pub use state::{
	Dispatch, SetState, SetStateExt, SharedSetState, SharedSignal, use_reducer, use_shared_state,
	use_state,
};
pub use sync::{SignalWithSubscription, SubscriptionHandle, use_sync_external_store};
pub use transition::{TransitionState, use_deferred_value, use_transition};
pub use websocket::{
	ConnectionState, UseWebSocketOptions, WebSocketHandle, WebSocketMessage, use_websocket,
};
