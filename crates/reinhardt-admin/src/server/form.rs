//! Shared admin form configuration resolution.

#[cfg(server)]
use crate::adapters::{AdminSite, FieldInfo, FieldType};
#[cfg(server)]
use crate::core::{ModelAdmin, resolve_form_fields};
#[cfg(server)]
use crate::server::relation::{resolve_relation, resolve_relation_configuration};
#[cfg(server)]
use crate::server::type_inference::{
	find_model_by_table_name, get_field_metadata, infer_admin_field_type, infer_required,
};
#[cfg(server)]
use crate::types::{
	AdminError, AdminResult, AdminWidget, Fieldset, FormFieldOverride, PrepopulatedField,
	RelationWidget,
};
#[cfg(server)]
use reinhardt_db::migrations::ModelMetadata;
#[cfg(server)]
use reinhardt_utils::utils_core::text::humanize_field_name;
#[cfg(server)]
use std::cmp::Reverse;
#[cfg(server)]
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Fully resolved form configuration shared by field rendering and mutations.
#[cfg(server)]
#[derive(Debug)]
pub(crate) struct ResolvedAdminForm {
	pub(crate) fields: Vec<FieldInfo>,
	pub(crate) fieldsets: Option<Vec<Fieldset>>,
	pub(crate) aliases: Vec<(String, String)>,
	pub(crate) prepopulated_fields: Vec<PrepopulatedField>,
}

#[cfg(server)]
struct ResolvedField {
	info: FieldInfo,
	original_type: FieldType,
	nullable: bool,
	registered: bool,
}

/// Resolve one admin form without runtime relation selections.
#[cfg(server)]
pub(crate) fn resolve_admin_form(
	site: &AdminSite,
	model_admin: &dyn ModelAdmin,
) -> AdminResult<ResolvedAdminForm> {
	let (configured_field_names, mut fieldsets) = resolve_form_fields(model_admin)?;
	let has_fieldsets = fieldsets.is_some();
	let model = find_model_by_table_name(model_admin.table_name());
	let relations = resolve_relation_configuration(site, model_admin)?;
	let selector_fields = model_admin
		.filter_horizontal()
		.into_iter()
		.chain(model_admin.filter_vertical())
		.map(str::to_owned)
		.collect::<Vec<_>>();
	let selector_field_set = selector_fields
		.iter()
		.map(String::as_str)
		.collect::<HashSet<_>>();
	let mut configured_field_names = configured_field_names;
	for field in &selector_fields {
		if !configured_field_names.iter().any(|name| name == field) {
			configured_field_names.push(field.clone());
		}
	}

	let mut field_names = Vec::with_capacity(configured_field_names.len() + relations.len());
	for name in configured_field_names {
		let canonical = canonical_form_field_name(&name, model.as_ref(), &relations);
		if !field_names.contains(&canonical) {
			field_names.push(canonical);
		}
	}
	for relation in &relations {
		if !field_names.contains(&relation.foreign_key.column_name) {
			field_names.push(relation.foreign_key.column_name.clone());
		}
	}

	if let Some(groups) = fieldsets.as_mut() {
		let mut grouped = Vec::new();
		for group in groups.iter_mut() {
			group.fields = group
				.fields
				.iter()
				.map(|name| canonical_form_field_name(name, model.as_ref(), &relations))
				.filter(|name| {
					if grouped.contains(name) {
						false
					} else {
						grouped.push(name.clone());
						true
					}
				})
				.collect();
		}
		if let Some(last) = groups.last_mut() {
			last.fields.extend(
				field_names
					.iter()
					.filter(|name| !grouped.contains(name))
					.cloned(),
			);
		}
	}

	let readonly_fields = model_admin.readonly_fields();
	let mut resolved_fields = Vec::with_capacity(field_names.len());
	for name in field_names {
		if selector_field_set.contains(name.as_str()) {
			let descriptor = resolve_relation(site, model_admin, &name)?;
			let info = FieldInfo {
				name: name.clone(),
				label: humanize_field_name(&name),
				field_type: FieldType::ManyToManySelector {
					layout: descriptor.layout,
					available: Vec::new(),
					selected: Vec::new(),
					has_more: false,
				},
				required: false,
				readonly: false,
				help_text: None,
				placeholder: None,
			};
			resolved_fields.push(ResolvedField {
				original_type: info.field_type.clone(),
				info,
				nullable: false,
				registered: true,
			});
			continue;
		}

		if let Some(relation) = relations.iter().find(|relation| {
			relation.foreign_key.logical_name == name || relation.foreign_key.column_name == name
		}) {
			let is_readonly = readonly_fields.contains(&name.as_str())
				|| readonly_fields.contains(&relation.foreign_key.logical_name.as_str())
				|| readonly_fields.contains(&relation.foreign_key.column_name.as_str());
			let info = FieldInfo {
				name: relation.foreign_key.column_name.clone(),
				label: humanize_field_name(&relation.foreign_key.logical_name),
				field_type: FieldType::Relation {
					field_name: relation.foreign_key.logical_name.clone(),
					widget: relation.widget,
					selected: None,
					readonly: is_readonly,
				},
				required: infer_required(&relation.foreign_key.field_metadata),
				readonly: is_readonly,
				help_text: None,
				placeholder: None,
			};
			resolved_fields.push(ResolvedField {
				original_type: info.field_type.clone(),
				info,
				nullable: relation.foreign_key.field_metadata.is_nullable(),
				registered: true,
			});
			continue;
		}

		let metadata = get_field_metadata(model_admin.table_name(), &name);
		let (field_type, required, nullable) = if has_fieldsets {
			let metadata = metadata.as_ref().ok_or_else(|| {
				AdminError::ValidationError(format!(
					"Fieldset field '{}' is not registered for model '{}'",
					name,
					model_admin.model_name()
				))
			})?;
			(
				infer_admin_field_type(&metadata.field_type),
				infer_required(metadata),
				metadata.is_nullable(),
			)
		} else if let Some(metadata) = metadata.as_ref() {
			(
				infer_admin_field_type(&metadata.field_type),
				infer_required(metadata),
				metadata.is_nullable(),
			)
		} else {
			(FieldType::Text, false, true)
		};
		let info = FieldInfo {
			name: name.clone(),
			label: humanize_field_name(&name),
			field_type,
			required,
			readonly: readonly_fields.contains(&name.as_str()),
			help_text: None,
			placeholder: None,
		};
		resolved_fields.push(ResolvedField {
			original_type: info.field_type.clone(),
			info,
			nullable,
			registered: metadata.is_some(),
		});
	}

	let aliases = collect_field_aliases(model.as_ref(), &relations);
	apply_overrides(
		&mut resolved_fields,
		model.as_ref(),
		&relations,
		&model_admin.formfield_overrides(),
		"Form field override",
	)?;
	if let Some(form) = model_admin.form() {
		apply_overrides(
			&mut resolved_fields,
			model.as_ref(),
			&relations,
			&form.schema(),
			"Custom form schema",
		)?;
	}
	let prepopulated_fields = resolve_prepopulated_fields(
		&resolved_fields,
		model.as_ref(),
		&relations,
		&model_admin.prepopulated_fields(),
	)?;

	Ok(ResolvedAdminForm {
		fields: resolved_fields
			.into_iter()
			.map(|field| field.info)
			.collect(),
		fieldsets,
		aliases,
		prepopulated_fields,
	})
}

#[cfg(server)]
fn canonical_form_field_name(
	name: &str,
	model: Option<&ModelMetadata>,
	relations: &[crate::server::relation::ResolvedRelationField],
) -> String {
	if let Some(relation) = relations.iter().find(|relation| {
		relation.foreign_key.logical_name == name || relation.foreign_key.column_name == name
	}) {
		return relation.foreign_key.column_name.clone();
	}
	let Some(model) = model else {
		return name.to_owned();
	};
	model
		.fields
		.iter()
		.find_map(|(column, metadata)| {
			(column == name
				|| metadata
					.params
					.get("rust_field_name")
					.is_some_and(|value| value == name)
				|| metadata
					.params
					.get("logical_name")
					.is_some_and(|value| value == name)
				|| metadata
					.params
					.get("db_column")
					.is_some_and(|value| value == name))
			.then(|| {
				metadata
					.params
					.get("rust_field_name")
					.or_else(|| metadata.params.get("logical_name"))
					.cloned()
					.unwrap_or_else(|| column.clone())
			})
		})
		.unwrap_or_else(|| name.to_owned())
}

#[cfg(server)]
fn collect_field_aliases(
	model: Option<&ModelMetadata>,
	relations: &[crate::server::relation::ResolvedRelationField],
) -> Vec<(String, String)> {
	let mut aliases = Vec::new();
	let mut seen = HashSet::new();
	if let Some(model) = model {
		for (column, metadata) in &model.fields {
			let physical = metadata
				.params
				.get("db_column")
				.cloned()
				.unwrap_or_else(|| column.clone());
			let logical = metadata
				.params
				.get("rust_field_name")
				.or_else(|| metadata.params.get("logical_name"))
				.cloned()
				.unwrap_or_else(|| column.clone());
			if logical != physical && seen.insert((logical.clone(), physical.clone())) {
				aliases.push((logical, physical));
			}
		}
	}
	for relation in relations {
		let logical = relation.foreign_key.logical_name.clone();
		let physical = relation.foreign_key.column_name.clone();
		if logical != physical && seen.insert((logical.clone(), physical.clone())) {
			aliases.push((logical, physical));
		}
	}
	aliases
}

#[cfg(server)]
fn apply_overrides(
	fields: &mut [ResolvedField],
	model: Option<&ModelMetadata>,
	relations: &[crate::server::relation::ResolvedRelationField],
	overrides: &[FormFieldOverride],
	layer: &str,
) -> AdminResult<()> {
	let mut seen = HashSet::new();
	for override_ in overrides {
		let canonical = canonical_form_field_name(&override_.field, model, relations);
		if !seen.insert(canonical.clone()) {
			return Err(AdminError::ValidationError(format!(
				"{layer} '{}' duplicates canonical field '{}'",
				override_.field, canonical
			)));
		}
		let field = fields
			.iter_mut()
			.find(|field| field.info.name == canonical && field.registered)
			.ok_or_else(|| {
				AdminError::ValidationError(format!(
					"{layer} '{}' is not included in the resolved form",
					override_.field
				))
			})?;
		if let Some(widget) = override_.widget.as_ref() {
			field.info.field_type = replace_widget(&field.info.name, &field.original_type, widget)?;
		}
		if let Some(label) = override_.label.as_ref() {
			field.info.label.clone_from(label);
		}
		if let Some(help_text) = override_.help_text.as_ref() {
			field.info.help_text = Some(help_text.clone());
		}
		if let Some(placeholder) = override_.placeholder.as_ref() {
			field.info.placeholder = Some(placeholder.clone());
		}
		if let Some(required) = override_.required {
			if !required && !field.nullable {
				return Err(AdminError::ValidationError(format!(
					"{layer} '{}' cannot make a non-null model field optional",
					override_.field
				)));
			}
			field.info.required = required;
		}
		validate_widget_options(
			&field.info.name,
			&field.info.field_type,
			field.info.required,
		)?;
	}
	Ok(())
}

#[cfg(server)]
fn replace_widget(
	field_name: &str,
	original: &FieldType,
	widget: &AdminWidget,
) -> AdminResult<FieldType> {
	let incompatible = || {
		Err(AdminError::ValidationError(format!(
			"Widget {} is incompatible with form field '{}'",
			widget_name(widget),
			field_name
		)))
	};
	match (original, widget) {
		(FieldType::Text | FieldType::TextArea { .. }, AdminWidget::TextInput) => {
			Ok(FieldType::Text)
		}
		(FieldType::Text | FieldType::TextArea { .. }, AdminWidget::TextArea { rows }) => {
			if rows.is_some_and(|rows| rows == 0) {
				return Err(AdminError::ValidationError(format!(
					"Textarea field '{}' must use at least one row",
					field_name
				)));
			}
			Ok(FieldType::TextArea { rows: *rows })
		}
		(
			FieldType::Text | FieldType::TextArea { .. } | FieldType::Email,
			AdminWidget::Select { choices },
		) => Ok(FieldType::Select {
			choices: choices.clone(),
		}),
		(
			FieldType::Text | FieldType::TextArea { .. } | FieldType::Email,
			AdminWidget::HiddenInput,
		) => Ok(FieldType::Hidden),
		(FieldType::Email, AdminWidget::EmailInput) => Ok(FieldType::Email),
		(FieldType::Number, AdminWidget::NumberInput) => Ok(FieldType::Number),
		(
			FieldType::Number | FieldType::Boolean | FieldType::Date | FieldType::DateTime,
			AdminWidget::Select { choices },
		) => Ok(FieldType::Select {
			choices: choices.clone(),
		}),
		(
			FieldType::Number | FieldType::Boolean | FieldType::Date | FieldType::DateTime,
			AdminWidget::HiddenInput,
		) => Ok(FieldType::Hidden),
		(FieldType::Boolean, AdminWidget::Checkbox) => Ok(FieldType::Boolean),
		(FieldType::Date, AdminWidget::DateInput) => Ok(FieldType::Date),
		(FieldType::DateTime, AdminWidget::DateTimeInput) => Ok(FieldType::DateTime),
		(FieldType::Select { .. }, AdminWidget::Select { choices }) => Ok(FieldType::Select {
			choices: choices.clone(),
		}),
		(FieldType::MultiSelect { .. }, AdminWidget::MultiSelect { choices }) => {
			Ok(FieldType::MultiSelect {
				choices: choices.clone(),
			})
		}
		(
			FieldType::Relation {
				field_name,
				selected,
				readonly,
				..
			},
			AdminWidget::Autocomplete,
		) => Ok(FieldType::Relation {
			field_name: field_name.clone(),
			widget: RelationWidget::Autocomplete,
			selected: selected.clone(),
			readonly: *readonly,
		}),
		(
			FieldType::Relation {
				field_name,
				selected,
				readonly,
				..
			},
			AdminWidget::RawId,
		) => Ok(FieldType::Relation {
			field_name: field_name.clone(),
			widget: RelationWidget::RawId,
			selected: selected.clone(),
			readonly: *readonly,
		}),
		(FieldType::ManyToManySelector { .. }, AdminWidget::ManyToMany { layout }) => {
			Ok(FieldType::ManyToManySelector {
				layout: *layout,
				available: Vec::new(),
				selected: Vec::new(),
				has_more: false,
			})
		}
		(FieldType::File, AdminWidget::FileInput) => Ok(FieldType::File),
		(FieldType::Hidden, AdminWidget::HiddenInput) => Ok(FieldType::Hidden),
		_ => incompatible(),
	}
}

#[cfg(server)]
fn widget_name(widget: &AdminWidget) -> &'static str {
	match widget {
		AdminWidget::TextInput => "TextInput",
		AdminWidget::EmailInput => "EmailInput",
		AdminWidget::NumberInput => "NumberInput",
		AdminWidget::Checkbox => "Checkbox",
		AdminWidget::DateInput => "DateInput",
		AdminWidget::DateTimeInput => "DateTimeInput",
		AdminWidget::TextArea { .. } => "TextArea",
		AdminWidget::Select { .. } => "Select",
		AdminWidget::MultiSelect { .. } => "MultiSelect",
		AdminWidget::Autocomplete => "Autocomplete",
		AdminWidget::RawId => "RawId",
		AdminWidget::ManyToMany { .. } => "ManyToMany",
		AdminWidget::FileInput => "FileInput",
		AdminWidget::HiddenInput => "HiddenInput",
	}
}

#[cfg(server)]
fn validate_widget_options(
	field_name: &str,
	field_type: &FieldType,
	required: bool,
) -> AdminResult<()> {
	let (kind, choices) = match field_type {
		FieldType::Select { choices } => ("Select", choices),
		FieldType::MultiSelect { choices } => ("MultiSelect", choices),
		_ => return Ok(()),
	};
	let mut values = HashSet::new();
	for (value, _) in choices {
		if !values.insert(value) {
			return Err(AdminError::ValidationError(format!(
				"{kind} field '{field_name}' contains duplicate choice value '{value}'"
			)));
		}
		if required && value.is_empty() {
			return Err(AdminError::ValidationError(format!(
				"{kind} field '{field_name}' cannot contain an empty choice value when required"
			)));
		}
	}
	Ok(())
}

#[cfg(server)]
fn resolve_prepopulated_fields(
	fields: &[ResolvedField],
	model: Option<&ModelMetadata>,
	relations: &[crate::server::relation::ResolvedRelationField],
	rules: &[PrepopulatedField],
) -> AdminResult<Vec<PrepopulatedField>> {
	let field_indexes = fields
		.iter()
		.enumerate()
		.map(|(index, field)| (field.info.name.as_str(), index))
		.collect::<HashMap<_, _>>();
	let mut targets = HashSet::new();
	let mut normalized = Vec::with_capacity(rules.len());
	for rule in rules {
		let target = canonical_form_field_name(&rule.target, model, relations);
		let target_index = field_indexes.get(target.as_str()).ok_or_else(|| {
			AdminError::ValidationError(format!(
				"Prepopulated target '{}' is not included in the resolved form",
				rule.target
			))
		})?;
		let target_field = &fields[*target_index];
		if target_field.info.readonly {
			return Err(AdminError::ValidationError(format!(
				"Prepopulated target '{}' cannot be readonly",
				rule.target
			)));
		}
		if !is_text_field(&target_field.info.field_type) {
			return Err(AdminError::ValidationError(format!(
				"Prepopulated target '{}' must use a text field",
				rule.target
			)));
		}
		if !targets.insert(target.clone()) {
			return Err(AdminError::ValidationError(format!(
				"Prepopulated target '{}' is configured more than once",
				rule.target
			)));
		}
		if rule.sources.is_empty() {
			return Err(AdminError::ValidationError(format!(
				"Prepopulated target '{}' must have at least one source",
				rule.target
			)));
		}
		let mut sources = Vec::with_capacity(rule.sources.len());
		let mut seen_sources = HashSet::new();
		for source in &rule.sources {
			let source_name = canonical_form_field_name(source, model, relations);
			let source_index = field_indexes.get(source_name.as_str()).ok_or_else(|| {
				AdminError::ValidationError(format!(
					"Prepopulated source '{}' is not included in the resolved form",
					source
				))
			})?;
			if !seen_sources.insert(source_name.clone()) {
				return Err(AdminError::ValidationError(format!(
					"Prepopulated target '{}' contains duplicate source '{}'",
					rule.target, source
				)));
			}
			if source_name == target {
				return Err(AdminError::ValidationError(format!(
					"Prepopulated target '{}' cannot use itself as a source",
					rule.target
				)));
			}
			if !is_text_field(&fields[*source_index].info.field_type) {
				return Err(AdminError::ValidationError(format!(
					"Prepopulated source '{}' must use a text field",
					source
				)));
			}
			sources.push(source_name);
		}
		normalized.push(PrepopulatedField { target, sources });
	}

	let rule_indexes = normalized
		.iter()
		.enumerate()
		.map(|(index, rule)| (rule.target.as_str(), index))
		.collect::<HashMap<_, _>>();
	let mut indegrees = vec![0usize; normalized.len()];
	let mut dependents = vec![Vec::new(); normalized.len()];
	for (target_index, rule) in normalized.iter().enumerate() {
		for source in &rule.sources {
			if let Some(source_index) = rule_indexes.get(source.as_str()) {
				dependents[*source_index].push(target_index);
				indegrees[target_index] += 1;
			}
		}
	}
	let mut ready = BinaryHeap::new();
	for (index, indegree) in indegrees.iter().enumerate() {
		if *indegree == 0 {
			ready.push(Reverse(index));
		}
	}
	let mut ordered = Vec::with_capacity(normalized.len());
	while let Some(Reverse(index)) = ready.pop() {
		ordered.push(normalized[index].clone());
		for dependent in &dependents[index] {
			indegrees[*dependent] -= 1;
			if indegrees[*dependent] == 0 {
				ready.push(Reverse(*dependent));
			}
		}
	}
	if ordered.len() != normalized.len() {
		return Err(AdminError::ValidationError(
			"Prepopulated fields contain a cycle".to_owned(),
		));
	}
	Ok(ordered)
}

#[cfg(server)]
fn is_text_field(field_type: &FieldType) -> bool {
	matches!(
		field_type,
		FieldType::Text | FieldType::TextArea { .. } | FieldType::Email
	)
}

#[cfg(all(test, server))]
mod tests {
	use super::resolve_admin_form;
	use crate::core::{AdminForm, AdminSite, ModelAdminConfig};
	use crate::types::{
		AdminWidget, FieldType, FormFieldOverride, PrepopulatedField, RelationOption,
		RelationSelectorLayout, RelationWidget,
	};
	use reinhardt_db::migrations::{
		FieldMetadata, FieldType as DatabaseFieldType, ModelMetadata, global_registry,
	};
	use serial_test::serial;
	use std::sync::Arc;

	const APP_LABEL: &str = "admin_form_resolver";
	const MODEL_NAME: &str = "Article";
	const TABLE_NAME: &str = "admin_form_resolver_articles";

	struct RegistryGuard;

	impl Drop for RegistryGuard {
		fn drop(&mut self) {
			global_registry().remove_model(APP_LABEL, MODEL_NAME);
		}
	}

	#[derive(Debug)]
	struct SchemaForm {
		overrides: Vec<FormFieldOverride>,
	}

	impl AdminForm for SchemaForm {
		fn schema(&self) -> Vec<FormFieldOverride> {
			self.overrides.clone()
		}
	}

	fn register_article_metadata() -> RegistryGuard {
		let mut model = ModelMetadata::new(APP_LABEL, MODEL_NAME, TABLE_NAME);
		model.add_field(
			"title".to_owned(),
			FieldMetadata::new(DatabaseFieldType::VarChar(200)),
		);
		model.add_field(
			"body".to_owned(),
			FieldMetadata::new(DatabaseFieldType::Text),
		);
		model.add_field(
			"summary".to_owned(),
			FieldMetadata::new(DatabaseFieldType::VarChar(200)).with_nullable(true),
		);
		model.add_field(
			"rank".to_owned(),
			FieldMetadata::new(DatabaseFieldType::Integer),
		);
		model.add_field(
			"slug".to_owned(),
			FieldMetadata::new(DatabaseFieldType::VarChar(200)),
		);
		model.add_field(
			"seo_slug".to_owned(),
			FieldMetadata::new(DatabaseFieldType::VarChar(200)),
		);
		model.add_field(
			"headline_col".to_owned(),
			FieldMetadata::new(DatabaseFieldType::VarChar(200))
				.with_param("rust_field_name", "headline")
				.with_param("db_column", "headline_col"),
		);
		global_registry().register_model(model);
		RegistryGuard
	}

	fn admin(fields: Vec<&str>) -> ModelAdminConfig {
		ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(fields)
			.allow_all(true)
			.build()
			.expect("admin configuration should build")
	}

	#[test]
	#[serial(admin_form_resolver)]
	fn resolves_inferred_fields_in_configured_order() {
		let _registry = register_article_metadata();
		let site = AdminSite::new("Form resolver test");
		let admin = admin(vec!["title", "body", "summary"]);

		let form = resolve_admin_form(&site, &admin).expect("inferred form should resolve");

		assert_eq!(
			form.fields
				.iter()
				.map(|field| field.name.as_str())
				.collect::<Vec<_>>(),
			vec!["title", "body", "summary"]
		);
		assert_eq!(form.fields[0].field_type, FieldType::Text);
		assert!(form.fields[0].required);
		assert_eq!(
			form.fields[1].field_type,
			FieldType::TextArea { rows: None }
		);
		assert!(!form.fields[2].required);
		assert_eq!(form.fieldsets, None);
		assert_eq!(
			form.aliases,
			vec![("headline".to_owned(), "headline_col".to_owned())]
		);
		assert!(form.prepopulated_fields.is_empty());
	}

	#[test]
	#[serial(admin_form_resolver)]
	fn custom_schema_overlays_only_its_explicit_properties_after_admin_overrides() {
		let _registry = register_article_metadata();
		let site = AdminSite::new("Form resolver test");
		let form_adapter = Arc::new(SchemaForm {
			overrides: vec![
				FormFieldOverride::new("title")
					.label("Custom title")
					.widget(AdminWidget::TextArea { rows: Some(7) }),
			],
		});
		let admin = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title"])
			.formfield_overrides(vec![
				FormFieldOverride::new("title")
					.label("Admin title")
					.help_text("Admin help")
					.placeholder("Set a title"),
			])
			.form(form_adapter)
			.allow_all(true)
			.build()
			.expect("admin configuration should build");

		let form = resolve_admin_form(&site, &admin).expect("custom schema should resolve");

		assert_eq!(form.fields[0].label, "Custom title");
		assert_eq!(form.fields[0].help_text.as_deref(), Some("Admin help"));
		assert_eq!(form.fields[0].placeholder.as_deref(), Some("Set a title"));
		assert_eq!(
			form.fields[0].field_type,
			FieldType::TextArea { rows: Some(7) }
		);
	}

	#[test]
	#[serial(admin_form_resolver)]
	fn aliases_canonicalize_overrides_and_reject_duplicate_layers() {
		let _registry = register_article_metadata();
		let site = AdminSite::new("Form resolver test");
		let configured = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["headline"])
			.formfield_overrides(vec![
				FormFieldOverride::new("headline_col").label("Headline"),
			])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");

		let form = resolve_admin_form(&site, &configured).expect("physical alias should resolve");

		assert_eq!(form.fields[0].name, "headline");
		assert_eq!(form.fields[0].label, "Headline");
		assert_eq!(
			form.aliases,
			vec![("headline".to_owned(), "headline_col".to_owned())]
		);

		let duplicate = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["headline"])
			.formfield_overrides(vec![
				FormFieldOverride::new("headline"),
				FormFieldOverride::new("headline_col"),
			])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");

		let error = resolve_admin_form(&site, &duplicate)
			.expect_err("canonical duplicate must be rejected");
		assert_eq!(
			error.to_string(),
			"Validation error: Form field override 'headline_col' duplicates canonical field 'headline'"
		);
	}

	#[test]
	#[serial(admin_form_resolver)]
	fn rejects_omitted_unknown_and_requiredness_weakening_overrides() {
		let _registry = register_article_metadata();
		let site = AdminSite::new("Form resolver test");
		let omitted = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title"])
			.formfield_overrides(vec![FormFieldOverride::new("summary")])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let omitted_error =
			resolve_admin_form(&site, &omitted).expect_err("omitted field override must fail");
		assert_eq!(
			omitted_error.to_string(),
			"Validation error: Form field override 'summary' is not included in the resolved form"
		);

		let unknown = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title"])
			.formfield_overrides(vec![FormFieldOverride::new("missing")])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let unknown_error =
			resolve_admin_form(&site, &unknown).expect_err("unknown override must fail");
		assert_eq!(
			unknown_error.to_string(),
			"Validation error: Form field override 'missing' is not included in the resolved form"
		);

		let weakening = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title"])
			.formfield_overrides(vec![FormFieldOverride::new("title").required(false)])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let required_error = resolve_admin_form(&site, &weakening)
			.expect_err("non-null requiredness cannot be weakened");
		assert_eq!(
			required_error.to_string(),
			"Validation error: Form field override 'title' cannot make a non-null model field optional"
		);

		let strengthened = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["summary"])
			.formfield_overrides(vec![FormFieldOverride::new("summary").required(true)])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let form =
			resolve_admin_form(&site, &strengthened).expect("nullable field may be strengthened");
		assert!(form.fields[0].required);
	}

	#[test]
	#[serial(admin_form_resolver)]
	fn validates_widget_options_and_categories() {
		let _registry = register_article_metadata();
		let site = AdminSite::new("Form resolver test");
		let invalid_rows = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["body"])
			.formfield_overrides(vec![
				FormFieldOverride::new("body").widget(AdminWidget::TextArea { rows: Some(0) }),
			])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let rows_error =
			resolve_admin_form(&site, &invalid_rows).expect_err("zero textarea rows must fail");
		assert_eq!(
			rows_error.to_string(),
			"Validation error: Textarea field 'body' must use at least one row"
		);

		let duplicate_choices = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title"])
			.formfield_overrides(vec![FormFieldOverride::new("title").widget(
				AdminWidget::Select {
					choices: vec![
						("draft".to_owned(), "Draft".to_owned()),
						("draft".to_owned(), "Again".to_owned()),
					],
				},
			)])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let choices_error = resolve_admin_form(&site, &duplicate_choices)
			.expect_err("duplicate choice values must fail");
		assert_eq!(
			choices_error.to_string(),
			"Validation error: Select field 'title' contains duplicate choice value 'draft'"
		);

		let required_empty_choice = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title"])
			.formfield_overrides(vec![FormFieldOverride::new("title").widget(
				AdminWidget::Select {
					choices: vec![(String::new(), "Choose".to_owned())],
				},
			)])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let empty_error = resolve_admin_form(&site, &required_empty_choice)
			.expect_err("required select cannot contain empty choice");
		assert_eq!(
			empty_error.to_string(),
			"Validation error: Select field 'title' cannot contain an empty choice value when required"
		);

		let optional_empty_choice = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["summary"])
			.formfield_overrides(vec![FormFieldOverride::new("summary").widget(
				AdminWidget::Select {
					choices: vec![(String::new(), "Choose".to_owned())],
				},
			)])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		assert!(resolve_admin_form(&site, &optional_empty_choice).is_ok());

		let incompatible = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["rank"])
			.formfield_overrides(vec![
				FormFieldOverride::new("rank").widget(AdminWidget::TextInput),
			])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let incompatible_error = resolve_admin_form(&site, &incompatible)
			.expect_err("numeric field cannot use text widget");
		assert_eq!(
			incompatible_error.to_string(),
			"Validation error: Widget TextInput is incompatible with form field 'rank'"
		);
	}

	#[test]
	fn relation_widgets_only_replace_matching_relation_categories() {
		let foreign_key = FieldType::Relation {
			field_name: "author".to_owned(),
			widget: RelationWidget::Autocomplete,
			selected: None,
			readonly: false,
		};
		let many_to_many = FieldType::ManyToManySelector {
			layout: RelationSelectorLayout::Horizontal,
			available: Vec::new(),
			selected: vec![RelationOption::new("1", "Existing")],
			has_more: false,
		};

		assert!(super::replace_widget("author", &foreign_key, &AdminWidget::RawId).is_ok());
		assert!(
			super::replace_widget(
				"tags",
				&many_to_many,
				&AdminWidget::ManyToMany {
					layout: RelationSelectorLayout::Vertical,
				},
			)
			.is_ok()
		);
		let foreign_key_error =
			super::replace_widget("author", &foreign_key, &AdminWidget::TextInput)
				.expect_err("foreign key cannot use scalar widget");
		assert_eq!(
			foreign_key_error.to_string(),
			"Validation error: Widget TextInput is incompatible with form field 'author'"
		);
		let many_to_many_error = super::replace_widget("tags", &many_to_many, &AdminWidget::RawId)
			.expect_err("many-to-many cannot use foreign-key widget");
		assert_eq!(
			many_to_many_error.to_string(),
			"Validation error: Widget RawId is incompatible with form field 'tags'"
		);
	}

	#[test]
	#[serial(admin_form_resolver)]
	fn validates_and_stably_orders_prepopulated_fields() {
		let _registry = register_article_metadata();
		let site = AdminSite::new("Form resolver test");
		let chained = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title", "summary", "slug", "seo_slug"])
			.prepopulated_fields(vec![
				PrepopulatedField::new("seo_slug", ["slug"]),
				PrepopulatedField::new("slug", ["title", "summary"]),
			])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");

		let form = resolve_admin_form(&site, &chained).expect("acyclic rules should resolve");
		assert_eq!(
			form.prepopulated_fields
				.iter()
				.map(|rule| rule.target.as_str())
				.collect::<Vec<_>>(),
			vec!["slug", "seo_slug"]
		);

		let readonly = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title", "slug"])
			.readonly_fields(vec!["slug"])
			.prepopulated_fields(vec![PrepopulatedField::new("slug", ["title"])])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let readonly_error = resolve_admin_form(&site, &readonly)
			.expect_err("readonly prepopulation target must fail");
		assert_eq!(
			readonly_error.to_string(),
			"Validation error: Prepopulated target 'slug' cannot be readonly"
		);

		let non_text_target = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title", "rank"])
			.prepopulated_fields(vec![PrepopulatedField::new("rank", ["title"])])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let non_text_target_error = resolve_admin_form(&site, &non_text_target)
			.expect_err("non-text prepopulation target must fail");
		assert_eq!(
			non_text_target_error.to_string(),
			"Validation error: Prepopulated target 'rank' must use a text field"
		);

		let invalid_source = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["rank", "slug"])
			.prepopulated_fields(vec![PrepopulatedField::new("slug", ["rank"])])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let invalid_source_error = resolve_admin_form(&site, &invalid_source)
			.expect_err("non-text prepopulation source must fail");
		assert_eq!(
			invalid_source_error.to_string(),
			"Validation error: Prepopulated source 'rank' must use a text field"
		);

		let duplicate_target = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title", "summary", "slug"])
			.prepopulated_fields(vec![
				PrepopulatedField::new("slug", ["title"]),
				PrepopulatedField::new("slug", ["summary"]),
			])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let duplicate_target_error = resolve_admin_form(&site, &duplicate_target)
			.expect_err("duplicate prepopulation target must fail");
		assert_eq!(
			duplicate_target_error.to_string(),
			"Validation error: Prepopulated target 'slug' is configured more than once"
		);

		let duplicate_source = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title", "slug"])
			.prepopulated_fields(vec![PrepopulatedField::new("slug", ["title", "title"])])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let duplicate_source_error = resolve_admin_form(&site, &duplicate_source)
			.expect_err("duplicate prepopulation source must fail");
		assert_eq!(
			duplicate_source_error.to_string(),
			"Validation error: Prepopulated target 'slug' contains duplicate source 'title'"
		);

		let self_cycle = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["slug"])
			.prepopulated_fields(vec![PrepopulatedField::new("slug", ["slug"])])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let self_cycle_error =
			resolve_admin_form(&site, &self_cycle).expect_err("self-cycle must fail");
		assert_eq!(
			self_cycle_error.to_string(),
			"Validation error: Prepopulated target 'slug' cannot use itself as a source"
		);

		let cycle = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title", "slug", "seo_slug"])
			.prepopulated_fields(vec![
				PrepopulatedField::new("slug", ["seo_slug"]),
				PrepopulatedField::new("seo_slug", ["slug"]),
			])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let cycle_error = resolve_admin_form(&site, &cycle).expect_err("cyclic rules must fail");
		assert_eq!(
			cycle_error.to_string(),
			"Validation error: Prepopulated fields contain a cycle"
		);

		let independent = ModelAdminConfig::builder()
			.model_name(MODEL_NAME)
			.table_name(TABLE_NAME)
			.fields(vec!["title", "summary", "slug", "seo_slug"])
			.prepopulated_fields(vec![
				PrepopulatedField::new("seo_slug", ["summary"]),
				PrepopulatedField::new("slug", ["title"]),
			])
			.allow_all(true)
			.build()
			.expect("admin configuration should build");
		let independent_form =
			resolve_admin_form(&site, &independent).expect("independent rules should resolve");
		assert_eq!(
			independent_form
				.prepopulated_fields
				.iter()
				.map(|rule| rule.target.as_str())
				.collect::<Vec<_>>(),
			vec!["seo_slug", "slug"]
		);
	}
}
