//! WASM frontend testing utilities.
//!
//! This module provides comprehensive testing utilities for WASM-based
//! frontend applications built with reinhardt-pages.
//!
//! # Features
//!
//! - **DOM Query API**: Testing Library-style element queries (`get_by_role`, `get_by_text`, etc.)
//! - **Event Simulation**: User interaction simulation (`click`, `type_text`, `keyboard_press`)
//! - **Async Utilities**: Wait helpers (`wait_for`, `sleep`, `flush_effects`)
//! - **Assertions**: DOM state assertions (`should_be_visible`, `should_have_text`)
//! - **Mock Infrastructure**: Server function and browser API mocking
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_test::wasm::{Screen, UserEvent, wait_for};
//! use reinhardt_test::wasm::assertions::ElementAssertions;
//! use wasm_bindgen_test::*;
//!
//! wasm_bindgen_test_configure!(run_in_browser);
//!
//! #[wasm_bindgen_test]
//! async fn test_counter() {
//!     let screen = Screen::new();
//!
//!     // Find and click button
//!     let button = screen.get_by_role("button").get();
//!     UserEvent::click(&button);
//!
//!     // Wait for update and verify
//!     wait_for(|| screen.get_by_text("Count: 1").query().is_some())
//!         .await
//!         .unwrap();
//!
//!     screen.get_by_role("heading").get().should_have_text("Count: 1");
//! }
//! ```

#![cfg(all(wasm, feature = "wasm"))]

mod assertions;
mod events;
mod mock;
mod query;
mod wait;

// Re-export all public items
pub use assertions::*;
pub use events::*;
pub use mock::*;
pub use query::*;
pub use wait::*;

// Re-export wasm-bindgen-test for convenience
#[cfg(wasm)]
pub use wasm_bindgen_test::*;
