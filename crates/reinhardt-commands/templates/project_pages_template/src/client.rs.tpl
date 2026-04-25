//! Client-side (WASM) modules for {{ project_name }}.
//!
//! - `lib`        — `#[wasm_bindgen(start)]` entry point
//! - `router`     — client-side router definition
//! - `pages`      — top-level page components
//! - `components` — reusable UI components grouped per app

pub mod lib;

pub mod router;

pub mod pages;

pub mod components;
