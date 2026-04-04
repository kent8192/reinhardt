//! Form Integration for Reinhardt WASM (Week 5 Day 3-4)
//!
//! This module provides client-side form rendering and handling
//! integrated with `reinhardt-forms` server-side forms.
//!
//! ## Architecture
//!
//! ```mermaid
//! flowchart LR
//!     subgraph Server["Server-side (reinhardt-forms)"]
//!         Form["Form<br/>to_metadata()"]
//!         FormMetadata["FormMetadata"]
//!     end
//!
//!     subgraph Client["Client-side WASM (reinhardt-pages)"]
//!         FormComponent["FormComponent<br/>render()<br/>validate()<br/>submit()"]
//!         FormBinding["FormBinding&lt;F&gt;<br/>bind()"]
//!     end
//!
//!     Form -->|JSON| FormComponent
//!     FormComponent --> DOM
//!     FormComponent --> AJAX
//!     FormBinding --> Signals
//! ```
//!
//! ## Components
//!
//! - **FormComponent**: Renders `FormMetadata` to DOM with CSRF protection
//! - **FormBinding**: Two-way data binding between Form and Signals
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::form::FormComponent;
//! use reinhardt_forms::wasm_compat::FormMetadata;
//!
//! // Receive FormMetadata from server
//! let metadata: FormMetadata = fetch_form_metadata().await?;
//!
//! // Create and render form
//! let form = FormComponent::new(metadata, "/api/submit");
//! let form_element = form.render();
//! document.body().append_child(&form_element)?;
//!
//! // Submit on user action
//! if form.validate() {
//!     form.submit().await?;
//! }
//! ```

pub mod binding;
pub mod component;
pub mod generated;
pub mod validators;

// Server-side only modules for HTML rendering and asset management
#[cfg(native)]
pub mod media;
#[cfg(native)]
pub mod rendering;

pub use binding::FormBinding;
pub use component::FormComponent;
pub use generated::{StaticFieldMetadata, StaticFormMetadata};
pub use validators::{ClientValidator, ValidatorRegistry};

// Re-export form metadata types for macro-generated code
// These are needed in both WASM and server environments
pub use reinhardt_forms::wasm_compat::{FieldMetadata, FormMetadata};

// Server-side only exports
#[cfg(native)]
pub use media::{Media, MediaDefiningWidget};
#[cfg(native)]
pub use rendering::{
	BootstrapRenderer, CheckboxInput, CheckboxSelectMultiple, CssFramework, DateInput, FileInput,
	RadioSelect, Select, SelectDateWidget, SelectMultiple, SplitDateTimeWidget, TailwindRenderer,
	TextInput, Widget, WidgetAttrs, WidgetType, html_escape,
};
