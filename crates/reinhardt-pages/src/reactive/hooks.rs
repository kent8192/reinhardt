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
//! - [`use_effect`] - Side effects with automatic dependency tracking
//! - [`use_layout_effect`] - Effects that run before paint
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
//! ### Other Hooks
//! - [`use_id`] - Generate unique IDs
//! - [`use_sync_external_store`] - Subscribe to external stores
//! - [`use_websocket`] - WebSocket connections (WASM only)
//! - [`use_action_state`] - Form action state
//! - [`use_optimistic`] - Optimistic UI updates
//! - [`use_debug_value`] - DevTools labels
//! - [`use_effect_event`] - Non-reactive event handlers
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::reactive::hooks::*;
//!
//! fn counter() -> Page {
//!     let (count, set_count) = use_state(0);
//!
//!     let increment = use_callback({
//!         let set_count = set_count.clone();
//!         move |_| set_count(count.get() + 1)
//!     });
//!
//!     use_effect({
//!         let count = count.clone();
//!         move || {
//!             log!("Count changed to: {}", count.get());
//!             None::<fn()>
//!         }
//!     });
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
pub mod context;
pub mod debug;
pub mod effect;
pub mod id;
pub mod memo;
pub mod refs;
pub mod state;
pub mod sync;
pub mod transition;
pub mod websocket;

// Re-export all hooks
pub use action::{ActionState, OptimisticState, use_action_state, use_optimistic};
pub use context::use_context;
pub use debug::{use_debug_value, use_effect_event};
pub use effect::{use_effect, use_layout_effect};
pub use id::use_id;
pub use memo::{use_callback, use_memo};
pub use refs::{Ref, use_ref};
pub use state::{
	Dispatch, SetState, SharedSetState, SharedSignal, use_reducer, use_shared_state, use_state,
};
pub use sync::{SignalWithSubscription, SubscriptionHandle, use_sync_external_store};
pub use transition::{TransitionState, use_deferred_value, use_transition};
pub use websocket::{
	ConnectionState, UseWebSocketOptions, WebSocketHandle, WebSocketMessage, use_websocket,
};
