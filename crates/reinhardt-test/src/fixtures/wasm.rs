//! WASM and browser testing fixtures.
//!
//! This module contains two categories of fixtures:
//!
//! - **In-browser WASM fixtures** (`browser` submodule): Fixtures for testing WASM
//!   frontends running inside the browser (requires `wasm32` target + `wasm` feature).
//! - **E2E browser testing** (`e2e` submodule): Fixtures for controlling a browser
//!   externally via WebDriver/fantoccini (native target, `e2e` feature).

// In-browser WASM test fixtures (wasm32 target only)
#[cfg(all(wasm, feature = "wasm"))]
mod browser;

// Backward-compatible re-exports: all items previously in fixtures::wasm::*
// remain accessible at the same path after the browser submodule extraction.
#[cfg(all(wasm, feature = "wasm"))]
pub use browser::{
	WasmTestEnv, mock_cookies, mock_fetch, mock_local_storage, mock_session_storage,
	populated_storage, screen, session_cookies, wasm_test_env,
};

// MSW fixtures (requires wasm + msw features)
#[cfg(all(wasm, feature = "msw"))]
pub mod msw;

#[cfg(all(wasm, feature = "msw"))]
pub use msw::{msw_worker, msw_worker_passthrough};

// E2E browser testing fixtures via WebDriver (native target only)
#[cfg(all(feature = "e2e", native))]
pub mod e2e;

// E2E browser testing fixtures via Chrome DevTools Protocol (native target only)
#[cfg(all(feature = "e2e-cdp", not(target_arch = "wasm32")))]
pub mod e2e_cdp;
