//! Advanced FormSet functionality
//!
//! This module provides advanced FormSet features including inline formsets,
//! model-based formsets, and dynamic formset generation.

use crate::formset::FormSet;
use crate::model_form::{FormModel, ModelForm, ModelFormError, ModelFormPersistenceMode};
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormFieldKind, ModelFormPolicy, ModelFormPrimaryKey,
	ModelFormPrimaryKeyFields, ModelFormSchema,
};
use reinhardt_db::orm::OrmExecutor;
use reinhardt_db::orm::transaction::AtomicTransactionOutcome;
use serde::Serialize;
use std::marker::PhantomData;

/// InlineFormSet for managing forms related to a parent model
///
/// InlineFormSets are used to edit related objects together with a parent object,
/// similar to Django's inline formsets for admin.
pub struct InlineFormSet<P: FormModel, C: FormModel> {
	parent: P,
	parent_persistence_mode: ModelFormPersistenceMode,
	pending_parent_save: Option<(AtomicTransactionOutcome, P, ModelFormPersistenceMode)>,
	_formset: FormSet,
	fk_field: String,
	child_forms: Vec<ModelForm<C, AllEditableModelFields>>,
	_phantom_parent: PhantomData<P>,
	_phantom_child: PhantomData<C>,
}

impl<P: FormModel, C: FormModel> InlineFormSet<P, C> {
	/// Creates an inline formset using the legacy primary-key heuristic.
	///
	/// A missing primary key or numeric zero sentinel selects create; any other
	/// primary key selects update. Models with assigned primary keys, including
	/// UUIDs, must use [`Self::for_create`] because this constructor cannot
	/// distinguish a new assigned identifier from an existing row.
	///
	/// # Arguments
	///
	/// * `parent` - The parent model instance
	/// * `fk_field` - The foreign key field name on the child model
	///
	/// # Examples
	///
	/// ```ignore
	/// let parent = Author { id: 1, name: "John".to_string() };
	/// let formset = InlineFormSet::new(parent, "author_id".to_string());
	/// ```
	pub fn new(parent: P, fk_field: String) -> Self {
		let primary_key = parent.primary_key();
		let uses_zero_sentinel = P::primary_key_uses_zero_sentinel()
			&& primary_key
				.as_ref()
				.is_some_and(|value| value.to_string() == "0");
		let mode = if primary_key.is_some() && !uses_zero_sentinel {
			ModelFormPersistenceMode::Update
		} else {
			ModelFormPersistenceMode::Create
		};
		Self::with_parent_mode(parent, fk_field, mode)
	}

	/// Creates an inline formset that inserts the parent before saving children.
	pub fn for_create(parent: P, fk_field: String) -> Self {
		Self::with_parent_mode(parent, fk_field, ModelFormPersistenceMode::Create)
	}

	/// Creates an inline formset that updates the parent before saving children.
	pub fn for_update(parent: P, fk_field: String) -> Self {
		Self::with_parent_mode(parent, fk_field, ModelFormPersistenceMode::Update)
	}

	fn with_parent_mode(
		parent: P,
		fk_field: String,
		parent_persistence_mode: ModelFormPersistenceMode,
	) -> Self {
		Self {
			parent,
			parent_persistence_mode,
			pending_parent_save: None,
			_formset: FormSet::new("inline".to_string()),
			fk_field,
			child_forms: Vec::new(),
			_phantom_parent: PhantomData,
			_phantom_child: PhantomData,
		}
	}

	/// Add a child form to the formset
	pub fn add_child_form(&mut self, form: ModelForm<C, AllEditableModelFields>) {
		self.child_forms.push(form);
	}

	/// Get the parent model instance
	pub fn parent(&self) -> &P {
		&self.parent
	}

	/// Get the foreign key field name
	pub fn fk_field(&self) -> &str {
		&self.fk_field
	}

	/// Get all child forms
	pub fn child_forms(&self) -> &[ModelForm<C, AllEditableModelFields>] {
		&self.child_forms
	}

	/// Save the formset and all related child instances.
	///
	/// This method saves the parent model first, retrieves the parent's primary
	/// key, sets the foreign key on each child instance, then saves each child.
	///
	/// # Errors
	///
	/// Returns an error if any save operation fails, the child foreign-key field
	/// cannot accept the parent's primary-key kind, or the parent model does not
	/// expose a primary key after saving.
	pub async fn save(&mut self, executor: &mut dyn OrmExecutor) -> Result<(), ModelFormError>
	where
		P::PrimaryKey: Serialize,
		P: ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
	{
		self.prepare_children()?;
		let parent_before_save = self.parent.clone();
		let parent_mode_before_save = self.parent_persistence_mode;
		if let Err(error) =
			FormModel::save_with_mode(&mut self.parent, executor, self.parent_persistence_mode)
				.await
		{
			if matches!(error, ModelFormError::PersistenceAfterCreate { .. }) {
				if let Some(outcome) = executor.transaction_outcome() {
					self.pending_parent_save =
						Some((outcome, parent_before_save, parent_mode_before_save));
				} else {
					self.parent_persistence_mode = ModelFormPersistenceMode::Update;
				}
			}
			return Err(error);
		}
		if let Some(outcome) = executor.transaction_outcome() {
			self.pending_parent_save = Some((outcome, parent_before_save, parent_mode_before_save));
		} else {
			self.parent_persistence_mode = ModelFormPersistenceMode::Update;
		}
		self.save_prepared_children(executor).await
	}

	/// Saves child forms using the parent's existing primary key without persisting the parent.
	pub async fn save_children(
		&mut self,
		executor: &mut dyn OrmExecutor,
	) -> Result<(), ModelFormError>
	where
		P::PrimaryKey: Serialize,
		P: ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
	{
		self.prepare_children()?;
		self.save_prepared_children(executor).await
	}

	/// Validates child forms and returns candidates with the trusted parent key assigned.
	///
	/// This is useful when a caller must apply additional persistence predicates
	/// while retaining the inline formset's relationship and form validation.
	pub fn prepare_child_instances(&mut self) -> Result<Vec<C>, ModelFormError>
	where
		P::PrimaryKey: Serialize,
		P: ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
	{
		self.prepare_children()?;
		self.child_forms
			.iter_mut()
			.map(ModelForm::build_instance)
			.collect()
	}

	fn prepare_children(&mut self) -> Result<(), ModelFormError>
	where
		P::PrimaryKey: Serialize,
		P: ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
	{
		self.finalize_parent_transaction()?;
		self.validate_foreign_key_kind()?;
		for child_form in &mut self.child_forms {
			child_form.finalize_transaction()?;
		}
		if let Some(parent_id) = self.trusted_parent_primary_key()? {
			for child_form in &mut self.child_forms {
				child_form.set_trusted_field_value(&self.fk_field, parent_id.clone())?;
			}
		}
		if self.is_valid() {
			Ok(())
		} else {
			Err(ModelFormError::ModelValidation {
				errors: vec!["inline formset contains invalid child fields".to_string()],
			})
		}
	}

	fn trusted_parent_primary_key(&self) -> Result<Option<serde_json::Value>, ModelFormError>
	where
		P::PrimaryKey: Serialize,
		P: ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
	{
		let parent_id_is_zero_sentinel = P::primary_key_uses_zero_sentinel()
			&& self
				.parent
				.primary_key()
				.as_ref()
				.is_some_and(|value| value.to_string() == "0");
		self.parent
			.primary_key()
			.filter(|_| !parent_id_is_zero_sentinel)
			.map(|parent_id| self.serialize_parent_primary_key(parent_id))
			.transpose()
	}

	fn serialize_parent_primary_key(
		&self,
		parent_id: P::PrimaryKey,
	) -> Result<serde_json::Value, ModelFormError>
	where
		P::PrimaryKey: Serialize,
	{
		serde_json::to_value(parent_id).map_err(|error| ModelFormError::FieldValidation {
			errors: std::collections::HashMap::from([(
				self.fk_field.clone(),
				vec![error.to_string()],
			)]),
		})
	}

	async fn save_prepared_children(
		&mut self,
		executor: &mut dyn OrmExecutor,
	) -> Result<(), ModelFormError>
	where
		P::PrimaryKey: Serialize,
		P: ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
	{
		let parent_id =
			self.trusted_parent_primary_key()?
				.ok_or(ModelFormError::MissingModelField {
					field: P::primary_key_field(),
				})?;
		let fk_field = self.fk_field.clone();
		for child_form in &mut self.child_forms {
			if child_form.has_deferred_required_field(&fk_field) {
				child_form.set_deferred_trusted_field_value(&fk_field, parent_id.clone())?;
			} else {
				child_form.set_trusted_field_value(&fk_field, parent_id.clone())?;
			}
			child_form.save(executor).await?;
		}
		Ok(())
	}

	fn finalize_parent_transaction(&mut self) -> Result<(), ModelFormError> {
		let Some((outcome, parent_before_save, mode_before_save)) = self.pending_parent_save.take()
		else {
			return Ok(());
		};
		if outcome.is_committed() {
			self.parent_persistence_mode = ModelFormPersistenceMode::Update;
			return Ok(());
		}
		if outcome.is_rolled_back() {
			self.parent = parent_before_save;
			self.parent_persistence_mode = mode_before_save;
			return Ok(());
		}
		self.pending_parent_save = Some((outcome, parent_before_save, mode_before_save));
		Err(ModelFormError::TransactionOutcomePending)
	}

	fn validate_foreign_key_kind(&self) -> Result<(), ModelFormError>
	where
		P: ModelFormPrimaryKey + ModelFormPrimaryKeyFields + 'static,
	{
		let descriptor = C::Schema::fields()
			.iter()
			.find(|descriptor| descriptor.name == self.fk_field);
		let child = match descriptor {
			Some(descriptor) if !descriptor.generated_relation_id => {
				return Err(foreign_key_validation_error(
					&self.fk_field,
					"foreign key field is not a generated relationship identifier",
				));
			}
			Some(_descriptor) if !C::Schema::relation_target_matches::<P>(&self.fk_field) => {
				return Err(foreign_key_validation_error(
					&self.fk_field,
					"foreign key field does not target the parent model",
				));
			}
			Some(descriptor) => descriptor.kind,
			None => match C::trusted_relation_field_kind(&self.fk_field) {
				Some(kind) if C::Schema::relation_target_matches::<P>(&self.fk_field) => kind,
				Some(_) => {
					return Err(foreign_key_validation_error(
						&self.fk_field,
						"foreign key field does not target the parent model",
					));
				}
				None => {
					return Err(foreign_key_validation_error(
						&self.fk_field,
						"unknown trusted foreign key field",
					));
				}
			},
		};
		if !foreign_key_kinds_are_compatible(P::FIELD_KIND, child) {
			return Err(ModelFormError::FieldValidation {
				errors: std::collections::HashMap::from([(
					self.fk_field.clone(),
					vec![
						"foreign key field is incompatible with the parent primary key".to_owned(),
					],
				)]),
			});
		}
		Ok(())
	}

	fn can_defer_foreign_key_requiredness(&self) -> bool {
		match C::Schema::fields()
			.iter()
			.find(|descriptor| descriptor.name == self.fk_field)
		{
			Some(descriptor) => descriptor.generated_relation_id && descriptor.required,
			None => {
				C::trusted_relation_field_kind(&self.fk_field).is_some()
					&& C::trusted_relation_field_is_required(&self.fk_field)
			}
		}
	}

	/// Validate all child forms
	pub fn is_valid(&mut self) -> bool {
		let mut all_valid = true;
		let can_defer_foreign_key = self.can_defer_foreign_key_requiredness();

		for child_form in &mut self.child_forms {
			match child_form.build_instance() {
				Ok(_) => {}
				Err(ModelFormError::MissingModelField { field })
					if field == self.fk_field && can_defer_foreign_key =>
				{
					// The trusted parent key is assigned after the create parent has been saved,
					// but every other required child field must still be validated first.
					if !child_form.is_valid_with_deferred_required_field(&self.fk_field) {
						all_valid = false;
					}
				}
				Err(ModelFormError::FieldValidation { errors })
					if can_defer_foreign_key
						&& errors.len() == 1
						&& errors.get(&self.fk_field).is_some_and(|messages| {
							messages.len() == 1 && messages[0] == "This field is required."
						}) =>
				{
					if !child_form.is_valid_with_deferred_required_field(&self.fk_field) {
						all_valid = false;
					}
				}
				Err(_) => {
					child_form.is_valid();
					all_valid = false;
				}
			}
		}

		all_valid
	}
}

fn foreign_key_validation_error(field: &str, message: &str) -> ModelFormError {
	ModelFormError::FieldValidation {
		errors: std::collections::HashMap::from([(field.to_owned(), vec![message.to_owned()])]),
	}
}

fn foreign_key_kinds_are_compatible(parent: ModelFormFieldKind, child: ModelFormFieldKind) -> bool {
	std::mem::discriminant(&parent) == std::mem::discriminant(&child)
}

/// ModelFormSet for managing multiple model instances
///
/// This is similar to the base FormSet but specifically designed for model instances.
pub struct ModelFormSet<T, P = AllEditableModelFields>
where
	T: FormModel,
	P: ModelFormPolicy,
{
	forms: Vec<ModelForm<T, P>>,
	prefix: String,
	can_delete: bool,
	can_order: bool,
	extra: usize,
	max_num: Option<usize>,
	min_num: usize,
	errors: Vec<String>,
	_phantom: PhantomData<(T, P)>,
}

impl<T, P> ModelFormSet<T, P>
where
	T: FormModel,
	P: ModelFormPolicy,
{
	/// Create a new ModelFormSet
	///
	/// # Examples
	///
	/// ```ignore
	/// let formset = ModelFormSet::<User>::new("user".to_string());
	/// ```
	pub fn new(prefix: String) -> Self {
		Self {
			forms: Vec::new(),
			prefix,
			can_delete: false,
			can_order: false,
			extra: 1,
			max_num: Some(1000),
			min_num: 0,
			errors: Vec::new(),
			_phantom: PhantomData,
		}
	}

	/// Set extra forms count
	pub fn with_extra(mut self, extra: usize) -> Self {
		self.extra = extra;
		self
	}

	/// Enable deletion
	pub fn with_can_delete(mut self, can_delete: bool) -> Self {
		self.can_delete = can_delete;
		self
	}

	/// Enable ordering
	pub fn with_can_order(mut self, can_order: bool) -> Self {
		self.can_order = can_order;
		self
	}

	/// Set maximum number of forms
	pub fn with_max_num(mut self, max_num: Option<usize>) -> Self {
		self.max_num = max_num;
		self
	}

	/// Set minimum number of forms
	pub fn with_min_num(mut self, min_num: usize) -> Self {
		self.min_num = min_num;
		self
	}

	/// Add a model form to the formset.
	///
	/// Returns an error if adding the form would exceed `max_num`.
	pub fn add_form(&mut self, form: ModelForm<T, P>) -> Result<(), String> {
		if let Some(max) = self.max_num
			&& self.forms.len() >= max
		{
			return Err(format!(
				"Cannot add form: maximum number of forms ({}) reached",
				max
			));
		}
		self.forms.push(form);
		Ok(())
	}

	/// Get all forms
	pub fn forms(&self) -> &[ModelForm<T, P>] {
		&self.forms
	}

	/// Get mutable access to forms
	pub fn forms_mut(&mut self) -> &mut Vec<ModelForm<T, P>> {
		&mut self.forms
	}

	/// Validate all forms in the formset
	pub fn is_valid(&mut self) -> bool {
		let mut all_valid = true;
		for form in &mut self.forms {
			if form.is_submission_candidate() && !form.is_valid() {
				all_valid = false;
			}
		}

		self.validate_cardinality().is_ok() && all_valid
	}

	fn validate_cardinality(&mut self) -> Result<(), ModelFormError> {
		self.errors.clear();
		let candidate_count = self
			.forms
			.iter()
			.filter(|form| form.is_submission_candidate())
			.count();

		if candidate_count < self.min_num {
			self.errors
				.push(format!("Please submit at least {} forms", self.min_num));
		}

		if let Some(max) = self.max_num
			&& candidate_count > max
		{
			self.errors
				.push(format!("Please submit no more than {} forms", max));
		}

		if self.errors.is_empty() {
			Ok(())
		} else {
			Err(ModelFormError::ModelValidation {
				errors: self.errors.clone(),
			})
		}
	}

	/// Get validation errors
	pub fn errors(&self) -> &[String] {
		&self.errors
	}

	/// Save all forms in the formset
	pub async fn save(&mut self, executor: &mut dyn OrmExecutor) -> Result<Vec<T>, ModelFormError> {
		self.validate_cardinality()?;
		for form in &mut self.forms {
			if form.is_submission_candidate() {
				form.build_instance()?;
			}
		}
		let mut saved = Vec::with_capacity(
			self.forms
				.iter()
				.filter(|form| form.is_submission_candidate())
				.count(),
		);
		for form in &mut self.forms {
			if form.is_submission_candidate() {
				saved.push(form.save(executor).await?);
			}
		}

		Ok(saved)
	}

	/// Get the formset prefix
	pub fn prefix(&self) -> &str {
		&self.prefix
	}
}

/// Factory for creating FormSets dynamically
///
/// This allows you to create FormSets with different configurations
/// without defining them statically.
pub struct FormSetFactory {
	prefix: String,
	extra: usize,
	can_delete: bool,
	can_order: bool,
	max_num: Option<usize>,
	min_num: usize,
}

impl FormSetFactory {
	/// Create a new FormSetFactory
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FormSetFactory;
	///
	/// let factory = FormSetFactory::new("form".to_string());
	/// assert_eq!(factory.extra(), 1);
	/// ```
	pub fn new(prefix: String) -> Self {
		Self {
			prefix,
			extra: 1,
			can_delete: false,
			can_order: false,
			max_num: Some(1000),
			min_num: 0,
		}
	}

	/// Set extra forms count
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FormSetFactory;
	///
	/// let factory = FormSetFactory::new("form".to_string())
	///     .with_extra(3);
	/// assert_eq!(factory.extra(), 3);
	/// ```
	pub fn with_extra(mut self, extra: usize) -> Self {
		self.extra = extra;
		self
	}

	/// Enable deletion
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FormSetFactory;
	///
	/// let factory = FormSetFactory::new("form".to_string())
	///     .with_can_delete(true);
	/// assert!(factory.can_delete());
	/// ```
	pub fn with_can_delete(mut self, can_delete: bool) -> Self {
		self.can_delete = can_delete;
		self
	}

	/// Enable ordering
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FormSetFactory;
	///
	/// let factory = FormSetFactory::new("form".to_string())
	///     .with_can_order(true);
	/// assert!(factory.can_order());
	/// ```
	pub fn with_can_order(mut self, can_order: bool) -> Self {
		self.can_order = can_order;
		self
	}

	/// Set maximum number of forms
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FormSetFactory;
	///
	/// let factory = FormSetFactory::new("form".to_string())
	///     .with_max_num(Some(10));
	/// assert_eq!(factory.max_num(), Some(10));
	/// ```
	pub fn with_max_num(mut self, max_num: Option<usize>) -> Self {
		self.max_num = max_num;
		self
	}

	/// Set minimum number of forms
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FormSetFactory;
	///
	/// let factory = FormSetFactory::new("form".to_string())
	///     .with_min_num(2);
	/// assert_eq!(factory.min_num(), 2);
	/// ```
	pub fn with_min_num(mut self, min_num: usize) -> Self {
		self.min_num = min_num;
		self
	}

	/// Get extra forms count
	pub fn extra(&self) -> usize {
		self.extra
	}

	/// Check if deletion is enabled
	pub fn can_delete(&self) -> bool {
		self.can_delete
	}

	/// Check if ordering is enabled
	pub fn can_order(&self) -> bool {
		self.can_order
	}

	/// Get maximum number of forms
	pub fn max_num(&self) -> Option<usize> {
		self.max_num
	}

	/// Get minimum number of forms
	pub fn min_num(&self) -> usize {
		self.min_num
	}

	/// Create a FormSet from this factory
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{FormSetFactory, FormSet};
	///
	/// let factory = FormSetFactory::new("form".to_string())
	///     .with_extra(3)
	///     .with_can_delete(true);
	///
	/// let formset = factory.create();
	/// assert_eq!(formset.prefix(), "form");
	/// assert!(formset.can_delete());
	/// ```
	pub fn create(&self) -> FormSet {
		FormSet::new(self.prefix.clone())
			.with_extra(self.extra)
			.with_can_delete(self.can_delete)
			.with_can_order(self.can_order)
			.with_max_num(self.max_num)
			.with_min_num(self.min_num)
	}

	/// Create a ModelFormSet from this factory
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::FormSetFactory;
	///
	/// let factory = FormSetFactory::new("user".to_string())
	///     .with_extra(2);
	///
	/// let formset = factory.create_model_formset::<User>();
	/// ```
	pub fn create_model_formset<T>(&self) -> ModelFormSet<T, AllEditableModelFields>
	where
		T: FormModel,
	{
		ModelFormSet::new(self.prefix.clone())
			.with_extra(self.extra)
			.with_can_delete(self.can_delete)
			.with_can_order(self.can_order)
			.with_max_num(self.max_num)
			.with_min_num(self.min_num)
	}

	/// Creates a model formset using an explicit generated field policy.
	pub fn create_model_formset_with_policy<T, P>(&self) -> ModelFormSet<T, P>
	where
		T: FormModel,
		P: ModelFormPolicy,
	{
		ModelFormSet::new(self.prefix.clone())
			.with_extra(self.extra)
			.with_can_delete(self.can_delete)
			.with_can_order(self.can_order)
			.with_max_num(self.max_num)
			.with_min_num(self.min_num)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::VecDeque;
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};

	use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
	use reinhardt_core::validators::{ValidationError, ValidationErrors};
	use reinhardt_db::associations::ForeignKeyField;
	use reinhardt_db::orm::connection::{
		DatabaseBackend, OrmExecutor, QueryResult, QueryValue, Row,
	};
	use reinhardt_macros::model;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use serde_json::json;
	use serial_test::serial;

	static REQUIRED_CHILD_GENERATED_VALIDATOR_CALLS: AtomicUsize = AtomicUsize::new(0);

	struct AtomicUsizeResetGuard {
		counter: &'static AtomicUsize,
	}

	impl AtomicUsizeResetGuard {
		fn new(counter: &'static AtomicUsize) -> Self {
			counter.store(0, Ordering::SeqCst);
			Self { counter }
		}
	}

	impl Drop for AtomicUsizeResetGuard {
		fn drop(&mut self) {
			self.counter.store(0, Ordering::SeqCst);
		}
	}

	// Test model implementation
	#[model(
		app_label = "forms",
		table_name = "advanced_formset_test_models",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct TestModel {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 100)]
		name: String,
		#[field(max_length = 254)]
		email: String,
	}

	#[model(
		app_label = "forms",
		table_name = "advanced_formset_default_models",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct AllDefaultModel {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 100, default = "generated")]
		name: String,
		#[field(default = true)]
		enabled: bool,
	}

	// Test child model
	#[model(
		app_label = "forms",
		table_name = "advanced_formset_child_models",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct ChildModel {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[rel(foreign_key, related_name = "child_models", null = true)]
		parent: ForeignKeyField<TestModel>,
		#[field(max_length = 1_000)]
		content: String,
	}

	fn validate_required_child<P: ModelFormPolicy>(
		payload: &CleanedRequiredChildModelModelFormData<P>,
	) -> Result<(), ValidationErrors> {
		REQUIRED_CHILD_GENERATED_VALIDATOR_CALLS.fetch_add(1, Ordering::SeqCst);
		let mut errors = ValidationErrors::new();
		if payload
			.content()
			.is_some_and(|content| content == "blocked child")
			&& payload.parent_id().is_none()
		{
			errors.add(
				"_all",
				ValidationError::Custom(
					"generated child validation requires the parent key".to_owned(),
				),
			);
		}
		if payload
			.content()
			.is_some_and(|content| content == "valid child")
			&& payload.parent_id().is_some()
		{
			errors.add(
				"_all",
				ValidationError::Custom(
					"generated validation reran after parent key injection".to_owned(),
				),
			);
		}
		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}

	#[model(
		app_label = "forms",
		table_name = "advanced_formset_required_child_models",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	#[form(validate = validate_required_child)]
	struct RequiredChildModel {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[rel(foreign_key, related_name = "required_child_models")]
		parent: ForeignKeyField<TestModel>,
		#[field(max_length = 1_000)]
		#[form(trim)]
		content: String,
	}

	#[model(
		app_label = "forms",
		table_name = "advanced_formset_scalar_child_models",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct ScalarChildModel {
		#[field(primary_key = true)]
		id: Option<i64>,
		position: i64,
		#[field(max_length = 1_000)]
		content: String,
	}

	#[model(
		app_label = "forms",
		table_name = "advanced_formset_alternate_parents",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct AlternateParent {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 100)]
		name: String,
	}

	#[model(
		app_label = "forms",
		table_name = "advanced_formset_alternate_children",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct AlternateChildModel {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[rel(foreign_key, related_name = "alternate_child_models")]
		parent: ForeignKeyField<AlternateParent>,
		#[field(max_length = 1_000)]
		content: String,
	}

	#[model(
		app_label = "forms",
		table_name = "advanced_formset_uuid_parents",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct UuidParent {
		#[field(primary_key = true, include_in_new = false)]
		id: uuid::Uuid,
		#[field(max_length = 100)]
		name: String,
	}

	#[model(
		app_label = "forms",
		table_name = "advanced_formset_uuid_children",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct UuidChild {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[rel(foreign_key, related_name = "uuid_children", null = true)]
		parent: ForeignKeyField<UuidParent>,
		#[field(max_length = 1_000)]
		content: String,
	}

	#[derive(Debug)]
	struct FormsetExecutor {
		rows: VecDeque<Result<Row, Error>>,
		fetch_one_calls: usize,
		queries: Vec<String>,
	}

	impl FormsetExecutor {
		fn new(rows: impl IntoIterator<Item = Result<Row, Error>>) -> Self {
			Self {
				rows: rows.into_iter().collect(),
				fetch_one_calls: 0,
				queries: Vec::new(),
			}
		}
	}

	#[reinhardt_core::async_trait]
	impl OrmExecutor for FormsetExecutor {
		fn backend(&self) -> DatabaseBackend {
			DatabaseBackend::Postgres
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<QueryResult, Error> {
			Err(DatabaseError::new(DatabaseErrorKind::Query, "unexpected execute call").into())
		}

		async fn fetch_one(&mut self, sql: &str, _params: Vec<QueryValue>) -> Result<Row, Error> {
			self.fetch_one_calls += 1;
			self.queries.push(sql.to_owned());
			self.rows.pop_front().unwrap_or_else(|| {
				Err(DatabaseError::new(DatabaseErrorKind::Query, "missing queued row").into())
			})
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Vec<Row>, Error> {
			Err(DatabaseError::new(DatabaseErrorKind::Query, "unexpected fetch_all call").into())
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>, Error> {
			Err(
				DatabaseError::new(DatabaseErrorKind::Query, "unexpected fetch_optional call")
					.into(),
			)
		}
	}

	fn test_model(id: i64, name: &str) -> TestModel {
		TestModel {
			id: Some(id),
			name: name.to_owned(),
			email: format!("{name}@example.com"),
		}
	}

	fn test_model_row(id: i64, name: &str) -> Row {
		let mut row = Row::new();
		row.insert("id".to_owned(), QueryValue::Int(id));
		row.insert("name".to_owned(), QueryValue::String(name.to_owned()));
		row.insert(
			"email".to_owned(),
			QueryValue::String(format!("{name}@example.com")),
		);
		row
	}

	fn child_model_row(id: i64, parent_id: i64, content: &str) -> Row {
		let mut row = Row::new();
		row.insert("id".to_owned(), QueryValue::Int(id));
		row.insert("parent_id".to_owned(), QueryValue::Int(parent_id));
		row.insert("content".to_owned(), QueryValue::String(content.to_owned()));
		row
	}

	fn uuid_parent_row(id: uuid::Uuid, name: &str) -> Row {
		let mut row = Row::new();
		row.insert("id".to_owned(), QueryValue::Uuid(id));
		row.insert("name".to_owned(), QueryValue::String(name.to_owned()));
		row
	}

	fn uuid_child_row(id: i64, parent_id: uuid::Uuid, content: &str) -> Row {
		let mut row = Row::new();
		row.insert("id".to_owned(), QueryValue::Int(id));
		row.insert("parent_id".to_owned(), QueryValue::Uuid(parent_id));
		row.insert("content".to_owned(), QueryValue::String(content.to_owned()));
		row
	}

	fn model_form(instance: TestModel) -> ModelForm<TestModel> {
		ModelForm::from_payload_and_instance(
			TestModelModelFormData::<AllEditableModelFields>::empty(),
			instance,
		)
	}

	#[test]
	fn test_inline_formset_creation() {
		let parent = TestModel {
			id: Some(1),
			name: "Parent".to_string(),
			email: "parent@example.com".to_string(),
		};

		let formset = InlineFormSet::<TestModel, ChildModel>::new(parent, "parent_id".to_string());

		assert_eq!(formset.fk_field(), "parent_id");
		assert_eq!(formset.parent().name, "Parent");
		assert_eq!(formset.child_forms().len(), 0);
	}

	#[test]
	fn test_inline_formset_add_child() {
		let parent = TestModel {
			id: Some(1),
			name: "Parent".to_string(),
			email: "parent@example.com".to_string(),
		};

		let mut formset =
			InlineFormSet::<TestModel, ChildModel>::new(parent, "parent_id".to_string());

		let child = ChildModel {
			id: None,
			parent: ForeignKeyField::new(),
			parent_id: None,
			content: "Child content".to_string(),
		};
		let child_form = ModelForm::from_payload_and_instance(
			ChildModelModelFormData::<AllEditableModelFields>::empty(),
			child,
		);
		formset.add_child_form(child_form);

		assert_eq!(formset.child_forms().len(), 1);
	}

	#[test]
	fn test_inline_formset_save_assigns_parent_primary_key() {
		let parent = test_model(1, "parent");
		let mut formset =
			InlineFormSet::<TestModel, ChildModel>::new(parent, "parent_id".to_owned());
		let mut data = ChildModelModelFormData::<AllEditableModelFields>::empty();
		data.set_content("Child content".to_owned());
		formset.add_child_form(ModelForm::from_payload(data));
		let mut executor = FormsetExecutor::new([
			Ok(test_model_row(1, "parent")),
			Ok(child_model_row(2, 1, "Child content")),
		]);

		assert!(formset.is_valid());
		tokio_test::block_on(formset.save(&mut executor)).unwrap();

		let saved_child = formset.child_forms()[0].instance().unwrap();
		assert_eq!(saved_child.parent_id, Some(1));
		assert_eq!(executor.fetch_one_calls, 2);
	}

	#[test]
	fn test_inline_formset_save_children_assigns_trusted_foreign_key_without_saving_parent() {
		let parent = test_model(1, "parent");
		let mut formset =
			InlineFormSet::<TestModel, ChildModel>::for_create(parent, "parent_id".to_owned());
		let mut data = ChildModelModelFormData::<AllEditableModelFields>::empty();
		data.set_content("Child content".to_owned());
		formset.add_child_form(ModelForm::from_payload(data));
		let mut executor = FormsetExecutor::new([Ok(child_model_row(2, 1, "Child content"))]);

		tokio_test::block_on(formset.save_children(&mut executor))
			.expect("child-only saving should use the trusted parent key");

		assert_eq!(formset.parent().id, Some(1));
		assert_eq!(
			formset.child_forms()[0].instance().unwrap().parent_id,
			Some(1)
		);
		assert_eq!(executor.fetch_one_calls, 1);
		assert_eq!(
			executor.queries[0].split_whitespace().next(),
			Some("INSERT")
		);
	}

	#[test]
	fn test_inline_formset_prepares_validated_children_with_the_trusted_parent_key() {
		let parent = test_model(1, "parent");
		let mut formset =
			InlineFormSet::<TestModel, ChildModel>::for_update(parent, "parent_id".to_owned());
		let mut data = ChildModelModelFormData::<AllEditableModelFields>::empty();
		data.set_content("Child content".to_owned());
		formset.add_child_form(ModelForm::from_payload(data));

		let children = formset.prepare_child_instances().unwrap();

		assert_eq!(children.len(), 1);
		assert_eq!(children[0].parent_id, Some(1));
		assert_eq!(children[0].content, "Child content");
		assert!(formset.child_forms()[0].instance().is_none());
	}

	#[rstest]
	#[case::parent_and_children(true)]
	#[case::children_only(false)]
	#[serial(inline_generated_validator)]
	fn test_inline_formset_reuses_prevalidated_children_for_an_unchanged_parent_key(
		#[case] save_parent: bool,
	) {
		// Arrange
		let _generated_validator_calls_reset =
			AtomicUsizeResetGuard::new(&REQUIRED_CHILD_GENERATED_VALIDATOR_CALLS);
		let validator_calls = Arc::new(AtomicUsize::new(0));
		let validator_calls_for_candidate = Arc::clone(&validator_calls);
		let mut formset = InlineFormSet::<TestModel, RequiredChildModel>::for_update(
			test_model(1, "parent"),
			"parent_id".to_owned(),
		);
		let mut data = RequiredChildModelModelFormData::<AllEditableModelFields>::empty();
		data.set_content(" prevalidated child ".to_owned()).unwrap();
		formset.add_child_form(
			ModelForm::<RequiredChildModel>::from_payload(data).with_model_validator(move |_| {
				if validator_calls_for_candidate.fetch_add(1, Ordering::SeqCst) == 0 {
					Ok(())
				} else {
					Err(vec!["child validation ran more than once".to_owned()])
				}
			}),
		);
		let mut rows = Vec::new();
		if save_parent {
			rows.push(Ok(test_model_row(1, "parent")));
		}
		rows.push(Ok(child_model_row(2, 1, "prevalidated child")));
		let mut executor = FormsetExecutor::new(rows);

		// Act
		if save_parent {
			tokio_test::block_on(formset.save(&mut executor)).unwrap();
		} else {
			tokio_test::block_on(formset.save_children(&mut executor)).unwrap();
		}

		// Assert
		let saved_child = formset.child_forms()[0].instance().unwrap();
		assert_eq!(saved_child.parent_id, 1);
		assert_eq!(saved_child.content, "prevalidated child");
		assert_eq!(validator_calls.load(Ordering::SeqCst), 1);
		assert_eq!(
			REQUIRED_CHILD_GENERATED_VALIDATOR_CALLS.load(Ordering::SeqCst),
			1
		);
		assert_eq!(executor.fetch_one_calls, if save_parent { 2 } else { 1 });
	}

	#[rstest]
	#[serial(inline_generated_validator)]
	fn test_inline_formset_defers_model_validator_until_generated_parent_key_is_assigned() {
		let _generated_validator_calls_reset =
			AtomicUsizeResetGuard::new(&REQUIRED_CHILD_GENERATED_VALIDATOR_CALLS);
		let validator_calls = Arc::new(AtomicUsize::new(0));
		let validator_calls_for_candidate = Arc::clone(&validator_calls);
		let parent = TestModel {
			id: None,
			name: "parent".to_owned(),
			email: "parent@example.com".to_owned(),
		};
		let mut formset = InlineFormSet::<TestModel, RequiredChildModel>::for_create(
			parent,
			"parent_id".to_owned(),
		);
		let mut data = RequiredChildModelModelFormData::<AllEditableModelFields>::empty();
		data.set_content(" valid child ".to_owned())
			.expect("child content should be accepted");
		formset.add_child_form(
			ModelForm::<RequiredChildModel>::from_payload(data).with_model_validator(
				move |candidate| {
					validator_calls_for_candidate.fetch_add(1, Ordering::SeqCst);
					if candidate.parent_id == 1 {
						Ok(())
					} else {
						Err(vec!["parent key must be assigned".to_owned()])
					}
				},
			),
		);
		let structurally_valid = formset.is_valid();
		assert_eq!(
			formset.child_forms()[0].form().errors(),
			&std::collections::HashMap::new()
		);
		assert_eq!(structurally_valid, true);
		let mut executor = FormsetExecutor::new([
			Ok(test_model_row(1, "parent")),
			Ok(child_model_row(2, 1, "valid child")),
		]);

		tokio_test::block_on(formset.save(&mut executor))
			.expect("child validator should observe the generated parent key");

		assert_eq!(formset.child_forms()[0].instance().unwrap().parent_id, 1);
		assert_eq!(
			formset.child_forms()[0].instance().unwrap().content,
			"valid child"
		);
		assert_eq!(validator_calls.load(Ordering::SeqCst), 1);
		assert_eq!(
			REQUIRED_CHILD_GENERATED_VALIDATOR_CALLS.load(Ordering::SeqCst),
			1
		);
		assert_eq!(executor.fetch_one_calls, 2);
	}

	#[rstest]
	#[serial(inline_generated_validator)]
	fn test_inline_formset_generated_validator_rejects_before_parent_persistence() {
		// Arrange
		let _generated_validator_calls_reset =
			AtomicUsizeResetGuard::new(&REQUIRED_CHILD_GENERATED_VALIDATOR_CALLS);
		let model_validator_calls = Arc::new(AtomicUsize::new(0));
		let model_validator_calls_for_candidate = Arc::clone(&model_validator_calls);
		let parent = TestModel {
			id: None,
			name: "parent".to_owned(),
			email: "parent@example.com".to_owned(),
		};
		let mut formset = InlineFormSet::<TestModel, RequiredChildModel>::for_create(
			parent,
			"parent_id".to_owned(),
		);
		let mut data = RequiredChildModelModelFormData::<AllEditableModelFields>::empty();
		data.set_content(" blocked child ".to_owned())
			.expect("child content should be accepted");
		formset.add_child_form(
			ModelForm::<RequiredChildModel>::from_payload(data).with_model_validator(move |_| {
				model_validator_calls_for_candidate.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}),
		);
		let mut executor = FormsetExecutor::new([Ok(test_model_row(1, "parent"))]);

		// Act
		let error = tokio_test::block_on(formset.save(&mut executor))
			.expect_err("generated child validation should reject before parent persistence");

		// Assert
		assert_eq!(
			error,
			ModelFormError::ModelValidation {
				errors: vec!["inline formset contains invalid child fields".to_owned()],
			}
		);
		assert_eq!(
			formset.child_forms()[0].form().errors(),
			&std::collections::HashMap::from([(
				"_all".to_owned(),
				vec!["generated child validation requires the parent key".to_owned()],
			)])
		);
		assert_eq!(model_validator_calls.load(Ordering::SeqCst), 0);
		assert_eq!(
			REQUIRED_CHILD_GENERATED_VALIDATOR_CALLS.load(Ordering::SeqCst),
			1
		);
		assert_eq!(executor.fetch_one_calls, 0);
		assert_eq!(executor.queries, Vec::<String>::new());
	}

	#[rstest]
	fn test_inline_formset_rejects_editable_scalar_as_foreign_key() {
		let parent = test_model(1, "parent");
		let mut formset =
			InlineFormSet::<TestModel, ScalarChildModel>::for_update(parent, "position".to_owned());
		let mut data = ScalarChildModelModelFormData::<AllEditableModelFields>::empty();
		data.set_content("child".to_owned())
			.expect("child content should be accepted");
		formset.add_child_form(ModelForm::from_payload(data));

		assert_eq!(formset.is_valid(), false);
		assert_eq!(
			formset.child_forms()[0].form().errors(),
			&std::collections::HashMap::from([(
				"position".to_owned(),
				vec!["This field is required.".to_owned()],
			)])
		);
		let mut executor = FormsetExecutor::new(Vec::<Result<Row, Error>>::new());

		let error = tokio_test::block_on(formset.save(&mut executor)).unwrap_err();

		assert_eq!(
			error,
			ModelFormError::FieldValidation {
				errors: std::collections::HashMap::from([(
					"position".to_owned(),
					vec!["foreign key field is not a generated relationship identifier".to_owned(),],
				)]),
			}
		);
		assert_eq!(executor.fetch_one_calls, 0);
	}

	#[test]
	fn test_inline_formset_rejects_relation_targeting_different_same_kind_parent() {
		let parent = test_model(1, "parent");
		let mut formset = InlineFormSet::<TestModel, AlternateChildModel>::for_update(
			parent,
			"parent_id".to_owned(),
		);
		let mut executor = FormsetExecutor::new(Vec::<Result<Row, Error>>::new());

		let error = tokio_test::block_on(formset.save(&mut executor)).unwrap_err();

		assert!(matches!(
			error,
			ModelFormError::FieldValidation { errors }
				if errors.get("parent_id")
					== Some(&vec!["foreign key field does not target the parent model".to_owned()])
		));
		assert_eq!(executor.fetch_one_calls, 0);
	}

	#[test]
	fn test_inline_formset_rejects_relation_targeting_different_primary_key_kind_parent() {
		let parent = TestModel {
			id: None,
			name: "parent".to_owned(),
			email: "parent@example.com".to_owned(),
		};
		let mut formset =
			InlineFormSet::<TestModel, UuidChild>::for_create(parent, "parent_id".to_owned());
		let mut data = UuidChildModelFormData::<AllEditableModelFields>::empty();
		data.set_content("child".to_owned());
		formset.add_child_form(ModelForm::from_payload(data));
		let mut executor = FormsetExecutor::new(Vec::<Result<Row, Error>>::new());

		let error = tokio_test::block_on(formset.save(&mut executor)).unwrap_err();

		assert!(matches!(
			error,
			ModelFormError::FieldValidation { errors }
				if errors.get("parent_id")
					== Some(&vec!["foreign key field does not target the parent model".to_owned()])
		));
		assert_eq!(executor.fetch_one_calls, 0);
	}

	#[test]
	fn test_inline_formset_assigned_uuid_parent_create_uses_insert_intent() {
		let parent_id = uuid::Uuid::from_u128(0x019c_1234_5678_7abc_8def_0123_4567_89ab);
		let parent = UuidParent {
			id: parent_id,
			name: "assigned".to_owned(),
		};
		let mut formset =
			InlineFormSet::<UuidParent, UuidChild>::for_create(parent, "parent_id".to_owned());
		let mut data = UuidChildModelFormData::<AllEditableModelFields>::empty();
		data.set_content("created child".to_owned());
		formset.add_child_form(ModelForm::from_payload(data));
		let mut executor = FormsetExecutor::new([
			Ok(uuid_parent_row(parent_id, "assigned")),
			Ok(uuid_child_row(4, parent_id, "created child")),
		]);

		tokio_test::block_on(formset.save(&mut executor))
			.expect("assigned UUID parent should be created without an existence query");

		assert_eq!(
			executor.queries[0].split_whitespace().next(),
			Some("INSERT")
		);
		assert_eq!(
			formset.child_forms()[0].instance().unwrap().parent_id,
			Some(parent_id)
		);
	}

	#[test]
	fn test_inline_formset_existing_parent_update_uses_update_intent() {
		let parent = test_model(11, "existing");
		let mut formset =
			InlineFormSet::<TestModel, ChildModel>::for_update(parent, "parent_id".to_owned());
		let mut executor = FormsetExecutor::new([Ok(test_model_row(11, "existing"))]);

		tokio_test::block_on(formset.save(&mut executor))
			.expect("existing parent should be updated with explicit intent");

		assert_eq!(
			executor.queries[0].split_whitespace().next(),
			Some("UPDATE")
		);
		assert_eq!(executor.fetch_one_calls, 1);
	}

	#[test]
	fn test_model_formset_creation() {
		let formset = ModelFormSet::<TestModel>::new("test".to_string());

		assert_eq!(formset.prefix(), "test");
		assert_eq!(formset.forms().len(), 0);
		assert!(!formset.can_delete);
		assert!(!formset.can_order);
	}

	#[test]
	fn test_model_formset_add_form() {
		let mut formset = ModelFormSet::<TestModel>::new("test".to_string());

		let instance = TestModel {
			id: None,
			name: "Test".to_string(),
			email: "test@example.com".to_string(),
		};
		let form = ModelForm::from_payload_and_instance(
			TestModelModelFormData::<AllEditableModelFields>::empty(),
			instance,
		);
		formset.add_form(form).unwrap();

		assert_eq!(formset.forms().len(), 1);
	}

	#[test]
	fn test_model_formset_validation() {
		let mut formset = ModelFormSet::<TestModel>::new("test".to_string())
			.with_min_num(2)
			.with_max_num(Some(5));

		let instance = TestModel {
			id: None,
			name: "Test".to_string(),
			email: "test@example.com".to_string(),
		};
		let form = ModelForm::from_payload_and_instance(
			TestModelModelFormData::<AllEditableModelFields>::empty(),
			instance,
		);
		formset.add_form(form).unwrap();

		assert!(!formset.is_valid());
		assert!(!formset.errors().is_empty());
	}

	#[test]
	fn test_model_formset_save_preserves_form_order() {
		let mut formset = ModelFormSet::<TestModel>::new("test".to_owned());
		formset
			.add_form(model_form(test_model(1, "first")))
			.unwrap();
		formset
			.add_form(model_form(test_model(2, "second")))
			.unwrap();
		let mut executor = FormsetExecutor::new([
			Ok(test_model_row(1, "first")),
			Ok(test_model_row(2, "second")),
		]);

		let saved = tokio_test::block_on(formset.save(&mut executor)).unwrap();

		assert_eq!(
			saved
				.iter()
				.map(|model| model.name.as_str())
				.collect::<Vec<_>>(),
			vec!["first", "second"]
		);
		assert_eq!(executor.fetch_one_calls, 2);
	}

	#[test]
	fn test_model_formset_default_extra_does_not_block_existing_only_save() {
		let mut formset = ModelFormSet::<TestModel>::new("test".to_owned());
		formset
			.add_form(model_form(test_model(1, "existing")))
			.unwrap();
		formset
			.add_form(ModelForm::from_payload(TestModelModelFormData::<
				AllEditableModelFields,
			>::empty()))
			.unwrap();
		let mut executor = FormsetExecutor::new([Ok(test_model_row(1, "existing"))]);

		let saved = tokio_test::block_on(formset.save(&mut executor))
			.expect("untouched default extra should be excluded");

		assert_eq!(saved.len(), 1);
		assert_eq!(saved[0].name, "existing");
		assert_eq!(executor.fetch_one_calls, 1);
	}

	#[test]
	fn test_model_formset_untouched_all_default_extra_does_not_create_phantom_row() {
		let mut formset = ModelFormSet::<AllDefaultModel>::new("defaults".to_owned());
		formset
			.add_form(ModelForm::from_payload(AllDefaultModelModelFormData::<
				AllEditableModelFields,
			>::empty()))
			.unwrap();
		assert!(formset.forms_mut()[0].is_valid());
		let mut executor = FormsetExecutor::new(Vec::<Result<Row, Error>>::new());

		let saved = tokio_test::block_on(formset.save(&mut executor))
			.expect("untouched all-default extra should be excluded");

		assert!(saved.is_empty());
		assert_eq!(executor.fetch_one_calls, 0);
	}

	#[test]
	fn test_model_formset_submitted_extra_is_persisted() {
		let mut data = TestModelModelFormData::<AllEditableModelFields>::empty();
		data.set_name("submitted".to_owned());
		data.set_email("submitted@example.com".to_owned());
		let mut formset = ModelFormSet::<TestModel>::new("test".to_owned());
		formset.add_form(ModelForm::from_payload(data)).unwrap();
		let mut executor = FormsetExecutor::new([Ok(test_model_row(7, "submitted"))]);

		let saved = tokio_test::block_on(formset.save(&mut executor))
			.expect("submitted extra should be persisted");

		assert_eq!(saved.len(), 1);
		assert_eq!(saved[0].id, Some(7));
		assert_eq!(saved[0].name, "submitted");
		assert_eq!(executor.fetch_one_calls, 1);
	}

	#[test]
	fn test_model_formset_save_stops_after_first_persistence_error() {
		let mut formset = ModelFormSet::<TestModel>::new("test".to_owned());
		for (id, name) in [(1, "first"), (2, "second"), (3, "third")] {
			formset.add_form(model_form(test_model(id, name))).unwrap();
		}
		let mut executor = FormsetExecutor::new([
			Ok(test_model_row(1, "first")),
			Err(DatabaseError::new(DatabaseErrorKind::Timeout, "write timed out").into()),
			Ok(test_model_row(3, "third")),
		]);

		let error = tokio_test::block_on(formset.save(&mut executor)).unwrap_err();

		assert_eq!(
			error.database_error().map(DatabaseError::kind),
			Some(DatabaseErrorKind::Timeout)
		);
		assert_eq!(executor.fetch_one_calls, 2);
	}

	#[test]
	fn test_model_formset_save_reuses_preflight_candidate() {
		let mut data = TestModelModelFormData::<AllEditableModelFields>::empty();
		data.set_name("candidate".to_owned());
		data.set_email("candidate@example.com".to_owned());
		let cleaner_calls = Arc::new(AtomicUsize::new(0));
		let cleaner_calls_for_field = Arc::clone(&cleaner_calls);
		let mut form = ModelForm::<TestModel>::from_payload(data);
		form.form_mut()
			.add_field_clean_function("name", move |value| {
				let call = cleaner_calls_for_field.fetch_add(1, Ordering::SeqCst) + 1;
				Ok(json!(format!(
					"{}-{call}",
					value.as_str().expect("name cleaner receives text")
				)))
			});
		let mut formset = ModelFormSet::<TestModel>::new("test".to_owned());
		formset.add_form(form).unwrap();
		let mut executor = FormsetExecutor::new([Ok(test_model_row(1, "candidate-1"))]);

		let saved = tokio_test::block_on(formset.save(&mut executor)).unwrap();

		assert_eq!(saved[0].name, "candidate-1");
		assert_eq!(cleaner_calls.load(Ordering::SeqCst), 1);
		assert_eq!(executor.fetch_one_calls, 1);
	}

	#[test]
	fn test_model_formset_save_rejects_below_minimum_before_persistence() {
		let mut formset = ModelFormSet::<TestModel>::new("test".to_owned()).with_min_num(2);
		formset.add_form(model_form(test_model(1, "only"))).unwrap();
		let mut executor = FormsetExecutor::new([Ok(test_model_row(1, "only"))]);

		let error = tokio_test::block_on(formset.save(&mut executor)).unwrap_err();

		assert!(matches!(
			error,
			ModelFormError::ModelValidation { errors }
				if errors == ["Please submit at least 2 forms"]
		));
		assert_eq!(executor.fetch_one_calls, 0);
	}

	#[test]
	fn test_model_formset_save_rejects_above_maximum_before_persistence() {
		let mut formset = ModelFormSet::<TestModel>::new("test".to_owned()).with_max_num(Some(1));
		formset.forms_mut().push(model_form(test_model(1, "first")));
		formset
			.forms_mut()
			.push(model_form(test_model(2, "second")));
		let mut executor = FormsetExecutor::new([
			Ok(test_model_row(1, "first")),
			Ok(test_model_row(2, "second")),
		]);

		let error = tokio_test::block_on(formset.save(&mut executor)).unwrap_err();

		assert!(matches!(
			error,
			ModelFormError::ModelValidation { errors }
				if errors == ["Please submit no more than 1 forms"]
		));
		assert_eq!(executor.fetch_one_calls, 0);
	}

	#[test]
	fn test_formset_factory_creation() {
		let factory = FormSetFactory::new("form".to_string());

		assert_eq!(factory.extra(), 1);
		assert!(!factory.can_delete());
		assert!(!factory.can_order());
		assert_eq!(factory.max_num(), Some(1000));
		assert_eq!(factory.min_num(), 0);
	}

	#[test]
	fn test_formset_factory_builder() {
		let factory = FormSetFactory::new("form".to_string())
			.with_extra(3)
			.with_can_delete(true)
			.with_can_order(true)
			.with_max_num(Some(10))
			.with_min_num(2);

		assert_eq!(factory.extra(), 3);
		assert!(factory.can_delete());
		assert!(factory.can_order());
		assert_eq!(factory.max_num(), Some(10));
		assert_eq!(factory.min_num(), 2);
	}

	#[test]
	fn test_formset_factory_create() {
		let factory = FormSetFactory::new("form".to_string())
			.with_extra(3)
			.with_can_delete(true);

		let formset = factory.create();

		assert_eq!(formset.prefix(), "form");
		assert!(formset.can_delete());
	}

	#[test]
	fn test_formset_factory_create_model_formset() {
		let factory = FormSetFactory::new("user".to_string())
			.with_extra(2)
			.with_min_num(1);

		let formset = factory.create_model_formset::<TestModel>();

		assert_eq!(formset.prefix(), "user");
		assert_eq!(formset.min_num, 1);
	}
}
