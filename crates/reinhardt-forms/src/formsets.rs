//! Advanced FormSet functionality
//!
//! This module provides advanced FormSet features including inline formsets,
//! model-based formsets, and dynamic formset generation.

use crate::FormError;
use crate::formset::FormSet;
use crate::model_form::{FormModel, ModelForm};
use std::marker::PhantomData;

/// InlineFormSet for managing forms related to a parent model
///
/// InlineFormSets are used to edit related objects together with a parent object,
/// similar to Django's inline formsets for admin.
pub struct InlineFormSet<P: FormModel, C: FormModel> {
	parent: P,
	_formset: FormSet,
	fk_field: String,
	child_forms: Vec<ModelForm<C>>,
	_phantom_parent: PhantomData<P>,
	_phantom_child: PhantomData<C>,
}

impl<P: FormModel, C: FormModel> InlineFormSet<P, C> {
	/// Create a new InlineFormSet
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
		Self {
			parent,
			_formset: FormSet::new("inline".to_string()),
			fk_field,
			child_forms: Vec::new(),
			_phantom_parent: PhantomData,
			_phantom_child: PhantomData,
		}
	}

	/// Add a child form to the formset
	pub fn add_child_form(&mut self, form: ModelForm<C>) {
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
	pub fn child_forms(&self) -> &[ModelForm<C>] {
		&self.child_forms
	}

	/// Save the formset and all related child instances.
	///
	/// This method saves the parent model first, retrieves the parent's primary
	/// key, sets the foreign key on each child instance, then saves each child.
	///
	/// # Errors
	///
	/// Returns an error if any save operation fails or if the parent model
	/// does not have an `id` field after saving.
	pub fn save(&mut self) -> Result<(), FormError> {
		// Save parent first
		self.parent
			.save()
			.map_err(|e| FormError::Validation(format!("Failed to save parent: {}", e)))?;

		// Get parent ID to set as foreign key on child instances
		let parent_id = self.parent.get_field("id").ok_or_else(|| {
			FormError::Validation("Parent model does not have an 'id' field".to_string())
		})?;

		// Save each child with the foreign key set to the parent ID
		let fk_field = self.fk_field.clone();
		for child_form in &mut self.child_forms {
			// Set the foreign key on the child instance before saving
			child_form.set_field_value(&fk_field, parent_id.clone());

			child_form
				.save()
				.map_err(|e| FormError::Validation(format!("Failed to save child: {}", e)))?;
		}

		Ok(())
	}

	/// Validate all child forms
	pub fn is_valid(&mut self) -> bool {
		let mut all_valid = true;

		for child_form in &mut self.child_forms {
			if !child_form.is_valid() {
				all_valid = false;
			}
		}

		all_valid
	}
}

/// ModelFormSet for managing multiple model instances
///
/// This is similar to the base FormSet but specifically designed for model instances.
pub struct ModelFormSet<T: FormModel> {
	forms: Vec<ModelForm<T>>,
	prefix: String,
	can_delete: bool,
	can_order: bool,
	extra: usize,
	max_num: Option<usize>,
	min_num: usize,
	errors: Vec<String>,
	_phantom: PhantomData<T>,
}

impl<T: FormModel> ModelFormSet<T> {
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
	pub fn add_form(&mut self, form: ModelForm<T>) -> Result<(), String> {
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
	pub fn forms(&self) -> &[ModelForm<T>] {
		&self.forms
	}

	/// Get mutable access to forms
	pub fn forms_mut(&mut self) -> &mut Vec<ModelForm<T>> {
		&mut self.forms
	}

	/// Validate all forms in the formset
	pub fn is_valid(&mut self) -> bool {
		self.errors.clear();

		let mut all_valid = true;
		for form in &mut self.forms {
			if !form.is_valid() {
				all_valid = false;
			}
		}

		// Check minimum number
		if self.forms.len() < self.min_num {
			self.errors
				.push(format!("Please submit at least {} forms", self.min_num));
			all_valid = false;
		}

		// Check maximum number
		if let Some(max) = self.max_num
			&& self.forms.len() > max
		{
			self.errors
				.push(format!("Please submit no more than {} forms", max));
			all_valid = false;
		}

		all_valid
	}

	/// Get validation errors
	pub fn errors(&self) -> &[String] {
		&self.errors
	}

	/// Save all forms in the formset
	pub fn save(&mut self) -> Result<(), FormError> {
		if !self.is_valid() {
			return Err(FormError::Validation(
				"Cannot save invalid formset".to_string(),
			));
		}

		for form in &mut self.forms {
			form.save()
				.map_err(|e| FormError::Validation(format!("Failed to save form: {}", e)))?;
		}

		Ok(())
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
	pub fn create_model_formset<T: FormModel>(&self) -> ModelFormSet<T> {
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
	use crate::ModelFormConfig;
	use crate::model_form::FieldType;
	use serde_json::Value;

	// Test model implementation
	#[derive(Clone)]
	struct TestModel {
		id: Option<i64>,
		name: String,
		email: String,
	}

	impl FormModel for TestModel {
		fn field_names() -> Vec<String> {
			vec!["id".to_string(), "name".to_string(), "email".to_string()]
		}

		fn field_type(name: &str) -> Option<FieldType> {
			match name {
				"id" => Some(FieldType::Integer),
				"name" => Some(FieldType::Char {
					max_length: Some(100),
				}),
				"email" => Some(FieldType::Email),
				_ => None,
			}
		}

		fn get_field(&self, name: &str) -> Option<Value> {
			match name {
				"id" => self.id.map(|id| Value::Number(id.into())),
				"name" => Some(Value::String(self.name.clone())),
				"email" => Some(Value::String(self.email.clone())),
				_ => None,
			}
		}

		fn set_field(&mut self, name: &str, value: Value) -> Result<(), String> {
			match name {
				"id" => {
					self.id = value.as_i64();
					Ok(())
				}
				"name" => {
					self.name = value
						.as_str()
						.ok_or("Expected string for name")?
						.to_string();
					Ok(())
				}
				"email" => {
					self.email = value
						.as_str()
						.ok_or("Expected string for email")?
						.to_string();
					Ok(())
				}
				_ => Err(format!("Unknown field: {}", name)),
			}
		}

		fn save(&mut self) -> Result<(), String> {
			if self.id.is_none() {
				self.id = Some(1);
			}
			Ok(())
		}
	}

	// Test child model
	#[derive(Clone)]
	struct ChildModel {
		id: Option<i64>,
		parent_id: Option<i64>,
		content: String,
	}

	impl FormModel for ChildModel {
		fn field_names() -> Vec<String> {
			vec![
				"id".to_string(),
				"parent_id".to_string(),
				"content".to_string(),
			]
		}

		fn field_type(name: &str) -> Option<FieldType> {
			match name {
				"id" | "parent_id" => Some(FieldType::Integer),
				"content" => Some(FieldType::Text),
				_ => None,
			}
		}

		fn get_field(&self, name: &str) -> Option<Value> {
			match name {
				"id" => self.id.map(|id| Value::Number(id.into())),
				"parent_id" => self.parent_id.map(|id| Value::Number(id.into())),
				"content" => Some(Value::String(self.content.clone())),
				_ => None,
			}
		}

		fn set_field(&mut self, name: &str, value: Value) -> Result<(), String> {
			match name {
				"id" => {
					self.id = value.as_i64();
					Ok(())
				}
				"parent_id" => {
					self.parent_id = value.as_i64();
					Ok(())
				}
				"content" => {
					self.content = value
						.as_str()
						.ok_or("Expected string for content")?
						.to_string();
					Ok(())
				}
				_ => Err(format!("Unknown field: {}", name)),
			}
		}

		fn save(&mut self) -> Result<(), String> {
			if self.id.is_none() {
				self.id = Some(1);
			}
			Ok(())
		}
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
			parent_id: None,
			content: "Child content".to_string(),
		};
		let child_form = ModelForm::new(Some(child), ModelFormConfig::new());
		formset.add_child_form(child_form);

		assert_eq!(formset.child_forms().len(), 1);
	}

	#[test]
	fn test_inline_formset_save() {
		let parent = TestModel {
			id: Some(1),
			name: "Parent".to_string(),
			email: "parent@example.com".to_string(),
		};

		let mut formset =
			InlineFormSet::<TestModel, ChildModel>::new(parent, "parent_id".to_string());

		let child = ChildModel {
			id: None,
			parent_id: None,
			content: "Child content".to_string(),
		};
		let child_form = ModelForm::new(Some(child), ModelFormConfig::new());
		formset.add_child_form(child_form);

		let result = formset.save();
		assert!(result.is_ok());
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
		let form = ModelForm::new(Some(instance), ModelFormConfig::new());
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
		let form = ModelForm::new(Some(instance), ModelFormConfig::new());
		formset.add_form(form).unwrap();

		assert!(!formset.is_valid());
		assert!(!formset.errors().is_empty());
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
