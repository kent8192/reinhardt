#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use reinhardt_core::model_form::{
	ModelFormCleanedPayload, ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload,
	ModelFormPayloadError, ModelFormPolicy, ModelFormSchema, ModelFormValidatingPayload,
	NativeModelFormPayload,
};
use reinhardt_core::reactive::{Effect, EffectTiming, runtime::with_runtime};
use reinhardt_pages::component::{
	ControlKind, ControlValue, ControlWriteOutcome, NumberParseErrorKind,
};
use reinhardt_pages::control_binding::__private::{
	CheckboxBinding, NumberBinding, RadioBinding, SelectOneBinding, TextBinding,
	into_control_binding,
};
use reinhardt_pages::{FormRuntimeSource, RuntimeControlBindingRequest, form, use_form};
use rstest::rstest;

struct BindingRecord;

struct BindingRecordPolicy;

impl ModelFormPolicy for BindingRecordPolicy {
	fn allows(field: &str) -> bool {
		matches!(
			field,
			"title" | "count" | "ratio" | "active" | "document" | "preview" | "metadata"
		)
	}
}

struct BindingRecordFormSchema;

const BINDING_FIELDS: [ModelFormFieldDescriptor; 7] = [
	ModelFormFieldDescriptor {
		name: "title",
		kind: ModelFormFieldKind::Text {
			min_length: None,
			max_length: Some(200),
			multiline: false,
		},
		required: false,
		has_default: true,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "count",
		kind: ModelFormFieldKind::Integer {
			min: None,
			max: None,
		},
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "ratio",
		kind: ModelFormFieldKind::Float {
			min: None,
			max: None,
		},
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "active",
		kind: ModelFormFieldKind::Boolean,
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "document",
		kind: ModelFormFieldKind::File,
		required: false,
		has_default: false,
		nullable: true,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "preview",
		kind: ModelFormFieldKind::Image,
		required: false,
		has_default: false,
		nullable: true,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "metadata",
		kind: ModelFormFieldKind::Json,
		required: false,
		has_default: false,
		nullable: true,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
];

impl ModelFormSchema for BindingRecordFormSchema {
	type Model = BindingRecord;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&BINDING_FIELDS
	}
}

impl BindingRecordFormSchema {
	const fn title() -> &'static ModelFormFieldDescriptor {
		&BINDING_FIELDS[0]
	}

	const fn count() -> &'static ModelFormFieldDescriptor {
		&BINDING_FIELDS[1]
	}

	const fn ratio() -> &'static ModelFormFieldDescriptor {
		&BINDING_FIELDS[2]
	}

	const fn active() -> &'static ModelFormFieldDescriptor {
		&BINDING_FIELDS[3]
	}

	const fn document() -> &'static ModelFormFieldDescriptor {
		&BINDING_FIELDS[4]
	}

	const fn preview() -> &'static ModelFormFieldDescriptor {
		&BINDING_FIELDS[5]
	}

	const fn metadata() -> &'static ModelFormFieldDescriptor {
		&BINDING_FIELDS[6]
	}
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound = "")]
struct BindingRecordModelFormData<P: ModelFormPolicy> {
	values: std::collections::HashMap<String, serde_json::Value>,
	_policy: PhantomData<P>,
}

impl<P: ModelFormPolicy> Default for BindingRecordModelFormData<P> {
	fn default() -> Self {
		Self {
			values: std::collections::HashMap::new(),
			_policy: PhantomData,
		}
	}
}

impl<P: ModelFormPolicy> ModelFormPayload<P> for BindingRecordModelFormData<P> {
	fn supplied_fields(&self) -> Vec<&'static str> {
		BINDING_FIELDS
			.iter()
			.filter(|field| self.values.contains_key(field.name))
			.map(|field| field.name)
			.collect()
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		self.values.get(field).cloned()
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		if !P::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}
		self.values.insert(field.to_owned(), value);
		Ok(())
	}
}

struct CleanedBindingRecordModelFormData<P: ModelFormPolicy>(BindingRecordModelFormData<P>);

impl<P: ModelFormPolicy> ModelFormCleanedPayload for CleanedBindingRecordModelFormData<P> {
	type Raw = BindingRecordModelFormData<P>;

	fn into_raw(self) -> Self::Raw {
		self.0
	}
}

impl<P: ModelFormPolicy> ModelFormValidatingPayload for BindingRecordModelFormData<P> {
	type Cleaned = CleanedBindingRecordModelFormData<P>;

	fn clean_and_validate(
		mut self,
	) -> Result<Self::Cleaned, reinhardt_core::validators::ValidationErrors> {
		reinhardt_forms::model_form::clean_generated_payload::<BindingRecordFormSchema, P, _>(
			&mut self,
		)?;
		Ok(CleanedBindingRecordModelFormData(self))
	}
}

impl<P: ModelFormPolicy> NativeModelFormPayload for BindingRecordModelFormData<P> {
	fn from_native_form_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
		let values = serde_json::from_value(value)?;
		Ok(Self {
			values,
			_policy: PhantomData,
		})
	}
}

#[reinhardt_pages::server_fn::server_fn(model_form = true)]
async fn save_binding_record(
	_payload: BindingRecordModelFormData<BindingRecordPolicy>,
) -> Result<(), reinhardt_pages::ServerFnError> {
	Ok(())
}

macro_rules! binding_form {
	() => {
		form! {
			name: BindingForm,
			model: BindingRecord,
			policy: BindingRecordPolicy,
			fields: [title, count, ratio, active, document, preview, metadata],
			server_fn: save_binding_record,
		}
	};
}

#[rstest]
fn static_model_form_bindings_read_and_write_the_dynamic_store() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let _: BindingRecordModelFormData<BindingRecordPolicy> = Default::default();
		let _ = save_binding_record;
		let _ = save_binding_record::marker;
		let form = binding_form!();
		let runtime = use_form(&form).build();
		let text = into_control_binding::<TextBinding, _>(runtime.field(form.title_field()), ());
		let radio = into_control_binding::<RadioBinding, _>(
			runtime.field(form.title_field()),
			"published".to_owned(),
		);
		let select =
			into_control_binding::<SelectOneBinding, _>(runtime.field(form.title_field()), ());
		let integer =
			into_control_binding::<NumberBinding, _>(runtime.field(form.count_field()), ());
		let float = into_control_binding::<NumberBinding, _>(runtime.field(form.ratio_field()), ());
		let checkbox =
			into_control_binding::<CheckboxBinding, _>(runtime.field(form.active_field()), ());

		// Act
		assert_eq!(
			text.write(ControlValue::Text("draft".to_owned())),
			Ok(ControlWriteOutcome::Committed)
		);
		assert_eq!(
			radio.write(ControlValue::Checked(true)),
			Ok(ControlWriteOutcome::Committed)
		);
		assert_eq!(
			select.write(ControlValue::Text("selected".to_owned())),
			Ok(ControlWriteOutcome::Committed)
		);
		assert_eq!(
			integer.write(ControlValue::Text("42".to_owned())),
			Ok(ControlWriteOutcome::Committed)
		);
		assert_eq!(
			float.write(ControlValue::Text("1.5".to_owned())),
			Ok(ControlWriteOutcome::Committed)
		);
		assert_eq!(
			checkbox.write(ControlValue::Checked(true)),
			Ok(ControlWriteOutcome::Committed)
		);

		// Assert
		assert_eq!(form.value("title"), Some(serde_json::json!("selected")));
		assert_eq!(form.value("count"), Some(serde_json::json!(42)));
		assert_eq!(form.value("ratio"), Some(serde_json::json!(1.5)));
		assert_eq!(form.value("active"), Some(serde_json::json!(true)));
		assert_eq!(select.read(), ControlValue::Text("selected".to_owned()));
		assert_eq!(radio.read(), ControlValue::Checked(false));
		assert_eq!(integer.read(), ControlValue::Text("42".to_owned()));
		assert_eq!(float.read(), ControlValue::Text("1.5".to_owned()));
		assert_eq!(checkbox.read(), ControlValue::Checked(true));
	});
}

#[rstest]
fn static_model_form_binding_reads_follow_writes_and_resets() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = binding_form!();
		let runtime = use_form(&form).build();
		let binding = into_control_binding::<TextBinding, _>(runtime.field(form.title_field()), ());
		let observations = Rc::new(RefCell::new(Vec::new()));
		let effect_binding = binding.clone();
		let effect_observations = Rc::clone(&observations);
		let _effect = Effect::new_with_timing(
			move || effect_observations.borrow_mut().push(effect_binding.read()),
			EffectTiming::Layout,
		);
		observations.borrow_mut().clear();

		// Act
		binding
			.write(ControlValue::Text("changed".to_owned()))
			.expect("text write commits");

		// Assert
		assert_eq!(
			observations.borrow().as_slice(),
			&[ControlValue::Text("changed".to_owned())]
		);

		observations.borrow_mut().clear();
		runtime.reset();
		assert_eq!(
			observations.borrow().as_slice(),
			&[ControlValue::Text(String::new())]
		);
	});
}

#[rstest]
fn static_model_form_bindings_use_stable_per_field_targets() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = binding_form!();
		let runtime = use_form(&form).build();

		// Act
		let text = into_control_binding::<TextBinding, _>(runtime.field(form.title_field()), ());
		let radio = into_control_binding::<RadioBinding, _>(
			runtime.field(form.title_field()),
			"published".to_owned(),
		);
		let number =
			into_control_binding::<NumberBinding, _>(runtime.field(form.count_field()), ());
		let cloned_form = form.clone();
		let cloned = cloned_form
			.runtime_control_binding(
				cloned_form.title_field(),
				RuntimeControlBindingRequest {
					kind: ControlKind::Text,
					radio_value: None,
				},
			)
			.expect("cloned form retains the title binding");
		let other_form = binding_form!();
		let other = other_form
			.runtime_control_binding(
				other_form.title_field(),
				RuntimeControlBindingRequest {
					kind: ControlKind::Text,
					radio_value: None,
				},
			)
			.expect("new form exposes the title binding");

		// Assert
		assert_eq!(text.target(), radio.target());
		assert_ne!(text.target(), number.target());
		assert_eq!(text.target(), cloned.target());
		assert_ne!(text.target(), other.target());
	});
}

#[rstest]
#[case(ControlKind::Text)]
#[case(ControlKind::Radio)]
#[case(ControlKind::SelectOne)]
fn string_binding_snapshot_restores_raw_optional_default_value(#[case] kind: ControlKind) {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = binding_form!();
		let binding = form
			.runtime_control_binding(
				form.title_field(),
				RuntimeControlBindingRequest {
					kind,
					radio_value: matches!(kind, ControlKind::Radio).then(String::new),
				},
			)
			.expect("string control pair is supported");
		let initial_value = match kind {
			ControlKind::Radio => ControlValue::Checked(true),
			_ => ControlValue::Text(String::new()),
		};
		binding
			.write(initial_value)
			.expect("raw empty text commits");
		let snapshot = binding.snapshot();

		// Act
		form.set_value("title", serde_json::json!("changed"))
			.expect("replacement text commits");
		drop(snapshot);

		// Assert
		assert_eq!(form.value("title"), Some(serde_json::json!("")));
		let expected_read = match kind {
			ControlKind::Radio => ControlValue::Checked(true),
			_ => ControlValue::Text(String::new()),
		};
		assert_eq!(binding.read(), expected_read);
	});
}

#[rstest]
fn numeric_binding_rejection_and_snapshot_preserve_value_and_error() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = binding_form!();
		let runtime = use_form(&form).build();
		let binding =
			into_control_binding::<NumberBinding, _>(runtime.field(form.count_field()), ());
		binding
			.write(ControlValue::Text("7".to_owned()))
			.expect("valid integer commits");

		// Act
		let rejected = binding
			.write(ControlValue::Text("1e".to_owned()))
			.expect("numeric rejection is a write outcome");

		// Assert
		let ControlWriteOutcome::Rejected(error) = rejected else {
			panic!("incomplete numeric text must be rejected");
		};
		assert_eq!(error.raw(), "1e");
		assert_eq!(error.kind(), NumberParseErrorKind::Incomplete);
		assert_eq!(form.value("count"), Some(serde_json::json!(7)));
		assert_eq!(
			runtime
				.get_field_state(form.count_field())
				.error
				.as_ref()
				.map(reinhardt_pages::FieldError::message),
			Some("cannot parse numeric control value \"1e\": Incomplete")
		);
		let validation = runtime
			.trigger()
			.expect_err("an invalid numeric editor value must block validation");
		assert_eq!(
			validation
				.field_errors()
				.get(&form.count_field())
				.map(reinhardt_pages::FieldError::message),
			Some("cannot parse numeric control value \"1e\": Incomplete")
		);

		// Clearing displayed errors must not discard the rejected editor value.
		runtime.clear_errors();
		runtime.clear_field_error(form.count_field());
		assert_eq!(
			form.runtime_custom_widget_error(form.count_field())
				.as_ref()
				.map(reinhardt_pages::FieldError::message),
			Some("cannot parse numeric control value \"1e\": Incomplete"),
		);
		assert!(runtime.trigger().is_err());

		let snapshot = binding.snapshot();
		assert_eq!(
			binding.write(ControlValue::Text("9".to_owned())),
			Ok(ControlWriteOutcome::Committed)
		);
		assert_eq!(form.value("count"), Some(serde_json::json!(9)));
		assert!(runtime.get_field_state(form.count_field()).error.is_none());

		drop(snapshot);
		assert_eq!(form.value("count"), Some(serde_json::json!(7)));
		assert_eq!(
			runtime
				.get_field_state(form.count_field())
				.error
				.as_ref()
				.map(reinhardt_pages::FieldError::message),
			Some("cannot parse numeric control value \"1e\": Incomplete")
		);

		runtime.reset();
		assert_eq!(form.value("count"), None);
		assert!(runtime.get_field_state(form.count_field()).error.is_none());
	});
}

#[rstest]
fn programmatic_numeric_updates_clear_parse_errors() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = binding_form!();
		let runtime = use_form(&form).build();
		let binding =
			into_control_binding::<NumberBinding, _>(runtime.field(form.count_field()), ());
		binding
			.write(ControlValue::Text("7".to_owned()))
			.expect("valid integer commits");
		binding
			.write(ControlValue::Text("1e".to_owned()))
			.expect("numeric rejection is a write outcome");
		assert!(runtime.get_field_state(form.count_field()).error.is_some());

		// Act
		runtime.set_value(form.count_field(), 9_i64);
		with_runtime(|runtime| runtime.flush_updates());

		// Assert
		assert_eq!(form.value("count"), Some(serde_json::json!(9)));
		assert!(
			form.runtime_custom_widget_error(form.count_field())
				.is_none(),
			"the generated numeric parse error must be cleared"
		);
		assert!(runtime.get_field_state(form.count_field()).error.is_none());

		binding
			.write(ControlValue::Text("1e".to_owned()))
			.expect("numeric rejection is a write outcome");
		let values = runtime.get_values();
		runtime.set_values(values);
		with_runtime(|runtime| runtime.flush_updates());
		assert!(runtime.get_field_state(form.count_field()).error.is_none());
	});
}

#[rstest]
fn static_model_form_bindings_reject_unsupported_control_pairs() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = binding_form!();
		let request = |kind| RuntimeControlBindingRequest {
			kind,
			radio_value: None,
		};

		// Act
		let file = form.runtime_control_binding(form.document_field(), request(ControlKind::Text));
		let image = form.runtime_control_binding(form.preview_field(), request(ControlKind::Text));
		let metadata =
			form.runtime_control_binding(form.metadata_field(), request(ControlKind::Text));
		let select_many =
			form.runtime_control_binding(form.title_field(), request(ControlKind::SelectMany));

		// Assert
		assert!(file.is_none());
		assert!(image.is_none());
		assert!(metadata.is_none());
		assert!(select_many.is_none());
	});
}

#[rstest]
fn rejected_numeric_edits_touch_pristine_generated_forms() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let regular = form! {
			name: NumericInteractionForm,
			action: "/count",
			fields: {
				count: IntegerField {}
			}
		};
		let regular_runtime = use_form(&regular).build();
		let model = binding_form!();
		let model_runtime = use_form(&model).build();
		let regular_binding = into_control_binding::<NumberBinding, _>(
			regular_runtime.field(regular.count_field()),
			(),
		);
		let model_binding =
			into_control_binding::<NumberBinding, _>(model_runtime.field(model.count_field()), ());
		assert!(!regular_runtime.form_state().is_touched.get());
		assert!(!model_runtime.form_state().is_touched.get());

		// Act
		for binding in [&regular_binding, &model_binding] {
			assert_eq!(
				binding.write(ControlValue::Text("1e".to_owned())),
				Ok(ControlWriteOutcome::Rejected(
					reinhardt_pages::NumberParseError::from_raw_kind(
						"1e",
						NumberParseErrorKind::Incomplete
					),
				)),
			);
		}

		// Assert
		assert!(
			regular_runtime
				.get_field_state(regular.count_field())
				.is_touched
		);
		assert!(
			model_runtime
				.get_field_state(model.count_field())
				.is_touched
		);
		assert!(regular_runtime.form_state().is_touched.get());
		assert!(model_runtime.form_state().is_touched.get());
		assert!(!regular_runtime.form_state().is_dirty.get());
		assert!(!model_runtime.form_state().is_dirty.get());
		regular_runtime.reset();
		model_runtime.reset();
		assert!(!regular_runtime.form_state().is_touched.get());
		assert!(!model_runtime.form_state().is_touched.get());
	});
}
