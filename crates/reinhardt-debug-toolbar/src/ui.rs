//! UI rendering
//!
//! This module provides HTML/CSS/JS rendering for the debug toolbar.

pub mod injection;
pub mod renderer;

pub use injection::inject_toolbar;
pub use renderer::render_toolbar;
