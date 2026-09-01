//! Runtime behavior for `form!` generated forms.

use std::any::{Any, type_name};
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll};

use crate::reactive::{
	Action, ActionPhase, Effect, EffectTiming, ReactiveScope, Signal, use_action,
};
use crate::server_fn::ServerFnError;
use reinhardt_core::reactive::{ScopeId, current_scope_id, scope::enter_scope};

/// Polls form submission work inside the scope that owns the form state.
///
/// A disposed form scope cancels the submit future before it can access
/// disposed reactive handles.
struct ScopedFormFuture<Fut> {
	scope: ScopeId,
	future: Option<Pin<Box<Fut>>>,
}

impl<Fut> Future for ScopedFormFuture<Fut>
where
	Fut: Future,
{
	type Output = Option<Fut::Output>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		let Some(future) = this.future.as_mut() else {
			return Poll::Ready(None);
		};
		match enter_scope(this.scope, || future.as_mut().poll(cx)) {
			Ok(Poll::Pending) => Poll::Pending,
			Ok(Poll::Ready(output)) => {
				this.future.take();
				Poll::Ready(Some(output))
			}
			Err(_) => {
				this.future.take();
				Poll::Ready(None)
			}
		}
	}
}

/// Default reset behavior when runtime dependencies change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetOnDeps {
	/// Keep dirty field values and update pristine fields from new defaults.
	KeepDirtyValues,
	/// Replace every value with the new defaults.
	ResetAll,
	/// Record the dependency change without changing values.
	ExplicitOnly,
}

/// Validation timing for runtime-managed forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevalidateOn {
	/// Do not revalidate automatically.
	Submit,
	/// Revalidate when dependencies change.
	DepsChange,
	/// Revalidate whenever values are written through the runtime handle.
	Change,
}

/// No dependency marker for `use_form(&form).build()`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoDeps;

/// Opaque runtime key for a collection item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CollectionItemKey(u64);

impl CollectionItemKey {
	const GENERATED_KEY_FLAG: u64 = 1 << 63;

	/// Creates a collection item key from a generated runtime index.
	#[doc(hidden)]
	pub fn from_runtime_index(value: u64) -> Self {
		Self(value | Self::GENERATED_KEY_FLAG)
	}

	fn next(counter: &Cell<u64>) -> Self {
		let value = counter.get();
		assert!(
			value < Self::GENERATED_KEY_FLAG,
			"collection item key counter exhausted runtime key namespace"
		);
		counter.set(value + 1);
		Self(value)
	}
}

/// Runtime value paired with its collection item key and current index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionItem<T> {
	key: CollectionItemKey,
	index: usize,
	value: T,
}

impl<T> CollectionItem<T> {
	/// Creates a runtime collection item.
	pub fn new(key: CollectionItemKey, index: usize, value: T) -> Self {
		Self { key, index, value }
	}

	/// Returns the runtime key for this collection item.
	pub fn key(&self) -> CollectionItemKey {
		self.key
	}

	/// Returns the current positional index for this collection item.
	pub fn index(&self) -> usize {
		self.index
	}

	/// Returns the current value for this collection item.
	pub fn value(&self) -> &T {
		&self.value
	}

	/// Consumes this collection item and returns its value.
	pub fn into_value(self) -> T {
		self.value
	}
}

/// A field-level error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldError {
	message: String,
}

impl FieldError {
	/// Creates a field error from a displayable message.
	pub fn new(message: impl Into<String>) -> Self {
		Self {
			message: message.into(),
		}
	}

	/// Returns the error message.
	pub fn message(&self) -> &str {
		&self.message
	}
}

/// Experimental raw value kind requested by a custom widget adapter.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormWidgetValueKind {
	/// A single string value.
	Value,
	/// A boolean checked value.
	Checked,
	/// A file value. File parsing is not implemented yet.
	File,
	/// Multiple string values.
	MultiValue,
}

/// Experimental raw value passed between generated forms and custom widget adapters.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CustomWidgetRawValue {
	/// A single string value.
	String(String),
	/// A boolean checked value.
	Bool(bool),
	/// Multiple string values.
	Strings(Vec<String>),
	/// A raw value kind not supported by the current experimental runtime.
	Unsupported,
}

/// Experimental error returned by a custom widget adapter.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormWidgetError {
	message: String,
}

impl FormWidgetError {
	/// Creates an experimental custom widget adapter error.
	pub fn new(message: impl Into<String>) -> Self {
		Self {
			message: message.into(),
		}
	}

	/// Returns the experimental custom widget adapter error message.
	pub fn message(&self) -> &str {
		&self.message
	}
}

/// Experimental context passed to custom widget adapters.
#[non_exhaustive]
#[derive(Clone)]
pub struct CustomWidgetContext<Value> {
	/// Current typed field value.
	pub value: Value,
	/// Whether the generated field should be disabled.
	pub disabled: bool,
	/// Whether the generated field is required.
	pub required: bool,
	/// Whether the generated field has been touched.
	pub touched: bool,
	/// Current field error, if any.
	pub error: Option<FieldError>,
	/// Generated form field name.
	pub name: String,
	/// Generated form field id.
	pub id: String,
	/// Experimental raw-change callback for custom widget components.
	///
	/// The generated form parses raw values through the adapter and updates the
	/// generated field state when parsing succeeds. Parse failures are returned
	/// to the caller as experimental adapter errors and surfaced through
	/// runtime field error state.
	pub on_raw_change: Rc<dyn Fn(CustomWidgetRawValue) -> Result<(), FormWidgetError>>,
}

impl<Value> CustomWidgetContext<Value> {
	/// Creates an experimental custom widget context.
	pub fn new(
		value: Value,
		on_raw_change: Rc<dyn Fn(CustomWidgetRawValue) -> Result<(), FormWidgetError>>,
	) -> Self {
		Self {
			value,
			disabled: false,
			required: false,
			touched: false,
			error: None,
			name: String::new(),
			id: String::new(),
			on_raw_change,
		}
	}
}

/// Experimental adapter trait for `CustomWidget(...)` generated form fields.
pub trait FormWidgetAdapter<FieldValue> {
	/// Props accepted by the custom widget component.
	type ComponentProps;

	/// Returns the raw value kind consumed by this experimental adapter.
	fn value_kind() -> FormWidgetValueKind;

	/// Builds component props from the generated field context.
	fn props(ctx: CustomWidgetContext<FieldValue>) -> Self::ComponentProps;

	/// Parses a raw widget value into the typed field value.
	fn parse(raw: CustomWidgetRawValue) -> Result<FieldValue, FormWidgetError>;

	/// Formats a typed field value into a raw widget value.
	fn format(value: &FieldValue) -> CustomWidgetRawValue;
}

/// Validation failure for a generated form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormValidationError<Field>
where
	Field: Copy + Eq + Hash,
{
	details: Box<FormValidationErrorDetails<Field>>,
	form_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FormValidationErrorDetails<Field>
where
	Field: Copy + Eq + Hash,
{
	field_errors: HashMap<Field, FieldError>,
	collection_errors: HashMap<String, FieldError>,
	path_errors: HashMap<String, FieldError>,
}

impl<Field> FormValidationErrorDetails<Field>
where
	Field: Copy + Eq + Hash,
{
	fn new() -> Self {
		Self {
			field_errors: HashMap::new(),
			collection_errors: HashMap::new(),
			path_errors: HashMap::new(),
		}
	}
}

impl<Field> FormValidationError<Field>
where
	Field: Copy + Eq + Hash,
{
	/// Creates an empty validation error.
	pub fn new() -> Self {
		Self {
			details: Box::new(FormValidationErrorDetails::new()),
			form_error: None,
		}
	}

	/// Creates a field validation error.
	pub fn field(field: Field, message: impl Into<String>) -> Self {
		let mut details = FormValidationErrorDetails::new();
		details.field_errors.insert(field, FieldError::new(message));
		Self {
			details: Box::new(details),
			form_error: None,
		}
	}

	/// Creates a form-level validation error.
	pub fn form(message: impl Into<String>) -> Self {
		Self {
			details: Box::new(FormValidationErrorDetails::new()),
			form_error: Some(message.into()),
		}
	}

	/// Returns field errors.
	pub fn field_errors(&self) -> &HashMap<Field, FieldError> {
		&self.details.field_errors
	}

	/// Returns generated collection validation errors.
	pub fn collection_errors(&self) -> &HashMap<String, FieldError> {
		&self.details.collection_errors
	}

	/// Returns nested collection path errors.
	pub fn path_errors(&self) -> &HashMap<String, FieldError> {
		&self.details.path_errors
	}

	/// Returns the form-level error, if any.
	pub fn form_error(&self) -> Option<&str> {
		self.form_error.as_deref()
	}

	/// Adds or replaces one field validation error.
	pub fn add_field_error(&mut self, field: Field, message: impl Into<String>) {
		self.details
			.field_errors
			.insert(field, FieldError::new(message));
	}

	/// Adds or replaces one generated collection validation error.
	pub fn add_collection_error(
		&mut self,
		collection_key: impl Into<String>,
		message: impl Into<String>,
	) {
		self.details
			.collection_errors
			.insert(collection_key.into(), FieldError::new(message));
	}

	/// Adds or replaces one nested collection path validation error.
	pub fn add_path_error(&mut self, path_key: impl Into<String>, message: impl Into<String>) {
		self.details
			.path_errors
			.insert(path_key.into(), FieldError::new(message));
	}

	/// Sets the form-level validation error.
	pub fn set_form_error(&mut self, message: impl Into<String>) {
		self.form_error = Some(message.into());
	}

	/// Returns whether this error contains no field, path, or form error.
	pub fn is_empty(&self) -> bool {
		self.details.field_errors.is_empty()
			&& self.details.collection_errors.is_empty()
			&& self.details.path_errors.is_empty()
			&& self.form_error.is_none()
	}
}

impl<Field> Default for FormValidationError<Field>
where
	Field: Copy + Eq + Hash,
{
	fn default() -> Self {
		Self::new()
	}
}

/// Aggregate state for a runtime form.
#[derive(Clone)]
pub struct FormState<Field>
where
	Field: Copy + Eq + Hash + 'static,
{
	/// Whether current values differ from defaults.
	pub is_dirty: Signal<bool>,
	/// Whether any value was changed through the runtime.
	pub is_touched: Signal<bool>,
	/// Field-level errors.
	pub field_errors: Signal<HashMap<Field, FieldError>>,
	/// Form-level validation error.
	pub form_error: Signal<Option<String>>,
	/// Last submit-level error.
	pub submit_error: Signal<Option<String>>,
	/// First visible validation or submit error.
	pub error: Signal<Option<String>>,
	/// Whether submit is pending.
	pub is_submitting: Signal<bool>,
	/// Whether the last submit succeeded.
	pub is_submit_successful: Signal<bool>,
}

/// Snapshot for one generated field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldState {
	/// Whether this field differs from its default value.
	pub is_dirty: bool,
	/// Whether this field was changed through the runtime.
	pub is_touched: bool,
	/// Current field-level error.
	pub error: Option<FieldError>,
}

/// Snapshot for a nested field path inside a generated collection field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldPathState {
	/// Whether this field path differs from its default value.
	pub is_dirty: bool,
	/// Whether this field path was changed through the runtime.
	pub is_touched: bool,
	/// Current field path-level error.
	pub error: Option<FieldError>,
}

/// Snapshot for a generated collection field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionState {
	/// Number of items currently present in the collection.
	pub len: usize,
	/// Whether the collection differs from its default value.
	pub is_dirty: bool,
	/// Whether the collection was changed through the runtime.
	pub is_touched: bool,
	/// Current collection-level error.
	pub error: Option<FieldError>,
}

/// Form runtime event sent to subscribers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormEvent<Form>
where
	Form: FormRuntimeSource,
{
	/// A field value changed through the runtime handle.
	ValueChanged {
		/// Field whose value changed.
		field: Form::Field,
	},
	/// Validation was run.
	Validated,
	/// Submit started.
	SubmitStarted,
	/// Submit completed successfully.
	Submitted,
	/// Submit failed before dispatch or during validation.
	SubmitFailed,
	/// Runtime dependencies changed.
	DepsChanged,
}

/// Result of `UseFormReturn::handle_submit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseFormSubmitOutcome {
	/// Submit was accepted.
	Submitted,
	/// Submit was rejected because another submit is pending.
	AlreadyPending,
	/// Submit was rejected by validation.
	ValidationFailed,
}

/// Result of `UseFormReturn::submit_async`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UseFormAsyncSubmitOutcome<T> {
	/// Submit was accepted and completed with the async output.
	Submitted(T),
	/// Submit was rejected because another submit is pending.
	AlreadyPending,
	/// Submit was rejected by validation.
	ValidationFailed,
}

/// Typed action handle returned by [`use_form_action`].
///
/// `FormAction` validates a [`UseFormReturn`] before dispatching the current
/// generated value struct through `use_action`. On WASM, the action result
/// completes the form submit lifecycle and updates pending, error, and success
/// state. On native targets, `use_action` intentionally does not poll async
/// mutations, so the helper validates and dispatches the payload, then clears
/// the pending flag without manufacturing a success result.
pub struct FormAction<Form, Deps, T, E>
where
	Form: FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
	T: Clone + 'static,
	E: Clone + Display + 'static,
{
	form: UseFormReturn<Form, Deps>,
	action: Action<T, E>,
}

/// Error returned by focus operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusError {
	/// Focus is not available for the active target.
	Unsupported,
	/// The generated form does not expose a focus target for the field.
	MissingTarget,
}

/// Trait implemented by `form!` generated forms.
pub trait FormRuntimeSource: Clone + 'static {
	/// Generated value struct for this form.
	type Values: Clone + 'static;
	/// Generated field token enum for this form.
	type Field: Copy + Eq + Hash + Debug + 'static;

	/// Returns the retained scope for a form constructed outside a page render.
	fn runtime_reactive_scope(&self) -> Option<Rc<ReactiveScope>> {
		None
	}

	/// Returns current defaults captured by the form definition.
	fn runtime_initial_values(&self) -> Self::Values;

	/// Reads current values from generated field controls.
	fn runtime_current_values(&self) -> Self::Values;

	/// Applies all values to generated field controls.
	fn runtime_apply_values(&self, values: &Self::Values);

	/// Applies one typed value to one generated field.
	fn runtime_set_field_value<T>(&self, field: Self::Field, value: T)
	where
		T: Any + 'static;

	/// Applies the matching value from `values` to one generated field.
	fn runtime_apply_field_value(&self, field: Self::Field, values: &Self::Values);

	/// Returns whether one field differs between `current` and `defaults`.
	fn runtime_field_is_dirty(
		&self,
		field: Self::Field,
		current: &Self::Values,
		defaults: &Self::Values,
	) -> bool;

	/// Returns whether the generated values differ from the comparison baseline.
	fn runtime_values_are_dirty(&self, current: &Self::Values, defaults: &Self::Values) -> bool {
		self.runtime_fields()
			.iter()
			.copied()
			.any(|field| self.runtime_field_is_dirty(field, current, defaults))
	}

	/// Applies new defaults to pristine runtime values while preserving dirty values.
	fn runtime_apply_pristine_values(
		&self,
		current: &Self::Values,
		old_defaults: &Self::Values,
		new_defaults: &Self::Values,
	) {
		for field in self.runtime_fields() {
			let field = *field;
			if !self.runtime_field_is_dirty(field, current, old_defaults) {
				self.runtime_apply_field_value(field, new_defaults);
			}
		}
	}

	/// Returns the generated signal for a field when `T` matches the field type.
	fn runtime_watch_field<T>(&self, field: Self::Field) -> Option<Signal<T>>
	where
		T: Clone + 'static;

	/// Resolves a serialized field name to its generated field token.
	fn runtime_field_by_name(&self, _name: &str) -> Option<Self::Field> {
		None
	}

	/// Returns the current custom-widget bridge error for one generated field.
	fn runtime_custom_widget_error(&self, _field: Self::Field) -> Option<FieldError> {
		None
	}

	/// Sets or clears the current custom-widget bridge error for one generated field.
	fn runtime_set_custom_widget_error(&self, _field: Self::Field, _error: Option<FieldError>) {}

	/// Captures nested collection field values using the current runtime item keys.
	fn runtime_path_values_from_values(
		&self,
		_values: &Self::Values,
	) -> HashMap<String, Rc<dyn Any>> {
		HashMap::new()
	}

	/// Captures current nested collection field values using runtime item keys.
	fn runtime_path_values(&self) -> HashMap<String, Rc<dyn Any>> {
		self.runtime_path_values_from_values(&self.runtime_current_values())
	}

	/// Returns whether a stable nested path key currently exists.
	fn runtime_path_exists(&self, _path_key: &str) -> bool {
		false
	}

	/// Compares a current nested path value with a captured default value.
	fn runtime_path_value_equals(&self, _path_key: &str, _default: &dyn Any) -> Option<bool> {
		None
	}

	/// Synchronizes cached nested field path signals from current form values.
	fn runtime_sync_path_signals(&self) {}

	/// Returns collection keys whose values changed between two value snapshots.
	fn runtime_changed_collection_keys(
		&self,
		_current: &Self::Values,
		_previous: &Self::Values,
	) -> Vec<String> {
		Vec::new()
	}

	/// Runs generated validation.
	fn runtime_validate(&self) -> Result<(), FormValidationError<Self::Field>> {
		Ok(())
	}

	/// Attempts to focus a generated field.
	fn runtime_set_focus(&self, _field: Self::Field) -> Result<(), FocusError> {
		Err(FocusError::Unsupported)
	}

	/// Returns generated field tokens in source order.
	fn runtime_fields(&self) -> &'static [Self::Field];
}

/// Trait implemented by `form!` generated forms that expose runtime collections.
pub trait FormCollectionRuntimeSource: FormRuntimeSource {
	/// Generated collection token enum for this form.
	type Collection: Copy + Eq + Hash + Debug + 'static;
	/// Generated field path token enum for nested collection fields.
	type FieldPath: Clone + Eq + Hash + Debug + 'static;

	/// Returns the stable runtime map key for a generated field path.
	fn runtime_field_path_key(path: &Self::FieldPath) -> String;

	/// Returns the stable runtime map key for a generated collection.
	fn runtime_collection_key(collection: Self::Collection) -> String;

	/// Returns stable runtime map keys for every nested field path in an item.
	fn runtime_field_path_keys_for_item(
		collection: Self::Collection,
		key: CollectionItemKey,
	) -> Vec<String>;

	/// Returns the current length for a generated collection.
	fn runtime_collection_len(&self, collection: Self::Collection) -> usize;

	/// Returns whether one generated collection differs between values and defaults.
	fn runtime_collection_is_dirty(
		&self,
		collection: Self::Collection,
		current: &Self::Values,
		defaults: &Self::Values,
	) -> bool;

	/// Inserts one collection item value at the requested index.
	fn runtime_insert_collection_item<T>(
		&self,
		collection: Self::Collection,
		index: usize,
		key: CollectionItemKey,
		value: T,
	) where
		T: Any + Clone + 'static;

	/// Removes one collection item by key.
	fn runtime_remove_collection_item(
		&self,
		collection: Self::Collection,
		key: CollectionItemKey,
	) -> bool;

	/// Moves one collection item by key.
	fn runtime_move_collection_item(
		&self,
		collection: Self::Collection,
		key: CollectionItemKey,
		target_index: usize,
	) -> Option<(usize, usize)>;

	/// Returns the generated signal for a nested field path when `T` matches.
	fn runtime_watch_path<T>(&self, path: Self::FieldPath) -> Option<Signal<T>>
	where
		T: Clone + 'static;

	/// Applies one typed value to a generated nested field path.
	fn runtime_set_path_value<T>(&self, path: Self::FieldPath, value: T) -> bool
	where
		T: Any + 'static;

	/// Returns whether one nested field path differs between values and defaults.
	fn runtime_path_is_dirty(
		&self,
		path: &Self::FieldPath,
		current: &Self::Values,
		defaults: &Self::Values,
	) -> bool;
}

/// Builder returned by `use_form(&form)`.
pub struct UseFormBuilder<Form, Deps = NoDeps>
where
	Form: FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
{
	form: Form,
	deps: Deps,
	reset_on_deps: ResetOnDeps,
	keep_errors: bool,
	revalidate_on: RevalidateOn,
	on_submit_start: Option<SubmitCallback<Form, Deps>>,
	on_submit_success: Option<SubmitCallback<Form, Deps>>,
	on_submit_error: Option<SubmitCallback<Form, Deps>>,
}

type SubmitCallback<Form, Deps> = Rc<dyn Fn(&UseFormReturn<Form, Deps>)>;
type Subscriber<Form> = Rc<dyn Fn(FormEvent<Form>)>;
type SubscriberSlots<Form> = Rc<RefCell<Vec<Option<Subscriber<Form>>>>>;

/// Owns the form synchronization effect until the final runtime handle drops.
///
/// Forms created outside an active render retain their own reactive scope. The
/// synchronization effect captures a form clone, so disposing it during handle
/// teardown is necessary to break that retention path.
struct SignalSyncEffectGuard {
	effect: Effect,
}

impl Drop for SignalSyncEffectGuard {
	fn drop(&mut self) {
		self.effect.dispose();
	}
}

fn form_values_are_dirty<Form>(form: &Form, current: &Form::Values, defaults: &Form::Values) -> bool
where
	Form: FormRuntimeSource,
{
	form.runtime_values_are_dirty(current, defaults)
}

#[allow(
	clippy::too_many_arguments,
	reason = "The sync effect needs the independent form state handles it updates atomically."
)]
fn build_signal_sync_effect<Form>(
	form: Form,
	default_values: Rc<RefCell<Form::Values>>,
	state: FormState<Form::Field>,
	touched_fields: Rc<RefCell<HashMap<Form::Field, bool>>>,
	touched_collections: Rc<RefCell<HashMap<String, bool>>>,
	touched_paths: Rc<RefCell<HashMap<String, bool>>>,
	collection_errors: Signal<HashMap<String, FieldError>>,
	path_errors: Signal<HashMap<String, FieldError>>,
	values_signal: Signal<Form::Values>,
	subscribers: SubscriberSlots<Form>,
	observed_values: Rc<RefCell<Form::Values>>,
	custom_widget_error_fields: Rc<RefCell<HashMap<Form::Field, FieldError>>>,
	signal_sync_suppressed: Rc<Cell<bool>>,
	revalidate_on: RevalidateOn,
) -> Rc<SignalSyncEffectGuard>
where
	Form: FormRuntimeSource,
{
	let effect = Effect::new_with_timing(
		move || {
			let current = form.runtime_current_values();
			let previous = observed_values.borrow().clone();
			let custom_widget_errors = collect_custom_widget_errors(&form);
			let changed_fields: Vec<Form::Field> = form
				.runtime_fields()
				.iter()
				.copied()
				.filter(|field| form.runtime_field_is_dirty(*field, &current, &previous))
				.collect();
			let values_changed = form.runtime_values_are_dirty(&current, &previous);
			let current_path_values = form.runtime_path_values_from_values(&current);
			let previous_path_values = form.runtime_path_values_from_values(&previous);
			let changed_path_keys: Vec<String> = current_path_values
				.keys()
				.filter(|path_key| {
					previous_path_values
						.get(*path_key)
						.and_then(|previous_value| {
							form.runtime_path_value_equals(path_key, previous_value.as_ref())
						})
						.map(|matches_current| !matches_current)
						.unwrap_or(true)
				})
				.cloned()
				.collect();
			let changed_collection_keys = form.runtime_changed_collection_keys(&current, &previous);
			*observed_values.borrow_mut() = current.clone();

			if signal_sync_suppressed.get() || (changed_fields.is_empty() && !values_changed) {
				if !signal_sync_suppressed.get() {
					sync_custom_widget_errors_in_state(
						&state,
						&custom_widget_error_fields,
						&custom_widget_errors,
						&collection_errors,
						&path_errors,
					);
				}
				return;
			}

			for field in &changed_fields {
				touched_fields.borrow_mut().insert(*field, true);
			}
			for collection_key in changed_collection_keys {
				touched_collections
					.borrow_mut()
					.insert(collection_key, true);
			}
			{
				let mut touched_paths = touched_paths.borrow_mut();
				touched_paths.retain(|path_key, _| current_path_values.contains_key(path_key));
				for path_key in changed_path_keys {
					touched_paths.insert(path_key, true);
				}
			}
			let mut path_errors_map = path_errors.get();
			path_errors_map.retain(|path_key, _| current_path_values.contains_key(path_key));
			path_errors.set(path_errors_map);
			form.runtime_sync_path_signals();
			state.is_touched.set(true);
			state.is_dirty.set(form_values_are_dirty(
				&form,
				&current,
				&default_values.borrow(),
			));
			values_signal.set(current);

			if revalidate_on == RevalidateOn::Change {
				let result = form.runtime_validate();
				apply_validation_result_to_state(&state, &collection_errors, &path_errors, &result);
				notify_subscribers(&subscribers, FormEvent::Validated);
			}
			sync_custom_widget_errors_in_state(
				&state,
				&custom_widget_error_fields,
				&custom_widget_errors,
				&collection_errors,
				&path_errors,
			);
			for field in changed_fields {
				notify_subscribers(&subscribers, FormEvent::ValueChanged { field });
			}
		},
		EffectTiming::Layout,
	);
	Rc::new(SignalSyncEffectGuard { effect })
}

fn collect_custom_widget_errors<Form>(form: &Form) -> HashMap<Form::Field, FieldError>
where
	Form: FormRuntimeSource,
{
	form.runtime_fields()
		.iter()
		.copied()
		.filter_map(|field| {
			form.runtime_custom_widget_error(field)
				.map(|error| (field, error))
		})
		.collect()
}

fn sync_custom_widget_errors_in_state<Field>(
	state: &FormState<Field>,
	tracked_fields: &Rc<RefCell<HashMap<Field, FieldError>>>,
	custom_widget_errors: &HashMap<Field, FieldError>,
	collection_errors: &Signal<HashMap<String, FieldError>>,
	path_errors: &Signal<HashMap<String, FieldError>>,
) where
	Field: Copy + Eq + Hash + 'static,
{
	let mut tracked = tracked_fields.borrow_mut();
	let mut field_errors = state.field_errors.get_untracked();
	let mut changed = false;

	let previously_tracked: Vec<(Field, FieldError)> = tracked
		.iter()
		.map(|(field, error)| (*field, error.clone()))
		.collect();
	for (field, previous_error) in previously_tracked {
		if !custom_widget_errors.contains_key(&field) {
			tracked.remove(&field);
			if field_errors.get(&field) == Some(&previous_error) {
				field_errors.remove(&field);
				changed = true;
			}
		}
	}

	for (field, error) in custom_widget_errors {
		if field_errors.get(field) != Some(error) {
			field_errors.insert(*field, error.clone());
			changed = true;
		}
		tracked.insert(*field, error.clone());
	}

	if changed {
		state.field_errors.set(field_errors);
		sync_first_error_in_state(state, collection_errors, path_errors);
	}
}

fn adapt_no_deps_callback<Form, Deps>(
	callback: SubmitCallback<Form, NoDeps>,
) -> SubmitCallback<Form, Deps>
where
	Form: FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
{
	Rc::new(move |handle| {
		let no_deps_handle = UseFormReturn {
			form: handle.form.clone(),
			scope: handle.scope,
			default_values: Rc::clone(&handle.default_values),
			deps: Rc::new(RefCell::new(NoDeps)),
			reset_on_deps: handle.reset_on_deps,
			keep_errors: handle.keep_errors,
			revalidate_on: handle.revalidate_on,
			state: handle.state.clone(),
			touched_fields: Rc::clone(&handle.touched_fields),
			touched_collections: Rc::clone(&handle.touched_collections),
			touched_paths: Rc::clone(&handle.touched_paths),
			collection_errors: handle.collection_errors,
			path_errors: handle.path_errors,
			path_default_values: Rc::clone(&handle.path_default_values),
			values_signal: handle.values_signal,
			subscribers: Rc::clone(&handle.subscribers),
			observed_values: Rc::clone(&handle.observed_values),
			custom_widget_error_fields: Rc::clone(&handle.custom_widget_error_fields),
			next_collection_item_key: Rc::clone(&handle.next_collection_item_key),
			signal_sync_suppressed: Rc::clone(&handle.signal_sync_suppressed),
			_signal_sync_effect: Rc::clone(&handle._signal_sync_effect),
			on_submit_start: None,
			on_submit_success: None,
			on_submit_error: None,
		};
		callback(&no_deps_handle);
	})
}

fn apply_validation_result_to_state<Field>(
	state: &FormState<Field>,
	collection_errors: &Signal<HashMap<String, FieldError>>,
	path_errors: &Signal<HashMap<String, FieldError>>,
	result: &Result<(), FormValidationError<Field>>,
) where
	Field: Copy + Eq + Hash + 'static,
{
	match result {
		Ok(()) => {
			state.field_errors.set(HashMap::new());
			collection_errors.set(HashMap::new());
			path_errors.set(HashMap::new());
			state.form_error.set(None);
			sync_first_error_in_state(state, collection_errors, path_errors);
		}
		Err(error) => {
			state.field_errors.set(error.field_errors().clone());
			collection_errors.set(error.collection_errors().clone());
			path_errors.set(error.path_errors().clone());
			state.form_error.set(error.form_error().map(str::to_string));
			sync_first_error_in_state(state, collection_errors, path_errors);
		}
	}
}

fn sync_first_error_in_state<Field>(
	state: &FormState<Field>,
	collection_errors: &Signal<HashMap<String, FieldError>>,
	path_errors: &Signal<HashMap<String, FieldError>>,
) where
	Field: Copy + Eq + Hash + 'static,
{
	let first_field_error = state
		.field_errors
		.get()
		.values()
		.next()
		.map(|error| error.message().to_string());
	let first_collection_error = collection_errors
		.get()
		.values()
		.next()
		.map(|error| error.message().to_string());
	let first_path_error = path_errors
		.get()
		.values()
		.next()
		.map(|error| error.message().to_string());
	state.error.set(
		first_field_error
			.or(first_collection_error)
			.or(first_path_error)
			.or_else(|| state.form_error.get())
			.or_else(|| state.submit_error.get()),
	);
}

fn notify_subscribers<Form>(subscribers: &SubscriberSlots<Form>, event: FormEvent<Form>)
where
	Form: FormRuntimeSource,
{
	for subscriber in subscribers.borrow().iter().flatten() {
		subscriber(event.clone());
	}
}

impl<Form> UseFormBuilder<Form, NoDeps>
where
	Form: FormRuntimeSource,
{
	/// Sets runtime dependencies.
	pub fn deps<Deps>(self, deps: Deps) -> UseFormBuilder<Form, Deps>
	where
		Deps: Clone + PartialEq + 'static,
	{
		UseFormBuilder {
			form: self.form,
			deps,
			reset_on_deps: self.reset_on_deps,
			keep_errors: self.keep_errors,
			revalidate_on: self.revalidate_on,
			on_submit_start: self.on_submit_start.map(adapt_no_deps_callback),
			on_submit_success: self.on_submit_success.map(adapt_no_deps_callback),
			on_submit_error: self.on_submit_error.map(adapt_no_deps_callback),
		}
	}
}

impl<Form, Deps> UseFormBuilder<Form, Deps>
where
	Form: FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
{
	/// Sets dependency reset behavior.
	pub fn reset_on_deps(mut self, policy: ResetOnDeps) -> Self {
		self.reset_on_deps = policy;
		self
	}

	/// Sets whether dependency changes keep existing errors.
	pub fn keep_errors(mut self, keep: bool) -> Self {
		self.keep_errors = keep;
		self
	}

	/// Sets revalidation timing.
	pub fn revalidate_on(mut self, timing: RevalidateOn) -> Self {
		self.revalidate_on = timing;
		self
	}

	/// Registers a submit-start callback.
	pub fn on_submit_start<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&UseFormReturn<Form, Deps>) + 'static,
	{
		self.on_submit_start = Some(Rc::new(callback));
		self
	}

	/// Registers a submit-success callback.
	pub fn on_submit_success<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&UseFormReturn<Form, Deps>) + 'static,
	{
		self.on_submit_success = Some(Rc::new(callback));
		self
	}

	/// Registers a submit-error callback.
	pub fn on_submit_error<Callback>(mut self, callback: Callback) -> Self
	where
		Callback: Fn(&UseFormReturn<Form, Deps>) + 'static,
	{
		self.on_submit_error = Some(Rc::new(callback));
		self
	}

	/// Builds the runtime form handle.
	pub fn build(self) -> UseFormReturn<Form, Deps> {
		if let Some(scope) = self.form.runtime_reactive_scope() {
			scope.enter(|| self.build_in_active_scope())
		} else {
			self.build_in_active_scope()
		}
	}

	fn build_in_active_scope(self) -> UseFormReturn<Form, Deps> {
		let scope = current_scope_id().expect("use_form requires an active ReactiveScope");
		let default_values = self.form.runtime_initial_values();
		let current_values = self.form.runtime_current_values();
		let is_dirty = form_values_are_dirty(&self.form, &current_values, &default_values);
		let form = self.form;
		let path_default_values = Rc::new(RefCell::new(
			form.runtime_path_values_from_values(&default_values),
		));
		let default_values = Rc::new(RefCell::new(default_values));
		let deps = Rc::new(RefCell::new(self.deps));
		let state = FormState {
			is_dirty: Signal::new(is_dirty),
			is_touched: Signal::new(false),
			field_errors: Signal::new(HashMap::new()),
			form_error: Signal::new(None),
			submit_error: Signal::new(None),
			error: Signal::new(None),
			is_submitting: Signal::new(false),
			is_submit_successful: Signal::new(false),
		};
		let touched_fields = Rc::new(RefCell::new(HashMap::new()));
		let touched_collections = Rc::new(RefCell::new(HashMap::new()));
		let touched_paths = Rc::new(RefCell::new(HashMap::new()));
		let collection_errors = Signal::new(HashMap::new());
		let path_errors = Signal::new(HashMap::new());
		let values_signal = Signal::new(current_values.clone());
		let subscribers = Rc::new(RefCell::new(Vec::new()));
		let observed_values = Rc::new(RefCell::new(current_values));
		let custom_widget_error_fields = Rc::new(RefCell::new(HashMap::new()));
		let next_collection_item_key = Rc::new(Cell::new(1));
		let signal_sync_suppressed = Rc::new(Cell::new(false));
		let signal_sync_effect = build_signal_sync_effect(
			form.clone(),
			Rc::clone(&default_values),
			state.clone(),
			Rc::clone(&touched_fields),
			Rc::clone(&touched_collections),
			Rc::clone(&touched_paths),
			collection_errors,
			path_errors,
			values_signal,
			Rc::clone(&subscribers),
			Rc::clone(&observed_values),
			Rc::clone(&custom_widget_error_fields),
			Rc::clone(&signal_sync_suppressed),
			self.revalidate_on,
		);
		UseFormReturn {
			form,
			scope,
			default_values,
			deps,
			reset_on_deps: self.reset_on_deps,
			keep_errors: self.keep_errors,
			revalidate_on: self.revalidate_on,
			state,
			touched_fields,
			touched_collections,
			touched_paths,
			collection_errors,
			path_errors,
			path_default_values,
			values_signal,
			subscribers,
			observed_values,
			custom_widget_error_fields,
			next_collection_item_key,
			signal_sync_suppressed,
			_signal_sync_effect: signal_sync_effect,
			on_submit_start: self.on_submit_start,
			on_submit_success: self.on_submit_success,
			on_submit_error: self.on_submit_error,
		}
	}
}

/// Dynamic behavior handle for a `form!` generated form.
pub struct UseFormReturn<Form, Deps = NoDeps>
where
	Form: FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
{
	form: Form,
	scope: ScopeId,
	default_values: Rc<RefCell<Form::Values>>,
	deps: Rc<RefCell<Deps>>,
	reset_on_deps: ResetOnDeps,
	keep_errors: bool,
	revalidate_on: RevalidateOn,
	state: FormState<Form::Field>,
	touched_fields: Rc<RefCell<HashMap<Form::Field, bool>>>,
	touched_collections: Rc<RefCell<HashMap<String, bool>>>,
	touched_paths: Rc<RefCell<HashMap<String, bool>>>,
	collection_errors: Signal<HashMap<String, FieldError>>,
	path_errors: Signal<HashMap<String, FieldError>>,
	path_default_values: Rc<RefCell<HashMap<String, Rc<dyn Any>>>>,
	values_signal: Signal<Form::Values>,
	subscribers: SubscriberSlots<Form>,
	observed_values: Rc<RefCell<Form::Values>>,
	custom_widget_error_fields: Rc<RefCell<HashMap<Form::Field, FieldError>>>,
	next_collection_item_key: Rc<Cell<u64>>,
	signal_sync_suppressed: Rc<Cell<bool>>,
	_signal_sync_effect: Rc<SignalSyncEffectGuard>,
	on_submit_start: Option<SubmitCallback<Form, Deps>>,
	on_submit_success: Option<SubmitCallback<Form, Deps>>,
	on_submit_error: Option<SubmitCallback<Form, Deps>>,
}

impl<Form, Deps> Clone for UseFormReturn<Form, Deps>
where
	Form: FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
{
	fn clone(&self) -> Self {
		Self {
			form: self.form.clone(),
			scope: self.scope,
			default_values: Rc::clone(&self.default_values),
			deps: Rc::clone(&self.deps),
			reset_on_deps: self.reset_on_deps,
			keep_errors: self.keep_errors,
			revalidate_on: self.revalidate_on,
			state: self.state.clone(),
			touched_fields: Rc::clone(&self.touched_fields),
			touched_collections: Rc::clone(&self.touched_collections),
			touched_paths: Rc::clone(&self.touched_paths),
			collection_errors: self.collection_errors,
			path_errors: self.path_errors,
			path_default_values: Rc::clone(&self.path_default_values),
			values_signal: self.values_signal,
			subscribers: Rc::clone(&self.subscribers),
			observed_values: Rc::clone(&self.observed_values),
			custom_widget_error_fields: Rc::clone(&self.custom_widget_error_fields),
			next_collection_item_key: Rc::clone(&self.next_collection_item_key),
			signal_sync_suppressed: Rc::clone(&self.signal_sync_suppressed),
			_signal_sync_effect: Rc::clone(&self._signal_sync_effect),
			on_submit_start: self.on_submit_start.clone(),
			on_submit_success: self.on_submit_success.clone(),
			on_submit_error: self.on_submit_error.clone(),
		}
	}
}

impl<Form, Deps> UseFormReturn<Form, Deps>
where
	Form: FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
{
	/// Returns the generated form source retained by this runtime.
	#[doc(hidden)]
	pub fn __reinhardt_form_source(&self) -> Form {
		self.form.clone()
	}

	/// Returns a signal containing the current value struct.
	pub fn watch(&self) -> Signal<Form::Values> {
		self.values_signal
	}

	/// Returns a typed field signal.
	pub fn watch_field<T>(&self, field: Form::Field) -> Signal<T>
	where
		T: Clone + 'static,
	{
		self.form.runtime_watch_field(field).unwrap_or_else(|| {
			panic!(
				"field {:?} is not compatible with requested Signal<{}>",
				field,
				type_name::<T>()
			)
		})
	}

	/// Returns current typed values.
	pub fn get_values(&self) -> Form::Values {
		self.form.runtime_current_values()
	}

	/// Returns current default values.
	pub fn default_values(&self) -> Form::Values {
		self.default_values.borrow().clone()
	}

	/// Returns state for one field.
	pub fn get_field_state(&self, field: Form::Field) -> FieldState {
		let current = self.get_values();
		let defaults = self.default_values.borrow();
		let errors = self.state.field_errors.get();
		FieldState {
			is_dirty: self.form.runtime_field_is_dirty(field, &current, &defaults),
			is_touched: self
				.touched_fields
				.borrow()
				.get(&field)
				.copied()
				.unwrap_or(false),
			error: errors.get(&field).cloned(),
		}
	}

	/// Applies one typed field value.
	pub fn set_value<T>(&self, field: Form::Field, value: T)
	where
		T: Any + 'static,
	{
		let _guard = self.suppress_signal_sync();
		self.form.runtime_set_field_value(field, value);
		self.touched_fields.borrow_mut().insert(field, true);
		self.state.is_touched.set(true);
		self.refresh_dirty();
		self.values_signal.set(self.get_values());
		self.sync_observed_values();
		if self.revalidate_on == RevalidateOn::Change {
			let _ = self.trigger();
		}
		self.notify(FormEvent::ValueChanged { field });
	}

	/// Applies all typed values.
	pub fn set_values(&self, values: Form::Values) {
		let _guard = self.suppress_signal_sync();
		self.form.runtime_apply_values(&values);
		self.state.is_touched.set(true);
		self.refresh_dirty();
		self.touched_collections.borrow_mut().clear();
		self.touched_paths.borrow_mut().clear();
		self.rebuild_path_default_values();
		self.collection_errors.set(HashMap::new());
		self.path_errors.set(HashMap::new());
		self.values_signal.set(values);
		self.sync_observed_values();
		self.sync_first_error();
		if self.revalidate_on == RevalidateOn::Change {
			let _ = self.trigger();
		}
	}

	/// Sets one field error.
	pub fn set_error(&self, field: Form::Field, error: FieldError) {
		let mut errors = self.state.field_errors.get();
		errors.insert(field, error);
		self.state.field_errors.set(errors);
		self.sync_first_error();
	}

	/// Routes a server-function error to matching field and form-level error state.
	pub fn apply_server_error(&self, error: &ServerFnError) {
		let mut matched_errors = HashMap::<Form::Field, Vec<String>>::new();
		let mut unmatched_errors = Vec::new();

		for field_error in error.field_errors() {
			if let Some(field) = self.form.runtime_field_by_name(field_error.field()) {
				matched_errors
					.entry(field)
					.or_default()
					.push(field_error.message().to_string());
			} else {
				unmatched_errors.push(format!(
					"{}: {}",
					field_error.field(),
					field_error.message()
				));
			}
		}

		let field_errors: HashMap<Form::Field, FieldError> = matched_errors
			.into_iter()
			.map(|(field, messages)| (field, FieldError::new(messages.join("\n"))))
			.collect();
		let form_error = if field_errors.is_empty() || !unmatched_errors.is_empty() {
			let mut messages = Vec::with_capacity(unmatched_errors.len() + 1);
			messages.push(error.user_message().to_string());
			messages.extend(unmatched_errors);
			Some(messages.join("\n"))
		} else {
			None
		};

		for field in self.form.runtime_fields() {
			self.form.runtime_set_custom_widget_error(*field, None);
		}
		{
			let mut custom_widget_errors = self.custom_widget_error_fields.borrow_mut();
			custom_widget_errors.clear();
			for (field, field_error) in &field_errors {
				self.form
					.runtime_set_custom_widget_error(*field, Some(field_error.clone()));
				custom_widget_errors.insert(*field, field_error.clone());
			}
		}
		self.state.field_errors.set(field_errors);
		self.state.form_error.set(form_error.clone());
		self.state.submit_error.set(form_error);
		self.sync_first_error();
	}

	/// Clears all validation and submit errors.
	pub fn clear_errors(&self) {
		for field in self.form.runtime_fields() {
			self.form.runtime_set_custom_widget_error(*field, None);
		}
		self.custom_widget_error_fields.borrow_mut().clear();
		self.state.field_errors.set(HashMap::new());
		self.collection_errors.set(HashMap::new());
		self.path_errors.set(HashMap::new());
		self.state.form_error.set(None);
		self.state.submit_error.set(None);
		self.state.error.set(None);
	}

	/// Clears one field error.
	pub fn clear_field_error(&self, field: Form::Field) {
		self.form.runtime_set_custom_widget_error(field, None);
		self.custom_widget_error_fields.borrow_mut().remove(&field);
		let mut errors = self.state.field_errors.get();
		errors.remove(&field);
		self.state.field_errors.set(errors);
		self.sync_first_error();
	}

	/// Runs generated validation.
	pub fn trigger(&self) -> Result<(), FormValidationError<Form::Field>> {
		let result = self.form.runtime_validate();
		self.apply_validation_result(&result);
		self.notify(FormEvent::Validated);
		result
	}

	/// Runs validation and returns whether one field remains error-free.
	pub fn trigger_field(
		&self,
		field: Form::Field,
	) -> Result<(), FormValidationError<Form::Field>> {
		let result = self.trigger();
		if self.state.field_errors.get().contains_key(&field) {
			result
		} else {
			Ok(())
		}
	}

	/// Returns aggregate form state signals.
	pub fn form_state(&self) -> FormState<Form::Field> {
		self.state.clone()
	}

	/// Resets all values to current defaults.
	pub fn reset(&self) {
		let _ = self.in_owner_scope(|| {
			let defaults = self.default_values.borrow().clone();
			let _guard = self.suppress_signal_sync();
			self.form.runtime_apply_values(&defaults);
			self.touched_fields.borrow_mut().clear();
			self.touched_collections.borrow_mut().clear();
			self.touched_paths.borrow_mut().clear();
			self.state.is_touched.set(false);
			self.state.is_dirty.set(false);
			self.state.is_submitting.set(false);
			self.state.is_submit_successful.set(false);
			self.rebuild_path_default_values();
			self.clear_errors();
			self.values_signal.set(defaults);
			self.sync_observed_values();
		});
	}

	/// Syncs runtime state after a native form reset has restored field values.
	pub fn sync_after_native_reset(&self) {
		let current = self.get_values();
		let is_dirty = form_values_are_dirty(&self.form, &current, &self.default_values.borrow());
		self.touched_fields.borrow_mut().clear();
		self.state.is_touched.set(false);
		self.state.is_dirty.set(is_dirty);
		self.values_signal.set(current);
		self.sync_observed_values();
		self.clear_errors();
	}

	/// Resets one field to its current default value.
	pub fn reset_field(&self, field: Form::Field) {
		let defaults = self.default_values.borrow();
		let _guard = self.suppress_signal_sync();
		self.form.runtime_apply_field_value(field, &defaults);
		self.touched_fields.borrow_mut().remove(&field);
		self.refresh_dirty();
		self.values_signal.set(self.get_values());
		self.sync_observed_values();
	}

	/// Makes the current values the defaults and clears dirty state.
	pub fn reset_default_values(&self) {
		let values = self.get_values();
		*self.default_values.borrow_mut() = values.clone();
		*self.path_default_values.borrow_mut() = self.form.runtime_path_values_from_values(&values);
		self.state.is_dirty.set(false);
	}

	/// Attempts to focus one field.
	pub fn set_focus(&self, field: Form::Field) -> Result<(), FocusError> {
		self.form.runtime_set_focus(field)
	}

	/// Subscribes to runtime form events.
	pub fn subscribe<Callback>(&self, callback: Callback) -> FormSubscription<Form>
	where
		Callback: Fn(FormEvent<Form>) + 'static,
	{
		let mut subscribers = self.subscribers.borrow_mut();
		let index = subscribers.len();
		subscribers.push(Some(Rc::new(callback)));
		FormSubscription {
			index,
			subscribers: Rc::downgrade(&self.subscribers),
		}
	}

	/// Runs validation and submit lifecycle callbacks.
	pub fn handle_submit(&self) -> UseFormSubmitOutcome {
		let outcome = self.begin_submit_lifecycle();
		if outcome == UseFormSubmitOutcome::Submitted {
			self.complete_submit_success();
		}
		outcome
	}

	pub(crate) fn begin_submit_lifecycle(&self) -> UseFormSubmitOutcome {
		let Ok(is_submitting) = self.state.is_submitting.try_get_untracked() else {
			return UseFormSubmitOutcome::AlreadyPending;
		};
		if is_submitting {
			return UseFormSubmitOutcome::AlreadyPending;
		}

		if self.state.is_submitting.try_set(true).is_err()
			|| self.state.is_submit_successful.try_set(false).is_err()
			|| self.state.submit_error.try_set(None).is_err()
		{
			return UseFormSubmitOutcome::AlreadyPending;
		}
		let _ = self.in_owner_scope(|| {
			self.notify(FormEvent::SubmitStarted);
			if let Some(callback) = &self.on_submit_start {
				callback(self);
			}
		});
		if self.state.is_submitting.try_get_untracked().is_err() {
			return UseFormSubmitOutcome::AlreadyPending;
		}

		let validation_failed = enter_scope(self.scope, || self.trigger().is_err()).unwrap_or(true);
		if validation_failed {
			if self.state.is_submitting.try_set(false).is_err() {
				return UseFormSubmitOutcome::AlreadyPending;
			}
			let _ = self.in_owner_scope(|| {
				if let Some(callback) = &self.on_submit_error {
					callback(self);
				}
				self.notify(FormEvent::SubmitFailed);
			});
			return UseFormSubmitOutcome::ValidationFailed;
		}
		if enter_scope(self.scope, || ()).is_err() {
			return UseFormSubmitOutcome::AlreadyPending;
		}

		UseFormSubmitOutcome::Submitted
	}

	pub(crate) fn complete_submit_success(&self) {
		let _ = self.in_owner_scope(|| {
			self.state.is_submitting.set(false);
			self.state.is_submit_successful.set(true);
			self.state.submit_error.set(None);
			self.sync_first_error();
			if let Some(callback) = &self.on_submit_success {
				callback(self);
			}
			self.notify(FormEvent::Submitted);
		});
	}

	// Native dispatch returns before the WASM-only validation path calls this helper.
	#[cfg_attr(not(wasm), allow(dead_code))]
	pub(crate) fn complete_mutation_validation_error(
		&self,
		error: FormValidationError<Form::Field>,
	) {
		self.apply_validation_result(&Err(error));
		self.state.is_submitting.set(false);
		let _ = self.in_owner_scope(|| {
			if let Some(callback) = &self.on_submit_error {
				callback(self);
			}
			self.notify(FormEvent::SubmitFailed);
		});
	}

	pub(crate) fn complete_mutation_server_error(&self, error: &ServerFnError) {
		let _ = self.in_owner_scope(|| {
			self.apply_server_error(error);
			self.state.is_submitting.set(false);
			self.state.is_submit_successful.set(false);
			if let Some(callback) = &self.on_submit_error {
				callback(self);
			}
			self.notify(FormEvent::SubmitFailed);
		});
	}

	fn complete_submit_error(&self, error: impl Into<String>) {
		let _ = self.in_owner_scope(|| {
			self.state.is_submitting.set(false);
			self.state.is_submit_successful.set(false);
			self.state.submit_error.set(Some(error.into()));
			self.sync_first_error();
			if let Some(callback) = &self.on_submit_error {
				callback(self);
			}
			self.notify(FormEvent::SubmitFailed);
		});
	}

	fn in_owner_scope(&self, callback: impl FnOnce()) -> bool {
		enter_scope(self.scope, callback).is_ok()
	}

	#[cfg(native)]
	fn complete_native_action_submit(&self) {
		self.state.is_submitting.set(false);
	}

	/// Runs validation and async submit lifecycle callbacks.
	pub async fn submit_async<Submit, Fut, Output, Error>(
		&self,
		submit: Submit,
	) -> Result<UseFormAsyncSubmitOutcome<Output>, Error>
	where
		Submit: FnOnce() -> Fut,
		Fut: Future<Output = Result<Output, Error>>,
		Error: Display,
	{
		let Ok(is_submitting) = self.state.is_submitting.try_get_untracked() else {
			return Ok(UseFormAsyncSubmitOutcome::AlreadyPending);
		};
		if is_submitting {
			return Ok(UseFormAsyncSubmitOutcome::AlreadyPending);
		}

		if self.state.is_submitting.try_set(true).is_err() {
			return Ok(UseFormAsyncSubmitOutcome::AlreadyPending);
		}
		let mut pending_guard = SubmitPendingGuard::new(self.state.is_submitting);
		if self.state.is_submit_successful.try_set(false).is_err() {
			return Ok(UseFormAsyncSubmitOutcome::AlreadyPending);
		}
		if self.state.submit_error.try_set(None).is_err() {
			return Ok(UseFormAsyncSubmitOutcome::AlreadyPending);
		}
		self.sync_first_error();
		let _ = self.in_owner_scope(|| {
			self.notify(FormEvent::SubmitStarted);
			if let Some(callback) = &self.on_submit_start {
				callback(self);
			}
		});

		let validation_failed = enter_scope(self.scope, || self.trigger().is_err()).unwrap_or(true);
		if validation_failed {
			let is_live = self.state.is_submitting.try_set(false).is_ok();
			pending_guard.disarm();
			if !is_live {
				return Ok(UseFormAsyncSubmitOutcome::ValidationFailed);
			}
			let _ = self.in_owner_scope(|| {
				if let Some(callback) = &self.on_submit_error {
					callback(self);
				}
				self.notify(FormEvent::SubmitFailed);
			});
			return Ok(UseFormAsyncSubmitOutcome::ValidationFailed);
		}

		let Ok(future) = enter_scope(self.scope, submit) else {
			pending_guard.disarm();
			return Ok(UseFormAsyncSubmitOutcome::AlreadyPending);
		};
		let Some(result) = (ScopedFormFuture {
			scope: self.scope,
			future: Some(Box::pin(future)),
		})
		.await
		else {
			pending_guard.disarm();
			return Ok(UseFormAsyncSubmitOutcome::AlreadyPending);
		};

		match result {
			Ok(output) => {
				let is_live = self.state.is_submitting.try_set(false).is_ok();
				pending_guard.disarm();
				if !is_live {
					return Ok(UseFormAsyncSubmitOutcome::Submitted(output));
				}
				self.complete_submit_success();
				Ok(UseFormAsyncSubmitOutcome::Submitted(output))
			}
			Err(error) => {
				let is_live = self.state.is_submitting.try_set(false).is_ok();
				pending_guard.disarm();
				if !is_live {
					return Err(error);
				}
				self.complete_submit_error(error.to_string());
				Err(error)
			}
		}
	}

	/// Runs an async server-function submit and routes structured errors to form state.
	pub async fn submit_server_fn<Submit, Fut, Output>(
		&self,
		submit: Submit,
	) -> Result<UseFormAsyncSubmitOutcome<Output>, ServerFnError>
	where
		Submit: FnOnce() -> Fut,
		Fut: Future<Output = Result<Output, ServerFnError>>,
	{
		let mut pending_guard = SubmitPendingGuard::new(self.state.is_submitting);
		let outcome = self.begin_submit_lifecycle();
		if outcome != UseFormSubmitOutcome::Submitted {
			pending_guard.disarm();
			return Ok(match outcome {
				UseFormSubmitOutcome::AlreadyPending => UseFormAsyncSubmitOutcome::AlreadyPending,
				UseFormSubmitOutcome::ValidationFailed => {
					UseFormAsyncSubmitOutcome::ValidationFailed
				}
				UseFormSubmitOutcome::Submitted => unreachable!(),
			});
		}

		let Ok(future) = enter_scope(self.scope, submit) else {
			pending_guard.disarm();
			return Ok(UseFormAsyncSubmitOutcome::AlreadyPending);
		};
		let Some(result) = (ScopedFormFuture {
			scope: self.scope,
			future: Some(Box::pin(future)),
		})
		.await
		else {
			pending_guard.disarm();
			return Ok(UseFormAsyncSubmitOutcome::AlreadyPending);
		};

		match result {
			Ok(output) => {
				let is_live = self.state.is_submitting.try_set(false).is_ok();
				pending_guard.disarm();
				if !is_live {
					return Ok(UseFormAsyncSubmitOutcome::Submitted(output));
				}
				self.complete_submit_success();
				Ok(UseFormAsyncSubmitOutcome::Submitted(output))
			}
			Err(error) => {
				let is_live = self.state.is_submitting.try_set(false).is_ok();
				pending_guard.disarm();
				if !is_live {
					return Err(error);
				}
				self.complete_mutation_server_error(&error);
				Err(error)
			}
		}
	}

	/// Reconciles values and defaults from a newly generated form instance.
	pub fn reconcile_from(&self, form: &Form, deps: Deps) {
		self.reconcile_defaults(form.runtime_initial_values(), deps);
	}

	/// Reconciles values and defaults from a new default value struct.
	pub fn reconcile_defaults(&self, new_defaults: Form::Values, deps: Deps) {
		if *self.deps.borrow() == deps {
			return;
		}
		*self.deps.borrow_mut() = deps;

		let old_defaults = self.default_values.borrow().clone();
		let current = self.get_values();
		let resets_all_values = matches!(self.reset_on_deps, ResetOnDeps::ResetAll);

		match self.reset_on_deps {
			ResetOnDeps::KeepDirtyValues => {
				let _guard = self.suppress_signal_sync();
				self.form
					.runtime_apply_pristine_values(&current, &old_defaults, &new_defaults);
			}
			ResetOnDeps::ResetAll => {
				let _guard = self.suppress_signal_sync();
				self.form.runtime_apply_values(&new_defaults);
				self.touched_fields.borrow_mut().clear();
				self.touched_collections.borrow_mut().clear();
				self.touched_paths.borrow_mut().clear();
			}
			ResetOnDeps::ExplicitOnly => {}
		}

		*self.default_values.borrow_mut() = new_defaults;
		if resets_all_values {
			self.rebuild_path_default_values();
		} else {
			self.merge_missing_path_default_values();
		}
		self.prune_path_state_to_current_paths();
		if !self.keep_errors {
			self.clear_errors();
		} else {
			self.collection_errors.set(HashMap::new());
			self.path_errors.set(HashMap::new());
			self.sync_first_error();
		}
		self.refresh_dirty();
		self.values_signal.set(self.get_values());
		self.sync_observed_values();
		if self.revalidate_on == RevalidateOn::DepsChange {
			let _ = self.trigger();
		}
		self.notify(FormEvent::DepsChanged);
	}

	fn apply_validation_result(&self, result: &Result<(), FormValidationError<Form::Field>>) {
		match result {
			Ok(()) => {
				self.state.field_errors.set(HashMap::new());
				self.collection_errors.set(HashMap::new());
				self.path_errors.set(HashMap::new());
				self.state.form_error.set(None);
				self.sync_first_error();
			}
			Err(error) => {
				self.state.field_errors.set(error.field_errors().clone());
				self.collection_errors
					.set(error.collection_errors().clone());
				self.path_errors.set(error.path_errors().clone());
				self.state
					.form_error
					.set(error.form_error().map(str::to_string));
				self.sync_first_error();
			}
		}
	}

	fn refresh_dirty(&self) {
		let current = self.get_values();
		self.state.is_dirty.set(form_values_are_dirty(
			&self.form,
			&current,
			&self.default_values.borrow(),
		));
		self.state.is_touched.set(true);
	}

	fn sync_first_error(&self) {
		let first_field_error = self
			.state
			.field_errors
			.get()
			.values()
			.next()
			.map(|error| error.message().to_string());
		let first_path_error = self
			.path_errors
			.get()
			.values()
			.next()
			.map(|error| error.message().to_string());
		let first_collection_error = self
			.collection_errors
			.get()
			.values()
			.next()
			.map(|error| error.message().to_string());
		self.state.error.set(
			first_field_error
				.or(first_collection_error)
				.or(first_path_error)
				.or_else(|| self.state.form_error.get())
				.or_else(|| self.state.submit_error.get()),
		);
	}

	fn notify(&self, event: FormEvent<Form>) {
		for subscriber in self.subscribers.borrow().iter().flatten() {
			subscriber(event.clone());
		}
	}

	fn suppress_signal_sync(&self) -> SignalSyncGuard {
		self.signal_sync_suppressed.set(true);
		SignalSyncGuard {
			suppressed: Rc::clone(&self.signal_sync_suppressed),
		}
	}

	fn sync_observed_values(&self) {
		*self.observed_values.borrow_mut() = self.get_values();
	}

	fn rebuild_path_default_values(&self) {
		*self.path_default_values.borrow_mut() = self
			.form
			.runtime_path_values_from_values(&self.default_values.borrow());
	}

	fn merge_missing_path_default_values(&self) {
		let defaults = self.default_values.borrow();
		let missing_defaults = self.form.runtime_path_values_from_values(&defaults);
		let mut path_default_values = self.path_default_values.borrow_mut();
		for (path_key, value) in missing_defaults {
			path_default_values.entry(path_key).or_insert(value);
		}
	}

	fn prune_path_state_to_current_paths(&self) {
		let current_paths = self.form.runtime_path_values();
		self.touched_paths
			.borrow_mut()
			.retain(|path_key, _| current_paths.contains_key(path_key));

		let mut path_errors = self.path_errors.get();
		path_errors.retain(|path_key, _| current_paths.contains_key(path_key));
		self.path_errors.set(path_errors);

		self.path_default_values
			.borrow_mut()
			.retain(|path_key, _| current_paths.contains_key(path_key));
	}
}

impl<Form, Deps, T, E> Clone for FormAction<Form, Deps, T, E>
where
	Form: FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
	T: Clone + 'static,
	E: Clone + Display + 'static,
{
	fn clone(&self) -> Self {
		Self {
			form: self.form.clone(),
			action: self.action,
		}
	}
}

impl<Form, Deps, T, E> FormAction<Form, Deps, T, E>
where
	Form: FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
	T: Clone + 'static,
	E: Clone + Display + 'static,
{
	/// Returns the underlying form runtime handle.
	pub fn form(&self) -> UseFormReturn<Form, Deps> {
		self.form.clone()
	}

	/// Returns the current action phase.
	pub fn phase(&self) -> ActionPhase<T, E> {
		self.action.phase()
	}

	/// Returns `true` while form validation or the async action is pending.
	pub fn is_pending(&self) -> bool {
		self.form.state.is_submitting.get() || self.action.is_pending()
	}

	/// Returns `true` when the action completed successfully.
	pub fn is_success(&self) -> bool {
		self.form.state.is_submit_successful.get()
	}

	/// Returns the latest successful action result.
	pub fn result(&self) -> Option<T> {
		self.action.result()
	}

	/// Returns the latest action error.
	pub fn error(&self) -> Option<E> {
		self.action.error()
	}

	/// Returns the first visible validation or submit error message.
	pub fn error_message(&self) -> Option<String> {
		self.form
			.state
			.error
			.get()
			.or_else(|| self.action.error().map(|error| error.to_string()))
	}

	pub(crate) fn action(&self) -> Action<T, E> {
		self.action
	}

	/// Returns a submit-event handler that validates and dispatches this action.
	///
	/// Attach this handler to the containing `form` element. Native form submit
	/// semantics then cover both submit-button activation and Enter-key submit.
	pub fn submit_handler(&self) -> crate::component::PageEventHandler {
		let action = self.clone();
		crate::callback::event_handler(move |event| {
			event.prevent_default();
			action.submit();
		})
	}

	/// Runs generated validation and dispatches the current values on success.
	pub fn submit(&self) -> UseFormSubmitOutcome {
		let outcome = self.form.begin_submit_lifecycle();
		if outcome != UseFormSubmitOutcome::Submitted {
			if outcome == UseFormSubmitOutcome::ValidationFailed {
				self.action.reset();
			}
			return outcome;
		}

		self.action.dispatch(self.form.get_values());

		#[cfg(native)]
		self.form.complete_native_action_submit();

		outcome
	}

	/// Registers a callback that runs after a successful WASM action.
	///
	/// The form submit lifecycle is completed before this callback runs.
	pub fn on_success<Callback>(self, callback: Callback) -> Self
	where
		Callback: Fn(&UseFormReturn<Form, Deps>, &T) + 'static,
	{
		let form_for_callback = self.form.clone();
		let action = self.action.on_success(move |value| {
			callback(&form_for_callback, value);
		});
		Self {
			form: self.form,
			action,
		}
	}

	/// Registers a callback that runs after a failed WASM action.
	///
	/// The form submit lifecycle is completed before this callback runs.
	pub fn on_error<Callback>(self, callback: Callback) -> Self
	where
		Callback: Fn(&UseFormReturn<Form, Deps>, &E) + 'static,
	{
		let form_for_callback = self.form.clone();
		let action = self.action.on_error(move |error| {
			callback(&form_for_callback, error);
		});
		Self {
			form: self.form,
			action,
		}
	}
}

impl<Form, Deps> UseFormReturn<Form, Deps>
where
	Form: FormCollectionRuntimeSource,
	Deps: Clone + PartialEq + 'static,
{
	/// Returns a typed signal for a nested collection field path.
	pub fn watch_path<T>(&self, path: Form::FieldPath) -> Signal<T>
	where
		T: Clone + 'static,
	{
		self.form
			.runtime_watch_path(path.clone())
			.unwrap_or_else(|| {
				panic!(
					"field path {:?} is not compatible with requested Signal<{}>",
					path,
					type_name::<T>()
				)
			})
	}

	/// Applies one typed value to a nested collection field path.
	pub fn set_path_value<T>(&self, path: Form::FieldPath, value: T)
	where
		T: Any + 'static,
	{
		let path_key = Form::runtime_field_path_key(&path);
		let _guard = self.suppress_signal_sync();
		if !self.form.runtime_set_path_value(path.clone(), value) {
			panic!(
				"field path {:?} does not exist in the current collection state",
				path
			);
		}
		self.touched_paths.borrow_mut().insert(path_key, true);
		self.state.is_touched.set(true);
		self.refresh_dirty();
		self.values_signal.set(self.get_values());
		self.sync_observed_values();
		if self.revalidate_on == RevalidateOn::Change {
			let _ = self.trigger();
		}
	}

	/// Sets one nested collection field path error.
	pub fn set_path_error(&self, path: Form::FieldPath, error: FieldError) {
		let path_key = Form::runtime_field_path_key(&path);
		let mut errors = self.path_errors.get();
		errors.insert(path_key, error);
		self.path_errors.set(errors);
		self.sync_first_error();
	}

	/// Clears one nested collection field path error.
	pub fn clear_path_error(&self, path: Form::FieldPath) {
		let path_key = Form::runtime_field_path_key(&path);
		let mut errors = self.path_errors.get();
		errors.remove(&path_key);
		self.path_errors.set(errors);
		self.sync_first_error();
	}

	/// Returns state for one nested collection field path.
	pub fn get_path_state(&self, path: Form::FieldPath) -> FieldPathState {
		let path_key = Form::runtime_field_path_key(&path);
		let current = self.get_values();
		let defaults = self.default_values.borrow();
		let errors = self.path_errors.get();
		let is_dirty = match self.path_default_values.borrow().get(&path_key) {
			Some(default) => self
				.form
				.runtime_path_value_equals(&path_key, default.as_ref())
				.map(|matches_default| !matches_default)
				.unwrap_or(false),
			None => {
				self.form.runtime_path_exists(&path_key)
					|| self.form.runtime_path_is_dirty(&path, &current, &defaults)
			}
		};
		FieldPathState {
			is_dirty,
			is_touched: self
				.touched_paths
				.borrow()
				.get(&path_key)
				.copied()
				.unwrap_or(false),
			error: errors.get(&path_key).cloned(),
		}
	}

	/// Returns state for one generated collection field.
	pub fn get_collection_state(&self, collection: Form::Collection) -> CollectionState {
		let collection_key = Form::runtime_collection_key(collection);
		let current = self.get_values();
		let defaults = self.default_values.borrow();
		let errors = self.collection_errors.get();
		CollectionState {
			len: self.form.runtime_collection_len(collection),
			is_dirty: self
				.form
				.runtime_collection_is_dirty(collection, &current, &defaults),
			is_touched: self
				.touched_collections
				.borrow()
				.get(&collection_key)
				.copied()
				.unwrap_or(false),
			error: errors.get(&collection_key).cloned(),
		}
	}

	/// Appends one item to a generated collection.
	pub fn push_item<T>(&self, collection: Form::Collection, value: T) -> CollectionItemKey
	where
		T: Any + Clone + 'static,
	{
		let index = self.form.runtime_collection_len(collection);
		self.insert_item(collection, index, value)
	}

	/// Inserts one item into a generated collection.
	pub fn insert_item<T>(
		&self,
		collection: Form::Collection,
		index: usize,
		value: T,
	) -> CollectionItemKey
	where
		T: Any + Clone + 'static,
	{
		let key = CollectionItemKey::next(&self.next_collection_item_key);
		let _guard = self.suppress_signal_sync();
		self.form
			.runtime_insert_collection_item(collection, index, key, value);
		self.sync_after_collection_change(collection);
		key
	}

	/// Removes one item from a generated collection.
	pub fn remove_item(&self, collection: Form::Collection, key: CollectionItemKey) -> bool {
		let _guard = self.suppress_signal_sync();
		let removed = self.form.runtime_remove_collection_item(collection, key);
		if removed {
			self.clear_item_path_state(collection, key);
			self.sync_after_collection_change(collection);
			self.sync_first_error();
		}
		removed
	}

	/// Moves one item in a generated collection.
	pub fn move_item(
		&self,
		collection: Form::Collection,
		key: CollectionItemKey,
		target_index: usize,
	) -> Option<(usize, usize)> {
		let _guard = self.suppress_signal_sync();
		let movement = self
			.form
			.runtime_move_collection_item(collection, key, target_index);
		if movement.is_some() {
			self.sync_after_collection_change(collection);
		}
		movement
	}

	fn sync_after_collection_change(&self, collection: Form::Collection) {
		let values = self.get_values();
		let is_dirty = {
			let defaults = self.default_values.borrow();
			self.form.runtime_values_are_dirty(&values, &defaults)
		};
		self.touched_collections
			.borrow_mut()
			.insert(Form::runtime_collection_key(collection), true);
		self.state.is_touched.set(true);
		self.state.is_dirty.set(is_dirty);
		self.values_signal.set(values.clone());
		*self.observed_values.borrow_mut() = values;
		if self.revalidate_on == RevalidateOn::Change {
			let _ = self.trigger();
		}
	}

	fn clear_item_path_state(&self, collection: Form::Collection, key: CollectionItemKey) {
		let path_keys = Form::runtime_field_path_keys_for_item(collection, key);
		if path_keys.is_empty() {
			return;
		}

		{
			let mut touched_paths = self.touched_paths.borrow_mut();
			for path_key in &path_keys {
				touched_paths.remove(path_key);
			}
		}

		let mut path_errors = self.path_errors.get();
		for path_key in path_keys {
			path_errors.remove(&path_key);
			self.path_default_values.borrow_mut().remove(&path_key);
		}
		self.path_errors.set(path_errors);
	}
}

struct SignalSyncGuard {
	suppressed: Rc<Cell<bool>>,
}

impl Drop for SignalSyncGuard {
	fn drop(&mut self) {
		self.suppressed.set(false);
	}
}

pub(crate) struct SubmitPendingGuard {
	is_submitting: Signal<bool>,
	active: bool,
}

impl SubmitPendingGuard {
	pub(crate) fn new(is_submitting: Signal<bool>) -> Self {
		Self {
			is_submitting,
			active: true,
		}
	}

	pub(crate) fn disarm(&mut self) {
		self.active = false;
	}
}

impl Drop for SubmitPendingGuard {
	fn drop(&mut self) {
		if self.active {
			let _ = self.is_submitting.try_set(false);
		}
	}
}

/// RAII subscription guard returned by `UseFormReturn::subscribe`.
pub struct FormSubscription<Form>
where
	Form: FormRuntimeSource,
{
	index: usize,
	subscribers: Weak<RefCell<Vec<Option<Subscriber<Form>>>>>,
}

impl<Form> Drop for FormSubscription<Form>
where
	Form: FormRuntimeSource,
{
	fn drop(&mut self) {
		if let Some(subscribers) = self.subscribers.upgrade()
			&& let Some(slot) = subscribers.borrow_mut().get_mut(self.index)
		{
			*slot = None;
		}
	}
}

/// Starts a runtime builder for a generated form.
pub fn use_form<Form>(form: &Form) -> UseFormBuilder<Form, NoDeps>
where
	Form: FormRuntimeSource,
{
	UseFormBuilder {
		form: form.clone(),
		deps: NoDeps,
		reset_on_deps: ResetOnDeps::KeepDirtyValues,
		keep_errors: false,
		revalidate_on: RevalidateOn::Submit,
		on_submit_start: None,
		on_submit_success: None,
		on_submit_error: None,
	}
}

/// Creates a typed submit action for a generated form runtime.
///
/// The helper composes [`use_form`] runtime state with [`use_action`] server
/// calls: validation runs first, valid current values are dispatched as the
/// action payload, and the action result updates submit pending/error/success
/// state.
pub fn use_form_action<Form, Deps, T, E, F, Fut>(
	form: &UseFormReturn<Form, Deps>,
	action_fn: F,
) -> FormAction<Form, Deps, T, E>
where
	Form: FormRuntimeSource,
	Deps: Clone + PartialEq + 'static,
	T: Clone + 'static,
	E: Clone + Display + 'static,
	F: Fn(Form::Values) -> Fut + 'static,
	Fut: Future<Output = Result<T, E>> + 'static,
{
	let form_for_success = form.clone();
	let form_for_error = form.clone();
	let action = use_action::<Form::Values, T, E, F, Fut>(action_fn)
		.on_success(move |_| {
			form_for_success.complete_submit_success();
		})
		.on_error(move |error| {
			form_for_error.complete_submit_error(error.to_string());
		});

	FormAction {
		form: form.clone(),
		action,
	}
}

#[cfg(test)]
mod tests {
	use super::{
		CollectionItem, CollectionItemKey, CollectionState, FieldError, FieldPathState,
		FormRuntimeSource, SubmitPendingGuard, UseFormSubmitOutcome, use_form,
	};
	use crate::reactive::Signal;
	use reinhardt_core::reactive::ReactiveScope;
	use serial_test::serial;
	use std::any::Any;
	use std::rc::Rc;
	use std::task::{Context, Poll, Waker};

	#[derive(Clone)]
	struct RetainedScopeForm {
		scope: Rc<ReactiveScope>,
		value: Signal<String>,
	}

	impl FormRuntimeSource for RetainedScopeForm {
		type Values = String;
		type Field = ();

		fn runtime_reactive_scope(&self) -> Option<Rc<ReactiveScope>> {
			Some(Rc::clone(&self.scope))
		}

		fn runtime_initial_values(&self) -> Self::Values {
			self.value.get_untracked()
		}

		fn runtime_current_values(&self) -> Self::Values {
			self.value.get_untracked()
		}

		fn runtime_apply_values(&self, values: &Self::Values) {
			self.value.set(values.clone());
		}

		fn runtime_set_field_value<T>(&self, _field: Self::Field, value: T)
		where
			T: Any + 'static,
		{
			let value = (&value as &dyn Any)
				.downcast_ref::<String>()
				.expect("test form only accepts String values");
			self.value.set(value.clone());
		}

		fn runtime_apply_field_value(&self, _field: Self::Field, values: &Self::Values) {
			self.value.set(values.clone());
		}

		fn runtime_field_is_dirty(
			&self,
			_field: Self::Field,
			current: &Self::Values,
			defaults: &Self::Values,
		) -> bool {
			current != defaults
		}

		fn runtime_watch_field<T>(&self, _field: Self::Field) -> Option<Signal<T>>
		where
			T: Clone + 'static,
		{
			None
		}

		fn runtime_fields(&self) -> &'static [Self::Field] {
			&[()]
		}
	}

	#[test]
	#[serial(reactive_runtime)]
	fn dropping_the_last_form_runtime_releases_its_retained_scope() {
		let scope = Rc::new(ReactiveScope::new());
		let form = scope.enter(|| RetainedScopeForm {
			scope: Rc::clone(&scope),
			value: Signal::new("initial".to_string()),
		});
		let weak_scope = Rc::downgrade(&scope);
		let runtime = use_form(&form).build();

		drop(form);
		drop(scope);
		assert!(
			weak_scope.upgrade().is_some(),
			"the live form runtime owns the retained scope"
		);

		drop(runtime);

		assert!(
			weak_scope.upgrade().is_none(),
			"dropping the final form runtime must dispose its sync effect and release the scope"
		);
	}

	#[test]
	#[serial(reactive_runtime)]
	fn submit_pending_guard_ignores_a_disposed_scope() {
		let scope = ReactiveScope::new();
		let pending_guard = scope.enter(|| SubmitPendingGuard::new(Signal::new(true)));

		scope.dispose();
		drop(pending_guard);
	}

	#[test]
	#[serial(reactive_runtime)]
	fn stale_form_submit_is_rejected_without_reading_disposed_state() {
		let scope = Rc::new(ReactiveScope::new());
		let form = scope.enter(|| RetainedScopeForm {
			scope: Rc::clone(&scope),
			value: Signal::new("initial".to_string()),
		});
		let runtime = use_form(&form).build();

		scope.dispose();

		let mut submit = Box::pin(runtime.submit_async(|| async { Ok::<_, String>(()) }));
		let mut context = Context::from_waker(Waker::noop());
		assert_eq!(
			submit.as_mut().poll(&mut context),
			Poll::Ready(Ok(super::UseFormAsyncSubmitOutcome::AlreadyPending))
		);
	}

	#[test]
	#[serial(reactive_runtime)]
	fn stale_server_fn_submit_is_rejected_without_reading_disposed_state() {
		let scope = Rc::new(ReactiveScope::new());
		let form = scope.enter(|| RetainedScopeForm {
			scope: Rc::clone(&scope),
			value: Signal::new("initial".to_string()),
		});
		let runtime = use_form(&form).build();

		scope.dispose();

		let mut submit = Box::pin(
			runtime.submit_server_fn(|| async { Ok::<_, crate::server_fn::ServerFnError>(()) }),
		);
		let mut context = Context::from_waker(Waker::noop());
		assert_eq!(
			submit.as_mut().poll(&mut context),
			Poll::Ready(Ok(super::UseFormAsyncSubmitOutcome::AlreadyPending))
		);
	}

	#[test]
	#[serial(reactive_runtime)]
	fn submit_start_disposal_stops_before_post_callback_validation() {
		let scope = Rc::new(ReactiveScope::new());
		let form = scope.enter(|| RetainedScopeForm {
			scope: Rc::clone(&scope),
			value: Signal::new("initial".to_string()),
		});
		let callback_scope = Rc::clone(&scope);
		let runtime = use_form(&form)
			.on_submit_start(move |_| callback_scope.dispose())
			.build();

		assert_eq!(
			runtime.begin_submit_lifecycle(),
			UseFormSubmitOutcome::AlreadyPending
		);
	}

	#[test]
	#[serial(reactive_runtime)]
	fn validated_event_disposal_stops_before_submit_completion() {
		let scope = Rc::new(ReactiveScope::new());
		let form = scope.enter(|| RetainedScopeForm {
			scope: Rc::clone(&scope),
			value: Signal::new("initial".to_string()),
		});
		let runtime = use_form(&form).build();
		let callback_scope = Rc::clone(&scope);
		let _subscription = runtime.subscribe(move |event| {
			if matches!(event, super::FormEvent::Validated) {
				callback_scope.dispose();
			}
		});

		assert_eq!(
			runtime.begin_submit_lifecycle(),
			UseFormSubmitOutcome::AlreadyPending
		);
	}

	#[test]
	#[serial(reactive_runtime)]
	fn late_server_error_is_ignored_after_form_scope_disposal() {
		let scope = Rc::new(ReactiveScope::new());
		let form = scope.enter(|| RetainedScopeForm {
			scope: Rc::clone(&scope),
			value: Signal::new("initial".to_string()),
		});
		let runtime = use_form(&form).build();
		scope.dispose();

		let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			runtime.complete_mutation_server_error(&crate::server_fn::ServerFnError::application(
				"late error",
			));
		}));
		assert!(
			result.is_ok(),
			"disposed forms must ignore late server errors"
		);
	}

	#[test]
	fn collection_item_key_is_opaque_and_stable() {
		let first_key = CollectionItemKey(1);
		let second_key = CollectionItemKey(2);
		let generated_key = CollectionItemKey::from_runtime_index(1);
		let runtime_boundary_key = CollectionItemKey(CollectionItemKey::GENERATED_KEY_FLAG - 1);
		let exhausted_counter = ::std::cell::Cell::new(runtime_boundary_key.0);

		assert_ne!(first_key, second_key);
		assert_ne!(generated_key, first_key);
		assert_ne!(
			CollectionItemKey::next(&exhausted_counter),
			CollectionItemKey::from_runtime_index(runtime_boundary_key.0)
		);
		assert_eq!(
			exhausted_counter.get(),
			CollectionItemKey::GENERATED_KEY_FLAG
		);
		assert!(
			::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
				CollectionItemKey::next(&exhausted_counter);
			}))
			.is_err()
		);
		assert!(format!("{first_key:?}").contains("CollectionItemKey"));

		let item = CollectionItem::new(first_key, 3, "Ada".to_string());

		assert_eq!(item.key(), first_key);
		assert_eq!(item.index(), 3);
		assert_eq!(item.value().as_str(), "Ada");
		assert_eq!(item.into_value(), "Ada".to_string());

		let error = FieldError::new("collection item is required");
		let collection_state = CollectionState {
			len: 2,
			is_dirty: true,
			is_touched: false,
			error: Some(error.clone()),
		};
		let field_path_state = FieldPathState {
			is_dirty: false,
			is_touched: true,
			error: Some(error.clone()),
		};

		assert_eq!(collection_state.len, 2);
		assert!(collection_state.is_dirty);
		assert!(!collection_state.is_touched);
		assert_eq!(
			collection_state.error.as_ref().map(FieldError::message),
			Some("collection item is required")
		);
		assert!(!field_path_state.is_dirty);
		assert!(field_path_state.is_touched);
		assert_eq!(field_path_state.error, Some(error));
	}
}
