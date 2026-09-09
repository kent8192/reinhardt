//! Independent reference values for the stable field and built-in widget matrix.

macro_rules! value_sync_form {
	() => {{
		let form = reinhardt_pages::form! {
			name: ValueSyncMatrix,
			action: "/value-sync",
			fields: {
				name: CharField { label: "Name", initial: "Ada" }
				bio: TextField { label: "Biography", initial: "Compiler engineer\nRust" }
				email: EmailField { label: "Email", initial: "ada@example.com" }
				password: PasswordField { label: "Password", initial: "correct-horse" }
				site: UrlField { label: "Website", initial: "https://example.com/ada" }
				slug: SlugField { label: "Slug", initial: "ada-lovelace" }
				count: IntegerField { label: "Count", initial: 42 }
				ratio: FloatField { label: "Ratio", initial: 1.25, step: "any" }
				price: DecimalField { label: "Price", initial: 12.5, step: "any" }
				subscribed: BooleanField { label: "Subscribe", initial: true }
				day: DateField {
					label: "Date", initial: chrono::NaiveDate::from_ymd_opt(2026, 9, 7),
				}
				time: TimeField {
					label: "Time", initial: chrono::NaiveTime::from_hms_opt(9, 30, 15), step: 1,
				}
				starts_at: DateTimeField {
					label: "Starts at",
					initial: chrono::NaiveDate::from_ymd_opt(2026, 9, 7)
						.and_then(|day| day.and_hms_opt(9, 30, 15)),
					step: 1,
				}
				attachment: FileField { label: "Attachment" }
				image: ImageField { label: "Image" }
				identifier: UuidField {
					label: "UUID",
					initial: Some(uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()),
				}
				address: IpAddressField {
					label: "IP address", initial: Some("192.0.2.7".parse::<std::net::IpAddr>().unwrap()),
				}
				token: HiddenField<String> { initial: "fixture-hidden" }
				status: ChoiceField<String> {
					label: "Status", initial: "review",
					choices: [("open", "Open"), ("review", "In review")],
				}
				tags: MultipleChoiceField<String> {
					label: "Tags", initial: vec![String::from("rust"), String::from("wasm")],
					choices: [("rust", "Rust"), ("web", "Web"), ("wasm", "WASM")],
				}
				preferences: JsonField<serde_json::Value> {
					label: "Preferences", initial: serde_json::json!({"theme": "dark"}),
				}
				radio: ChoiceField<String> {
					label: "Radio status", widget: RadioSelect, initial: "review",
					choices_from: "radio_options", choice_value: "value", choice_label: "label",
				}
				color: CharField { label: "Color", widget: ColorInput, initial: "#336699" }
				level: IntegerField {
					label: "Level", widget: RangeInput, initial: 25, min: 0, max: 100,
				}
				schedule: FieldGroup {
					label: "Grouped controls",
					fields: {
						phone: CharField { label: "Telephone", widget: TelInput, initial: "+81-3-1234-5678" }
						search: CharField { label: "Search", widget: SearchInput, initial: "rust forms" }
						month: CharField { label: "Month", widget: MonthInput, initial: "2026-09" }
						week: CharField { label: "Week", widget: WeekInput, initial: "2026-W37" }
					}
				}
				typed_choice: ChoiceField<i64> {
					label: "Typed choice", initial: 2_i64,
					choices: [(1_i64, "One"), (2_i64, "Two")],
				}
				unbound: CharField { label: "Unbound snapshot", initial: "Snapshot", bind: false }
			}
		};
		form.radio_choices().set(vec![
			(String::from("open"), String::from("Open")),
			(String::from("review"), String::from("In review")),
		]);
		form
	}};
}

pub(crate) use value_sync_form;

/// Field name, native control category, and the independently specified DOM value.
/// Multiple selections use comma-separated option values; radio uses its checked value.
pub(crate) const INITIAL_VALUES: &[(&str, &str, &str)] = &[
	("name", "text", "Ada"),
	("bio", "textarea", "Compiler engineer\nRust"),
	("email", "email", "ada@example.com"),
	("password", "password", "correct-horse"),
	("site", "url", "https://example.com/ada"),
	("slug", "text", "ada-lovelace"),
	("count", "number", "42"),
	("ratio", "number", "1.25"),
	("price", "number", "12.5"),
	("subscribed", "checkbox", "true"),
	("day", "date", "2026-09-07"),
	("time", "time", "09:30:15"),
	("starts_at", "datetime-local", "2026-09-07T09:30:15"),
	("attachment", "file", ""),
	("image", "file", ""),
	("identifier", "text", "550e8400-e29b-41d4-a716-446655440000"),
	("address", "text", "192.0.2.7"),
	("token", "hidden", "fixture-hidden"),
	("status", "select-one", "review"),
	("tags", "select-multiple", "rust,wasm"),
	("preferences", "textarea", r#"{"theme":"dark"}"#),
	("radio", "radio", "review"),
	("color", "color", "#336699"),
	("level", "range", "25"),
	("phone", "tel", "+81-3-1234-5678"),
	("search", "search", "rust forms"),
	("month", "month", "2026-09"),
	("week", "week", "2026-W37"),
	("typed_choice", "select-one", "2"),
	("unbound", "text", "Snapshot"),
];

/// Every writable bound control starts from a different DOM value before reset.
#[cfg(target_arch = "wasm32")]
pub(crate) const EDITED_VALUES: &[(&str, &str, &str)] = &[
	("name", "text", "Grace"),
	("bio", "textarea", "Rear admiral\nCOBOL"),
	("email", "email", "grace@example.com"),
	("password", "password", "another-password"),
	("site", "url", "https://example.com/grace"),
	("slug", "text", "grace-hopper"),
	("count", "number", "7"),
	("ratio", "number", "2.75"),
	("price", "number", "18.25"),
	("subscribed", "checkbox", "false"),
	("day", "date", "2026-10-08"),
	("time", "time", "14:45:20"),
	("starts_at", "datetime-local", "2026-10-08T14:45:20"),
	("identifier", "text", "550e8400-e29b-41d4-a716-446655440001"),
	("address", "text", "2001:db8::8"),
	("token", "hidden", "changed-hidden"),
	("status", "select-one", "open"),
	("tags", "select-multiple", "web"),
	("preferences", "textarea", r#"{"theme":"light"}"#),
	("radio", "radio", "open"),
	("color", "color", "#996633"),
	("level", "range", "75"),
	("phone", "tel", "+1-202-555-0108"),
	("search", "search", "typed state"),
	("month", "month", "2026-10"),
	("week", "week", "2026-W41"),
	("typed_choice", "select-one", "1"),
];
