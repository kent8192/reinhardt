//! ModelFormSet implementation for managing multiple model forms
//!
//! ModelFormSets allow editing multiple model instances at once, handling
//! creation, updates, and deletion in a single form submission.

use crate::formset::FormSet;
use crate::model_form::{FormModel, ModelForm, ModelFormError};
use reinhardt_core::model_form::{AllEditableModelFields, ModelFormPolicy};
use reinhardt_db::orm::OrmExecutor;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Configuration for ModelFormSet
#[derive(Debug, Clone)]
pub struct ModelFormSetConfig {
	/// Allow deletion of instances
	pub can_delete: bool,
	/// Allow ordering of instances
	pub can_order: bool,
	/// Number of extra forms to display
	pub extra: usize,
	/// Maximum number of forms
	pub max_num: Option<usize>,
	/// Minimum number of forms
	pub min_num: usize,
}

impl Default for ModelFormSetConfig {
	fn default() -> Self {
		Self {
			can_delete: false,
			can_order: false,
			extra: 1,
			max_num: Some(1000),
			min_num: 0,
		}
	}
}

impl ModelFormSetConfig {
	/// Create a new ModelFormSetConfig
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::ModelFormSetConfig;
	///
	/// let config = ModelFormSetConfig::new();
	/// assert_eq!(config.extra, 1);
	/// assert!(!config.can_delete);
	/// ```
	pub fn new() -> Self {
		Self::default()
	}
	/// Set the number of extra forms
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::ModelFormSetConfig;
	///
	/// let config = ModelFormSetConfig::new().with_extra(3);
	/// assert_eq!(config.extra, 3);
	/// ```
	pub fn with_extra(mut self, extra: usize) -> Self {
		self.extra = extra;
		self
	}
	/// Enable or disable deletion
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::ModelFormSetConfig;
	///
	/// let config = ModelFormSetConfig::new().with_can_delete(true);
	/// assert!(config.can_delete);
	/// ```
	pub fn with_can_delete(mut self, can_delete: bool) -> Self {
		self.can_delete = can_delete;
		self
	}
	/// Enable or disable ordering
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::ModelFormSetConfig;
	///
	/// let config = ModelFormSetConfig::new().with_can_order(true);
	/// assert!(config.can_order);
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
	/// use reinhardt_forms::ModelFormSetConfig;
	///
	/// let config = ModelFormSetConfig::new().with_max_num(Some(10));
	/// assert_eq!(config.max_num, Some(10));
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
	/// use reinhardt_forms::ModelFormSetConfig;
	///
	/// let config = ModelFormSetConfig::new().with_min_num(2);
	/// assert_eq!(config.min_num, 2);
	/// ```
	pub fn with_min_num(mut self, min_num: usize) -> Self {
		self.min_num = min_num;
		self
	}
}

/// A formset for managing multiple model instances
pub struct ModelFormSet<T, P = AllEditableModelFields>
where
	T: FormModel,
	P: ModelFormPolicy,
{
	model_forms: Vec<ModelForm<T, P>>,
	formset: FormSet,
	max_num: Option<usize>,
	min_num: usize,
	errors: Vec<String>,
	_phantom: PhantomData<(T, P)>,
}

impl<T, P> ModelFormSet<T, P>
where
	T: FormModel,
	P: ModelFormPolicy,
	T::Data<P>: Default,
{
	/// Create a new ModelFormSet with instances
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelFormSet, ModelFormSetConfig};
	///
	/// let config = ModelFormSetConfig::new();
	/// let instances = vec![]; // Empty list of model instances
	/// let formset = ModelFormSet::<MyModel>::new("formset".to_string(), instances, config);
	/// assert_eq!(formset.prefix(), "formset");
	/// ```
	pub fn new(prefix: String, instances: Vec<T>, config: ModelFormSetConfig) -> Self {
		let mut model_forms = Vec::new();
		let max_num = config.max_num;
		let min_num = config.min_num;

		// Create ModelForm for each instance
		for instance in instances {
			let model_form =
				ModelForm::from_payload_and_instance(T::Data::<P>::default(), instance);
			model_forms.push(model_form);
		}

		// Add extra empty forms
		for _ in 0..config.extra {
			let model_form = ModelForm::from_payload(T::Data::<P>::default());
			model_forms.push(model_form);
		}

		// Create FormSet for management data
		let formset = FormSet::new(prefix)
			.with_extra(config.extra)
			.with_can_delete(config.can_delete)
			.with_can_order(config.can_order)
			.with_max_num(config.max_num)
			.with_min_num(config.min_num);

		Self {
			model_forms,
			formset,
			max_num,
			min_num,
			errors: Vec::new(),
			_phantom: PhantomData,
		}
	}
	/// Create an empty ModelFormSet (for creating new instances)
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelFormSet, ModelFormSetConfig};
	///
	/// let config = ModelFormSetConfig::new().with_extra(3);
	/// let formset = ModelFormSet::<MyModel>::empty("formset".to_string(), config);
	/// assert_eq!(formset.total_form_count(), 3);
	/// ```
	pub fn empty(prefix: String, config: ModelFormSetConfig) -> Self {
		Self::new(prefix, Vec::new(), config)
	}
	/// Returns the prefix used for form field naming.
	pub fn prefix(&self) -> &str {
		self.formset.prefix()
	}
	/// Returns references to all model instances that have been loaded.
	pub fn instances(&self) -> Vec<&T> {
		self.model_forms
			.iter()
			.filter_map(|form| form.instance())
			.collect()
	}
	/// Returns the number of forms backed by existing model instances.
	pub fn form_count(&self) -> usize {
		// Return number of forms with instances (not including extra empty forms)
		self.model_forms
			.iter()
			.filter(|form| form.instance().is_some())
			.count()
	}
	/// Returns the total number of forms, including extra empty forms.
	pub fn total_form_count(&self) -> usize {
		// Return total number of forms including extras
		self.model_forms.len()
	}
	/// Returns mutable access to existing and extra model forms.
	///
	/// Use [`ModelForm::set_field_value`] or replace an extra with
	/// [`ModelForm::from_payload`] to mark it as genuinely submitted.
	pub fn forms_mut(&mut self) -> &mut [ModelForm<T, P>] {
		&mut self.model_forms
	}
	/// Validate all forms in the formset
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelFormSet, ModelFormSetConfig};
	///
	/// let config = ModelFormSetConfig::new();
	/// let mut formset = ModelFormSet::<MyModel>::empty("formset".to_string(), config);
	/// let is_valid = formset.is_valid();
	/// ```
	pub fn is_valid(&mut self) -> bool {
		let mut all_valid = true;
		for form in &mut self.model_forms {
			if form.is_submission_candidate() && !form.is_valid() {
				all_valid = false;
			}
		}

		self.validate_cardinality().is_ok() && all_valid
	}
	/// Collects and returns all validation errors from every form in the set.
	pub fn errors(&self) -> Vec<String> {
		let mut errors = self.errors.clone();
		errors.extend(self.model_forms.iter().flat_map(|model_form| {
			model_form
				.form()
				.errors()
				.values()
				.flat_map(|errors| errors.iter().cloned())
		}));
		errors
	}

	fn validate_cardinality(&mut self) -> Result<(), ModelFormError> {
		self.errors.clear();
		let candidate_count = self
			.model_forms
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

	/// Save all valid forms to the database
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelFormSet, ModelFormSetConfig};
	///
	/// let config = ModelFormSetConfig::new();
	/// let mut formset = ModelFormSet::<MyModel>::empty("formset".to_string(), config);
	/// let result = formset.save();
	/// ```
	pub async fn save(&mut self, executor: &mut dyn OrmExecutor) -> Result<Vec<T>, ModelFormError> {
		self.validate_cardinality()?;
		for model_form in &mut self.model_forms {
			if model_form.is_submission_candidate() {
				model_form.build_instance()?;
			}
		}

		let mut saved_instances = Vec::with_capacity(
			self.model_forms
				.iter()
				.filter(|form| form.is_submission_candidate())
				.count(),
		);
		for model_form in &mut self.model_forms {
			if model_form.is_submission_candidate() {
				saved_instances.push(model_form.save(executor).await?);
			}
		}
		Ok(saved_instances)
	}
	/// Get management form data for HTML rendering
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::{ModelFormSet, ModelFormSetConfig};
	///
	/// let config = ModelFormSetConfig::new().with_extra(2);
	/// let formset = ModelFormSet::<MyModel>::empty("article".to_string(), config);
	/// let mgmt_data = formset.management_form_data();
	///
	/// assert!(mgmt_data.contains_key("article-TOTAL_FORMS"));
	/// assert_eq!(mgmt_data.get("article-TOTAL_FORMS"), Some(&"2".to_string()));
	/// ```
	pub fn management_form_data(&self) -> HashMap<String, String> {
		self.formset.management_form_data()
	}
}

/// Builder for creating ModelFormSet instances
pub struct ModelFormSetBuilder<T, P = AllEditableModelFields>
where
	T: FormModel,
	P: ModelFormPolicy,
{
	config: ModelFormSetConfig,
	_phantom: PhantomData<(T, P)>,
}

impl<T, P> ModelFormSetBuilder<T, P>
where
	T: FormModel,
	P: ModelFormPolicy,
	T::Data<P>: Default,
{
	/// Create a new builder
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::ModelFormSetBuilder;
	///
	/// let builder = ModelFormSetBuilder::<MyModel>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			config: ModelFormSetConfig::default(),
			_phantom: PhantomData,
		}
	}
	/// Set the number of extra forms
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::ModelFormSetBuilder;
	///
	/// let builder = ModelFormSetBuilder::<MyModel>::new().extra(5);
	/// ```
	pub fn extra(mut self, extra: usize) -> Self {
		self.config.extra = extra;
		self
	}
	/// Enable deletion
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::ModelFormSetBuilder;
	///
	/// let builder = ModelFormSetBuilder::<MyModel>::new().can_delete(true);
	/// ```
	pub fn can_delete(mut self, can_delete: bool) -> Self {
		self.config.can_delete = can_delete;
		self
	}
	/// Enable ordering
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::ModelFormSetBuilder;
	///
	/// let builder = ModelFormSetBuilder::<MyModel>::new().can_order(true);
	/// ```
	pub fn can_order(mut self, can_order: bool) -> Self {
		self.config.can_order = can_order;
		self
	}
	/// Set maximum number of forms
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::ModelFormSetBuilder;
	///
	/// let builder = ModelFormSetBuilder::<MyModel>::new().max_num(10);
	/// ```
	pub fn max_num(mut self, max_num: usize) -> Self {
		self.config.max_num = Some(max_num);
		self
	}
	/// Set minimum number of forms
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::ModelFormSetBuilder;
	///
	/// let builder = ModelFormSetBuilder::<MyModel>::new().min_num(1);
	/// ```
	pub fn min_num(mut self, min_num: usize) -> Self {
		self.config.min_num = min_num;
		self
	}
	/// Build the formset with instances
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::ModelFormSetBuilder;
	///
	/// let instances = vec![]; // Empty list of model instances
	/// let builder = ModelFormSetBuilder::<MyModel>::new();
	/// let formset = builder.build("formset".to_string(), instances);
	/// ```
	pub fn build(self, prefix: String, instances: Vec<T>) -> ModelFormSet<T, P> {
		ModelFormSet::new(prefix, instances, self.config)
	}
	/// Build an empty formset
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_forms::ModelFormSetBuilder;
	///
	/// let builder = ModelFormSetBuilder::<MyModel>::new().extra(3);
	/// let formset = builder.build_empty("formset".to_string());
	/// ```
	pub fn build_empty(self, prefix: String) -> ModelFormSet<T, P> {
		ModelFormSet::empty(prefix, self.config)
	}
}

impl<T, P> Default for ModelFormSetBuilder<T, P>
where
	T: FormModel,
	P: ModelFormPolicy,
	T::Data<P>: Default,
{
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::VecDeque;

	use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
	use reinhardt_core::model_form::ModelFormPolicy;
	use reinhardt_db::orm::connection::{
		DatabaseBackend, OrmExecutor, QueryResult, QueryValue, Row,
	};
	use reinhardt_macros::model;
	use serde::{Deserialize, Serialize};
	use serde_json::json;

	#[model(
		app_label = "forms",
		table_name = "model_formset_articles",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct Article {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200)]
		title: String,
		#[field(max_length = 2_000)]
		content: String,
	}

	#[model(
		app_label = "forms",
		table_name = "model_formset_default_articles",
		form = true,
		info = false
	)]
	#[derive(Clone, Deserialize, Serialize)]
	struct DefaultArticle {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 200, default = "generated")]
		title: String,
		#[field(default = true)]
		published: bool,
	}

	struct TitleOnly;

	impl ModelFormPolicy for TitleOnly {
		fn allows(field: &str) -> bool {
			field == "title"
		}
	}

	#[derive(Debug)]
	struct FormsetExecutor {
		rows: VecDeque<Result<Row, Error>>,
		fetch_one_calls: usize,
	}

	impl FormsetExecutor {
		fn new(rows: impl IntoIterator<Item = Result<Row, Error>>) -> Self {
			Self {
				rows: rows.into_iter().collect(),
				fetch_one_calls: 0,
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

		async fn fetch_one(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row, Error> {
			self.fetch_one_calls += 1;
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

	fn article(id: i64, title: &str) -> Article {
		Article {
			id: Some(id),
			title: title.to_owned(),
			content: format!("{title} content"),
		}
	}

	fn article_row(id: i64, title: &str) -> Row {
		let mut row = Row::new();
		row.insert("id".to_owned(), QueryValue::Int(id));
		row.insert("title".to_owned(), QueryValue::String(title.to_owned()));
		row.insert(
			"content".to_owned(),
			QueryValue::String(format!("{title} content")),
		);
		row
	}

	#[test]
	fn test_model_formset_config() {
		let config = ModelFormSetConfig::new()
			.with_extra(3)
			.with_can_delete(true)
			.with_max_num(Some(10))
			.with_min_num(1);

		assert_eq!(config.extra, 3);
		assert!(config.can_delete);
		assert_eq!(config.max_num, Some(10));
		assert_eq!(config.min_num, 1);
	}

	#[test]
	fn test_model_formset_empty() {
		let config = ModelFormSetConfig::new().with_extra(2);
		let formset = ModelFormSet::<Article>::empty("article".to_string(), config);

		assert_eq!(formset.prefix(), "article");
		assert_eq!(formset.instances().len(), 0);
		assert_eq!(formset.total_form_count(), 2);
	}

	#[test]
	fn test_model_formset_with_instances() {
		let instances = vec![
			Article {
				id: Some(1),
				title: "First Article".to_string(),
				content: "Content 1".to_string(),
			},
			Article {
				id: Some(2),
				title: "Second Article".to_string(),
				content: "Content 2".to_string(),
			},
		];

		let config = ModelFormSetConfig::new();
		let formset = ModelFormSet::<Article>::new("article".to_string(), instances, config);

		assert_eq!(formset.instances().len(), 2);
		assert_eq!(formset.form_count(), 2);
	}

	#[test]
	fn test_model_formset_builder() {
		let formset = ModelFormSetBuilder::<Article>::new()
			.extra(3)
			.can_delete(true)
			.max_num(5)
			.build_empty("article".to_string());

		assert_eq!(formset.total_form_count(), 3);
	}

	#[test]
	fn test_model_formset_management_data() {
		let config = ModelFormSetConfig::new().with_extra(2).with_min_num(1);
		let formset = ModelFormSet::<Article>::empty("article".to_string(), config);

		let mgmt_data = formset.management_form_data();

		assert_eq!(mgmt_data.get("article-TOTAL_FORMS"), Some(&"2".to_string()));
		assert_eq!(
			mgmt_data.get("article-INITIAL_FORMS"),
			Some(&"0".to_string())
		);
		assert_eq!(
			mgmt_data.get("article-MIN_NUM_FORMS"),
			Some(&"1".to_string())
		);
	}

	#[test]
	fn public_model_formset_existing_instance_ignores_untouched_default_extra() {
		let config = ModelFormSetConfig::new()
			.with_extra(1)
			.with_min_num(1)
			.with_max_num(Some(1));
		let mut formset = ModelFormSet::<Article>::new(
			"article".to_owned(),
			vec![article(1, "existing")],
			config,
		);
		let mut executor = FormsetExecutor::new([Ok(article_row(1, "existing"))]);

		assert!(formset.is_valid());
		let saved = tokio_test::block_on(formset.save(&mut executor))
			.expect("untouched extra should not block existing-only persistence");

		assert_eq!(saved.len(), 1);
		assert_eq!(saved[0].id, Some(1));
		assert_eq!(saved[0].title, "existing");
		assert_eq!(executor.fetch_one_calls, 1);
	}

	#[test]
	fn public_model_formset_all_default_extra_does_not_create_phantom_row() {
		let config = ModelFormSetConfig::new()
			.with_extra(1)
			.with_max_num(Some(0));
		let mut formset = ModelFormSet::<DefaultArticle>::empty("article".to_owned(), config);
		let mut executor = FormsetExecutor::new(Vec::<Result<Row, Error>>::new());

		assert!(formset.is_valid());
		let saved = tokio_test::block_on(formset.save(&mut executor))
			.expect("untouched all-default extra should be ignored");

		assert!(saved.is_empty());
		assert_eq!(executor.fetch_one_calls, 0);
	}

	#[test]
	fn public_model_formset_submitted_extra_is_persisted() {
		let config = ModelFormSetConfig::new().with_extra(1);
		let mut formset = ModelFormSet::<Article>::empty("article".to_owned(), config);
		formset.forms_mut()[0]
			.set_field_value("title", json!("submitted"))
			.expect("title should be accepted");
		formset.forms_mut()[0]
			.set_field_value("content", json!("submitted content"))
			.expect("content should be accepted");
		let mut executor = FormsetExecutor::new([Ok(article_row(7, "submitted"))]);

		assert!(formset.is_valid());
		let saved = tokio_test::block_on(formset.save(&mut executor))
			.expect("submitted extra should be persisted");

		assert_eq!(saved.len(), 1);
		assert_eq!(saved[0].id, Some(7));
		assert_eq!(saved[0].title, "submitted");
		assert_eq!(executor.fetch_one_calls, 1);
	}

	#[test]
	fn public_model_formset_forbidden_extra_is_not_silently_discarded() {
		let payload: ArticleModelFormData<TitleOnly> = serde_json::from_value(json!({
			"content": "forbidden content"
		}))
		.expect("forbidden wire field should be recorded in the payload");
		let config = ModelFormSetConfig::new().with_extra(1);
		let mut formset = ModelFormSet::<Article, TitleOnly>::empty("article".to_owned(), config);
		formset.forms_mut()[0] = ModelForm::from_payload(payload);
		let mut executor = FormsetExecutor::new(Vec::<Result<Row, Error>>::new());

		assert!(!formset.is_valid());
		let error = tokio_test::block_on(formset.save(&mut executor))
			.expect_err("forbidden submitted extra must not be discarded");

		assert_eq!(error, ModelFormError::ForbiddenInput { field: "content" });
		assert_eq!(executor.fetch_one_calls, 0);
	}
}
