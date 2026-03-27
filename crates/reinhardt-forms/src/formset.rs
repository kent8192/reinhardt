use crate::form::Form;
use std::collections::HashMap;

/// FormSet manages multiple forms
pub struct FormSet {
	forms: Vec<Form>,
	prefix: String,
	can_delete: bool,
	can_order: bool,
	extra: usize,
	max_num: Option<usize>,
	min_num: usize,
	errors: Vec<String>,
}

impl FormSet {
	/// Create a new FormSet with the given prefix
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FormSet;
	///
	/// let formset = FormSet::new("form".to_string());
	/// assert_eq!(formset.prefix(), "form");
	/// assert!(!formset.can_delete());
	/// ```
	pub fn new(prefix: String) -> Self {
		// Normalize: strip trailing delimiter so that `format!("{}-", prefix)`
		// in `process_data` never produces a double-dash (e.g. "form--")
		let prefix = prefix
			.strip_suffix('-')
			.map_or(prefix.clone(), |s| s.to_owned());
		Self {
			forms: vec![],
			prefix,
			can_delete: false,
			can_order: false,
			extra: 1,
			max_num: Some(1000),
			min_num: 0,
			errors: vec![],
		}
	}

	/// Returns the prefix used for form field naming.
	pub fn prefix(&self) -> &str {
		&self.prefix
	}

	/// Returns whether forms in this set can be deleted.
	pub fn can_delete(&self) -> bool {
		self.can_delete
	}
	/// Sets the number of extra empty forms to include.
	pub fn with_extra(mut self, extra: usize) -> Self {
		self.extra = extra;
		self
	}
	/// Enables or disables form deletion within this set.
	pub fn with_can_delete(mut self, can_delete: bool) -> Self {
		self.can_delete = can_delete;
		self
	}
	/// Enables or disables ordering of forms within this set.
	pub fn with_can_order(mut self, can_order: bool) -> Self {
		self.can_order = can_order;
		self
	}
	/// Sets the maximum number of forms allowed.
	pub fn with_max_num(mut self, max_num: Option<usize>) -> Self {
		self.max_num = max_num;
		self
	}
	/// Sets the minimum number of forms required for validation.
	pub fn with_min_num(mut self, min_num: usize) -> Self {
		self.min_num = min_num;
		self
	}
	/// Add a form to the formset.
	///
	/// Returns an error if adding the form would exceed `max_num`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{FormSet, Form};
	///
	/// let mut formset = FormSet::new("form".to_string());
	/// let form = Form::new();
	/// assert!(formset.add_form(form).is_ok());
	/// assert_eq!(formset.forms().len(), 1);
	/// ```
	///
	/// ```
	/// use reinhardt_forms::{FormSet, Form};
	///
	/// let mut formset = FormSet::new("form".to_string()).with_max_num(Some(1));
	/// assert!(formset.add_form(Form::new()).is_ok());
	/// assert!(formset.add_form(Form::new()).is_err());
	/// ```
	pub fn add_form(&mut self, form: Form) -> Result<(), String> {
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
	/// Returns a slice of all forms in this set.
	pub fn forms(&self) -> &[Form] {
		&self.forms
	}
	/// Returns a mutable reference to the forms vector.
	pub fn forms_mut(&mut self) -> &mut Vec<Form> {
		&mut self.forms
	}
	/// Returns the number of forms currently in this set.
	pub fn form_count(&self) -> usize {
		self.forms.len()
	}
	/// Returns the total form count including extra empty forms.
	pub fn total_form_count(&self) -> usize {
		self.forms.len() + self.extra
	}
	/// Validate all forms in the formset
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{FormSet, Form};
	///
	/// let mut formset = FormSet::new("form".to_string());
	/// formset.add_form(Form::new()).unwrap();
	// Note: is_valid() requires mutable reference
	// let is_valid = formset.is_valid();
	/// ```
	pub fn is_valid(&mut self) -> bool {
		self.errors.clear();

		// Validate individual forms
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

		all_valid && self.errors.is_empty()
	}
	/// Returns the formset-level validation errors.
	pub fn errors(&self) -> &[String] {
		&self.errors
	}
	/// Returns cleaned data from all forms in the set.
	pub fn cleaned_data(&self) -> Vec<&HashMap<String, serde_json::Value>> {
		self.forms.iter().map(|f| f.cleaned_data()).collect()
	}
	/// Get management form data (for tracking forms in HTML)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FormSet;
	///
	/// let formset = FormSet::new("form".to_string());
	/// let data = formset.management_form_data();
	/// assert!(data.contains_key("form-TOTAL_FORMS"));
	/// ```
	pub fn management_form_data(&self) -> HashMap<String, String> {
		let mut data = HashMap::new();
		data.insert(
			format!("{}-TOTAL_FORMS", self.prefix),
			self.total_form_count().to_string(),
		);
		data.insert(
			format!("{}-INITIAL_FORMS", self.prefix),
			self.forms.len().to_string(),
		);
		data.insert(
			format!("{}-MIN_NUM_FORMS", self.prefix),
			self.min_num.to_string(),
		);
		if let Some(max) = self.max_num {
			data.insert(format!("{}-MAX_NUM_FORMS", self.prefix), max.to_string());
		}
		data
	}
	/// Process bound data from HTML forms.
	///
	/// Respects `max_num` and silently stops adding forms once the limit is reached.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FormSet;
	/// use std::collections::HashMap;
	/// use serde_json::json;
	///
	/// let mut formset = FormSet::new("form".to_string());
	/// let mut data = HashMap::new();
	/// let mut form_data = HashMap::new();
	/// form_data.insert("field".to_string(), json!("value"));
	/// data.insert("form-0".to_string(), form_data);
	///
	/// formset.process_data(&data);
	/// assert_eq!(formset.form_count(), 1);
	/// ```
	pub fn process_data(&mut self, data: &HashMap<String, HashMap<String, serde_json::Value>>) {
		self.forms.clear();

		// Sort keys for deterministic ordering when max_num limit is applied
		let mut keys: Vec<&String> = data.keys().collect();
		keys.sort();

		// Each form should have a key like "form-0", "form-1", etc.
		// Use exact prefix matching with delimiter to prevent collisions
		// (e.g., prefix "item" should not match "item_extra-0")
		let prefix_with_delimiter = format!("{}-", self.prefix);
		for key in keys {
			if key.starts_with(&prefix_with_delimiter) {
				// Enforce max_num limit during data processing
				if let Some(max) = self.max_num
					&& self.forms.len() >= max
				{
					break;
				}
				if let Some(form_data) = data.get(key) {
					let mut form = Form::new();
					form.bind(form_data.clone());
					self.forms.push(form);
				}
			}
		}
	}
}

impl Default for FormSet {
	fn default() -> Self {
		Self::new("form".to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::fields::CharField;
	use rstest::rstest;

	#[test]
	fn test_formset_basic() {
		let mut formset = FormSet::new("person".to_string());

		let mut form1 = Form::new();
		form1.add_field(Box::new(CharField::new("name".to_string())));

		let mut form2 = Form::new();
		form2.add_field(Box::new(CharField::new("name".to_string())));

		formset.add_form(form1).unwrap();
		formset.add_form(form2).unwrap();

		assert_eq!(formset.form_count(), 2);
	}

	#[test]
	fn test_formset_min_num_validation() {
		let mut formset = FormSet::new("person".to_string()).with_min_num(2);

		let mut form1 = Form::new();
		form1.add_field(Box::new(CharField::new("name".to_string())));
		formset.add_form(form1).unwrap();

		assert!(!formset.is_valid());
		assert!(!formset.errors().is_empty());
	}

	#[test]
	fn test_formset_max_num_enforced_on_add() {
		let mut formset = FormSet::new("person".to_string()).with_max_num(Some(2));

		let mut form1 = Form::new();
		form1.add_field(Box::new(CharField::new("name".to_string())));
		assert!(formset.add_form(form1).is_ok());

		let mut form2 = Form::new();
		form2.add_field(Box::new(CharField::new("name".to_string())));
		assert!(formset.add_form(form2).is_ok());

		// Third form should be rejected
		let mut form3 = Form::new();
		form3.add_field(Box::new(CharField::new("name".to_string())));
		assert!(formset.add_form(form3).is_err());

		assert_eq!(formset.form_count(), 2);
	}

	#[rstest]
	fn test_process_data_basic_two_forms() {
		// Arrange
		let mut formset = FormSet::new("form".to_string());
		let mut data = HashMap::new();

		let mut form0_data = HashMap::new();
		form0_data.insert("name".to_string(), serde_json::json!("Alice"));
		data.insert("form-0".to_string(), form0_data);

		let mut form1_data = HashMap::new();
		form1_data.insert("name".to_string(), serde_json::json!("Bob"));
		data.insert("form-1".to_string(), form1_data);

		// Act
		formset.process_data(&data);

		// Assert
		assert_eq!(formset.form_count(), 2);
	}

	#[rstest]
	fn test_process_data_deterministic_ordering() {
		// Arrange
		let mut formset = FormSet::new("form".to_string());
		let mut data = HashMap::new();

		// Insert in reverse order to verify sorting
		let mut form2_data = HashMap::new();
		form2_data.insert("name".to_string(), serde_json::json!("Charlie"));
		data.insert("form-2".to_string(), form2_data);

		let mut form0_data = HashMap::new();
		form0_data.insert("name".to_string(), serde_json::json!("Alice"));
		data.insert("form-0".to_string(), form0_data);

		let mut form1_data = HashMap::new();
		form1_data.insert("name".to_string(), serde_json::json!("Bob"));
		data.insert("form-1".to_string(), form1_data);

		// Act
		formset.process_data(&data);

		// Assert
		assert_eq!(formset.form_count(), 3);
		let cleaned: Vec<_> = formset.cleaned_data();
		assert_eq!(cleaned[0].get("name"), Some(&serde_json::json!("Alice")));
		assert_eq!(cleaned[1].get("name"), Some(&serde_json::json!("Bob")));
		assert_eq!(cleaned[2].get("name"), Some(&serde_json::json!("Charlie")));
	}

	#[rstest]
	fn test_process_data_max_num_constraint() {
		// Arrange
		let mut formset = FormSet::new("form".to_string()).with_max_num(Some(2));
		let mut data = HashMap::new();

		for i in 0..5 {
			let mut form_data = HashMap::new();
			form_data.insert("name".to_string(), serde_json::json!(format!("User{}", i)));
			data.insert(format!("form-{}", i), form_data);
		}

		// Act
		formset.process_data(&data);

		// Assert
		assert_eq!(formset.form_count(), 2);
	}

	#[rstest]
	fn test_process_data_prefix_mismatch_keys_ignored() {
		// Arrange
		let mut formset = FormSet::new("person".to_string());
		let mut data = HashMap::new();

		let mut matching = HashMap::new();
		matching.insert("name".to_string(), serde_json::json!("Alice"));
		data.insert("person-0".to_string(), matching);

		let mut mismatched = HashMap::new();
		mismatched.insert("name".to_string(), serde_json::json!("Bob"));
		data.insert("form-0".to_string(), mismatched);

		// Act
		formset.process_data(&data);

		// Assert
		assert_eq!(formset.form_count(), 1);
		let cleaned = formset.cleaned_data();
		assert_eq!(cleaned[0].get("name"), Some(&serde_json::json!("Alice")));
	}

	#[rstest]
	fn test_process_data_prefix_collision_prevented() {
		// Arrange: prefix "item" should NOT match "item_extra-0"
		let mut formset = FormSet::new("item".to_string());
		let mut data = HashMap::new();

		let mut matching = HashMap::new();
		matching.insert("name".to_string(), serde_json::json!("Apple"));
		data.insert("item-0".to_string(), matching);

		let mut colliding = HashMap::new();
		colliding.insert("name".to_string(), serde_json::json!("Banana"));
		data.insert("item_extra-0".to_string(), colliding);

		// Act
		formset.process_data(&data);

		// Assert: only "item-0" should be processed, not "item_extra-0"
		assert_eq!(formset.form_count(), 1);
		let cleaned = formset.cleaned_data();
		assert_eq!(cleaned[0].get("name"), Some(&serde_json::json!("Apple")));
	}

	#[rstest]
	fn test_process_data_similar_prefix_no_collision() {
		// Arrange: prefix "form" should NOT match "formset-0" or "form2-0"
		let mut formset = FormSet::new("form".to_string());
		let mut data = HashMap::new();

		let mut matching = HashMap::new();
		matching.insert("name".to_string(), serde_json::json!("Valid"));
		data.insert("form-0".to_string(), matching);

		let mut similar1 = HashMap::new();
		similar1.insert("name".to_string(), serde_json::json!("Invalid1"));
		data.insert("formset-0".to_string(), similar1);

		let mut similar2 = HashMap::new();
		similar2.insert("name".to_string(), serde_json::json!("Invalid2"));
		data.insert("form2-0".to_string(), similar2);

		// Act
		formset.process_data(&data);

		// Assert: only "form-0" should be processed
		assert_eq!(formset.form_count(), 1);
		let cleaned = formset.cleaned_data();
		assert_eq!(cleaned[0].get("name"), Some(&serde_json::json!("Valid")));
	}

	#[test]
	fn test_forms_formset_management_data() {
		let formset = FormSet::new("person".to_string())
			.with_extra(3)
			.with_min_num(1)
			.with_max_num(Some(10));

		let mgmt_data = formset.management_form_data();

		assert_eq!(mgmt_data.get("person-TOTAL_FORMS"), Some(&"3".to_string()));
		assert_eq!(
			mgmt_data.get("person-INITIAL_FORMS"),
			Some(&"0".to_string())
		);
		assert_eq!(
			mgmt_data.get("person-MIN_NUM_FORMS"),
			Some(&"1".to_string())
		);
		assert_eq!(
			mgmt_data.get("person-MAX_NUM_FORMS"),
			Some(&"10".to_string())
		);
	}
}
