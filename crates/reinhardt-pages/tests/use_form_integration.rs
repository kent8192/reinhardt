#![cfg(not(target_arch = "wasm32"))]

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use reinhardt_core::reactive::ReactiveScope;
use reinhardt_pages::reactive::Signal;
use reinhardt_pages::server_fn::ServerFnErrorKind;
use reinhardt_pages::ui::{FormActionButton, FormActionResultPanel};
use reinhardt_pages::{
	CollectionItem, CollectionItemKey, CustomWidgetContext, CustomWidgetRawValue, FieldError,
	FormEvent, FormWidgetAdapter, FormWidgetError, FormWidgetValueKind, IntoPage, Page,
	PageElement, ResetOnDeps, RevalidateOn, ServerFnError, UseFormAsyncSubmitOutcome,
	UseFormSubmitOutcome, form, use_form, use_form_action, use_resource,
};

thread_local! {
	static LAST_CUSTOM_WIDGET_PROPS: RefCell<Option<DateRangeProps>> = const { RefCell::new(None) };
}

#[derive(Clone)]
struct DateRangeProps {
	value: String,
	disabled: bool,
	touched: bool,
	error: Option<FieldError>,
	on_raw_change: Rc<dyn Fn(CustomWidgetRawValue) -> Result<(), FormWidgetError>>,
}

fn capturing_date_range_picker(props: DateRangeProps) -> Page {
	LAST_CUSTOM_WIDGET_PROPS.with(|slot| {
		*slot.borrow_mut() = Some(props);
	});
	Page::Fragment(Vec::new())
}

fn take_last_custom_widget_props() -> DateRangeProps {
	LAST_CUSTOM_WIDGET_PROPS.with(|slot| {
		slot.borrow_mut()
			.take()
			.expect("custom widget props should be captured")
	})
}

struct DateRangeAdapter;

impl FormWidgetAdapter<String> for DateRangeAdapter {
	type ComponentProps = DateRangeProps;

	fn value_kind() -> FormWidgetValueKind {
		FormWidgetValueKind::Value
	}

	fn props(ctx: CustomWidgetContext<String>) -> Self::ComponentProps {
		DateRangeProps {
			value: ctx.value,
			disabled: ctx.disabled,
			touched: ctx.touched,
			error: ctx.error,
			on_raw_change: ctx.on_raw_change,
		}
	}

	fn parse(raw: CustomWidgetRawValue) -> Result<String, FormWidgetError> {
		match raw {
			CustomWidgetRawValue::String(value) if value.contains("..") => Ok(value),
			CustomWidgetRawValue::String(_) => Err(FormWidgetError::new("invalid date range")),
			_ => Err(FormWidgetError::new("expected string value")),
		}
	}

	fn format(value: &String) -> CustomWidgetRawValue {
		CustomWidgetRawValue::String(value.clone())
	}
}

fn first_tag_html(html: &str, tag: &str) -> String {
	let start = format!("<{tag}");
	let start_index = html
		.find(&start)
		.expect("tag should exist in rendered HTML");
	let tag_html = &html[start_index..];
	let end_index = tag_html
		.find('>')
		.expect("tag should close in rendered HTML");
	tag_html[..=end_index].to_string()
}

fn first_element_html(html: &str, tag: &str) -> String {
	let start = format!("<{tag}");
	let end = format!("</{tag}>");
	let start_index = html
		.find(&start)
		.expect("element should exist in rendered HTML");
	let element_html = &html[start_index..];
	let end_index = element_html
		.find(&end)
		.expect("element should close in rendered HTML")
		+ end.len();
	element_html[..end_index].to_string()
}

fn attr_values(html: &str, attr: &str) -> Vec<String> {
	let pattern = format!("{attr}=\"");
	let mut values = Vec::new();
	let mut rest = html;
	while let Some((_, after_pattern)) = rest.split_once(&pattern) {
		let (value, after_value) = after_pattern
			.split_once('"')
			.expect("attribute value should close in rendered HTML");
		values.push(value.to_string());
		rest = after_value;
	}
	values
}

fn input_tags(html: &str) -> Vec<String> {
	let mut tags = Vec::new();
	let mut rest = html;
	while let Some(start_index) = rest.find("<input") {
		let after_start = &rest[start_index..];
		let end_index = after_start
			.find('>')
			.expect("input tag should close in rendered HTML");
		tags.push(after_start[..=end_index].to_string());
		rest = &after_start[end_index + 1..];
	}
	tags
}

fn attr_value(tag: &str, attr: &str) -> Option<String> {
	let pattern = format!("{attr}=\"");
	let (_, after_pattern) = tag.split_once(&pattern)?;
	let (value, _) = after_pattern.split_once('"')?;
	Some(value.to_string())
}

fn input_attr_values_for_name_prefix(html: &str, prefix: &str, attr: &str) -> Vec<String> {
	input_tags(html)
		.into_iter()
		.filter(|tag| {
			attr_value(tag, "name")
				.as_deref()
				.is_some_and(|name| name.starts_with(prefix))
		})
		.map(|tag| {
			attr_value(&tag, attr).unwrap_or_else(|| {
				panic!("input tag with matching name should have {attr} attribute")
			})
		})
		.collect()
}

fn string_values(values: &[&str]) -> Vec<String> {
	values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn use_form_builds_runtime_from_generated_form_contract() {
	let profile = form! {
		name: ProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
				required,
			}
			bio: TextField {
				initial: "Compiler engineer",
			}
		}
	};

	let runtime = use_form(&profile).build();

	assert_eq!(runtime.get_values().display_name, "Ada".to_string());
	assert_eq!(runtime.get_values().bio, "Compiler engineer".to_string());
	assert!(!runtime.form_state().is_dirty.get());
	assert!(
		!runtime
			.get_field_state(profile.display_name_field())
			.is_dirty
	);
}

#[test]
fn use_form_routes_server_field_errors_and_preserves_unmatched_errors() {
	let profile = form! {
		name: ServerErrorProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada"
			},
		}
	};
	let runtime = use_form(&profile).build();
	let error = ServerFnError::validation_with_message(
		"Please correct the submitted values",
		[
			("display_name", "Display name is already used"),
			("missing_field", "Unknown field"),
		],
	);

	runtime.apply_server_error(&error);

	assert_eq!(
		runtime
			.get_field_state(profile.display_name_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("Display name is already used")
	);
	assert_eq!(
		runtime.form_state().form_error.get(),
		Some("Please correct the submitted values\nmissing_field: Unknown field".to_string())
	);
	assert_eq!(
		runtime.form_state().submit_error.get(),
		Some("Please correct the submitted values\nmissing_field: Unknown field".to_string())
	);
}

#[test]
fn use_form_routes_all_server_field_errors_without_form_error() {
	let profile = form! {
		name: AllMatchedServerErrorProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada"
			},
			bio: TextField {
				initial: "Compiler engineer"
			},
		}
	};
	let runtime = use_form(&profile).build();
	runtime
		.form_state()
		.submit_error
		.set(Some("previous submit error".to_string()));
	runtime
		.form_state()
		.error
		.set(Some("previous first error".to_string()));
	let error = ServerFnError::validation_with_message(
		"Please correct the submitted values",
		[
			("display_name", "Display name is already used"),
			("bio", "Biography is too long"),
		],
	);

	runtime.apply_server_error(&error);

	assert_eq!(
		runtime
			.get_field_state(profile.display_name_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("Display name is already used")
	);
	assert_eq!(
		runtime
			.get_field_state(profile.bio_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("Biography is too long")
	);
	assert_eq!(runtime.form_state().form_error.get(), None);
	assert_eq!(runtime.form_state().submit_error.get(), None);
}

#[test]
fn use_form_syncs_first_error_when_all_server_errors_match_fields() {
	let profile = form! {
		name: FirstErrorServerErrorProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada"
			},
		}
	};
	let runtime = use_form(&profile).build();
	runtime
		.form_state()
		.error
		.set(Some("previous first error".to_string()));
	let error = ServerFnError::validation_with_message(
		"Please correct the submitted values",
		[("display_name", "Display name is already used")],
	);

	runtime.apply_server_error(&error);

	assert_eq!(
		runtime.form_state().error.get(),
		Some("Display name is already used".to_string())
	);
}

#[test]
fn use_form_aggregates_duplicate_server_messages_for_one_field() {
	let profile = form! {
		name: DuplicateServerErrorProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada"
			},
		}
	};
	let runtime = use_form(&profile).build();
	let error = ServerFnError::validation_with_message(
		"Please correct the submitted values",
		[
			("display_name", "Display name is already used"),
			("display_name", "Display name must be unique per tenant"),
		],
	);

	runtime.apply_server_error(&error);

	assert_eq!(
		runtime
			.get_field_state(profile.display_name_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("Display name is already used\nDisplay name must be unique per tenant")
	);
	assert_eq!(runtime.form_state().form_error.get(), None);
	assert_eq!(runtime.form_state().submit_error.get(), None);
}

#[test]
fn use_form_aggregates_unmatched_nested_server_errors_at_form_level() {
	let profile = form! {
		name: NestedServerErrorProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada"
			},
		}
	};
	let runtime = use_form(&profile).build();
	runtime
		.form_state()
		.error
		.set(Some("previous first error".to_string()));
	let error = ServerFnError::validation_with_message(
		"Please correct the submitted values",
		[("addresses.0.street", "Street is required")],
	);

	runtime.apply_server_error(&error);

	assert_eq!(
		runtime.form_state().form_error.get(),
		Some(
			"Please correct the submitted values\naddresses.0.street: Street is required"
				.to_string()
		)
	);
	assert_eq!(
		runtime.form_state().submit_error.get(),
		Some(
			"Please correct the submitted values\naddresses.0.street: Street is required"
				.to_string()
		)
	);
	assert_eq!(
		runtime.get_field_state(profile.display_name_field()).error,
		None
	);
	assert_eq!(
		runtime.form_state().error.get(),
		Some(
			"Please correct the submitted values\naddresses.0.street: Street is required"
				.to_string()
		)
	);
}

#[test]
fn use_form_routes_server_errors_without_field_entries_to_form_level() {
	let profile = form! {
		name: FormOnlyServerErrorProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada"
			},
		}
	};
	let runtime = use_form(&profile).build();
	let error = ServerFnError::validation_with_message(
		"Please correct the submitted values",
		std::iter::empty::<(&str, &str)>(),
	);

	runtime.apply_server_error(&error);

	assert_eq!(
		runtime.form_state().form_error.get(),
		Some("Please correct the submitted values".to_string())
	);
	assert_eq!(
		runtime.form_state().submit_error.get(),
		Some("Please correct the submitted values".to_string())
	);
	assert_eq!(
		runtime.get_field_state(profile.display_name_field()).error,
		None
	);
}

#[test]
fn use_form_tracks_field_updates_errors_and_resets() {
	let profile = form! {
		name: ProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
				required,
			}
			bio: TextField {
				initial: "Compiler engineer",
			}
		}
	};

	let runtime = use_form(&profile).build();

	runtime.set_value(profile.display_name_field(), "Grace".to_string());
	runtime.set_error(
		profile.display_name_field(),
		FieldError::new("display name is already taken"),
	);

	assert_eq!(runtime.get_values().display_name, "Grace".to_string());
	assert!(runtime.form_state().is_dirty.get());
	assert!(runtime.form_state().is_touched.get());
	assert_eq!(
		runtime
			.get_field_state(profile.display_name_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("display name is already taken")
	);

	runtime.clear_field_error(profile.display_name_field());
	assert!(
		runtime
			.get_field_state(profile.display_name_field())
			.error
			.is_none()
	);

	runtime.reset_field(profile.display_name_field());
	assert_eq!(runtime.get_values().display_name, "Ada".to_string());
	assert!(
		!runtime
			.get_field_state(profile.display_name_field())
			.is_dirty
	);

	runtime.set_value(profile.bio_field(), "COBOL pioneer".to_string());
	runtime.reset_default_values();
	assert_eq!(runtime.default_values().bio, "COBOL pioneer".to_string());
	assert!(!runtime.form_state().is_dirty.get());

	runtime.set_value(profile.bio_field(), "Compiler engineer".to_string());
	assert!(runtime.form_state().is_dirty.get());

	runtime.reset();
	assert_eq!(runtime.get_values().bio, "COBOL pioneer".to_string());
	assert!(!runtime.form_state().is_dirty.get());
}

#[test]
fn use_form_tracks_month_and_week_as_strings() {
	let schedule = form! {
		name: ScheduleForm,
		action: "/schedule",
		fields: {
			billing_month: CharField {
				widget: MonthInput,
				initial: "2026-06".to_string(),
			}
			sprint_week: CharField {
				widget: WeekInput,
				initial: "2026-W24".to_string(),
			}
		}
	};

	let runtime = use_form(&schedule).build();
	let billing_month = runtime.watch_field::<String>(schedule.billing_month_field());
	let sprint_week = runtime.watch_field::<String>(schedule.sprint_week_field());

	assert_eq!(runtime.get_values().billing_month, "2026-06".to_string());
	assert_eq!(runtime.get_values().sprint_week, "2026-W24".to_string());
	assert_eq!(billing_month.get(), "2026-06".to_string());
	assert_eq!(sprint_week.get(), "2026-W24".to_string());
	assert!(!runtime.form_state().is_dirty.get());
	assert!(
		!runtime
			.get_field_state(schedule.billing_month_field())
			.is_dirty
	);
	assert!(
		!runtime
			.get_field_state(schedule.sprint_week_field())
			.is_dirty
	);

	runtime.set_value(schedule.billing_month_field(), "2026-07".to_string());

	assert_eq!(runtime.get_values().billing_month, "2026-07".to_string());
	assert!(runtime.form_state().is_dirty.get());
	assert!(
		runtime
			.get_field_state(schedule.billing_month_field())
			.is_dirty
	);

	runtime.reset();

	assert_eq!(runtime.get_values().billing_month, "2026-06".to_string());
	assert_eq!(runtime.get_values().sprint_week, "2026-W24".to_string());
	assert!(!runtime.form_state().is_dirty.get());
	assert!(
		!runtime
			.get_field_state(schedule.billing_month_field())
			.is_dirty
	);
	assert!(
		!runtime
			.get_field_state(schedule.sprint_week_field())
			.is_dirty
	);
}

#[test]
fn use_form_can_sync_after_native_reset() {
	let profile = form! {
		name: NativeResetForm,
		action: "/profile",
		fields: {
			name: CharField {
				initial: "before".to_string(),
			}
			reset: ResetButton {
				label: "Reset",
			}
		}
	};

	let runtime = use_form(&profile).build();

	runtime.set_value(profile.name_field(), "after".to_string());
	runtime.set_error(profile.name_field(), FieldError::new("name is invalid"));

	assert_eq!(runtime.get_values().name, "after".to_string());
	assert!(runtime.form_state().is_dirty.get());
	assert!(runtime.form_state().is_touched.get());
	assert!(runtime.get_field_state(profile.name_field()).is_dirty);
	assert!(runtime.get_field_state(profile.name_field()).is_touched);
	assert_eq!(
		runtime
			.get_field_state(profile.name_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("name is invalid")
	);

	profile.name().set("before".to_string());
	runtime.sync_after_native_reset();

	assert_eq!(runtime.get_values().name, "before".to_string());
	assert!(!runtime.form_state().is_dirty.get());
	assert!(!runtime.form_state().is_touched.get());
	assert!(!runtime.get_field_state(profile.name_field()).is_dirty);
	assert!(!runtime.get_field_state(profile.name_field()).is_touched);
	assert!(
		runtime
			.get_field_state(profile.name_field())
			.error
			.is_none()
	);

	profile.name().set("native value".to_string());
	runtime.set_value(profile.name_field(), "runtime touched".to_string());
	profile.name().set("native value".to_string());
	runtime.set_error(profile.name_field(), FieldError::new("native error"));

	assert_eq!(runtime.get_values().name, "native value".to_string());
	assert!(runtime.form_state().is_dirty.get());
	assert!(runtime.form_state().is_touched.get());
	assert!(runtime.get_field_state(profile.name_field()).is_dirty);
	assert!(runtime.get_field_state(profile.name_field()).is_touched);
	assert_eq!(
		runtime
			.get_field_state(profile.name_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("native error")
	);

	runtime.sync_after_native_reset();
	let name_state = runtime.get_field_state(profile.name_field());

	assert_eq!(runtime.get_values().name, "native value".to_string());
	assert!(runtime.form_state().is_dirty.get());
	assert!(name_state.is_dirty);
	assert_eq!(runtime.form_state().is_dirty.get(), name_state.is_dirty);
	assert!(!runtime.form_state().is_touched.get());
	assert!(!name_state.is_touched);
	assert!(name_state.error.is_none());
	assert!(runtime.form_state().error.get().is_none());
}

#[test]
fn custom_widget_bridge_parse_error_sets_runtime_field_error() {
	let booking = form! {
		name: CustomWidgetRuntimeForm,
		action: "/booking",
		fields: {
			date_range: CharField {
				initial: "2026-06-01..2026-06-02".to_string(),
				disabled,
				widget: CustomWidget(capturing_date_range_picker) {
					experimental,
					adapter: DateRangeAdapter,
				},
			}
		}
	};
	let runtime = use_form(&booking).build();
	let _page = booking.clone().into_page();
	let props = take_last_custom_widget_props();

	assert_eq!(props.value, "2026-06-01..2026-06-02");
	assert!(props.disabled);
	assert!(!props.touched);
	assert!(props.error.is_none());

	let invalid_result = (props.on_raw_change)(CustomWidgetRawValue::String("invalid".to_string()));

	assert!(invalid_result.is_err());
	assert_eq!(
		invalid_result.err().as_ref().map(FormWidgetError::message),
		Some("invalid date range")
	);
	assert_eq!(
		runtime
			.get_field_state(booking.date_range_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("invalid date range")
	);
	assert_eq!(
		runtime.get_values().date_range,
		"2026-06-01..2026-06-02".to_string()
	);
	assert!(runtime.trigger().is_err());
	assert_eq!(
		runtime.handle_submit(),
		UseFormSubmitOutcome::ValidationFailed
	);

	let _page = booking.clone().into_page();
	let props = take_last_custom_widget_props();
	assert!(props.disabled);
	assert!(props.touched);
	assert_eq!(
		props.error.as_ref().map(FieldError::message),
		Some("invalid date range")
	);

	let valid_result = (props.on_raw_change)(CustomWidgetRawValue::String(
		"2026-06-03..2026-06-04".to_string(),
	));

	assert!(valid_result.is_ok());
	assert_eq!(
		runtime.get_values().date_range,
		"2026-06-03..2026-06-04".to_string()
	);
	assert!(
		runtime
			.get_field_state(booking.date_range_field())
			.error
			.is_none()
	);

	let _page = booking.clone().into_page();
	let props = take_last_custom_widget_props();
	assert_eq!(props.value, "2026-06-03..2026-06-04");
	assert!(props.disabled);
	assert!(props.touched);
	assert!(props.error.is_none());
}

#[test]
fn use_form_keeps_generated_initial_defaults_separate_from_live_signals() {
	let profile = form! {
		name: ProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
			}
		}
	};

	profile.display_name().set("Grace".to_string());
	let runtime = use_form(&profile).build();

	assert_eq!(runtime.default_values().display_name, "Ada".to_string());
	assert_eq!(runtime.get_values().display_name, "Grace".to_string());
	assert!(runtime.form_state().is_dirty.get());
	assert!(
		runtime
			.get_field_state(profile.display_name_field())
			.is_dirty
	);
}

#[test]
fn use_form_maps_generated_validator_errors_to_field_state() {
	let profile = form! {
		name: ProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
			}
		}
		validators: {
			display_name: [|v| v == "Ada" =>"display name must remain Ada", ],
		}
	};
	let runtime = use_form(&profile).build();

	runtime.set_value(profile.display_name_field(), "Grace".to_string());
	let result = runtime.trigger();

	assert!(result.is_err());
	assert_eq!(runtime.form_state().form_error.get(), None);
	assert_eq!(
		runtime
			.get_field_state(profile.display_name_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("display name must remain Ada")
	);
}

#[test]
fn use_form_syncs_direct_generated_signal_changes() {
	let profile = form! {
		name: ProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
			}
		}
		validators: {
			display_name: [|v| v == "Ada" =>"display name must remain Ada", ],
		}
	};
	let runtime = use_form(&profile)
		.revalidate_on(RevalidateOn::Change)
		.build();
	let values = runtime.watch();
	let event_count = Rc::new(Cell::new(0));
	let event_count_for_subscription = Rc::clone(&event_count);
	let _subscription = runtime.subscribe(move |event| match event {
		FormEvent::ValueChanged { .. } | FormEvent::Validated => {
			event_count_for_subscription.set(event_count_for_subscription.get() + 1);
		}
		_ => {}
	});

	profile.display_name().set("Grace".to_string());

	assert_eq!(runtime.get_values().display_name, "Grace".to_string());
	assert_eq!(values.get().display_name, "Grace".to_string());
	assert!(runtime.form_state().is_dirty.get());
	assert!(runtime.form_state().is_touched.get());
	assert!(
		runtime
			.get_field_state(profile.display_name_field())
			.is_touched
	);
	assert_eq!(
		runtime
			.get_field_state(profile.display_name_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("display name must remain Ada")
	);
	assert_eq!(event_count.get(), 2);
}

#[test]
fn use_form_accepts_json_field_runtime_contracts() {
	let settings = form! {
		name: SettingsForm,
		action: "/settings",
		fields: {
			payload: JsonField<::serde_json::Value> {
				initial: ::serde_json::json!( {
					"theme": "dark"
				}),
			}
		}
	};

	let runtime = use_form(&settings).build();

	assert_eq!(
		runtime.get_values().payload,
		::serde_json::json!({"theme": "dark"})
	);
}

#[test]
fn use_form_pushes_inserts_removes_and_moves_collection_items() {
	let invoice = form! {
		name: InvoiceForm,
		action: "/invoices",
		fields: {
			customer_name: CharField {
				initial: "Ada",
			}
			line_items: FieldArray {
				fields: {
					description: CharField {
						required,
					}
					quantity: IntegerField {
						required,
					}
				}
			}
		}
	};
	let runtime = use_form(&invoice).build();
	let values = runtime.watch();
	let collection = invoice.line_items_collection();
	let new_line_item = |description: &str, quantity: i64| {
		let mut item = invoice.new_line_items_item();
		item.description = description.to_string();
		item.quantity = quantity;
		item
	};

	assert!(runtime.get_values().line_items.is_empty());
	assert!(!runtime.form_state().is_dirty.get());
	assert!(!runtime.form_state().is_touched.get());

	let keyboard_key = runtime.push_item(collection, new_line_item("Keyboard", 2));
	let mouse_key = runtime.push_item(collection, new_line_item("Mouse", 1));
	let monitor_key = runtime.insert_item(collection, 1, new_line_item("Monitor", 3));

	assert_eq!(
		runtime
			.get_values()
			.line_items
			.iter()
			.map(|item| item.description.as_str())
			.collect::<Vec<_>>(),
		vec!["Keyboard", "Monitor", "Mouse"]
	);
	assert_eq!(
		invoice
			.line_items()
			.get()
			.iter()
			.map(|item| (item.key(), item.index()))
			.collect::<Vec<_>>(),
		vec![(keyboard_key, 0), (monitor_key, 1), (mouse_key, 2)]
	);
	assert_eq!(values.get().line_items.len(), 3);
	assert!(runtime.form_state().is_dirty.get());
	assert!(runtime.form_state().is_touched.get());

	assert_eq!(runtime.move_item(collection, mouse_key, 0), Some((2, 0)));
	assert_eq!(
		runtime
			.get_values()
			.line_items
			.iter()
			.map(|item| item.description.as_str())
			.collect::<Vec<_>>(),
		vec!["Mouse", "Keyboard", "Monitor"]
	);
	assert_eq!(
		invoice
			.line_items()
			.get()
			.iter()
			.map(|item| (item.key(), item.index()))
			.collect::<Vec<_>>(),
		vec![(mouse_key, 0), (keyboard_key, 1), (monitor_key, 2)]
	);

	assert!(runtime.remove_item(collection, keyboard_key));
	assert!(!runtime.remove_item(collection, keyboard_key));
	assert_eq!(runtime.move_item(collection, keyboard_key, 0), None);
	assert_eq!(
		runtime
			.get_values()
			.line_items
			.iter()
			.map(|item| item.description.as_str())
			.collect::<Vec<_>>(),
		vec!["Mouse", "Monitor"]
	);
	assert_eq!(
		invoice
			.line_items()
			.get()
			.iter()
			.map(|item| (item.key(), item.index()))
			.collect::<Vec<_>>(),
		vec![(mouse_key, 0), (monitor_key, 1)]
	);

	assert!(runtime.remove_item(collection, mouse_key));
	assert!(runtime.remove_item(collection, monitor_key));
	assert!(runtime.get_values().line_items.is_empty());
	assert!(values.get().line_items.is_empty());
	assert!(!runtime.form_state().is_dirty.get());
	assert!(runtime.form_state().is_touched.get());

	let seeded_runtime = use_form(&invoice).build();
	let mut seeded_values = seeded_runtime.get_values();
	seeded_values
		.line_items
		.push(new_line_item("Replacement cable", 4));
	seeded_values
		.line_items
		.push(new_line_item("Docking station", 1));
	seeded_runtime.set_values(seeded_values);
	assert!(seeded_runtime.form_state().is_dirty.get());
	assert_eq!(seeded_runtime.watch().get().line_items.len(), 2);

	let generated_keys = invoice
		.line_items()
		.get()
		.iter()
		.map(|item| item.key())
		.collect::<Vec<_>>();
	let extra_key = seeded_runtime.push_item(collection, new_line_item("Stand", 2));
	assert!(!generated_keys.contains(&extra_key));

	let keys = invoice
		.line_items()
		.get()
		.iter()
		.map(|item| item.key())
		.collect::<Vec<_>>();
	for (index, key) in keys.iter().enumerate() {
		assert!(!keys[index + 1..].contains(key));
	}
}

#[test]
fn field_array_renders_runtime_items_with_deterministic_names() {
	let invoice = form! {
		name: InvoiceForm,
		action: "/invoices",
		fields: {
			customer_name: CharField {
				initial: "Ada",
			}
			line_items: FieldArray {
				label: "Line items",
				class: "invoice-lines",
				min_items: 1,
				max_items: 3,
				fields: {
					description: CharField {
						label: "Description",
						required,
					}
					quantity: IntegerField {
						label: "Quantity",
						required,
					}
				}
			}
		}
	};
	let runtime = use_form(&invoice).build();
	let collection = invoice.line_items_collection();
	let mut first = invoice.new_line_items_item();
	first.description = "Keyboard".to_string();
	first.quantity = 2;
	let mut second = invoice.new_line_items_item();
	second.description = "Mouse".to_string();
	second.quantity = 1;

	let first_key = runtime.push_item(collection, first);
	let second_key = runtime.push_item(collection, second);
	let html = invoice.clone().into_page().render_to_string();

	assert_eq!(
		first_tag_html(&html, "fieldset"),
		r#"<fieldset class="invoice-lines" data-reinhardt-field-array="line_items" data-reinhardt-min-items="1" data-reinhardt-max-items="3">"#
	);
	assert_eq!(
		first_element_html(&html, "legend"),
		r#"<legend class="reinhardt-field-array-label">Line items</legend>"#
	);
	assert_eq!(
		attr_values(&html, "data-reinhardt-item-key"),
		vec![format!("{first_key:?}"), format!("{second_key:?}"),]
	);
	assert_eq!(
		input_attr_values_for_name_prefix(&html, "line_items[", "name"),
		string_values(&[
			"line_items[0][description]",
			"line_items[0][quantity]",
			"line_items[1][description]",
			"line_items[1][quantity]",
		])
	);
	assert_eq!(
		input_attr_values_for_name_prefix(&html, "line_items[", "id"),
		string_values(&[
			"line_items_0_description",
			"line_items_0_quantity",
			"line_items_1_description",
			"line_items_1_quantity",
		])
	);
	assert_eq!(
		input_attr_values_for_name_prefix(&html, "line_items[", "value"),
		string_values(&["Keyboard", "2", "Mouse", "1"])
	);

	assert_eq!(runtime.move_item(collection, second_key, 0), Some((1, 0)));
	let moved_html = invoice.into_page().render_to_string();
	assert_eq!(
		input_attr_values_for_name_prefix(&moved_html, "line_items[", "name"),
		string_values(&[
			"line_items[0][description]",
			"line_items[0][quantity]",
			"line_items[1][description]",
			"line_items[1][quantity]",
		])
	);
	assert_eq!(
		input_attr_values_for_name_prefix(&moved_html, "line_items[", "value"),
		string_values(&["Mouse", "1", "Keyboard", "2"])
	);
}

#[test]
fn field_array_renders_datetime_local_values() {
	let schedule = form! {
		name: ScheduleForm,
		action: "/schedule",
		fields: {
			slots: FieldArray {
				fields: {
					starts_at: DateTimeField {}
				}
			}
		}
	};
	let runtime = use_form(&schedule).build();
	let collection = schedule.slots_collection();
	let mut slot = schedule.new_slots_item();
	slot.starts_at = Some(
		chrono::NaiveDate::from_ymd_opt(2026, 6, 16)
			.expect("valid date")
			.and_hms_opt(8, 30, 45)
			.expect("valid time"),
	);

	runtime.push_item(collection, slot);
	let html = schedule.into_page().render_to_string();

	assert_eq!(
		input_attr_values_for_name_prefix(&html, "slots[", "type"),
		string_values(&["datetime-local"])
	);
	assert_eq!(
		input_attr_values_for_name_prefix(&html, "slots[", "value"),
		string_values(&["2026-06-16T08:30:45"])
	);
}

#[test]
fn field_array_required_validation_sets_path_errors_by_item_key() {
	let invoice = form! {
		name: InvoiceForm,
		action: "/invoices",
		fields: {
			line_items: FieldArray {
				fields: {
					description: CharField {
						required,
					}
					quantity: IntegerField {
						required,
					}
				}
			}
		}
	};
	let runtime = use_form(&invoice).build();
	let collection = invoice.line_items_collection();
	let mut first = invoice.new_line_items_item();
	first.quantity = 2;
	let mut second = invoice.new_line_items_item();
	second.description = "Mouse".to_string();
	second.quantity = 1;

	let first_key = runtime.push_item(collection, first);
	let second_key = runtime.push_item(collection, second);
	let first_description_path = invoice.line_items_description_path(first_key);
	let second_description_path = invoice.line_items_description_path(second_key);

	let result = runtime
		.trigger()
		.expect_err("empty required collection field should fail validation");
	assert!(result.field_errors().is_empty());
	assert_eq!(result.path_errors().len(), 1);
	assert_eq!(
		runtime
			.get_path_state(first_description_path.clone())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("line_items.description is required"),
	);
	assert!(
		runtime
			.get_path_state(second_description_path)
			.error
			.is_none()
	);

	assert_eq!(runtime.move_item(collection, first_key, 1), Some((0, 1)));
	let result = runtime
		.trigger()
		.expect_err("reordered empty required collection field should still fail validation");
	assert_eq!(result.path_errors().len(), 1);
	assert_eq!(
		runtime
			.get_path_state(first_description_path.clone())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("line_items.description is required"),
	);

	runtime.set_path_value(first_description_path.clone(), "Keyboard".to_string());
	runtime
		.trigger()
		.expect("filled required collection field should pass validation");
	assert!(
		runtime
			.get_path_state(first_description_path)
			.error
			.is_none()
	);
}

#[test]
fn field_array_min_and_max_items_set_collection_errors() {
	let invoice = form! {
		name: InvoiceForm,
		action: "/invoices",
		fields: {
			line_items: FieldArray {
				min_items: 1,
				max_items: 2,
				fields: {
					description: CharField {}
				}
			}
		}
	};
	let runtime = use_form(&invoice).build();
	let collection = invoice.line_items_collection();

	let result = runtime
		.trigger()
		.expect_err("empty collection should fail min_items validation");
	assert_eq!(result.collection_errors().len(), 1);
	assert_eq!(
		runtime
			.get_collection_state(collection)
			.error
			.as_ref()
			.map(FieldError::message),
		Some("line_items requires at least 1 item"),
	);

	for description in ["A", "B", "C"] {
		let mut item = invoice.new_line_items_item();
		item.description = description.to_string();
		runtime.push_item(collection, item);
	}

	let result = runtime
		.trigger()
		.expect_err("oversized collection should fail max_items validation");
	assert_eq!(result.collection_errors().len(), 1);
	let collection_state = runtime.get_collection_state(collection);
	assert_eq!(collection_state.len, 3);
	assert!(collection_state.is_touched);
	assert!(collection_state.is_dirty);
	assert_eq!(
		collection_state.error.as_ref().map(FieldError::message),
		Some("line_items allows at most 2 items"),
	);
}

#[test]
fn use_form_watches_and_sets_collection_field_paths() {
	ReactiveScope::run(|| {
		let invoice = form! {
			name: InvoiceForm,
			action: "/invoices",
			fields: {
				line_items: FieldArray {
					fields: {
						description: CharField {
							required,
						}
						quantity: IntegerField {
							required,
						}
					}
				}
			}
		};
		let runtime = use_form(&invoice).build();
		let collection = invoice.line_items_collection();
		let mut item = invoice.new_line_items_item();
		item.description = "Keyboard".to_string();
		item.quantity = 2;

		let key = runtime.push_item(collection, item);
		let quantity_path = invoice.line_items_quantity_path(key);
		let description_path = invoice.line_items_description_path(key);
		let quantity = runtime.watch_path::<i64>(quantity_path.clone());
		let description = runtime.watch_path::<String>(description_path.clone());

		assert_eq!(quantity.get(), 2);
		assert_eq!(description.get(), "Keyboard".to_string());
		assert!(runtime.get_path_state(quantity_path.clone()).is_dirty);
		assert!(!runtime.get_path_state(quantity_path.clone()).is_touched);

		runtime.reset_default_values();

		assert!(!runtime.get_path_state(quantity_path.clone()).is_dirty);

		runtime.set_path_value(quantity_path.clone(), 3_i64);

		assert_eq!(quantity.get(), 3);
		assert_eq!(runtime.get_values().line_items[0].quantity, 3);
		assert_eq!(runtime.watch().get().line_items[0].quantity, 3);
		assert!(runtime.get_path_state(quantity_path.clone()).is_touched);
		assert!(runtime.get_path_state(quantity_path.clone()).is_dirty);

		let updated_item = invoice
			.line_items()
			.get()
			.into_iter()
			.next()
			.map(|item| {
				let mut value = item.into_value();
				value.quantity = 5;
				CollectionItem::new(key, 0, value)
			})
			.expect("line item exists");
		invoice.line_items().set(vec![updated_item]);

		let quantity_after_direct_set = runtime.watch_path::<i64>(quantity_path.clone());
		assert_eq!(quantity_after_direct_set.get(), 5);
		assert_eq!(quantity.get(), 5);

		runtime.set_path_value(description_path.clone(), "Mechanical keyboard".to_string());

		assert_eq!(description.get(), "Mechanical keyboard".to_string());
		assert_eq!(
			runtime.get_values().line_items[0].description,
			"Mechanical keyboard".to_string()
		);
		assert!(runtime.get_path_state(description_path.clone()).is_touched);

		assert!(runtime.remove_item(collection, key));
		assert!(
			::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
				runtime.watch_path::<String>(description_path.clone());
			}))
			.is_err()
		);
		assert!(
			::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
				runtime.set_path_value(quantity_path.clone(), 4_i64);
			}))
			.is_err()
		);
	});
}

#[test]
fn direct_collection_signal_set_syncs_path_watchers_and_state() {
	ReactiveScope::run(|| {
		let invoice = form! {
			name: InvoiceForm,
			action: "/invoices",
			fields: {
				line_items: FieldArray {
					fields: {
						description: CharField {}
						quantity: IntegerField {}
					}
				}
			}
		};
		let runtime = use_form(&invoice).build();
		let collection = invoice.line_items_collection();
		let key = CollectionItemKey::from_runtime_index(0);
		let mut item = invoice.new_line_items_item();
		item.description = "Keyboard".to_string();
		item.quantity = 2;

		invoice
			.line_items()
			.set(vec![CollectionItem::new(key, 0, item)]);

		let quantity_path = invoice.line_items_quantity_path(key);
		let quantity = runtime.watch_path::<i64>(quantity_path.clone());
		assert_eq!(quantity.get(), 2);
		assert!(runtime.get_collection_state(collection).is_touched);
		assert!(runtime.get_path_state(quantity_path.clone()).is_touched);

		let mut updated_item = invoice
			.line_items()
			.get()
			.into_iter()
			.next()
			.expect("line item exists")
			.into_value();
		updated_item.quantity = 5;
		invoice
			.line_items()
			.set(vec![CollectionItem::new(key, 0, updated_item)]);

		assert_eq!(quantity.get(), 5);
		assert_eq!(runtime.get_values().line_items[0].quantity, 5);
		assert!(runtime.get_path_state(quantity_path).is_touched);
	});
}

#[test]
fn collection_path_state_follows_item_key_across_reorder() {
	let invoice = form! {
		name: InvoiceForm,
		action: "/invoices",
		fields: {
			line_items: FieldArray {
				fields: {
					description: CharField {}
					quantity: IntegerField {}
				}
			}
		}
	};
	let runtime = use_form(&invoice).build();
	let collection = invoice.line_items_collection();
	let new_line_item = |description: &str, quantity: i64| {
		let mut item = invoice.new_line_items_item();
		item.description = description.to_string();
		item.quantity = quantity;
		item
	};

	let first = runtime.push_item(collection, new_line_item("Compiler", 1));
	let second = runtime.push_item(collection, new_line_item("Notebook", 2));
	let first_quantity = invoice.line_items_quantity_path(first);

	runtime.reset_default_values();
	assert!(!runtime.get_path_state(first_quantity.clone()).is_dirty);
	assert_eq!(runtime.move_item(collection, first, 1), Some((0, 1)));
	assert_eq!(runtime.get_values().line_items[1].quantity, 1);
	assert!(!runtime.get_path_state(first_quantity.clone()).is_dirty);

	runtime.set_path_value(first_quantity.clone(), 5_i64);
	runtime.set_path_error(first_quantity.clone(), FieldError::new("invalid quantity"));

	assert_eq!(runtime.get_values().line_items[1].quantity, 5);
	let first_quantity_state = runtime.get_path_state(first_quantity.clone());
	assert!(first_quantity_state.is_dirty);
	assert!(first_quantity_state.is_touched);
	assert_eq!(
		first_quantity_state.error.as_ref().map(FieldError::message),
		Some("invalid quantity")
	);

	let removed_quantity = invoice.line_items_quantity_path(second);
	runtime.set_path_value(removed_quantity.clone(), 3_i64);
	runtime.set_path_error(
		removed_quantity.clone(),
		FieldError::new("removed quantity"),
	);

	assert!(runtime.remove_item(collection, second));
	assert_eq!(runtime.get_values().line_items.len(), 1);
	let removed_quantity_state = runtime.get_path_state(removed_quantity);
	assert!(!removed_quantity_state.is_touched);
	assert_eq!(removed_quantity_state.error, None);
}

#[test]
fn reconcile_defaults_preserves_path_defaults_across_dirty_reorder() {
	let invoice = form! {
		name: InvoiceForm,
		action: "/invoices",
		fields: {
			line_items: FieldArray {
				fields: {
					description: CharField {}
					quantity: IntegerField {}
				}
			}
		}
	};
	let runtime = use_form(&invoice)
		.deps(0_u8)
		.reset_on_deps(ResetOnDeps::KeepDirtyValues)
		.build();
	let collection = invoice.line_items_collection();
	let new_line_item = |description: &str, quantity: i64| {
		let mut item = invoice.new_line_items_item();
		item.description = description.to_string();
		item.quantity = quantity;
		item
	};

	let first = runtime.push_item(collection, new_line_item("Compiler", 1));
	let second = runtime.push_item(collection, new_line_item("Notebook", 2));
	let first_quantity = invoice.line_items_quantity_path(first);
	runtime.reset_default_values();
	let defaults = runtime.default_values();

	assert_eq!(runtime.move_item(collection, first, 1), Some((0, 1)));
	assert_eq!(
		runtime
			.get_values()
			.line_items
			.iter()
			.map(|item| item.description.as_str())
			.collect::<Vec<_>>(),
		vec!["Notebook", "Compiler"]
	);
	runtime.reconcile_defaults(defaults, 1_u8);

	assert_eq!(runtime.get_values().line_items[1].quantity, 1);
	assert!(!runtime.get_path_state(first_quantity).is_dirty);
	assert_eq!(runtime.move_item(collection, second, 0), Some((0, 0)));
}

#[test]
fn set_values_clears_replaced_collection_path_state() {
	let invoice = form! {
		name: InvoiceForm,
		action: "/invoices",
		fields: {
			line_items: FieldArray {
				fields: {
					description: CharField {}
					quantity: IntegerField {}
				}
			}
		}
	};
	let runtime = use_form(&invoice).build();
	let collection = invoice.line_items_collection();
	let mut item = invoice.new_line_items_item();
	item.description = "Compiler".to_string();
	item.quantity = 1;
	let key = runtime.push_item(collection, item);
	let quantity_path = invoice.line_items_quantity_path(key);

	runtime.set_path_value(quantity_path.clone(), 2_i64);
	runtime.set_path_error(quantity_path.clone(), FieldError::new("stale quantity"));
	assert_eq!(
		runtime.form_state().error.get(),
		Some("stale quantity".to_string())
	);

	let mut values = runtime.get_values();
	values.line_items.clear();
	runtime.set_values(values);

	assert_eq!(runtime.form_state().error.get(), None);
	let quantity_state = runtime.get_path_state(quantity_path.clone());
	assert!(!quantity_state.is_touched);
	assert_eq!(quantity_state.error, None);
	assert!(
		::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
			runtime.watch_path::<i64>(quantity_path.clone());
		}))
		.is_err()
	);
}

#[test]
fn reconcile_defaults_clears_collection_path_errors_when_errors_are_kept() {
	let invoice = form! {
		name: InvoiceForm,
		action: "/invoices",
		fields: {
			line_items: FieldArray {
				fields: {
					description: CharField {}
					quantity: IntegerField {}
				}
			}
		}
	};
	let runtime = use_form(&invoice)
		.deps(0_u8)
		.reset_on_deps(ResetOnDeps::ResetAll)
		.keep_errors(true)
		.build();
	let collection = invoice.line_items_collection();
	let mut item = invoice.new_line_items_item();
	item.description = "Compiler".to_string();
	item.quantity = 1;
	let key = runtime.push_item(collection, item);
	let quantity_path = invoice.line_items_quantity_path(key);

	runtime.set_path_error(quantity_path.clone(), FieldError::new("old path error"));
	assert_eq!(
		runtime.form_state().error.get(),
		Some("old path error".to_string())
	);

	let mut defaults = runtime.get_values();
	defaults.line_items.clear();
	runtime.reconcile_defaults(defaults, 1_u8);

	assert_eq!(runtime.form_state().error.get(), None);
	assert_eq!(runtime.get_values().line_items.len(), 0);
	assert_eq!(runtime.get_path_state(quantity_path).error, None);
}

#[test]
fn use_form_marks_collection_only_set_values_dirty() {
	let invoice = form! {
		name: InvoiceForm,
		action: "/invoices",
		fields: {
			customer_name: CharField {
				initial: "Ada",
			}
			line_items: FieldArray {
				fields: {
					description: CharField {
						required,
					}
					quantity: IntegerField {
						required,
					}
				}
			}
		}
	};
	let runtime = use_form(&invoice).build();
	let _collection = invoice.line_items_collection();
	let watched_values = runtime.watch();
	let mut values = runtime.get_values();
	let mut item = invoice.new_line_items_item();
	item.description = "Replacement cable".to_string();
	item.quantity = 4;

	values.line_items.push(item);
	runtime.set_values(values);

	assert_eq!(runtime.get_values().customer_name, "Ada".to_string());
	assert_eq!(runtime.get_values().line_items.len(), 1);
	assert_eq!(watched_values.get().line_items.len(), 1);
	assert!(runtime.form_state().is_dirty.get());
	assert!(runtime.form_state().is_touched.get());
}

#[test]
fn use_form_syncs_direct_collection_signal_changes() {
	let invoice = form! {
		name: InvoiceForm,
		action: "/invoices",
		fields: {
			customer_name: CharField {
				initial: "Ada",
			}
			line_items: FieldArray {
				fields: {
					description: CharField {
						required,
					}
					quantity: IntegerField {
						required,
					}
				}
			}
		}
	};
	let runtime = use_form(&invoice)
		.revalidate_on(RevalidateOn::Change)
		.build();
	let _collection = invoice.line_items_collection();
	let watched_values = runtime.watch();
	let event_count = Rc::new(Cell::new(0));
	let event_count_for_subscription = Rc::clone(&event_count);
	let validated_count = Rc::new(Cell::new(0));
	let validated_count_for_subscription = Rc::clone(&validated_count);
	let _subscription = runtime.subscribe(move |event| {
		if matches!(event, FormEvent::ValueChanged { .. }) {
			event_count_for_subscription.set(event_count_for_subscription.get() + 1);
		}
		if matches!(event, FormEvent::Validated) {
			validated_count_for_subscription.set(validated_count_for_subscription.get() + 1);
		}
	});
	let mut item = invoice.new_line_items_item();
	item.description = "Docking station".to_string();
	item.quantity = 1;

	invoice.line_items().set(vec![CollectionItem::new(
		CollectionItemKey::from_runtime_index(0),
		0,
		item,
	)]);

	assert_eq!(runtime.get_values().customer_name, "Ada".to_string());
	assert_eq!(
		runtime.get_values().line_items[0].description,
		"Docking station".to_string()
	);
	assert_eq!(watched_values.get().line_items.len(), 1);
	assert!(runtime.form_state().is_dirty.get());
	assert!(runtime.form_state().is_touched.get());
	assert_eq!(event_count.get(), 0);
	assert_eq!(validated_count.get(), 1);
}

#[test]
fn deps_reconciliation_updates_pristine_collections_and_preserves_dirty_collections() {
	let pristine_invoice = form! {
		name: PristineInvoiceForm,
		action: "/invoices",
		fields: {
			customer_name: CharField {
				initial: "Ada",
			}
			line_items: FieldArray {
				fields: {
					description: CharField {}
					quantity: IntegerField {}
				}
			}
		}
	};
	let pristine_runtime = use_form(&pristine_invoice)
		.deps((1_u64,))
		.reset_on_deps(ResetOnDeps::KeepDirtyValues)
		.build();
	let _pristine_collection = pristine_invoice.line_items_collection();
	let mut refreshed_defaults = pristine_runtime.default_values();
	let mut default_item = pristine_invoice.new_line_items_item();
	default_item.description = "Default cable".to_string();
	default_item.quantity = 2;
	refreshed_defaults.line_items.push(default_item);

	pristine_runtime.reconcile_defaults(refreshed_defaults, (2_u64,));

	assert_eq!(
		pristine_runtime.get_values().line_items[0].description,
		"Default cable".to_string()
	);
	assert_eq!(pristine_runtime.watch().get().line_items.len(), 1);
	assert!(!pristine_runtime.form_state().is_dirty.get());

	let dirty_invoice = form! {
		name: DirtyInvoiceForm,
		action: "/invoices",
		fields: {
			customer_name: CharField {
				initial: "Ada",
			}
			line_items: FieldArray {
				fields: {
					description: CharField {}
					quantity: IntegerField {}
				}
			}
		}
	};
	let dirty_runtime = use_form(&dirty_invoice)
		.deps((1_u64,))
		.reset_on_deps(ResetOnDeps::KeepDirtyValues)
		.build();
	let dirty_collection = dirty_invoice.line_items_collection();
	let mut user_item = dirty_invoice.new_line_items_item();
	user_item.description = "User stand".to_string();
	user_item.quantity = 1;
	dirty_runtime.push_item(dirty_collection, user_item);

	let mut dirty_refreshed_defaults = dirty_runtime.default_values();
	let mut changed_default_item = dirty_invoice.new_line_items_item();
	changed_default_item.description = "Default dock".to_string();
	changed_default_item.quantity = 4;
	dirty_refreshed_defaults
		.line_items
		.push(changed_default_item);

	dirty_runtime.reconcile_defaults(dirty_refreshed_defaults, (2_u64,));

	assert_eq!(
		dirty_runtime.get_values().line_items[0].description,
		"User stand".to_string()
	);
	assert_eq!(
		dirty_runtime.default_values().line_items[0].description,
		"Default dock".to_string()
	);
	assert!(dirty_runtime.form_state().is_dirty.get());
}

#[test]
fn deps_reconciliation_keeps_dirty_values_and_updates_pristine_values() {
	let profile = form! {
		name: ProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
				required,
			}
			bio: TextField {
				initial: "Compiler engineer",
			}
		}
	};

	let runtime = use_form(&profile)
		.deps((1_u64,))
		.reset_on_deps(ResetOnDeps::KeepDirtyValues)
		.keep_errors(false)
		.revalidate_on(RevalidateOn::DepsChange)
		.build();

	runtime.set_value(profile.display_name_field(), "Grace".to_string());

	let mut refreshed = runtime.default_values();
	refreshed.display_name = "Katherine".to_string();
	refreshed.bio = "NASA mathematician".to_string();

	runtime.reconcile_defaults(refreshed, (2_u64,));

	assert_eq!(runtime.get_values().display_name, "Grace".to_string());
	assert_eq!(runtime.get_values().bio, "NASA mathematician".to_string());
	assert_eq!(
		runtime.default_values().display_name,
		"Katherine".to_string()
	);
}

#[test]
fn subscriptions_receive_value_and_submit_events() {
	let profile = form! {
		name: ProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
			}
		}
	};
	let runtime = use_form(&profile).build();
	let event_count = Rc::new(Cell::new(0));
	let event_count_for_subscription = Rc::clone(&event_count);
	let profile_for_subscription = profile.clone();
	let _subscription = runtime.subscribe(move |event| match event {
		FormEvent::ValueChanged { field } => {
			assert_eq!(field, profile_for_subscription.display_name_field());
			event_count_for_subscription.set(event_count_for_subscription.get() + 1);
		}
		FormEvent::Submitted => {
			event_count_for_subscription.set(event_count_for_subscription.get() + 1);
		}
		_ => {}
	});

	runtime.set_value(profile.display_name_field(), "Grace".to_string());
	assert_eq!(runtime.handle_submit(), UseFormSubmitOutcome::Submitted);
	assert_eq!(event_count.get(), 2);
}

#[test]
fn watch_field_tracks_runtime_value_changes() {
	let profile = form! {
		name: ProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
			}
		}
	};
	let runtime = use_form(&profile).build();
	let display_name = runtime.watch_field::<String>(profile.display_name_field());

	assert_eq!(display_name.get(), "Ada".to_string());

	runtime.set_value(profile.display_name_field(), "Grace".to_string());

	assert_eq!(display_name.get(), "Grace".to_string());
	assert_eq!(runtime.watch().get().display_name, "Grace".to_string());
}

#[test]
fn validation_failure_sets_form_error_and_submit_failure_state() {
	let signup = form! {
		name: SignupForm,
		action: "/signup",
		fields: {
			email: CharField {
				initial: "",
				required,
			}
		}
	};
	let runtime = use_form(&signup).build();

	assert!(runtime.trigger().is_err());
	assert!(runtime.form_state().form_error.get().is_none());
	assert_eq!(
		runtime
			.get_field_state(signup.email_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("email is required")
	);
	assert_eq!(
		runtime.handle_submit(),
		UseFormSubmitOutcome::ValidationFailed
	);
	assert!(!runtime.form_state().is_submitting.get());
	assert!(!runtime.form_state().is_submit_successful.get());
	assert!(runtime.form_state().error.get().is_some());

	runtime.set_value(signup.email_field(), "ada@example.com".to_string());

	assert!(runtime.trigger_field(signup.email_field()).is_ok());
	assert!(runtime.form_state().error.get().is_none());
}

#[tokio::test]
async fn submit_async_success_updates_state_and_runs_callbacks() {
	let profile = form! {
		name: AsyncSuccessForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
				required,
			}
		}
	};
	let order = Rc::new(Cell::new(0));
	let start_order = Rc::clone(&order);
	let success_order = Rc::clone(&order);
	let runtime = use_form(&profile)
		.on_submit_start(move |handle| {
			assert!(handle.form_state().is_submitting.get());
			assert_eq!(start_order.get(), 0);
			start_order.set(1);
		})
		.on_submit_success(move |handle| {
			assert!(!handle.form_state().is_submitting.get());
			assert!(handle.form_state().is_submit_successful.get());
			let callback_signal = Signal::new(1_i32);
			assert_eq!(callback_signal.get(), 1);
			assert_eq!(success_order.get(), 1);
			success_order.set(2);
		})
		.build();

	let outcome = runtime
		.submit_async(|| async { Ok::<_, String>("saved".to_string()) })
		.await
		.expect("async submit should succeed");

	assert_eq!(
		outcome,
		UseFormAsyncSubmitOutcome::Submitted("saved".to_string())
	);
	assert!(!runtime.form_state().is_submitting.get());
	assert!(runtime.form_state().is_submit_successful.get());
	assert!(runtime.form_state().submit_error.get().is_none());
	assert!(runtime.form_state().error.get().is_none());
	assert_eq!(order.get(), 2);
}

#[tokio::test]
async fn submit_server_fn_returns_submitted_outcome() {
	let profile = form! {
		name: TypedAsyncSuccessForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada"
			},
		}
	};
	let runtime = use_form(&profile).build();

	let outcome = runtime
		.submit_server_fn(|| async { Ok::<_, ServerFnError>("saved".to_string()) })
		.await
		.expect("typed server-function submit should succeed");

	assert_eq!(
		outcome,
		UseFormAsyncSubmitOutcome::Submitted("saved".to_string())
	);
}

#[rstest::rstest]
#[tokio::test]
async fn submit_server_fn_routes_typed_server_errors() {
	// Arrange
	let profile = form! {
		name: TypedAsyncServerErrorForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada"
			},
		}
	};
	let runtime = use_form(&profile).build();

	// Act
	let result = runtime
		.submit_server_fn(|| async {
			Err::<(), _>(ServerFnError::validation([(
				"display_name",
				"Display name is already used",
			)]))
		})
		.await;

	// Assert
	assert!(matches!(result, Err(error) if error.kind() == ServerFnErrorKind::Validation));
	assert_eq!(
		runtime
			.get_field_state(profile.display_name_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("Display name is already used")
	);
}

#[tokio::test]
async fn submit_async_validation_failure_blocks_submit_closure() {
	let signup = form! {
		name: AsyncValidationForm,
		action: "/signup",
		fields: {
			email: CharField {
				initial: "",
				required,
			}
		}
	};
	let submit_calls = Rc::new(Cell::new(0));
	let submit_calls_for_closure = Rc::clone(&submit_calls);
	let runtime = use_form(&signup).build();

	let outcome = runtime
		.submit_async(move || {
			submit_calls_for_closure.set(submit_calls_for_closure.get() + 1);
			async { Ok::<_, String>(()) }
		})
		.await
		.expect("validation failure is reported as an outcome");

	assert_eq!(outcome, UseFormAsyncSubmitOutcome::ValidationFailed);
	assert_eq!(submit_calls.get(), 0);
	assert!(!runtime.form_state().is_submitting.get());
	assert!(!runtime.form_state().is_submit_successful.get());
	assert_eq!(
		runtime
			.get_field_state(signup.email_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("email is required")
	);
}

#[tokio::test]
async fn submit_async_submit_error_records_error_text() {
	let profile = form! {
		name: AsyncSubmitErrorForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
				required,
			}
		}
	};
	let error_count = Rc::new(Cell::new(0));
	let error_count_for_callback = Rc::clone(&error_count);
	let runtime = use_form(&profile)
		.on_submit_error(move |handle| {
			assert!(!handle.form_state().is_submitting.get());
			assert!(!handle.form_state().is_submit_successful.get());
			assert_eq!(
				handle.form_state().submit_error.get().as_deref(),
				Some("network unavailable")
			);
			error_count_for_callback.set(error_count_for_callback.get() + 1);
		})
		.build();

	let result = runtime
		.submit_async(|| async { Err::<(), _>("network unavailable".to_string()) })
		.await;

	assert_eq!(result, Err("network unavailable".to_string()));
	assert_eq!(
		runtime.form_state().submit_error.get().as_deref(),
		Some("network unavailable")
	);
	assert_eq!(
		runtime.form_state().error.get().as_deref(),
		Some("network unavailable")
	);
	assert!(!runtime.form_state().is_submitting.get());
	assert!(!runtime.form_state().is_submit_successful.get());
	assert_eq!(error_count.get(), 1);
}

#[tokio::test]
async fn submit_async_returns_already_pending_for_reentrant_submit() {
	let profile = form! {
		name: AsyncPendingForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
				required,
			}
		}
	};
	let runtime = use_form(&profile).build();
	let runtime_for_inner = runtime.clone();

	let outcome = runtime
		.submit_async(move || {
			let runtime_for_inner = runtime_for_inner.clone();
			async move {
				runtime_for_inner
					.submit_async(|| async { Ok::<_, String>("inner".to_string()) })
					.await
			}
		})
		.await
		.expect("outer async submit should succeed");

	assert_eq!(
		outcome,
		UseFormAsyncSubmitOutcome::Submitted(UseFormAsyncSubmitOutcome::AlreadyPending)
	);
	assert!(!runtime.form_state().is_submitting.get());
	assert!(runtime.form_state().is_submit_successful.get());
}

#[test]
fn submit_async_cancellation_clears_submitting_state() {
	let profile = form! {
		name: AsyncCancelledForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
				required,
			}
		}
	};
	let runtime = use_form(&profile).build();

	let mut submit = Box::pin(
		runtime.submit_async(|| async { std::future::pending::<Result<(), String>>().await }),
	);
	let mut context = Context::from_waker(Waker::noop());

	assert!(matches!(submit.as_mut().poll(&mut context), Poll::Pending));
	assert!(runtime.form_state().is_submitting.get());

	drop(submit);

	assert!(!runtime.form_state().is_submitting.get());
	assert!(!runtime.form_state().is_submit_successful.get());
}

#[test]
fn use_form_accepts_file_field_runtime_contracts() {
	let upload = form! {
		name: UploadForm,
		action: "/upload",
		fields: {
			document: FileField {
				required,
			}
			avatar: ImageField {}
		}
	};

	let runtime = use_form(&upload).build();
	let document = runtime.watch_field::<Option<web_sys::File>>(upload.document_field());
	let avatar = runtime.watch_field::<Option<web_sys::File>>(upload.avatar_field());

	assert!(runtime.get_values().document.is_none());
	assert!(runtime.get_values().avatar.is_none());
	assert!(document.get().is_none());
	assert!(avatar.get().is_none());
	assert!(!runtime.form_state().is_dirty.get());
	assert!(!runtime.get_field_state(upload.document_field()).is_dirty);

	let result = runtime.trigger();

	assert!(result.is_err());
	assert_eq!(
		runtime
			.get_field_state(upload.document_field())
			.error
			.as_ref()
			.map(FieldError::message),
		Some("document is required")
	);

	runtime.set_value(upload.document_field(), None::<web_sys::File>);

	assert!(runtime.get_field_state(upload.document_field()).is_touched);
	assert!(!runtime.get_field_state(upload.document_field()).is_dirty);
}

#[test]
fn submit_callbacks_run_in_order_after_dependencies_are_configured() {
	let profile = form! {
		name: ProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
			}
		}
	};
	let order = Rc::new(Cell::new(0));
	let start_order = Rc::clone(&order);
	let success_order = Rc::clone(&order);
	let runtime = use_form(&profile)
		.deps(("tenant-a",))
		.on_submit_start(move |handle| {
			assert_eq!(start_order.get(), 0);
			assert_eq!(handle.get_values().display_name, "Ada".to_string());
			start_order.set(1);
		})
		.on_submit_success(move |handle| {
			assert_eq!(success_order.get(), 1);
			assert!(handle.form_state().is_submit_successful.get());
			success_order.set(2);
		})
		.build();

	assert_eq!(runtime.handle_submit(), UseFormSubmitOutcome::Submitted);
	assert_eq!(order.get(), 2);
}

#[test]
fn submit_callbacks_survive_deps_configured_after_callback_registration() {
	let profile = form! {
		name: ProfileForm,
		action: "/profile",
		fields: {
			display_name: CharField {
				initial: "Ada",
			}
		}
	};
	let success_count = Rc::new(Cell::new(0));
	let success_count_for_callback = Rc::clone(&success_count);

	let runtime = use_form(&profile)
		.on_submit_success(move |handle| {
			assert_eq!(handle.get_values().display_name, "Ada".to_string());
			success_count_for_callback.set(success_count_for_callback.get() + 1);
		})
		.deps(("tenant-a",))
		.build();

	assert_eq!(runtime.handle_submit(), UseFormSubmitOutcome::Submitted);
	assert_eq!(success_count.get(), 1);
}

#[test]
fn use_form_action_validates_before_dispatching() {
	ReactiveScope::run(|| {
		let signup = form! {
			name: SignupForm,
			action: "/signup",
			fields: {
				email: CharField {
					initial: "",
					required,
				}
			}
		};
		let runtime = use_form(&signup).build();
		let dispatch_count = Rc::new(Cell::new(0));
		let dispatch_count_for_action = Rc::clone(&dispatch_count);
		let save = use_form_action(&runtime, move |_values| {
			dispatch_count_for_action.set(dispatch_count_for_action.get() + 1);
			async { Ok::<(), String>(()) }
		});

		assert_eq!(save.submit(), UseFormSubmitOutcome::ValidationFailed);
		assert_eq!(dispatch_count.get(), 0);
		assert!(!save.is_pending());
		assert!(!runtime.form_state().is_submit_successful.get());
		assert_eq!(save.error_message().as_deref(), Some("email is required"));
		assert_eq!(
			runtime
				.get_field_state(signup.email_field())
				.error
				.as_ref()
				.map(FieldError::message),
			Some("email is required")
		);
	});
}

#[test]
fn use_form_action_dispatches_current_values_after_validation() {
	ReactiveScope::run(|| {
		let profile = form! {
			name: ProfileForm,
			action: "/profile",
			fields: {
				display_name: CharField {
					initial: "Ada",
					required,
				}
			}
		};
		let runtime = use_form(&profile).build();
		runtime.set_value(profile.display_name_field(), "Grace".to_string());

		let captured_name = Rc::new(RefCell::new(None));
		let captured_name_for_action = Rc::clone(&captured_name);
		let save = use_form_action(&runtime, move |values| {
			*captured_name_for_action.borrow_mut() = Some(values.display_name);
			async { Ok::<(), String>(()) }
		});

		assert_eq!(save.submit(), UseFormSubmitOutcome::Submitted);
		assert_eq!(captured_name.borrow().as_deref(), Some("Grace"));
		assert!(!save.is_pending());
		assert!(save.error_message().is_none());
	});
}

#[test]
fn form_action_ui_preserves_submit_semantics_and_renders_validation_errors() {
	ReactiveScope::run(|| {
		let signup = form! {
			name: SignupForm,
			action: "/signup",
			fields: {
				email: CharField {
					initial: "",
					required,
				}
			}
		};
		let runtime = use_form(&signup).build();
		let save = use_form_action(&runtime, |_values| async { Ok::<(), String>(()) });
		let button = FormActionButton::new(save.clone(), Page::text("Save"))
			.attr("type", "button")
			.attr("aria-label", "Save signup");
		let panel = FormActionResultPanel::new(save.clone())
			.idle(|| Page::text("idle"))
			.validation_error(|message| Page::text(format!("validation:{message}")));
		let resource = use_resource(|| async { Ok::<(), String>(()) }, reinhardt_pages::deps![]);
		let latest = resource.latest_after_form(&save);
		let form_page = PageElement::new("form")
			.on(reinhardt_pages::EventType::Submit, save.submit_handler())
			.child(button)
			.into_page();

		assert_eq!(
			form_page.render_to_string(),
			r#"<form><button type="submit" formnovalidate="formnovalidate" aria-label="Save signup">Save</button></form>"#
		);
		assert_eq!(panel.render().render_to_string(), "idle");
		assert_eq!(latest.value(), None);

		assert_eq!(save.submit(), UseFormSubmitOutcome::ValidationFailed);
		assert_eq!(
			panel.render().render_to_string(),
			"validation:email is required"
		);
	});
}
