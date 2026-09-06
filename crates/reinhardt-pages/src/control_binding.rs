//! Stable support types for controlled `page!` form elements.
//!
//! The `bind:` directive accepts owned or borrowed [`Signal`](crate::reactive::Signal)
//! values directly for text, checkbox, radio, and select controls. Numeric controls
//! can additionally report rejected input through [`NumberParseError`].
//! Binding lowering passes these `Copy` signal handles by value, so generated
//! call sites remain clean under Clippy's `clone_on_copy` lint.
//!
//! # Target parity
//!
//! This is a P2 API: the same support types and binding contract are available
//! for browser DOM controls, server rendering, and native component tests.

pub use reinhardt_core::types::page::{
	ControlBindingError, NumberParseError, NumberParseErrorKind, NumberValue,
};

/// Macro-facing adapters for typed control bindings.
#[doc(hidden)]
pub mod __private {
	use super::{NumberParseError, NumberValue};
	use crate::component::ControlBinding;
	use crate::form_state::{FormRuntimeSource, RuntimeControlBindingRequest, RuntimeFieldBinding};
	use crate::reactive::Signal;
	use reinhardt_core::types::page::ControlKind;

	pub struct TextBinding;
	pub struct NumberBinding;
	pub struct CheckboxBinding;
	pub struct RadioBinding;
	pub struct SelectOneBinding;
	pub struct SelectManyBinding;

	pub trait IntoControlBinding<Kind> {
		type Config;

		fn into_control_binding(self, config: Self::Config) -> ControlBinding;
	}

	pub fn into_control_binding<Kind, Source>(
		source: Source,
		config: <Source as IntoControlBinding<Kind>>::Config,
	) -> ControlBinding
	where
		Source: IntoControlBinding<Kind>,
	{
		source.into_control_binding(config)
	}

	impl<Kind, T: 'static> IntoControlBinding<Kind> for &Signal<T>
	where
		Signal<T>: IntoControlBinding<Kind>,
	{
		type Config = <Signal<T> as IntoControlBinding<Kind>>::Config;

		fn into_control_binding(self, config: Self::Config) -> ControlBinding {
			(*self).into_control_binding(config)
		}
	}

	impl<Kind, T: 'static> IntoControlBinding<Kind> for &mut Signal<T>
	where
		Signal<T>: IntoControlBinding<Kind>,
	{
		type Config = <Signal<T> as IntoControlBinding<Kind>>::Config;

		fn into_control_binding(self, config: Self::Config) -> ControlBinding {
			(*self).into_control_binding(config)
		}
	}

	impl IntoControlBinding<TextBinding> for Signal<String> {
		type Config = ();

		fn into_control_binding(self, (): Self::Config) -> ControlBinding {
			ControlBinding::text(self)
		}
	}

	impl IntoControlBinding<RadioBinding> for Signal<String> {
		type Config = String;

		fn into_control_binding(self, config: Self::Config) -> ControlBinding {
			ControlBinding::radio(self, config)
		}
	}

	impl IntoControlBinding<SelectOneBinding> for Signal<String> {
		type Config = ();

		fn into_control_binding(self, (): Self::Config) -> ControlBinding {
			ControlBinding::select_one(self)
		}
	}

	impl<T: NumberValue> IntoControlBinding<NumberBinding> for Signal<T> {
		type Config = ();

		fn into_control_binding(self, (): Self::Config) -> ControlBinding {
			ControlBinding::number(self)
		}
	}

	impl<T: NumberValue> IntoControlBinding<NumberBinding>
		for (Signal<T>, Signal<Option<NumberParseError>>)
	{
		type Config = ();

		fn into_control_binding(self, (): Self::Config) -> ControlBinding {
			ControlBinding::number_with_error(self.0, self.1)
		}
	}

	impl IntoControlBinding<CheckboxBinding> for Signal<bool> {
		type Config = ();

		fn into_control_binding(self, (): Self::Config) -> ControlBinding {
			ControlBinding::checkbox(self)
		}
	}

	impl IntoControlBinding<SelectManyBinding> for Signal<Vec<String>> {
		type Config = ();

		fn into_control_binding(self, (): Self::Config) -> ControlBinding {
			ControlBinding::select_many(self)
		}
	}

	macro_rules! impl_runtime_binding {
		($marker:ty, $kind:expr, $config:ty, $label:literal) => {
			impl<Form, Deps> IntoControlBinding<$marker> for RuntimeFieldBinding<Form, Deps>
			where
				Form: FormRuntimeSource,
				Deps: Clone + PartialEq + 'static,
			{
				type Config = $config;

				fn into_control_binding(self, config: Self::Config) -> ControlBinding {
					let _ = config;
					self.into_control_binding(
						RuntimeControlBindingRequest {
							kind: $kind,
							radio_value: None,
						},
						$label,
					)
				}
			}
		};
	}

	macro_rules! impl_runtime_radio_binding {
		($marker:ty, $kind:expr, $label:literal) => {
			impl<Form, Deps> IntoControlBinding<$marker> for RuntimeFieldBinding<Form, Deps>
			where
				Form: FormRuntimeSource,
				Deps: Clone + PartialEq + 'static,
			{
				type Config = String;

				fn into_control_binding(self, config: Self::Config) -> ControlBinding {
					self.into_control_binding(
						RuntimeControlBindingRequest {
							kind: $kind,
							radio_value: Some(config),
						},
						$label,
					)
				}
			}
		};
	}

	impl_runtime_binding!(TextBinding, ControlKind::Text, (), "text");
	impl_runtime_binding!(NumberBinding, ControlKind::Number, (), "number");
	impl_runtime_binding!(CheckboxBinding, ControlKind::Checkbox, (), "checkbox");
	impl_runtime_radio_binding!(RadioBinding, ControlKind::Radio, "radio");
	impl_runtime_binding!(SelectOneBinding, ControlKind::SelectOne, (), "select-one");
	impl_runtime_binding!(
		SelectManyBinding,
		ControlKind::SelectMany,
		(),
		"select-many"
	);

	impl<Form, Deps> RuntimeFieldBinding<Form, Deps>
	where
		Form: FormRuntimeSource,
		Deps: Clone + PartialEq + 'static,
	{
		fn into_control_binding(
			self,
			request: RuntimeControlBindingRequest,
			label: &'static str,
		) -> ControlBinding {
			let field = self.field_token();
			self.runtime_control_binding(request)
				.unwrap_or_else(|| panic!("field {:?} cannot bind to {label} control", field))
		}
	}
}
