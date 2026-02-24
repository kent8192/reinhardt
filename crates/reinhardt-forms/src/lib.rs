//! # Reinhardt Forms
//!
//! Form processing and validation for the Reinhardt framework.
//!
//! ## Overview
//!
//! This crate provides comprehensive form processing capabilities inspired by Django's
//! form system, focusing on data validation and multi-step form wizards.
//!
//! This crate is designed to be WASM-compatible, providing a pure form processing layer
//! without HTML generation or platform-specific features.
//!
//! ## Features
//!
//! - **[`Form`]**: Base form class with validation
//! - **[`ModelForm`]**: Auto-generated forms from model definitions
//! - **[`FormSet`]**: Handle multiple forms of the same type
//! - **[`FormWizard`]**: Multi-step form workflows
//! - **Field Types**: 20+ field types (CharField, IntegerField, EmailField, etc.)
//! - **WASM Support**: Compatible with WebAssembly targets via `wasm_compat` module
//!
//! ## Quick Start
//!
//! ### Basic Form
//!
//! ```rust,ignore
//! use reinhardt_forms::{Form, CharField, EmailField, IntegerField};
//!
//! #[derive(Form)]
//! struct ContactForm {
//!     name: CharField,
//!     email: EmailField,
//!     age: IntegerField,
//!     message: CharField,
//! }
//!
//! // Validate form data
//! let form = ContactForm::from_data(&request_data);
//! if form.is_valid() {
//!     let name = form.cleaned_data.name;
//!     // Process the form...
//! } else {
//!     let errors = form.errors();
//! }
//! ```
//!
//! ### Model Form
//!
//! ```rust,ignore
//! use reinhardt_forms::{ModelForm, ModelFormBuilder};
//!
//! // Auto-generate form from User model
//! let form = ModelFormBuilder::<User>::new()
//!     .fields(&["username", "email", "bio"])
//!     .exclude(&["password"])
//!     .build();
//! ```
//!
//! ## Available Field Types
//!
//! | Field | Description |
//! |-------|-------------|
//! | [`CharField`] | Text input with max_length validation |
//! | [`IntegerField`] | Integer input with min/max validation |
//! | [`FloatField`] | Floating-point number input |
//! | [`DecimalField`] | Decimal number with precision control |
//! | [`BooleanField`] | Checkbox input |
//! | [`EmailField`] | Email address validation |
//! | [`URLField`] | URL validation |
//! | [`DateField`] | Date input with format parsing |
//! | [`DateTimeField`] | DateTime input |
//! | [`TimeField`] | Time input |
//! | [`DurationField`] | Duration input |
//! | [`FileField`] | File upload |
//! | [`ImageField`] | Image upload with dimension validation |
//! | [`ChoiceField`] | Select dropdown |
//! | [`MultipleChoiceField`] | Multi-select |
//! | [`ModelChoiceField`] | Foreign key selection |
//! | [`JSONField`] | JSON data input |
//! | [`UUIDField`] | UUID input |
//! | [`SlugField`] | URL-safe slug input |
//! | [`RegexField`] | Custom regex validation |
//!
//! ## FormSets
//!
//! Handle multiple forms of the same type:
//!
//! ```rust,ignore
//! use reinhardt_forms::{FormSet, FormSetFactory};
//!
//! // Create a formset with 3 forms
//! let formset = FormSetFactory::<ItemForm>::new()
//!     .extra(3)
//!     .min_num(1)
//!     .max_num(10)
//!     .build();
//!
//! if formset.is_valid() {
//!     for form in formset.forms() {
//!         // Process each form
//!     }
//! }
//! ```
//!
//! ## Form Wizard
//!
//! Multi-step forms:
//!
//! ```rust,ignore
//! use reinhardt_forms::{FormWizard, WizardStep};
//!
//! let wizard = FormWizard::new()
//!     .add_step(WizardStep::new("account", AccountForm::new()))
//!     .add_step(WizardStep::new("profile", ProfileForm::new()))
//!     .add_step(WizardStep::new("confirmation", ConfirmForm::new()));
//!
//! // Process wizard step
//! let result = wizard.process_step(&request).await?;
//! ```

pub mod bound_field;
pub mod field;
pub mod fields;
pub mod form;
pub mod formset;
pub mod formsets;
pub mod model_form;
pub mod model_formset;
pub mod validators;
pub mod wasm_compat;
pub mod wizard;

pub use bound_field::BoundField;
pub use field::{
	ErrorType,
	FieldError,
	FieldResult,
	FormField as Field, // Alias for compatibility
	FormField,
	Widget,
	escape_attribute,
	html_escape,
};
pub use fields::{
	BooleanField, CharField, ChoiceField, ColorField, ComboField, DateField, DateTimeField,
	DecimalField, DurationField, EmailField, FileField, FloatField, GenericIPAddressField,
	IPProtocol, ImageField, IntegerField, JSONField, ModelChoiceField, ModelMultipleChoiceField,
	MultiValueField, MultipleChoiceField, PASSWORD_REDACTED, PasswordField, RegexField, SlugField,
	SplitDateTimeField, TimeField, URLField, UUIDField,
};
pub use form::{Form, FormError, FormResult};
pub use formset::FormSet;
pub use formsets::{
	FormSetFactory,
	InlineFormSet,
	ModelFormSet as AdvancedModelFormSet, // Renamed to avoid conflict
};
pub use model_form::{FieldType, FormModel, ModelForm, ModelFormBuilder, ModelFormConfig};
pub use model_formset::{ModelFormSet, ModelFormSetBuilder, ModelFormSetConfig};
pub use validators::{SlugValidator, UrlValidator};
pub use wizard::{FormWizard, WizardStep};
