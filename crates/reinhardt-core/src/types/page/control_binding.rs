//! Type-erased descriptors for controlled form elements.

use std::fmt;
use std::num::IntErrorKind;

use crate::reactive::{Signal, runtime::NodeId};

use super::is_boolean_attr_truthy;

/// Identifies the form control represented by a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
	/// A string-valued input or multi-line text control.
	Text,
	/// A numeric input control.
	Number,
	/// A checkbox control.
	Checkbox,
	/// A radio control.
	Radio,
	/// A single-selection control.
	SelectOne,
	/// A multiple-selection control.
	SelectMany,
}

impl fmt::Display for ControlKind {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Text => f.write_str("text"),
			Self::Number => f.write_str("number"),
			Self::Checkbox => f.write_str("checkbox"),
			Self::Radio => f.write_str("radio"),
			Self::SelectOne => f.write_str("select-one"),
			Self::SelectMany => f.write_str("select-many"),
		}
	}
}

/// Returns whether a reactive attribute update preserves a controlled element's kind.
#[doc(hidden)]
pub fn controlled_attribute_update_is_supported(
	tag: &str,
	kind: ControlKind,
	name: &str,
	value: Option<&str>,
) -> bool {
	if tag.eq_ignore_ascii_case("input") && name.eq_ignore_ascii_case("type") {
		return match kind {
			ControlKind::Text => is_effective_text_input_type(value),
			ControlKind::Number => value.is_some_and(is_number_input_type),
			ControlKind::Checkbox => {
				value.is_some_and(|value| value.eq_ignore_ascii_case("checkbox"))
			}
			ControlKind::Radio => value.is_some_and(|value| value.eq_ignore_ascii_case("radio")),
			ControlKind::SelectOne | ControlKind::SelectMany => true,
		};
	}
	if tag.eq_ignore_ascii_case("select") && name.eq_ignore_ascii_case("multiple") {
		let multiple = value.is_some_and(is_boolean_attr_truthy);
		return match kind {
			ControlKind::SelectOne => !multiple,
			ControlKind::SelectMany => multiple,
			ControlKind::Text
			| ControlKind::Number
			| ControlKind::Checkbox
			| ControlKind::Radio => true,
		};
	}
	true
}

fn is_effective_text_input_type(input_type: Option<&str>) -> bool {
	let Some(input_type) = input_type else {
		return true;
	};
	is_text_input_type(input_type)
		|| [
			"button",
			"checkbox",
			"date",
			"datetime-local",
			"file",
			"hidden",
			"image",
			"month",
			"number",
			"radio",
			"range",
			"reset",
			"submit",
			"time",
			"week",
		]
		.iter()
		.all(|known| !input_type.eq_ignore_ascii_case(known))
}

fn is_text_input_type(input_type: &str) -> bool {
	["text", "search", "tel", "url", "email", "password", "color"]
		.iter()
		.any(|known| input_type.eq_ignore_ascii_case(known))
}

fn is_number_input_type(input_type: &str) -> bool {
	["number", "range"]
		.iter()
		.any(|known| input_type.eq_ignore_ascii_case(known))
}

/// Cross-target value read from or written to a form control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlValue {
	/// A string-valued control value.
	Text(String),
	/// A checked-state control value.
	Checked(bool),
	/// Values selected by a multiple-selection control.
	SelectedValues(Vec<String>),
}

impl ControlValue {
	fn kind_name(&self) -> &'static str {
		match self {
			Self::Text(_) => "text",
			Self::Checked(_) => "checked",
			Self::SelectedValues(_) => "selected-values",
		}
	}
}

/// Stable classification for numeric input parsing failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberParseErrorKind {
	/// The input is empty.
	Empty,
	/// The input is a valid prefix but not a complete number.
	Incomplete,
	/// The input is not a valid numeric lexeme.
	Invalid,
	/// The input overflows or a nonzero value underflows the target primitive.
	OutOfRange,
}

/// A numeric input parsing failure that retains the submitted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberParseError {
	raw: String,
	kind: NumberParseErrorKind,
}

impl NumberParseError {
	fn new(raw: &str, kind: NumberParseErrorKind) -> Self {
		Self {
			raw: raw.to_owned(),
			kind,
		}
	}

	/// Returns the unmodified input text.
	pub fn raw(&self) -> &str {
		&self.raw
	}

	/// Returns the stable failure classification.
	pub fn kind(&self) -> NumberParseErrorKind {
		self.kind
	}
}

impl fmt::Display for NumberParseError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"cannot parse numeric control value {:?}: {:?}",
			self.raw, self.kind
		)
	}
}

impl std::error::Error for NumberParseError {}

/// Result of applying a control value to its bound signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlWriteOutcome {
	/// The signal was updated.
	Committed,
	/// A numeric value was rejected without changing the numeric signal.
	Rejected(NumberParseError),
	/// The input did not require a signal update.
	Ignored,
}

/// Framework-level failure while reading or writing a bound form control.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ControlBindingError {
	/// The normalized value does not match the binding's control kind.
	ValueKindMismatch {
		/// The binding's control kind.
		control: ControlKind,
		/// The normalized value kind that was supplied.
		actual: &'static str,
	},
	/// The binding cannot be attached to the supplied element.
	UnsupportedElement {
		/// The binding's control kind.
		control: ControlKind,
		/// The element tag that was supplied.
		actual_tag: String,
	},
	/// A required DOM property is unavailable.
	MissingProperty {
		/// The binding's control kind.
		control: ControlKind,
		/// The missing property name.
		property: &'static str,
	},
	/// A browser-normalized numeric value cannot be represented by the binding.
	RejectedValue {
		/// The binding's control kind.
		control: ControlKind,
		/// The numeric value rejected by the binding.
		error: NumberParseError,
	},
}

impl fmt::Display for ControlBindingError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::ValueKindMismatch { control, actual } => {
				write!(f, "{control} control cannot accept a {actual} value")
			}
			Self::UnsupportedElement {
				control,
				actual_tag,
			} => write!(
				f,
				"{control} control does not support a <{actual_tag}> element"
			),
			Self::MissingProperty { control, property } => {
				write!(f, "{control} control is missing the {property} property")
			}
			Self::RejectedValue { control, error } => {
				write!(
					f,
					"{control} control rejected its browser-normalized value: {error}"
				)
			}
		}
	}
}

impl std::error::Error for ControlBindingError {}

type Shared<T> = std::rc::Rc<T>;

type ReadValue = Shared<dyn Fn() -> ControlValue + 'static>;
type WriteValue =
	Shared<dyn Fn(ControlValue) -> Result<ControlWriteOutcome, ControlBindingError> + 'static>;
type SnapshotValue = Shared<dyn Fn() -> ControlBindingSnapshot + 'static>;

/// Restores the signals mutated by a control binding unless the snapshot is committed.
#[doc(hidden)]
pub struct ControlBindingSnapshot {
	restore: Option<Box<dyn FnOnce() + 'static>>,
}

impl ControlBindingSnapshot {
	/// Keeps signal changes made after this snapshot was captured.
	pub fn commit(mut self) {
		self.restore = None;
	}
}

impl Drop for ControlBindingSnapshot {
	fn drop(&mut self) {
		if let Some(restore) = self.restore.take() {
			restore();
		}
	}
}

/// Cloneable type-erased reader and writer for a controlled form element.
#[derive(Clone)]
pub struct ControlBinding {
	kind: ControlKind,
	radio_value: Option<String>,
	target: NodeId,
	read: ReadValue,
	write: WriteValue,
	snapshot: SnapshotValue,
}

impl fmt::Debug for ControlBinding {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ControlBinding")
			.field("kind", &self.kind)
			.field("radio_value", &self.radio_value)
			.finish_non_exhaustive()
	}
}

impl ControlBinding {
	/// Creates a binding for a string-valued signal.
	pub fn text(signal: Signal<String>) -> Self {
		Self::string_value(ControlKind::Text, signal)
	}

	/// Creates a binding for a checkbox signal.
	pub fn checkbox(signal: Signal<bool>) -> Self {
		let read_signal = signal;
		let snapshot = signal_snapshot(signal);
		Self {
			kind: ControlKind::Checkbox,
			radio_value: None,
			target: signal.id(),
			read: Shared::new(move || ControlValue::Checked(read_signal.get())),
			write: Shared::new(move |value| match value {
				ControlValue::Checked(value) => {
					signal.set(value);
					Ok(ControlWriteOutcome::Committed)
				}
				actual => Err(value_kind_mismatch(ControlKind::Checkbox, &actual)),
			}),
			snapshot,
		}
	}

	/// Creates a binding for one radio choice within a string-valued group.
	pub fn radio(signal: Signal<String>, value: String) -> Self {
		let read_signal = signal;
		let snapshot = signal_snapshot(signal);
		let read_value = value.clone();
		let write_value = value.clone();
		Self {
			kind: ControlKind::Radio,
			radio_value: Some(value),
			target: signal.id(),
			read: Shared::new(move || ControlValue::Checked(read_signal.get() == read_value)),
			write: Shared::new(move |value| match value {
				ControlValue::Checked(true) => {
					signal.set(write_value.clone());
					Ok(ControlWriteOutcome::Committed)
				}
				ControlValue::Checked(false) => Ok(ControlWriteOutcome::Ignored),
				actual => Err(value_kind_mismatch(ControlKind::Radio, &actual)),
			}),
			snapshot,
		}
	}

	/// Creates a binding for a single-selection signal.
	pub fn select_one(signal: Signal<String>) -> Self {
		Self::string_value(ControlKind::SelectOne, signal)
	}

	/// Creates a binding for a multiple-selection signal.
	pub fn select_many(signal: Signal<Vec<String>>) -> Self {
		let read_signal = signal;
		let snapshot = signal_snapshot(signal);
		Self {
			kind: ControlKind::SelectMany,
			radio_value: None,
			target: signal.id(),
			read: Shared::new(move || ControlValue::SelectedValues(read_signal.get())),
			write: Shared::new(move |value| match value {
				ControlValue::SelectedValues(values) => {
					signal.set(values);
					Ok(ControlWriteOutcome::Committed)
				}
				actual => Err(value_kind_mismatch(ControlKind::SelectMany, &actual)),
			}),
			snapshot,
		}
	}

	/// Creates a numeric binding without an application-visible error signal.
	pub fn number<T: NumberValue>(signal: Signal<T>) -> Self {
		Self::number_binding(signal, None)
	}

	/// Creates a numeric binding that reports rejected input through a signal.
	pub fn number_with_error<T: NumberValue>(
		signal: Signal<T>,
		error: Signal<Option<NumberParseError>>,
	) -> Self {
		Self::number_binding(signal, Some(error))
	}

	/// Returns the binding's control kind.
	pub fn kind(&self) -> ControlKind {
		self.kind
	}

	/// Returns the configured radio choice, if this is a radio binding.
	pub fn radio_value(&self) -> Option<&str> {
		self.radio_value.as_deref()
	}

	/// Returns the signal receiving values from this binding.
	#[doc(hidden)]
	pub fn target(&self) -> NodeId {
		self.target
	}

	/// Reads the current signal value in its cross-target representation.
	pub fn read(&self) -> ControlValue {
		(self.read)()
	}

	/// Applies a cross-target value to the bound signal.
	pub fn write(&self, value: ControlValue) -> Result<ControlWriteOutcome, ControlBindingError> {
		(self.write)(value)
	}

	/// Captures the complete signal state that this binding may mutate.
	#[doc(hidden)]
	pub fn snapshot(&self) -> ControlBindingSnapshot {
		(self.snapshot)()
	}

	fn string_value(kind: ControlKind, signal: Signal<String>) -> Self {
		let read_signal = signal;
		let snapshot = signal_snapshot(signal);
		Self {
			kind,
			radio_value: None,
			target: signal.id(),
			read: Shared::new(move || ControlValue::Text(read_signal.get())),
			write: Shared::new(move |value| match value {
				ControlValue::Text(value) => {
					signal.set(value);
					Ok(ControlWriteOutcome::Committed)
				}
				actual => Err(value_kind_mismatch(kind, &actual)),
			}),
			snapshot,
		}
	}

	fn number_binding<T: NumberValue>(
		signal: Signal<T>,
		error: Option<Signal<Option<NumberParseError>>>,
	) -> Self {
		let read_signal = signal;
		let snapshot_signal = signal;
		let snapshot_error = error;
		Self {
			kind: ControlKind::Number,
			radio_value: None,
			target: signal.id(),
			read: Shared::new(move || ControlValue::Text(read_signal.get().format_control_value())),
			write: Shared::new(move |value| {
				let ControlValue::Text(raw) = value else {
					return Err(value_kind_mismatch(ControlKind::Number, &value));
				};

				match T::parse_control_value(&raw) {
					Ok(value) => {
						signal.set_without_notify(value);
						if let Some(error) = &error {
							error.set_without_notify(None);
							crate::reactive::runtime::with_runtime(|runtime| {
								runtime.notify_signal_changes(&[signal.id(), error.id()]);
							});
						} else {
							signal.notify_subscribers();
						}
						Ok(ControlWriteOutcome::Committed)
					}
					Err(parse_error) => {
						if let Some(error) = &error {
							error.set(Some(parse_error.clone()));
						}
						Ok(ControlWriteOutcome::Rejected(parse_error))
					}
				}
			}),
			snapshot: Shared::new(move || {
				let value = snapshot_signal.get();
				let parse_error = snapshot_error.as_ref().and_then(|error| error.get());
				let restore_signal = snapshot_signal;
				let restore_error = snapshot_error;
				ControlBindingSnapshot {
					restore: Some(Box::new(move || {
						restore_signal.set_without_notify(value);
						if let Some(error) = &restore_error {
							error.set_without_notify(parse_error);
						}
						if let Some(error) = restore_error {
							crate::reactive::runtime::with_runtime(|runtime| {
								runtime.notify_signal_changes(&[restore_signal.id(), error.id()]);
							});
						} else {
							restore_signal.notify_subscribers();
						}
					})),
				}
			}),
		}
	}
}

fn signal_snapshot<T: Clone + 'static>(signal: Signal<T>) -> SnapshotValue {
	Shared::new(move || {
		let value = signal.get();
		let restore_signal = signal;
		ControlBindingSnapshot {
			restore: Some(Box::new(move || restore_signal.set(value))),
		}
	})
}

fn value_kind_mismatch(control: ControlKind, actual: &ControlValue) -> ControlBindingError {
	ControlBindingError::ValueKindMismatch {
		control,
		actual: actual.kind_name(),
	}
}

mod sealed {
	pub trait Sealed {}
}

/// Primitive numeric type supported by controlled numeric inputs.
pub trait NumberValue: sealed::Sealed + Clone + fmt::Display + 'static {
	/// Formats the primitive for a numeric control.
	fn format_control_value(&self) -> String {
		self.to_string()
	}

	/// Parses a complete control value into this primitive.
	fn parse_control_value(raw: &str) -> Result<Self, NumberParseError>;
}

fn lexical_error(raw: &str) -> Option<NumberParseError> {
	match classify_number_lexeme(raw) {
		NumberLexemeState::Empty => Some(NumberParseError::new(raw, NumberParseErrorKind::Empty)),
		NumberLexemeState::Incomplete => {
			Some(NumberParseError::new(raw, NumberParseErrorKind::Incomplete))
		}
		NumberLexemeState::Invalid => {
			Some(NumberParseError::new(raw, NumberParseErrorKind::Invalid))
		}
		NumberLexemeState::Complete => None,
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NumberLexemeState {
	Empty,
	Incomplete,
	Complete,
	Invalid,
}

fn classify_number_lexeme(raw: &str) -> NumberLexemeState {
	if raw.is_empty() {
		return NumberLexemeState::Empty;
	}

	let bytes = raw.as_bytes();
	let mut index = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
	if index == bytes.len() {
		return NumberLexemeState::Incomplete;
	}

	let integer_start = index;
	while bytes.get(index).is_some_and(u8::is_ascii_digit) {
		index += 1;
	}
	let integer_digits = index - integer_start;
	let mut fraction_digits = 0;
	if bytes.get(index) == Some(&b'.') {
		index += 1;
		let fraction_start = index;
		while bytes.get(index).is_some_and(u8::is_ascii_digit) {
			index += 1;
		}
		fraction_digits = index - fraction_start;
		if fraction_digits == 0 {
			return if index == bytes.len() {
				NumberLexemeState::Incomplete
			} else {
				NumberLexemeState::Invalid
			};
		}
	}
	if integer_digits == 0 && fraction_digits == 0 {
		return NumberLexemeState::Invalid;
	}

	if matches!(bytes.get(index), Some(b'e' | b'E')) {
		index += 1;
		if matches!(bytes.get(index), Some(b'+' | b'-')) {
			index += 1;
		}
		let exponent_start = index;
		while bytes.get(index).is_some_and(u8::is_ascii_digit) {
			index += 1;
		}
		if exponent_start == index {
			return if index == bytes.len() {
				NumberLexemeState::Incomplete
			} else {
				NumberLexemeState::Invalid
			};
		}
	}

	if index == bytes.len() {
		NumberLexemeState::Complete
	} else {
		NumberLexemeState::Invalid
	}
}

fn is_valid_unsigned_number_lexeme(raw: &str) -> bool {
	!raw.starts_with(['+', '-']) && classify_number_lexeme(raw) == NumberLexemeState::Complete
}

fn significand_has_nonzero_digit(raw: &str) -> bool {
	raw.trim_start_matches(['+', '-'])
		.split(['e', 'E'])
		.next()
		.unwrap_or_default()
		.bytes()
		.any(|digit| digit.is_ascii_digit() && digit != b'0')
}

const MAX_INTEGER_CONTROL_DIGITS: usize = 39;

fn normalize_integer_control_value(raw: &str) -> Result<String, NumberParseErrorKind> {
	let bytes = raw.as_bytes();
	let (negative, significand_start) = match bytes.first() {
		Some(b'-') => (true, 1),
		Some(b'+') => (false, 1),
		_ => (false, 0),
	};
	let exponent_start = bytes.iter().position(|byte| matches!(byte, b'e' | b'E'));
	let significand_end = exponent_start.unwrap_or(bytes.len());
	let mut digits = Vec::with_capacity(significand_end - significand_start);
	let mut fraction_digits = 0_usize;
	let mut after_decimal = false;
	for byte in &bytes[significand_start..significand_end] {
		if *byte == b'.' {
			after_decimal = true;
			continue;
		}
		if after_decimal {
			fraction_digits += 1;
		}
		digits.push(*byte);
	}

	let Some(first_nonzero) = digits.iter().position(|digit| *digit != b'0') else {
		return Ok("0".to_owned());
	};
	let exponent = parse_integer_control_exponent(bytes, exponent_start)?;
	let fraction_digits = i64::try_from(fraction_digits).map_err(|_| {
		if exponent.is_negative() {
			NumberParseErrorKind::Invalid
		} else {
			NumberParseErrorKind::OutOfRange
		}
	})?;
	let scale = exponent.checked_sub(fraction_digits).ok_or_else(|| {
		if exponent.is_negative() {
			NumberParseErrorKind::Invalid
		} else {
			NumberParseErrorKind::OutOfRange
		}
	})?;

	if scale.is_negative() {
		let removed_digits =
			usize::try_from(scale.unsigned_abs()).map_err(|_| NumberParseErrorKind::Invalid)?;
		let end = digits
			.len()
			.checked_sub(removed_digits)
			.ok_or(NumberParseErrorKind::Invalid)?;
		if digits[end..].iter().any(|digit| *digit != b'0') {
			return Err(NumberParseErrorKind::Invalid);
		}
		let Some(output_start) = digits[..end].iter().position(|digit| *digit != b'0') else {
			return Ok("0".to_owned());
		};
		return normalized_integer_digits(negative, &digits[output_start..end], 0);
	}

	let appended_zeros = usize::try_from(scale).map_err(|_| NumberParseErrorKind::OutOfRange)?;
	normalized_integer_digits(negative, &digits[first_nonzero..], appended_zeros)
}

fn parse_integer_control_exponent(
	bytes: &[u8],
	exponent_start: Option<usize>,
) -> Result<i64, NumberParseErrorKind> {
	let Some(exponent_start) = exponent_start else {
		return Ok(0);
	};
	let mut index = exponent_start + 1;
	let negative = bytes.get(index) == Some(&b'-');
	if matches!(bytes.get(index), Some(b'+' | b'-')) {
		index += 1;
	}
	let mut magnitude = 0_u64;
	for digit in &bytes[index..] {
		magnitude = magnitude
			.checked_mul(10)
			.and_then(|value| value.checked_add(u64::from(*digit - b'0')))
			.ok_or(if negative {
				NumberParseErrorKind::Invalid
			} else {
				NumberParseErrorKind::OutOfRange
			})?;
	}
	if negative {
		let minimum_magnitude = i64::MAX as u64 + 1;
		if magnitude > minimum_magnitude {
			return Err(NumberParseErrorKind::Invalid);
		}
		if magnitude == minimum_magnitude {
			Ok(i64::MIN)
		} else {
			Ok(-(magnitude as i64))
		}
	} else if magnitude > i64::MAX as u64 {
		Err(NumberParseErrorKind::OutOfRange)
	} else {
		Ok(magnitude as i64)
	}
}

fn normalized_integer_digits(
	negative: bool,
	digits: &[u8],
	appended_zeros: usize,
) -> Result<String, NumberParseErrorKind> {
	let output_len = digits
		.len()
		.checked_add(appended_zeros)
		.filter(|length| *length <= MAX_INTEGER_CONTROL_DIGITS)
		.ok_or(NumberParseErrorKind::OutOfRange)?;
	let mut normalized = String::with_capacity(output_len + usize::from(negative));
	if negative {
		normalized.push('-');
	}
	for digit in digits {
		normalized.push(char::from(*digit));
	}
	for _ in 0..appended_zeros {
		normalized.push('0');
	}
	Ok(normalized)
}

macro_rules! impl_signed_number_value {
	($($type:ty),+ $(,)?) => {
		$(
			impl sealed::Sealed for $type {}

			impl NumberValue for $type {
				fn parse_control_value(raw: &str) -> Result<Self, NumberParseError> {
					if let Some(error) = lexical_error(raw) {
						return Err(error);
					}
					let parse_error = |error: &std::num::ParseIntError| {
						let kind = match error.kind() {
							IntErrorKind::PosOverflow | IntErrorKind::NegOverflow => {
								NumberParseErrorKind::OutOfRange
							}
							_ => NumberParseErrorKind::Invalid,
						};
						NumberParseError::new(raw, kind)
					};
					raw.parse::<Self>().or_else(|_| {
						normalize_integer_control_value(raw)
							.map_err(|kind| NumberParseError::new(raw, kind))?
							.parse::<Self>()
							.map_err(|error| parse_error(&error))
					})
				}
			}
		)+
	};
}

macro_rules! impl_unsigned_number_value {
	($($type:ty),+ $(,)?) => {
		$(
			impl sealed::Sealed for $type {}

			impl NumberValue for $type {
				fn parse_control_value(raw: &str) -> Result<Self, NumberParseError> {
					if let Some(error) = lexical_error(raw) {
						return Err(error);
					}
					if raw
						.strip_prefix('-')
						.is_some_and(is_valid_unsigned_number_lexeme)
						&& significand_has_nonzero_digit(raw)
					{
						return Err(NumberParseError::new(raw, NumberParseErrorKind::OutOfRange));
					}
					let parse_error = |error: &std::num::ParseIntError| {
						let kind = match error.kind() {
							IntErrorKind::PosOverflow | IntErrorKind::NegOverflow => {
								NumberParseErrorKind::OutOfRange
							}
							_ => NumberParseErrorKind::Invalid,
						};
						NumberParseError::new(raw, kind)
					};
					raw.parse::<Self>().or_else(|_| {
						normalize_integer_control_value(raw)
							.map_err(|kind| NumberParseError::new(raw, kind))?
							.parse::<Self>()
							.map_err(|error| parse_error(&error))
					})
				}
			}
		)+
	};
}

macro_rules! impl_float_number_value {
	($($type:ty),+ $(,)?) => {
		$(
			impl sealed::Sealed for $type {}

			impl NumberValue for $type {
				fn format_control_value(&self) -> String {
					if self.is_finite() {
						self.to_string()
					} else {
						String::new()
					}
				}

				fn parse_control_value(raw: &str) -> Result<Self, NumberParseError> {
					if let Some(error) = lexical_error(raw) {
						return Err(error);
					}
					let value = raw
						.parse::<Self>()
						.map_err(|_| NumberParseError::new(raw, NumberParseErrorKind::Invalid))?;
					if !value.is_finite() {
						return Err(NumberParseError::new(raw, NumberParseErrorKind::OutOfRange));
					}
					if value == 0.0 && significand_has_nonzero_digit(raw) {
						return Err(NumberParseError::new(raw, NumberParseErrorKind::OutOfRange));
					}
					Ok(value)
				}
			}
		)+
	};
}

impl_signed_number_value!(i8, i16, i32, i64, i128, isize);
impl_unsigned_number_value!(u8, u16, u32, u64, u128, usize);
impl_float_number_value!(f32, f64);

#[cfg(test)]
mod tests {
	use super::*;
	use crate::reactive::{Effect, EffectTiming, ReactiveScope, Signal};
	use rstest::rstest;
	use std::cell::RefCell;
	use std::rc::Rc;

	#[rstest]
	#[case("")]
	#[case("-")]
	#[case("1.")]
	#[case("1e-")]
	#[case("abc")]
	fn number_binding_preserves_last_value_on_invalid_input(#[case] raw: &str) {
		ReactiveScope::run(|| {
			// Arrange
			let value = Signal::new(7.5_f64);
			let error = Signal::new(None);
			let binding = ControlBinding::number_with_error(value.clone(), error.clone());

			// Act
			let outcome = binding.write(ControlValue::Text(raw.to_owned())).unwrap();

			// Assert
			assert_eq!(value.get(), 7.5);
			assert_eq!(outcome, ControlWriteOutcome::Rejected(error.get().unwrap()));
			assert_eq!(
				error.get().unwrap().kind(),
				match raw {
					"" => NumberParseErrorKind::Empty,
					"-" | "1." | "1e-" => NumberParseErrorKind::Incomplete,
					"abc" => NumberParseErrorKind::Invalid,
					_ => unreachable!("unexpected invalid numeric input case"),
				}
			);
		});
	}

	#[rstest]
	fn number_binding_notifies_shared_consumers_once_when_clearing_error() {
		ReactiveScope::run(|| {
			// Arrange
			let value = Signal::new(7_i32);
			let original_error = NumberParseError::new("invalid", NumberParseErrorKind::Invalid);
			let error = Signal::new(Some(original_error));
			let binding = ControlBinding::number_with_error(value.clone(), error.clone());
			let observations = Rc::new(RefCell::new(Vec::new()));
			let observed_value = value.clone();
			let observed_error = error.clone();
			let observations_for_effect = Rc::clone(&observations);
			let _effect = Effect::new_with_timing(
				move || {
					observations_for_effect
						.borrow_mut()
						.push((observed_value.get(), observed_error.get()));
				},
				EffectTiming::Layout,
			);
			observations.borrow_mut().clear();

			// Act
			binding.write(ControlValue::Text("8".to_owned())).unwrap();

			// Assert
			assert_eq!(observations.borrow().as_slice(), &[(8, None)]);
		});
	}

	#[rstest]
	fn text_binding_reads_and_writes_the_signal() {
		ReactiveScope::run(|| {
			// Arrange
			let signal = Signal::new("old".to_owned());
			let binding = ControlBinding::text(signal.clone());

			// Act
			binding.write(ControlValue::Text("new".to_owned())).unwrap();

			// Assert
			assert_eq!(binding.read(), ControlValue::Text("new".to_owned()));
			assert_eq!(signal.get(), "new");
		});
	}

	#[rstest]
	fn integer_number_binding_accepts_complete_exponent_lexemes() {
		ReactiveScope::run(|| {
			// Arrange
			let value = Signal::new(7_i32);
			let binding = ControlBinding::number(value.clone());

			// Act
			let outcome = binding.write(ControlValue::Text("1e2".to_owned())).unwrap();

			// Assert
			assert_eq!(outcome, ControlWriteOutcome::Committed);
			assert_eq!(value.get(), 100);
		});
	}

	#[rstest]
	fn integer_number_binding_preserves_large_exponent_values_exactly() {
		ReactiveScope::run(|| {
			// Arrange
			let signed = Signal::new(0_i64);
			let unsigned = Signal::new(0_u64);
			let signed_binding = ControlBinding::number(signed.clone());
			let unsigned_binding = ControlBinding::number(unsigned.clone());

			// Act
			signed_binding
				.write(ControlValue::Text("9007199254740993e0".to_owned()))
				.unwrap();
			unsigned_binding
				.write(ControlValue::Text("9007199254740993e0".to_owned()))
				.unwrap();

			// Assert
			assert_eq!(signed.get(), 9_007_199_254_740_993_i64);
			assert_eq!(unsigned.get(), 9_007_199_254_740_993_u64);
		});
	}

	#[rstest]
	#[case("-0")]
	#[case("-0.0")]
	#[case("-0e2")]
	fn unsigned_number_binding_accepts_negative_zero(#[case] raw: &str) {
		ReactiveScope::run(|| {
			// Arrange
			let value = Signal::new(7_u32);
			let binding = ControlBinding::number(value.clone());

			// Act
			let outcome = binding.write(ControlValue::Text(raw.to_owned())).unwrap();

			// Assert
			assert_eq!(outcome, ControlWriteOutcome::Committed);
			assert_eq!(value.get(), 0);
		});
	}

	#[rstest]
	fn float_number_binding_reads_nonfinite_signal_values_as_empty_controls() {
		ReactiveScope::run(|| {
			// Arrange
			let nan = ControlBinding::number(Signal::new(f32::NAN));
			let infinity = ControlBinding::number(Signal::new(f64::INFINITY));

			// Act
			let nan_value = nan.read();
			let infinity_value = infinity.read();

			// Assert
			assert_eq!(nan_value, ControlValue::Text(String::new()));
			assert_eq!(infinity_value, ControlValue::Text(String::new()));
		});
	}

	#[rstest]
	fn binding_snapshot_restores_numeric_value_and_error_state() {
		ReactiveScope::run(|| {
			// Arrange
			let value = Signal::new(7_i32);
			let original_error = NumberParseError::new("pending", NumberParseErrorKind::Invalid);
			let error = Signal::new(Some(original_error.clone()));
			let binding = ControlBinding::number_with_error(value.clone(), error.clone());
			let snapshot = binding.snapshot();

			// Act
			binding.write(ControlValue::Text("12".to_owned())).unwrap();
			drop(snapshot);

			// Assert
			assert_eq!(value.get(), 7);
			assert_eq!(error.get(), Some(original_error));
		});
	}

	#[rstest]
	fn binding_snapshot_restores_an_empty_numeric_error_state() {
		ReactiveScope::run(|| {
			// Arrange
			let value = Signal::new(7_i32);
			let error = Signal::new(None);
			let binding = ControlBinding::number_with_error(value.clone(), error.clone());
			let snapshot = binding.snapshot();

			// Act
			binding
				.write(ControlValue::Text("invalid".to_owned()))
				.unwrap();
			drop(snapshot);

			// Assert
			assert_eq!(value.get(), 7);
			assert_eq!(error.get(), None);
		});
	}

	#[rstest]
	fn committed_binding_snapshot_keeps_the_new_state() {
		ReactiveScope::run(|| {
			// Arrange
			let signal = Signal::new("server".to_owned());
			let binding = ControlBinding::text(signal.clone());
			let snapshot = binding.snapshot();

			// Act
			binding
				.write(ControlValue::Text("browser".to_owned()))
				.unwrap();
			snapshot.commit();

			// Assert
			assert_eq!(signal.get(), "browser");
		});
	}

	#[rstest]
	fn numeric_snapshot_rollback_never_notifies_a_mixed_state() {
		ReactiveScope::run(|| {
			// Arrange
			let value = Signal::new(7_i32);
			let original_error = NumberParseError::new("pending", NumberParseErrorKind::Invalid);
			let error = Signal::new(Some(original_error.clone()));
			let binding = ControlBinding::number_with_error(value.clone(), error.clone());
			let snapshot = binding.snapshot();
			value.set(12);
			error.set(None);
			let observations = Rc::new(RefCell::new(Vec::new()));
			let effect_value = value.clone();
			let effect_error = error.clone();
			let effect_observations = Rc::clone(&observations);
			let _effect = Effect::new_with_timing(
				move || {
					effect_observations
						.borrow_mut()
						.push((effect_value.get(), effect_error.get()));
				},
				EffectTiming::Layout,
			);
			observations.borrow_mut().clear();

			// Act
			drop(snapshot);

			// Assert
			assert_eq!(observations.borrow().len(), 1);
			assert!(
				observations
					.borrow()
					.iter()
					.all(|pair| pair == &(7, Some(original_error.clone())))
			);
		});
	}

	#[rstest]
	fn unsigned_number_bindings_classify_valid_negative_lexemes_as_out_of_range() {
		macro_rules! assert_negative_lexemes_are_out_of_range {
			($($type:ty),+ $(,)?) => {
				$(
					for raw in ["-1.5", "-1e2"] {
						let error = <$type as NumberValue>::parse_control_value(raw).unwrap_err();
						assert_eq!(error.raw(), raw);
						assert_eq!(error.kind(), NumberParseErrorKind::OutOfRange);
					}
				)+
			};
		}

		assert_negative_lexemes_are_out_of_range!(u8, u16, u32, u64, u128, usize);
	}

	#[rstest]
	#[case("-invalid", NumberParseErrorKind::Invalid)]
	#[case("-1e2e", NumberParseErrorKind::Invalid)]
	#[case("--1", NumberParseErrorKind::Invalid)]
	#[case("-", NumberParseErrorKind::Incomplete)]
	#[case("-1e-", NumberParseErrorKind::Incomplete)]
	fn unsigned_number_bindings_preserve_invalid_and_incomplete_classification(
		#[case] raw: &str,
		#[case] expected: NumberParseErrorKind,
	) {
		let error = <u8 as NumberValue>::parse_control_value(raw).unwrap_err();

		assert_eq!(error.raw(), raw);
		assert_eq!(error.kind(), expected);
	}

	#[rstest]
	#[case("NaN")]
	#[case("nan")]
	#[case("NAN")]
	#[case("+NaN")]
	#[case("-NaN")]
	#[case("inf")]
	#[case("INF")]
	#[case("+inf")]
	#[case("-inf")]
	#[case("infinity")]
	#[case("INFINITY")]
	fn float_number_bindings_reject_non_numeric_special_lexemes(#[case] raw: &str) {
		macro_rules! assert_special_lexeme_is_invalid {
			($($type:ty),+ $(,)?) => {
				$(
					let error = <$type as NumberValue>::parse_control_value(raw).unwrap_err();
					assert_eq!(error.raw(), raw);
					assert_eq!(error.kind(), NumberParseErrorKind::Invalid);
				)+
			};
		}

		assert_special_lexeme_is_invalid!(f32, f64);
	}

	#[rstest]
	#[case("", Some(NumberParseErrorKind::Empty))]
	#[case("-", Some(NumberParseErrorKind::Incomplete))]
	#[case("+", Some(NumberParseErrorKind::Incomplete))]
	#[case(".", Some(NumberParseErrorKind::Incomplete))]
	#[case("+.", Some(NumberParseErrorKind::Incomplete))]
	#[case("1.", Some(NumberParseErrorKind::Incomplete))]
	#[case("1e", Some(NumberParseErrorKind::Incomplete))]
	#[case("1e+", Some(NumberParseErrorKind::Incomplete))]
	#[case("1e-", Some(NumberParseErrorKind::Incomplete))]
	#[case("0", None)]
	#[case("+12", None)]
	#[case(".5", None)]
	#[case("12.5", None)]
	#[case("12e3", None)]
	#[case("12.5E-3", None)]
	#[case("NaN", Some(NumberParseErrorKind::Invalid))]
	#[case("NaNe", Some(NumberParseErrorKind::Invalid))]
	#[case("inf", Some(NumberParseErrorKind::Invalid))]
	#[case("infe", Some(NumberParseErrorKind::Invalid))]
	#[case("1e2e", Some(NumberParseErrorKind::Invalid))]
	#[case("1ee", Some(NumberParseErrorKind::Invalid))]
	#[case("--1", Some(NumberParseErrorKind::Invalid))]
	#[case("1..2", Some(NumberParseErrorKind::Invalid))]
	#[case("1e+-2", Some(NumberParseErrorKind::Invalid))]
	#[case(".e", Some(NumberParseErrorKind::Invalid))]
	#[case(" 1", Some(NumberParseErrorKind::Invalid))]
	#[case("1 ", Some(NumberParseErrorKind::Invalid))]
	fn float_number_bindings_use_decimal_grammar_classifier(
		#[case] raw: &str,
		#[case] expected_error: Option<NumberParseErrorKind>,
	) {
		macro_rules! assert_classification {
			($($type:ty),+ $(,)?) => {
				$(
					let result = <$type as NumberValue>::parse_control_value(raw);
					match expected_error {
						Some(expected) => assert_eq!(result.unwrap_err().kind(), expected),
						None => assert!(result.is_ok(), "{raw} should be complete"),
					}
				)+
			};
		}

		assert_classification!(f32, f64);
	}

	#[rstest]
	fn float_number_bindings_accept_finite_limits_and_reject_numeric_overflow() {
		let f32_max = f32::MAX.to_string();
		let f64_max = f64::MAX.to_string();

		assert_eq!(
			<f32 as NumberValue>::parse_control_value(&f32_max).unwrap(),
			f32::MAX
		);
		assert_eq!(
			<f64 as NumberValue>::parse_control_value(&f64_max).unwrap(),
			f64::MAX
		);

		for raw in ["3.4028236e38", "1e9999"] {
			let error = <f32 as NumberValue>::parse_control_value(raw).unwrap_err();
			assert_eq!(error.raw(), raw);
			assert_eq!(error.kind(), NumberParseErrorKind::OutOfRange);
		}
		for raw in ["1.7976931348623159e308", "1e9999"] {
			let error = <f64 as NumberValue>::parse_control_value(raw).unwrap_err();
			assert_eq!(error.raw(), raw);
			assert_eq!(error.kind(), NumberParseErrorKind::OutOfRange);
		}
	}

	#[rstest]
	#[case("1e-46")]
	#[case("-1e-46")]
	fn f32_number_bindings_reject_nonzero_underflow(#[case] raw: &str) {
		let error = <f32 as NumberValue>::parse_control_value(raw).unwrap_err();
		assert_eq!(error.raw(), raw);
		assert_eq!(error.kind(), NumberParseErrorKind::OutOfRange);
	}

	#[rstest]
	#[case("1e-324")]
	#[case("-1e-324")]
	fn f64_number_bindings_reject_nonzero_underflow(#[case] raw: &str) {
		let error = <f64 as NumberValue>::parse_control_value(raw).unwrap_err();
		assert_eq!(error.raw(), raw);
		assert_eq!(error.kind(), NumberParseErrorKind::OutOfRange);
	}

	#[rstest]
	#[case("0")]
	#[case("+0")]
	#[case("-0")]
	#[case("0.000e-999")]
	#[case("-0e-999")]
	fn float_number_bindings_accept_mathematical_zero(#[case] raw: &str) {
		assert_eq!(<f32 as NumberValue>::parse_control_value(raw).unwrap(), 0.0);
		assert_eq!(<f64 as NumberValue>::parse_control_value(raw).unwrap(), 0.0);
	}

	#[test]
	fn float_number_bindings_accept_smallest_nonzero_subnormal_values() {
		let f32_min = f32::from_bits(1);
		let f64_min = f64::from_bits(1);
		assert_eq!(
			<f32 as NumberValue>::parse_control_value(&f32_min.to_string()).unwrap(),
			f32_min
		);
		assert_eq!(
			<f64 as NumberValue>::parse_control_value(&f64_min.to_string()).unwrap(),
			f64_min
		);
	}
}
