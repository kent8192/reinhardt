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
//! - **ModelFormState**: Converts schema-backed controls into one policy-safe payload
//!
//! ## Model-backed forms
//!
//! A `form!` declaration with `model`, `policy`, and exactly one of `fields` or
//! `exclude` uses the target-neutral schema emitted by `#[model(form = true)]`.
//! The named policy must implement [`ModelFormPolicy`] and be the same policy
//! used by the explicit `server_fn` payload; it is the authoritative
//! server-side field allowlist. The `fields` or `exclude` clause controls
//! rendered controls, while field overrides affect presentation only.
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

#[cfg(native)]
pub mod binding;
#[cfg(native)]
pub mod component;
pub mod generated;
pub mod model;
pub mod validators;

// Server-side only modules for HTML rendering and asset management
#[cfg(native)]
pub mod media;
#[cfg(native)]
pub mod rendering;

#[cfg(native)]
pub use binding::FormBinding;
#[cfg(native)]
pub use component::FormComponent;
pub use generated::{StaticFieldMetadata, StaticFormMetadata};
pub use model::ModelFormState;
pub use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload,
	ModelFormPayloadError, ModelFormPolicy, ModelFormPrimaryKey, ModelFormSchema,
	NativeModelFormPayload,
};
pub use validators::{ClientValidator, ValidatorRegistry};

// Re-export form metadata types for macro-generated code
// These are needed in both WASM and server environments
#[cfg(native)]
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
