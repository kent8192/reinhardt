//! ModelFormSet implementation for managing multiple model forms
//!
//! ModelFormSets allow editing multiple model instances at once, handling
//! creation, updates, and deletion in a single form submission.

use crate::FormError;
use crate::formset::FormSet;
use crate::model_form::{FormModel, ModelForm, ModelFormConfig};
use std::collections::HashMap;
use std::marker::PhantomData;

/// Configuration for ModelFormSet
#[derive(Debug, Clone)]
pub struct ModelFormSetConfig {
	/// Configuration for individual model forms
	pub form_config: ModelFormConfig,
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
			form_config: ModelFormConfig::default(),
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
	/// Set the form configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{ModelFormSetConfig, ModelFormConfig};
	///
	/// let form_config = ModelFormConfig::new()
	///     .fields(vec!["name".to_string(), "email".to_string()]);
	/// let config = ModelFormSetConfig::new().with_form_config(form_config);
	/// assert!(config.form_config.fields.is_some());
	/// ```
	pub fn with_form_config(mut self, form_config: ModelFormConfig) -> Self {
		self.form_config = form_config;
		self
	}
}

/// A formset for managing multiple model instances
pub struct ModelFormSet<T: FormModel> {
	model_forms: Vec<ModelForm<T>>,
	formset: FormSet,
	_phantom: PhantomData<T>,
}

impl<T: FormModel> ModelFormSet<T> {
	/// Create a new ModelFormSet with instances
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_forms::{ModelFormSet, ModelFormSetConfig};
	///
	/// let config = ModelFormSetConfig::new();
	/// let instances = vec![]; // Empty list of model instances
	/// let formset = ModelFormSet::<MyModel>::new("formset".to_string(), instances, config);
	/// assert_eq!(formset.prefix(), "formset");
	/// ```
	pub fn new(prefix: String, instances: Vec<T>, config: ModelFormSetConfig) -> Self {
		let mut model_forms = Vec::new();

		// Create ModelForm for each instance
		for instance in instances {
			let model_form = ModelForm::new(Some(instance), config.form_config.clone());
			model_forms.push(model_form);
		}

		// Add extra empty forms
		for _ in 0..config.extra {
			let model_form = ModelForm::empty(config.form_config.clone());
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
			_phantom: PhantomData,
		}
	}
	/// Create an empty ModelFormSet (for creating new instances)
	///
	/// # Examples
	///
	/// ```no_run
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
	/// Validate all forms in the formset
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_forms::{ModelFormSet, ModelFormSetConfig};
	///
	/// let config = ModelFormSetConfig::new();
	/// let mut formset = ModelFormSet::<MyModel>::empty("formset".to_string(), config);
	/// let is_valid = formset.is_valid();
	/// ```
	pub fn is_valid(&mut self) -> bool {
		// Validate all model forms
		self.model_forms.iter_mut().all(|form| form.is_valid())
	}
	/// Collects and returns all validation errors from every form in the set.
	pub fn errors(&self) -> Vec<String> {
		// Collect errors from all model forms
		self.model_forms
			.iter()
			.flat_map(|model_form| {
				model_form
					.form()
					.errors()
					.values()
					.flat_map(|errors| errors.iter().cloned())
			})
			.collect()
	}
	/// Save all valid forms to the database
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_forms::{ModelFormSet, ModelFormSetConfig};
	///
	/// let config = ModelFormSetConfig::new();
	/// let mut formset = ModelFormSet::<MyModel>::empty("formset".to_string(), config);
	/// let result = formset.save();
	/// ```
	pub fn save(&mut self) -> Result<Vec<T>, FormError> {
		if !self.is_valid() {
			return Err(FormError::Validation("Formset is not valid".to_string()));
		}

		let mut saved_instances = Vec::new();

		// Iterate through each ModelForm and save if it has changes
		for model_form in &mut self.model_forms {
			// Check if form has an instance (skip empty forms)
			if model_form.instance().is_some() {
				// Save the instance
				let instance = model_form.save()?;
				saved_instances.push(instance);
			}
		}

		Ok(saved_instances)
	}
	/// Get management form data for HTML rendering
	///
	/// # Examples
	///
	/// ```no_run
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
pub struct ModelFormSetBuilder<T: FormModel> {
	config: ModelFormSetConfig,
	_phantom: PhantomData<T>,
}

impl<T: FormModel> ModelFormSetBuilder<T> {
	/// Create a new builder
	///
	/// # Examples
	///
	/// ```no_run
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
	/// ```no_run
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
	/// ```no_run
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
	/// ```no_run
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
	/// ```no_run
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
	/// ```no_run
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
	/// ```no_run
	/// use reinhardt_forms::ModelFormSetBuilder;
	///
	/// let instances = vec![]; // Empty list of model instances
	/// let builder = ModelFormSetBuilder::<MyModel>::new();
	/// let formset = builder.build("formset".to_string(), instances);
	/// ```
	pub fn build(self, prefix: String, instances: Vec<T>) -> ModelFormSet<T> {
		ModelFormSet::new(prefix, instances, self.config)
	}
	/// Build an empty formset
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_forms::ModelFormSetBuilder;
	///
	/// let builder = ModelFormSetBuilder::<MyModel>::new().extra(3);
	/// let formset = builder.build_empty("formset".to_string());
	/// ```
	pub fn build_empty(self, prefix: String) -> ModelFormSet<T> {
		ModelFormSet::empty(prefix, self.config)
	}
}

impl<T: FormModel> Default for ModelFormSetBuilder<T> {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::Value;

	// Mock model for testing
	struct Article {
		id: i32,
		title: String,
		content: String,
	}

	impl FormModel for Article {
		fn field_names() -> Vec<String> {
			vec!["id".to_string(), "title".to_string(), "content".to_string()]
		}

		fn get_field(&self, name: &str) -> Option<Value> {
			match name {
				"id" => Some(Value::Number(self.id.into())),
				"title" => Some(Value::String(self.title.clone())),
				"content" => Some(Value::String(self.content.clone())),
				_ => None,
			}
		}

		fn set_field(&mut self, name: &str, value: Value) -> Result<(), String> {
			match name {
				"id" => {
					if let Value::Number(n) = value {
						self.id = n.as_i64().unwrap() as i32;
						Ok(())
					} else {
						Err("Invalid type for id".to_string())
					}
				}
				"title" => {
					if let Value::String(s) = value {
						self.title = s;
						Ok(())
					} else {
						Err("Invalid type for title".to_string())
					}
				}
				"content" => {
					if let Value::String(s) = value {
						self.content = s;
						Ok(())
					} else {
						Err("Invalid type for content".to_string())
					}
				}
				_ => Err(format!("Unknown field: {}", name)),
			}
		}

		fn save(&mut self) -> Result<(), String> {
			// Mock save
			Ok(())
		}
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
				id: 1,
				title: "First Article".to_string(),
				content: "Content 1".to_string(),
			},
			Article {
				id: 2,
				title: "Second Article".to_string(),
				content: "Content 2".to_string(),
			},
		];

		let config = ModelFormSetConfig::new();
		let formset = ModelFormSet::new("article".to_string(), instances, config);

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
}
