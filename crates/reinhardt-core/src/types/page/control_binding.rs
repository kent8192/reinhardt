//! Internal state descriptors shared by generated forms and DOM mounting.

use std::sync::Arc;

/// Identifies the browser property controlled by a generated field.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
	/// The serialized value of an input or textarea.
	Text,
	/// A checkbox's checked state.
	Checkbox,
	/// A radio's checked state.
	Radio,
	/// The selected value of a select.
	SelectOne,
	/// All selected values of a multiple select.
	SelectMany,
	/// A browser-owned file selection; only clearing is writable.
	File,
}

/// A cross-target snapshot of a generated form control.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlValue {
	/// A serialized input, textarea, or select value.
	Text(String),
	/// A checked state, or the presence of a file selection.
	Checked(bool),
	/// Values selected by a multiple select.
	SelectedValues(Vec<String>),
}

/// Type-erased state access for generated form controls.
#[doc(hidden)]
#[derive(Clone)]
pub struct ControlBinding {
	kind: ControlKind,
	read: Arc<dyn Fn() -> ControlValue>,
	write: Arc<dyn Fn(ControlValue)>,
	prefer_source: Arc<dyn Fn() -> bool>,
}

impl std::fmt::Debug for ControlBinding {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ControlBinding")
			.field("kind", &self.kind)
			.finish_non_exhaustive()
	}
}

impl ControlBinding {
	/// Constructs a binding without exposing the generated signal's value type.
	pub fn from_parts(
		kind: ControlKind,
		read: impl Fn() -> ControlValue + 'static,
		write: impl Fn(ControlValue) + 'static,
	) -> Self {
		Self {
			kind,
			read: Arc::new(read),
			write: Arc::new(write),
			prefer_source: Arc::new(|| false),
		}
	}

	/// Preserves an explicit runtime reset or replacement performed before hydration.
	pub fn prefer_source_on_hydration(mut self, prefer: impl Fn() -> bool + 'static) -> Self {
		self.prefer_source = Arc::new(prefer);
		self
	}

	/// Returns the browser control kind.
	pub fn kind(&self) -> ControlKind {
		self.kind
	}

	/// Reads the source while tracking its reactive dependencies.
	pub fn read(&self) -> ControlValue {
		(self.read)()
	}

	/// Reads the source without subscribing the caller's reactive context.
	pub fn read_untracked(&self) -> ControlValue {
		#[cfg(feature = "reactive")]
		{
			crate::reactive::runtime::run_without_observer(|| self.read())
		}
		#[cfg(not(feature = "reactive"))]
		self.read()
	}

	/// Adopts a browser value through the generated field's conversion.
	pub fn write(&self, value: ControlValue) {
		(self.write)(value);
	}

	/// Returns whether the source takes precedence over the pre-hydration DOM.
	pub fn source_preferred_on_hydration(&self) -> bool {
		(self.prefer_source)()
	}
}
