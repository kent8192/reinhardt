//! Target-neutral runtime state for model-backed forms.

use regex::Regex;
use rust_decimal::Decimal;
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::LazyLock;

use reinhardt_core::model_form::{
	ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload, ModelFormPayloadError,
	ModelFormPolicy, ModelFormSchema,
};
use reinhardt_core::types::page::{NumberParseError, NumberParseErrorKind, NumberValue};

/// Hidden compile-time selection marker for one model-form argument.
#[doc(hidden)]
pub trait ModelFormSelectionArgument<const INDEX: usize> {
	/// Opaque marker retained for generated-code compatibility.
	type Name: 'static;
	/// Selected model field name for this argument position.
	const NAME: &'static str;
	/// Selected model field kind used by multipart contract validation.
	const KIND: Option<ModelFormFieldKind> = None;
	/// Selected model field requiredness used by multipart contract validation.
	const REQUIRED: Option<bool> = None;
}

/// Validates the type and requiredness of one multipart model-form argument.
#[doc(hidden)]
pub const fn assert_model_form_argument_compatibility<Selection, ServerFn, const INDEX: usize>()
where
	Selection: ModelFormSelectionArgument<INDEX>,
	ServerFn: crate::server_fn::ServerFnArgument<INDEX>,
{
	use crate::server_fn::ServerFnArgumentKind;

	match (
		Selection::KIND,
		Selection::REQUIRED,
		<ServerFn as crate::server_fn::ServerFnArgument<INDEX>>::METADATA.kind,
		<ServerFn as crate::server_fn::ServerFnArgument<INDEX>>::OPTIONAL,
	) {
		(
			Some(ModelFormFieldKind::File | ModelFormFieldKind::Image),
			Some(true),
			ServerFnArgumentKind::File,
			_,
		)
		| (
			Some(ModelFormFieldKind::File | ModelFormFieldKind::Image),
			Some(false),
			ServerFnArgumentKind::OptionalFile,
			_,
		) => {}
		(Some(_), Some(false), ServerFnArgumentKind::Json, false) => {
			panic!("omittable model fields require an optional server-function argument");
		}
		(Some(kind), Some(false), ServerFnArgumentKind::Json, true)
		| (Some(kind), Some(true), ServerFnArgumentKind::Json, _)
		| (Some(kind), None, ServerFnArgumentKind::Json, _) => {
			assert!(
				!matches!(kind, ModelFormFieldKind::File | ModelFormFieldKind::Image),
				"file/image model fields require a multipart file argument"
			);
		}
		_ => {
			panic!("model-form field type or requiredness does not match server-function argument")
		}
	}
}

/// Hidden compile-time model-form/server-function argument name check.
#[doc(hidden)]
pub struct ModelFormSelectionArgumentNameCheck<Selection, ServerFn, const INDEX: usize>(
	PhantomData<fn() -> (Selection, ServerFn)>,
);

impl<Selection, ServerFn, const INDEX: usize>
	ModelFormSelectionArgumentNameCheck<Selection, ServerFn, INDEX>
where
	Selection: ModelFormSelectionArgument<INDEX>,
	ServerFn: crate::server_fn::ServerFnArgument<INDEX>,
{
	/// Fails const evaluation when the selected field name differs from the server argument.
	pub const ASSERT: () = {
		let selected = Selection::NAME.as_bytes();
		let server = ServerFn::METADATA.name.as_bytes();
		assert!(
			selected.len() == server.len(),
			"model-form field name does not match server-function argument"
		);
		let mut index = 0;
		while index < selected.len() {
			assert!(
				selected[index] == server[index],
				"model-form field name does not match server-function argument"
			);
			index += 1;
		}
	};
}

/// Hidden compile-time proof of a model-form argument count.
#[doc(hidden)]
pub trait ModelFormSelectionCount<const COUNT: usize> {}

/// Hidden payload builder used by JSON model-form dispatch.
#[doc(hidden)]
pub trait ModelFormSelectionPayload<S, P>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
{
	/// Payload passed to the JSON server function.
	type Payload;

	/// Builds the endpoint payload from the current form state.
	fn build_payload(state: &ModelFormState<S, P>) -> Result<Self::Payload, ModelFormPayloadError>;
}

/// Hidden JSON selection used when a model form excludes fields.
#[doc(hidden)]
pub struct ModelFormPayloadSelection<D, Q>(PhantomData<fn() -> (D, Q)>);

impl<S, P, D, Q> ModelFormSelectionPayload<S, P> for ModelFormPayloadSelection<D, Q>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
	D: Default + ModelFormPayload<Q>,
	Q: ModelFormPolicy,
{
	type Payload = D;

	fn build_payload(state: &ModelFormState<S, P>) -> Result<Self::Payload, ModelFormPayloadError> {
		state.build_json_payload_for::<D, Q>()
	}
}

/// Hidden model-form submission contract implemented by server-function markers.
#[doc(hidden)]
pub trait ModelFormServerFn<Selection, S, P>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
{
	/// Forces compile-time validation of a multipart selection.
	#[doc(hidden)]
	const VALIDATE_SELECTION: () = ();

	/// Successful server-function response type.
	type Response;
	/// Error returned by the server-function adapter before model-form mapping.
	type Error;

	/// Submits the current model-form state through the selected server function.
	fn submit(
		state: &ModelFormState<S, P>,
	) -> impl Future<Output = Result<Self::Response, Self::Error>>;
}

/// Validates the error conversion required by a selected model-form adapter.
#[doc(hidden)]
pub const fn assert_model_form_error_compatibility<ServerFn, Selection, S, P>()
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
	ServerFn: ModelFormServerFn<Selection, S, P>,
	<ServerFn as ModelFormServerFn<Selection, S, P>>::Error: Into<crate::ServerFnError>,
{
}

/// Dynamic control state for a model-backed form.
pub struct ModelFormState<S, P>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
{
	values: HashMap<&'static str, serde_json::Value>,
	#[cfg(wasm)]
	selected_files: HashMap<&'static str, web_sys::File>,
	_schema: PhantomData<S>,
	_policy: PhantomData<P>,
}

impl<S, P> Clone for ModelFormState<S, P>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
{
	fn clone(&self) -> Self {
		Self {
			values: self.values.clone(),
			#[cfg(wasm)]
			selected_files: self.selected_files.clone(),
			_schema: PhantomData,
			_policy: PhantomData,
		}
	}
}

impl<S, P> ModelFormState<S, P>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
{
	/// Creates empty model-form control state.
	pub fn new() -> Self {
		let mut values = HashMap::new();
		for descriptor in S::fields() {
			if descriptor.editable
				&& P::allows(descriptor.name)
				&& !descriptor.nullable
				&& !descriptor.has_default
				&& matches!(descriptor.kind, ModelFormFieldKind::Boolean)
			{
				values.insert(descriptor.name, serde_json::Value::Bool(false));
			}
		}
		Self {
			values,
			#[cfg(wasm)]
			selected_files: HashMap::new(),
			_schema: PhantomData,
			_policy: PhantomData,
		}
	}

	/// Stores a control value after validating it against the generated schema.
	/// An empty string clears a nullable field and removes other optional values.
	///
	/// # Errors
	///
	/// Returns a typed payload error when the field is unknown, forbidden by the
	/// form policy, or cannot be converted according to its model field kind.
	pub fn set_value(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		let descriptor = S::fields()
			.iter()
			.find(|descriptor| descriptor.name == field)
			.ok_or_else(|| ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})?;
		if !P::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}
		if is_file_kind(descriptor.kind) {
			return Err(invalid_value(
				field,
				"file fields must be set with set_file",
			));
		}
		if descriptor.nullable && value.is_null() {
			self.values.insert(descriptor.name, serde_json::Value::Null);
			return Ok(());
		}
		if !descriptor.required
			&& matches!(&value, serde_json::Value::String(text) if text.is_empty())
		{
			if descriptor.nullable {
				if descriptor.has_default && !self.values.contains_key(descriptor.name) {
					return Ok(());
				}
				self.values.insert(descriptor.name, serde_json::Value::Null);
				return Ok(());
			}
			if !matches!(
				descriptor.kind,
				ModelFormFieldKind::Text { .. }
					| ModelFormFieldKind::Email { .. }
					| ModelFormFieldKind::Url { .. }
			) {
				self.values.remove(descriptor.name);
				return Ok(());
			}
			if descriptor.has_default {
				self.values.remove(descriptor.name);
				return Ok(());
			}
		}

		match convert_control_value(descriptor, value) {
			Ok(converted) => {
				self.values.insert(descriptor.name, converted);
				Ok(())
			}
			Err(error) => Err(error),
		}
	}

	/// Stores raw text emitted by a shared controlled text binding.
	#[doc(hidden)]
	pub fn set_binding_text(
		&mut self,
		field: &str,
		value: String,
	) -> Result<(), ModelFormPayloadError> {
		let descriptor = self.binding_descriptor(field)?;
		if !matches!(
			descriptor.kind,
			ModelFormFieldKind::Text { .. }
				| ModelFormFieldKind::Email { .. }
				| ModelFormFieldKind::Url { .. }
		) {
			return Err(invalid_value(field, "expected a shared text control field"));
		}
		self.values
			.insert(descriptor.name, serde_json::Value::String(value));
		Ok(())
	}

	/// Stores a number emitted by a shared controlled numeric binding.
	#[doc(hidden)]
	pub fn set_binding_number(&mut self, field: &str, raw: &str) -> Result<(), NumberParseError> {
		let descriptor = self
			.binding_descriptor(field)
			.map_err(|_| NumberParseError::from_raw_kind(raw, NumberParseErrorKind::OutOfRange))?;
		if raw.is_empty() {
			return self
				.set_value(field, serde_json::Value::String(String::new()))
				.map_err(|_| NumberParseError::from_raw_kind(raw, NumberParseErrorKind::Empty));
		}
		let value = match descriptor.kind {
			ModelFormFieldKind::Integer { .. } if raw.starts_with('-') => {
				let value = <i64 as NumberValue>::parse_control_value(raw)?;
				serde_json::Value::from(value)
			}
			ModelFormFieldKind::Integer { .. } => {
				let value = <u64 as NumberValue>::parse_control_value(raw)?;
				serde_json::Value::from(value)
			}
			ModelFormFieldKind::Float { .. } => {
				let value = <f64 as NumberValue>::parse_control_value(raw)?;
				serde_json::Number::from_f64(value)
					.map(serde_json::Value::Number)
					.ok_or_else(|| {
						NumberParseError::from_raw_kind(raw, NumberParseErrorKind::OutOfRange)
					})?
			}
			_ => {
				return Err(NumberParseError::from_raw_kind(
					raw,
					NumberParseErrorKind::OutOfRange,
				));
			}
		};
		self.set_value(field, value)
			.map_err(|_| NumberParseError::from_raw_kind(raw, NumberParseErrorKind::OutOfRange))
	}

	/// Stores a typed runtime value after converting it to the model-form JSON representation.
	#[doc(hidden)]
	pub fn set_any_value<T>(&mut self, field: &str, value: T) -> Result<(), ModelFormPayloadError>
	where
		T: Any + 'static,
	{
		let value = any_value_to_json(value).ok_or_else(|| {
			invalid_value(
				field,
				format!(
					"unsupported runtime value type `{}`",
					std::any::type_name::<T>()
				),
			)
		})?;
		self.set_value(field, value)
	}

	/// Removes one model-form value and any selected file associated with it.
	#[doc(hidden)]
	pub fn clear_value(&mut self, field: &str) -> Result<(), ModelFormPayloadError> {
		let descriptor = S::fields()
			.iter()
			.find(|descriptor| descriptor.name == field)
			.ok_or_else(|| ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})?;
		if !P::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}
		self.values.remove(descriptor.name);
		#[cfg(wasm)]
		if is_file_kind(descriptor.kind) {
			self.selected_files.remove(descriptor.name);
		}
		Ok(())
	}

	/// Returns the converted value stored for a model field.
	pub fn value(&self, field: &str) -> Option<&serde_json::Value> {
		self.values.get(field)
	}

	/// Deserializes one scalar server-function argument from model-form state.
	///
	/// # Errors
	///
	/// Returns a typed payload error when the field is unknown, forbidden, a file field,
	/// or cannot be deserialized as the requested argument type.
	#[cfg(wasm)]
	pub fn json_argument<T>(&self, field: &str) -> Result<T, ModelFormPayloadError>
	where
		T: serde::de::DeserializeOwned,
	{
		let descriptor = S::fields()
			.iter()
			.find(|descriptor| descriptor.name == field)
			.ok_or_else(|| ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})?;
		if !P::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}
		if is_file_kind(descriptor.kind) {
			return Err(invalid_value(field, "expected a scalar field"));
		}
		serde_json::from_value(
			self.values
				.get(descriptor.name)
				.cloned()
				.unwrap_or(serde_json::Value::Null),
		)
		.map_err(|error| invalid_value(field, error.to_string()))
	}

	/// Stores a browser-selected file for a file or image model field.
	///
	/// # Errors
	///
	/// Returns a typed payload error when the field is unknown, forbidden, or not a file field.
	#[cfg(wasm)]
	pub fn set_file(
		&mut self,
		field: &str,
		file: web_sys::File,
	) -> Result<(), ModelFormPayloadError> {
		let descriptor = Self::file_descriptor(field)?;
		self.selected_files.insert(descriptor.name, file);
		Ok(())
	}

	/// Clears the browser-selected file for a file or image model field.
	///
	/// # Errors
	///
	/// Returns a typed payload error when the field is unknown, forbidden, or not a file field.
	#[cfg(wasm)]
	pub fn clear_file(&mut self, field: &str) -> Result<(), ModelFormPayloadError> {
		let descriptor = Self::file_descriptor(field)?;
		self.selected_files.remove(descriptor.name);
		Ok(())
	}

	/// Returns the browser-selected file for a model field.
	#[cfg(wasm)]
	pub fn file(&self, field: &str) -> Option<&web_sys::File> {
		self.selected_files.get(field)
	}

	/// Clears every browser-selected file.
	#[cfg(wasm)]
	pub fn clear_selected_files(&mut self) -> bool {
		let changed = !self.selected_files.is_empty();
		self.selected_files.clear();
		changed
	}

	/// Clears only files that still match a submitted model-form snapshot.
	#[cfg(wasm)]
	pub fn clear_selected_files_matching(&mut self, submitted: &Self) -> bool {
		let previous_len = self.selected_files.len();
		self.selected_files.retain(|field, file| {
			submitted
				.selected_files
				.get(field)
				.is_none_or(|submitted_file| {
					let submitted_file = wasm_bindgen::JsValue::from(submitted_file.clone());
					let file = wasm_bindgen::JsValue::from(file.clone());
					submitted_file != file
				})
		});
		previous_len != self.selected_files.len()
	}

	/// Returns a selected file required by a server-function argument.
	///
	/// # Errors
	///
	/// Returns an exact required-field error when no file is selected.
	#[cfg(wasm)]
	pub fn required_file_argument(
		&self,
		field: &str,
	) -> Result<web_sys::File, ModelFormPayloadError> {
		let descriptor = Self::file_descriptor(field)?;
		self.selected_files
			.get(descriptor.name)
			.cloned()
			.ok_or_else(|| invalid_value(field, "is required"))
	}

	/// Returns an optional file for a server-function argument.
	///
	/// # Errors
	///
	/// Returns a typed payload error when the field is unknown, forbidden, or not a file field.
	#[cfg(wasm)]
	pub fn optional_file_argument(
		&self,
		field: &str,
	) -> Result<Option<web_sys::File>, ModelFormPayloadError> {
		let descriptor = Self::file_descriptor(field)?;
		Ok(self.selected_files.get(descriptor.name).cloned())
	}

	/// Returns selected editable descriptors in generated schema order.
	pub fn selected_descriptors(&self) -> Vec<&'static ModelFormFieldDescriptor> {
		S::fields()
			.iter()
			.filter(|descriptor| descriptor.editable && P::allows(descriptor.name))
			.collect()
	}

	/// Validates values retained by controlled bindings before submission.
	///
	/// Controlled text bindings intentionally retain raw editor text so that
	/// incomplete input is not lost. Submission must therefore run the normal
	/// descriptor conversion again instead of trusting the stored JSON shape.
	#[doc(hidden)]
	pub fn validate_values(&self) -> Result<(), ModelFormPayloadError> {
		self.validated_for_submission().map(|_| ())
	}

	/// Returns a submission snapshot with controlled values converted to payload values.
	#[doc(hidden)]
	pub fn validated_for_submission(&self) -> Result<Self, ModelFormPayloadError> {
		let mut validated = self.clone();
		for descriptor in self.selected_descriptors() {
			if is_file_kind(descriptor.kind) {
				continue;
			}
			match self.values.get(descriptor.name) {
				Some(serde_json::Value::Null) if descriptor.nullable => {}
				Some(value) => {
					let converted = convert_control_value(descriptor, value.clone())?;
					validated.values.insert(descriptor.name, converted);
				}
				None if descriptor.required => {
					return Err(invalid_value(descriptor.name, "is required"));
				}
				None => {}
			}
		}
		Ok(validated)
	}

	/// Clears every value that belongs to the active form policy.
	pub fn clear_selected_values(&mut self) {
		self.values.retain(|field, _| {
			!S::fields().iter().any(|descriptor| {
				descriptor.name == *field && descriptor.editable && P::allows(field)
			})
		});
		#[cfg(wasm)]
		self.selected_files.retain(|field, _| {
			!S::fields().iter().any(|descriptor| {
				descriptor.name == *field && descriptor.editable && P::allows(field)
			})
		});
	}

	/// Builds the one typed payload sent to the configured server function.
	///
	/// # Errors
	///
	/// Returns the first typed error raised while applying a converted control
	/// value to the generated payload.
	pub fn build_payload<D>(&self) -> Result<D, ModelFormPayloadError>
	where
		D: Default + ModelFormPayload<P>,
	{
		let mut payload = D::default();
		for descriptor in self.selected_descriptors() {
			if is_file_kind(descriptor.kind) {
				continue;
			}
			if let Some(value) = self.values.get(descriptor.name) {
				let value = match value {
					serde_json::Value::Null if descriptor.nullable => value.clone(),
					value => convert_control_value(descriptor, value.clone())?,
				};
				payload.set_json(descriptor.name, value)?;
			}
		}
		Ok(payload)
	}

	/// Builds a payload using a nameable policy while retaining this form's
	/// field-selection policy for the values copied into it.
	pub fn build_payload_for<D, Q>(&self) -> Result<D, ModelFormPayloadError>
	where
		D: Default + ModelFormPayload<Q>,
		Q: ModelFormPolicy,
	{
		self.build_payload_for_with_file_policy::<D, Q>(false)
	}

	/// Builds a JSON model-form payload and rejects file fields instead of
	/// silently omitting them.
	#[doc(hidden)]
	pub fn build_json_payload_for<D, Q>(&self) -> Result<D, ModelFormPayloadError>
	where
		D: Default + ModelFormPayload<Q>,
		Q: ModelFormPolicy,
	{
		self.build_payload_for_with_file_policy::<D, Q>(true)
	}

	fn build_payload_for_with_file_policy<D, Q>(
		&self,
		reject_file_fields: bool,
	) -> Result<D, ModelFormPayloadError>
	where
		D: Default + ModelFormPayload<Q>,
		Q: ModelFormPolicy,
	{
		let mut payload = D::default();
		for descriptor in self.selected_descriptors() {
			if is_file_kind(descriptor.kind) {
				if reject_file_fields {
					return Err(invalid_value(
						descriptor.name,
						"file fields require a typed multipart server function",
					));
				}
				continue;
			}
			if let Some(value) = self.values.get(descriptor.name) {
				payload.set_json(descriptor.name, value.clone())?;
			}
		}
		Ok(payload)
	}

	fn binding_descriptor(
		&self,
		field: &str,
	) -> Result<&'static ModelFormFieldDescriptor, ModelFormPayloadError> {
		let descriptor = S::fields()
			.iter()
			.find(|descriptor| descriptor.name == field)
			.ok_or_else(|| ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})?;
		if !descriptor.editable || !P::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}
		if is_file_kind(descriptor.kind) {
			return Err(invalid_value(
				field,
				"file fields must be set with set_file",
			));
		}
		Ok(descriptor)
	}

	#[cfg(wasm)]
	fn file_descriptor(
		field: &str,
	) -> Result<&'static ModelFormFieldDescriptor, ModelFormPayloadError> {
		let descriptor = S::fields()
			.iter()
			.find(|descriptor| descriptor.name == field)
			.ok_or_else(|| ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})?;
		if !P::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}
		if !is_file_kind(descriptor.kind) {
			return Err(invalid_value(field, "expected a file field"));
		}
		Ok(descriptor)
	}
}

fn any_value_to_json<T>(value: T) -> Option<serde_json::Value>
where
	T: Any + 'static,
{
	let mut value: Box<dyn Any> = Box::new(value);

	macro_rules! downcast {
		($type:ty, $convert:expr) => {
			value = match value.downcast::<$type>() {
				Ok(value) => return Some(($convert)(*value)),
				Err(value) => value,
			};
		};
	}

	macro_rules! downcast_fallible {
		($type:ty, $convert:expr) => {
			value = match value.downcast::<$type>() {
				Ok(value) => return ($convert)(*value),
				Err(value) => value,
			};
		};
	}

	downcast!(serde_json::Value, |value: serde_json::Value| value);
	downcast!(String, serde_json::Value::String);
	downcast!(&'static str, |value: &'static str| {
		serde_json::Value::String(value.to_owned())
	});
	downcast!(bool, serde_json::Value::Bool);
	downcast!(i8, serde_json::Value::from);
	downcast!(i16, serde_json::Value::from);
	downcast!(i32, serde_json::Value::from);
	downcast!(i64, serde_json::Value::from);
	downcast!(isize, serde_json::Value::from);
	downcast!(u8, serde_json::Value::from);
	downcast!(u16, serde_json::Value::from);
	downcast!(u32, serde_json::Value::from);
	downcast!(u64, serde_json::Value::from);
	downcast!(usize, serde_json::Value::from);
	downcast_fallible!(f32, |value: f32| {
		serde_json::Number::from_f64(f64::from(value)).map(serde_json::Value::Number)
	});
	downcast_fallible!(f64, |value: f64| {
		serde_json::Number::from_f64(value).map(serde_json::Value::Number)
	});
	downcast!(Option<String>, |value: Option<String>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::String)
	});
	downcast!(Option<bool>, |value: Option<bool>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::Bool)
	});
	downcast!(Option<i8>, |value: Option<i8>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::from)
	});
	downcast!(Option<i16>, |value: Option<i16>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::from)
	});
	downcast!(Option<i32>, |value: Option<i32>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::from)
	});
	downcast!(Option<i64>, |value: Option<i64>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::from)
	});
	downcast!(Option<isize>, |value: Option<isize>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::from)
	});
	downcast!(Option<u8>, |value: Option<u8>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::from)
	});
	downcast!(Option<u16>, |value: Option<u16>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::from)
	});
	downcast!(Option<u32>, |value: Option<u32>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::from)
	});
	downcast!(Option<u64>, |value: Option<u64>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::from)
	});
	downcast!(Option<usize>, |value: Option<usize>| {
		value.map_or(serde_json::Value::Null, serde_json::Value::from)
	});
	downcast_fallible!(Option<f32>, |value: Option<f32>| {
		value.map_or(Some(serde_json::Value::Null), |value| {
			serde_json::Number::from_f64(f64::from(value)).map(serde_json::Value::Number)
		})
	});
	downcast_fallible!(Option<f64>, |value: Option<f64>| {
		value.map_or(Some(serde_json::Value::Null), |value| {
			serde_json::Number::from_f64(value).map(serde_json::Value::Number)
		})
	});
	downcast!(Option<serde_json::Value>, |value: Option<
		serde_json::Value,
	>| {
		value.map_or(serde_json::Value::Null, |value| value)
	});
	downcast!(Decimal, |value: Decimal| {
		serde_json::Value::String(value.to_string())
	});
	downcast!(Option<Decimal>, |value: Option<Decimal>| {
		value.map_or(serde_json::Value::Null, |value| {
			serde_json::Value::String(value.to_string())
		})
	});

	#[cfg(feature = "chrono")]
	{
		downcast!(chrono::NaiveDate, |value: chrono::NaiveDate| {
			serde_json::Value::String(value.to_string())
		});
		downcast!(chrono::NaiveTime, |value: chrono::NaiveTime| {
			serde_json::Value::String(value.to_string())
		});
		downcast!(chrono::NaiveDateTime, |value: chrono::NaiveDateTime| {
			serde_json::Value::String(value.to_string())
		});
		downcast!(chrono::DateTime<chrono::Utc>, |value: chrono::DateTime<
			chrono::Utc,
		>| {
			serde_json::Value::String(value.to_rfc3339())
		});
		downcast!(Option<chrono::NaiveDate>, |value: Option<
			chrono::NaiveDate,
		>| {
			value.map_or(serde_json::Value::Null, |value| {
				serde_json::Value::String(value.to_string())
			})
		});
		downcast!(Option<chrono::NaiveTime>, |value: Option<
			chrono::NaiveTime,
		>| {
			value.map_or(serde_json::Value::Null, |value| {
				serde_json::Value::String(value.to_string())
			})
		});
		downcast!(Option<chrono::NaiveDateTime>, |value: Option<
			chrono::NaiveDateTime,
		>| {
			value.map_or(serde_json::Value::Null, |value| {
				serde_json::Value::String(value.to_string())
			})
		});
		downcast!(Option<chrono::DateTime<chrono::Utc>>, |value: Option<
			chrono::DateTime<chrono::Utc>,
		>| {
			value.map_or(serde_json::Value::Null, |value| {
				serde_json::Value::String(value.to_rfc3339())
			})
		});
	}

	#[cfg(feature = "uuid")]
	{
		downcast!(uuid::Uuid, |value: uuid::Uuid| {
			serde_json::Value::String(value.to_string())
		});
		downcast!(Option<uuid::Uuid>, |value: Option<uuid::Uuid>| {
			value.map_or(serde_json::Value::Null, |value| {
				serde_json::Value::String(value.to_string())
			})
		});
	}

	drop(value);
	None
}

impl<S, P> Default for ModelFormState<S, P>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
{
	fn default() -> Self {
		Self::new()
	}
}

fn convert_control_value(
	descriptor: &ModelFormFieldDescriptor,
	value: serde_json::Value,
) -> Result<serde_json::Value, ModelFormPayloadError> {
	match descriptor.kind {
		ModelFormFieldKind::Text {
			min_length,
			max_length,
			..
		} => {
			let text = expect_string(descriptor.name, value)?.trim().to_owned();
			if text.is_empty() && !descriptor.required {
				return Ok(serde_json::Value::String(text));
			}
			if text.is_empty() {
				return Err(invalid_value(descriptor.name, "must not be empty"));
			}
			if let Some(min_length) = min_length
				&& text.chars().count() < min_length
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must contain at least {min_length} characters"),
				));
			}
			if let Some(max_length) = max_length
				&& text.chars().count() > max_length
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must contain at most {max_length} characters"),
				));
			}
			Ok(serde_json::Value::String(text))
		}
		ModelFormFieldKind::Email {
			min_length,
			max_length,
		}
		| ModelFormFieldKind::Url {
			min_length,
			max_length,
		} => {
			let text = expect_string(descriptor.name, value)?.trim().to_owned();
			if text.is_empty() && !descriptor.required {
				return Ok(serde_json::Value::String(text));
			}
			if let Some(min_length) = min_length
				&& text.chars().count() < min_length
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must contain at least {min_length} characters"),
				));
			}
			if let Some(max_length) = max_length
				&& text.chars().count() > max_length
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must contain at most {max_length} characters"),
				));
			}
			let is_valid = match descriptor.kind {
				ModelFormFieldKind::Email { .. } => is_email(&text),
				ModelFormFieldKind::Url { .. } => is_url(&text),
				_ => unreachable!("email and URL fields are handled by this conversion branch"),
			};
			if !is_valid {
				return Err(invalid_value(descriptor.name, "has an invalid format"));
			}
			Ok(serde_json::Value::String(text))
		}
		ModelFormFieldKind::Integer { min, max } => {
			let number = match value {
				serde_json::Value::Number(number)
					if number.as_i64().is_some() || number.as_u64().is_some() =>
				{
					number
				}
				serde_json::Value::Number(_) => {
					return Err(invalid_value(descriptor.name, "expected an integer"));
				}
				serde_json::Value::String(text) => match text.parse::<i64>() {
					Ok(integer) => serde_json::Number::from(integer),
					Err(signed_error) => match text.parse::<u64>() {
						Ok(integer) => serde_json::Number::from(integer),
						Err(_) => {
							return Err(invalid_value(
								descriptor.name,
								format!("invalid integer: {signed_error}"),
							));
						}
					},
				},
				_ => return Err(invalid_value(descriptor.name, "expected an integer")),
			};

			if let Some(integer) = number.as_i64() {
				if let Some(min) = min
					&& integer < min
				{
					return Err(invalid_value(
						descriptor.name,
						format!("must be greater than or equal to {min}"),
					));
				}
				if let Some(max) = max
					&& integer > max
				{
					return Err(invalid_value(
						descriptor.name,
						format!("must be less than or equal to {max}"),
					));
				}
			} else if let Some(integer) = number.as_u64() {
				if let Some(min) = min
					&& min >= 0 && integer < min as u64
				{
					return Err(invalid_value(
						descriptor.name,
						format!("must be greater than or equal to {min}"),
					));
				}
				if let Some(max) = max
					&& (max < 0 || integer > max as u64)
				{
					return Err(invalid_value(
						descriptor.name,
						format!("must be less than or equal to {max}"),
					));
				}
			}
			Ok(serde_json::Value::Number(number))
		}
		ModelFormFieldKind::Float { min, max } => {
			let float = match value {
				serde_json::Value::Number(number) => number
					.as_f64()
					.ok_or_else(|| invalid_value(descriptor.name, "expected a finite number"))?,
				serde_json::Value::String(text) => text.parse::<f64>().map_err(|error| {
					invalid_value(descriptor.name, format!("invalid number: {error}"))
				})?,
				_ => return Err(invalid_value(descriptor.name, "expected a number")),
			};
			let number = serde_json::Number::from_f64(float)
				.ok_or_else(|| invalid_value(descriptor.name, "expected a finite number"))?;
			if let Some(min) = min
				&& float < min
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must be greater than or equal to {min}"),
				));
			}
			if let Some(max) = max
				&& float > max
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must be less than or equal to {max}"),
				));
			}
			Ok(serde_json::Value::Number(number))
		}
		ModelFormFieldKind::Decimal { min, max } => {
			let decimal_text = match &value {
				serde_json::Value::Number(number) => number.to_string(),
				serde_json::Value::String(text) => text.clone(),
				_ => return Err(invalid_value(descriptor.name, "expected a decimal number")),
			};
			let decimal = Decimal::from_str(&decimal_text).map_err(|error| {
				invalid_value(descriptor.name, format!("invalid decimal: {error}"))
			})?;
			if let Some(min) = min
				&& decimal < Decimal::from_str(min).expect("generated decimal minimum is valid")
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must be greater than or equal to {min}"),
				));
			}
			if let Some(max) = max
				&& decimal > Decimal::from_str(max).expect("generated decimal maximum is valid")
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must be less than or equal to {max}"),
				));
			}
			Ok(value)
		}
		ModelFormFieldKind::Boolean => match value {
			serde_json::Value::Bool(value) => Ok(serde_json::Value::Bool(value)),
			serde_json::Value::String(text) => match text.as_str() {
				"true" => Ok(serde_json::Value::Bool(true)),
				"false" => Ok(serde_json::Value::Bool(false)),
				_ => Err(invalid_value(descriptor.name, "expected true or false")),
			},
			_ => Err(invalid_value(descriptor.name, "expected a boolean")),
		},
		ModelFormFieldKind::Date => {
			let text = expect_string(descriptor.name, value)?;
			if !is_date(&text) {
				return Err(invalid_value(descriptor.name, "expected YYYY-MM-DD"));
			}
			Ok(serde_json::Value::String(text))
		}
		ModelFormFieldKind::Time => {
			let text = expect_string(descriptor.name, value)?;
			normalize_time(&text)
				.map(serde_json::Value::String)
				.ok_or_else(|| invalid_value(descriptor.name, "expected HH:MM[:SS]"))
		}
		ModelFormFieldKind::DateTime | ModelFormFieldKind::NaiveDateTime => {
			let text = expect_string(descriptor.name, value)?;
			normalize_datetime_local(
				&text,
				matches!(descriptor.kind, ModelFormFieldKind::DateTime),
			)
			.map(serde_json::Value::String)
			.ok_or_else(|| {
				invalid_value(descriptor.name, "expected YYYY-MM-DDTHH:MM[:SS[.fraction]]")
			})
		}
		ModelFormFieldKind::Uuid => {
			let text = expect_string(descriptor.name, value)?;
			uuid::Uuid::parse_str(&text).map_err(|error| {
				invalid_value(descriptor.name, format!("invalid UUID: {error}"))
			})?;
			Ok(serde_json::Value::String(text))
		}
		ModelFormFieldKind::Json => match value {
			serde_json::Value::String(text) => serde_json::from_str(&text)
				.map_err(|error| invalid_value(descriptor.name, format!("invalid JSON: {error}")))
				.and_then(|value| validate_json_depth(descriptor.name, value)),
			value => validate_json_depth(descriptor.name, value),
		},
		ModelFormFieldKind::File | ModelFormFieldKind::Image => Err(invalid_value(
			descriptor.name,
			"file fields must be set with set_file",
		)),
	}
}

fn is_file_kind(kind: ModelFormFieldKind) -> bool {
	matches!(kind, ModelFormFieldKind::File | ModelFormFieldKind::Image)
}

fn validate_json_depth(
	field: &str,
	value: serde_json::Value,
) -> Result<serde_json::Value, ModelFormPayloadError> {
	fn within_limit(value: &serde_json::Value, depth: usize) -> bool {
		if depth > 64 {
			return false;
		}
		match value {
			serde_json::Value::Array(values) => {
				values.iter().all(|value| within_limit(value, depth + 1))
			}
			serde_json::Value::Object(values) => {
				values.values().all(|value| within_limit(value, depth + 1))
			}
			_ => true,
		}
	}
	if within_limit(&value, 0) {
		Ok(value)
	} else {
		Err(invalid_value(
			field,
			"JSON exceeds maximum nesting depth of 64",
		))
	}
}

fn expect_string(field: &str, value: serde_json::Value) -> Result<String, ModelFormPayloadError> {
	match value {
		serde_json::Value::String(text) => Ok(text),
		_ => Err(invalid_value(field, "expected a string")),
	}
}

fn invalid_value(field: &str, message: impl Into<String>) -> ModelFormPayloadError {
	ModelFormPayloadError::InvalidValue {
		field: field.to_owned(),
		message: message.into(),
	}
}

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$")
		.expect("native email validation pattern is valid")
});

fn is_email(value: &str) -> bool {
	EMAIL_REGEX.is_match(value)
}

fn is_url(value: &str) -> bool {
	static URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
		Regex::new(r"^https?://(?:(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}|localhost|\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})(?::\d+)?(?:/[^\s]*)?$")
			.expect("native URL validation pattern is valid")
	});
	URL_REGEX.is_match(value)
}

fn is_date(value: &str) -> bool {
	let bytes = value.as_bytes();
	if !(bytes.len() == 10
		&& bytes[4] == b'-'
		&& bytes[7] == b'-'
		&& bytes
			.iter()
			.enumerate()
			.all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit()))
	{
		return false;
	}
	let Some(year) = value[0..4].parse::<u32>().ok() else {
		return false;
	};
	if !(1_000..=9_999).contains(&year) {
		return false;
	}
	let Some(month) = value[5..7].parse::<u32>().ok() else {
		return false;
	};
	let Some(day) = value[8..10].parse::<u32>().ok() else {
		return false;
	};
	let max_day = match month {
		1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
		4 | 6 | 9 | 11 => 30,
		2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
			29
		}
		2 => 28,
		_ => return false,
	};
	(1..=max_day).contains(&day)
}

fn normalize_time(value: &str) -> Option<String> {
	let value = value.trim();
	let (time, meridiem) = match value.rsplit_once(' ') {
		Some((time, meridiem @ ("AM" | "PM"))) => (time, Some(meridiem)),
		Some(_) => return None,
		None => (value, None),
	};
	let (time, fraction) = time
		.split_once('.')
		.map_or((time, None), |(time, fraction)| (time, Some(fraction)));
	if fraction.is_some_and(|fraction| {
		fraction.is_empty()
			|| !(1..=9).contains(&fraction.len())
			|| !fraction.bytes().all(|byte| byte.is_ascii_digit())
	}) {
		return None;
	}
	let parts: Vec<_> = time.split(':').collect();
	if !matches!(parts.len(), 2 | 3)
		|| !parts
			.iter()
			.all(|part| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_digit()))
	{
		return None;
	}
	let mut hour = parts[0].parse::<u32>().ok()?;
	let minute = parts[1].parse::<u32>().ok()?;
	let second = parts
		.get(2)
		.map_or(Some(0), |part| part.parse::<u32>().ok())?;
	if minute > 59 || second > 59 {
		return None;
	}
	match meridiem {
		Some("AM") if (1..=12).contains(&hour) => hour %= 12,
		Some("PM") if (1..=12).contains(&hour) => hour = (hour % 12) + 12,
		Some(_) => return None,
		None if hour > 23 => return None,
		None => {}
	}
	Some(match fraction {
		Some(fraction) => format!("{hour:02}:{minute:02}:{second:02}.{fraction}"),
		None => format!("{hour:02}:{minute:02}:{second:02}"),
	})
}

fn normalize_datetime_local(value: &str, aware: bool) -> Option<String> {
	let (date, time) = value.split_once('T').or_else(|| value.split_once(' '))?;
	if !is_date(date) {
		return None;
	}
	let time = if aware {
		time.strip_suffix('Z').unwrap_or(time)
	} else if time.ends_with('Z') {
		return None;
	} else {
		time
	};
	if time.contains(['+', '-']) {
		return None;
	}
	let (whole_time, fraction) = time
		.split_once('.')
		.map_or((time, None), |(whole, fraction)| (whole, Some(fraction)));
	if fraction.is_some_and(|fraction| {
		fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit())
	}) {
		return None;
	}
	let mut parts = whole_time.split(':');
	let hour = parts.next()?.parse::<u32>().ok()?;
	let minute = parts.next()?.parse::<u32>().ok()?;
	let second = parts
		.next()
		.map_or(Some(0), |second| second.parse::<u32>().ok())?;
	if parts.next().is_some() || hour > 23 || minute > 59 || second > 59 {
		return None;
	}
	let fraction = fraction.map_or_else(String::new, |fraction| format!(".{fraction}"));
	let timezone = if aware { "Z" } else { "" };
	Some(format!(
		"{date}T{hour:02}:{minute:02}:{second:02}{fraction}{timezone}"
	))
}

#[cfg(test)]
mod tests {
	use super::{ModelFormState, any_value_to_json, is_date};
	use reinhardt_core::model_form::{
		AllEditableModelFields, ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload,
		ModelFormPayloadError, ModelFormSchema,
	};

	struct NullableBooleanSchema;

	#[test]
	fn nullable_numeric_values_convert_to_json() {
		assert_eq!(any_value_to_json(Some(1_i8)), Some(serde_json::json!(1)));
		assert_eq!(any_value_to_json(Some(2_i16)), Some(serde_json::json!(2)));
		assert_eq!(any_value_to_json(Some(3_i32)), Some(serde_json::json!(3)));
		assert_eq!(any_value_to_json(Some(4_i64)), Some(serde_json::json!(4)));
		assert_eq!(any_value_to_json(Some(5_isize)), Some(serde_json::json!(5)));
		assert_eq!(any_value_to_json(Some(6_u8)), Some(serde_json::json!(6)));
		assert_eq!(any_value_to_json(Some(7_u16)), Some(serde_json::json!(7)));
		assert_eq!(any_value_to_json(Some(8_u32)), Some(serde_json::json!(8)));
		assert_eq!(any_value_to_json(Some(9_u64)), Some(serde_json::json!(9)));
		assert_eq!(
			any_value_to_json(Some(10_usize)),
			Some(serde_json::json!(10))
		);
		assert_eq!(
			any_value_to_json(Some(1.5_f32)),
			Some(serde_json::json!(1.5))
		);
		assert_eq!(
			any_value_to_json(Some(2.5_f64)),
			Some(serde_json::json!(2.5))
		);
		assert_eq!(
			any_value_to_json(None::<i32>),
			Some(serde_json::Value::Null)
		);
		assert!(any_value_to_json(f64::NAN).is_none());
		assert!(any_value_to_json(f64::INFINITY).is_none());
		assert!(any_value_to_json(Some(f64::NEG_INFINITY)).is_none());
		assert_eq!(
			any_value_to_json(Some(serde_json::json!({"enabled": true}))),
			Some(serde_json::json!({"enabled": true}))
		);
		assert_eq!(
			any_value_to_json(None::<serde_json::Value>),
			Some(serde_json::Value::Null)
		);
		assert_eq!(
			any_value_to_json(rust_decimal::Decimal::new(125, 2)),
			Some(serde_json::json!("1.25"))
		);
	}

	#[cfg(feature = "chrono")]
	#[test]
	fn native_chrono_values_convert_to_form_strings() {
		use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

		assert_eq!(
			any_value_to_json(NaiveDate::from_ymd_opt(2026, 8, 24).unwrap()),
			Some(serde_json::json!("2026-08-24"))
		);
		assert_eq!(
			any_value_to_json(NaiveTime::from_hms_opt(12, 34, 56).unwrap()),
			Some(serde_json::json!("12:34:56"))
		);
		assert_eq!(
			any_value_to_json(
				NaiveDateTime::parse_from_str("2026-08-24 12:34:56", "%Y-%m-%d %H:%M:%S").unwrap()
			),
			Some(serde_json::json!("2026-08-24 12:34:56"))
		);
	}

	#[cfg(feature = "uuid")]
	#[test]
	fn native_uuid_values_convert_to_form_strings() {
		let uuid = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000042").unwrap();
		assert_eq!(
			any_value_to_json(Some(uuid)),
			Some(serde_json::json!("00000000-0000-0000-0000-000000000042"))
		);
	}

	impl ModelFormSchema for NullableBooleanSchema {
		type Model = ();

		fn fields() -> &'static [ModelFormFieldDescriptor] {
			const FIELDS: [ModelFormFieldDescriptor; 1] = [ModelFormFieldDescriptor {
				name: "published",
				kind: ModelFormFieldKind::Boolean,
				required: false,
				has_default: false,
				nullable: true,
				editable: true,
				generated_relation_id: false,
			}];
			&FIELDS
		}
	}

	#[test]
	fn nullable_boolean_starts_unset() {
		let state = ModelFormState::<NullableBooleanSchema, AllEditableModelFields>::new();

		assert_eq!(state.value("published"), None);
	}

	struct NullableDefaultSchema;

	impl ModelFormSchema for NullableDefaultSchema {
		type Model = ();

		fn fields() -> &'static [ModelFormFieldDescriptor] {
			const FIELDS: [ModelFormFieldDescriptor; 1] = [ModelFormFieldDescriptor {
				name: "summary",
				kind: ModelFormFieldKind::Text {
					min_length: None,
					max_length: None,
					multiline: false,
				},
				required: false,
				has_default: true,
				nullable: true,
				editable: true,
				generated_relation_id: false,
			}];
			&FIELDS
		}
	}

	#[test]
	fn untouched_nullable_default_remains_omitted() {
		let mut state = ModelFormState::<NullableDefaultSchema, AllEditableModelFields>::new();

		state
			.set_value("summary", serde_json::Value::String(String::new()))
			.expect("an empty optional control should be accepted");

		assert_eq!(state.value("summary"), None);
	}

	#[test]
	fn explicit_nullable_default_clear_is_preserved() {
		let mut state = ModelFormState::<NullableDefaultSchema, AllEditableModelFields>::new();

		state
			.set_value("summary", serde_json::Value::Null)
			.expect("an explicit nullable clear should be accepted");

		assert_eq!(state.value("summary"), Some(&serde_json::Value::Null));
	}

	struct F32Schema;

	impl ModelFormSchema for F32Schema {
		type Model = ();

		fn fields() -> &'static [ModelFormFieldDescriptor] {
			const FIELDS: [ModelFormFieldDescriptor; 1] = [ModelFormFieldDescriptor {
				name: "ratio",
				kind: ModelFormFieldKind::Float {
					min: Some(f32::MIN as f64),
					max: Some(f32::MAX as f64),
				},
				required: true,
				has_default: false,
				nullable: false,
				editable: true,
				generated_relation_id: false,
			}];
			&FIELDS
		}
	}

	#[test]
	fn f32_fields_reject_values_that_would_narrow_to_infinity() {
		let mut state = ModelFormState::<F32Schema, AllEditableModelFields>::new();

		assert!(
			state
				.set_value("ratio", serde_json::Value::String("1e100".to_owned()))
				.is_err()
		);
		assert_eq!(state.value("ratio"), None);
	}

	struct OptionalContactSchema;

	impl ModelFormSchema for OptionalContactSchema {
		type Model = ();

		fn fields() -> &'static [ModelFormFieldDescriptor] {
			const FIELDS: [ModelFormFieldDescriptor; 2] = [
				ModelFormFieldDescriptor {
					name: "email",
					kind: ModelFormFieldKind::Email {
						min_length: None,
						max_length: None,
					},
					required: false,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
				},
				ModelFormFieldDescriptor {
					name: "website",
					kind: ModelFormFieldKind::Url {
						min_length: None,
						max_length: None,
					},
					required: false,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
				},
			];
			&FIELDS
		}
	}

	#[test]
	fn optional_email_and_url_accept_empty_controls() {
		let mut state = ModelFormState::<OptionalContactSchema, AllEditableModelFields>::new();

		for field in ["email", "website"] {
			state
				.set_value(field, serde_json::Value::String(String::new()))
				.expect("an empty optional contact control should be accepted");
			assert_eq!(
				state.value(field),
				Some(&serde_json::Value::String(String::new()))
			);
		}
	}

	#[test]
	fn date_validation_rejects_years_outside_html_date_range() {
		assert!(!is_date("0000-01-01"));
		assert!(!is_date("0999-12-31"));
		assert!(is_date("1000-01-01"));
		assert!(is_date("9999-12-31"));
	}

	struct FileSchema;

	impl ModelFormSchema for FileSchema {
		type Model = ();

		fn fields() -> &'static [ModelFormFieldDescriptor] {
			const FIELDS: [ModelFormFieldDescriptor; 2] = [
				ModelFormFieldDescriptor {
					name: "title",
					kind: ModelFormFieldKind::Text {
						min_length: None,
						max_length: None,
						multiline: false,
					},
					required: true,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
				},
				ModelFormFieldDescriptor {
					name: "document",
					kind: ModelFormFieldKind::File,
					required: true,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
				},
			];
			&FIELDS
		}
	}

	#[derive(Debug, Default)]
	struct FilePayload {
		title: Option<String>,
	}

	impl ModelFormPayload<AllEditableModelFields> for FilePayload {
		fn supplied_fields(&self) -> Vec<&'static str> {
			self.title.as_ref().map_or_else(Vec::new, |_| vec!["title"])
		}

		fn forbidden_fields(&self) -> &[&'static str] {
			&[]
		}

		fn get_json(&self, field: &str) -> Option<serde_json::Value> {
			(field == "title")
				.then(|| self.title.clone().map(serde_json::Value::String))
				.flatten()
		}

		fn set_json(
			&mut self,
			field: &str,
			value: serde_json::Value,
		) -> Result<(), ModelFormPayloadError> {
			match field {
				"title" => {
					self.title = Some(serde_json::from_value(value).expect("title JSON string"));
					Ok(())
				}
				_ => Err(ModelFormPayloadError::UnknownField {
					field: field.to_owned(),
				}),
			}
		}
	}

	#[test]
	fn file_fields_reject_scalar_json_and_stay_out_of_payloads() {
		let mut state = ModelFormState::<FileSchema, AllEditableModelFields>::new();

		let error = state
			.set_value("document", serde_json::json!("document.pdf"))
			.expect_err("file fields must reject scalar JSON values");
		assert_eq!(
			error,
			ModelFormPayloadError::InvalidValue {
				field: "document".to_owned(),
				message: "file fields must be set with set_file".to_owned(),
			}
		);
		state
			.set_value("title", serde_json::json!("Document"))
			.expect("scalar fields should keep their existing payload behavior");

		let error = state
			.build_json_payload_for::<FilePayload, AllEditableModelFields>()
			.expect_err("JSON model-form dispatch must reject file fields");
		assert_eq!(
			error,
			ModelFormPayloadError::InvalidValue {
				field: "document".to_owned(),
				message: "file fields require a typed multipart server function".to_owned(),
			}
		);

		let payload = state
			.build_payload::<FilePayload>()
			.expect("file state must not enter the JSON payload");
		assert_eq!(payload.supplied_fields(), ["title"]);
	}
}
