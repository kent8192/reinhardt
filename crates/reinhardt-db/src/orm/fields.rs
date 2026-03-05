// Field definitions and deconstruction API
// Corresponds to Django's field system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Field deconstruction result
/// Returns (name, path, args, kwargs) similar to Django's deconstruct()
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDeconstruction {
	pub name: Option<String>,
	pub path: String,
	pub args: Vec<FieldArg>,
	pub kwargs: HashMap<String, FieldKwarg>,
}

/// Positional argument for field construction
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldArg {
	String(String),
	Int(i64),
	Bool(bool),
	Float(f64),
}

/// Keyword argument for field construction
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldKwarg {
	String(String),
	Int(i64),
	Uint(u64),
	Bool(bool),
	Float(f64),
	Choices(Vec<(String, String)>),
	Callable(String), // Function name as string
}

/// Core Field trait - all field types implement this
pub trait Field: Send + Sync {
	/// Deconstruct the field into a serializable representation
	/// Returns (name, path, args, kwargs)
	fn deconstruct(&self) -> FieldDeconstruction;

	/// Set field name and attributes from model introspection
	fn set_attributes_from_name(&mut self, name: &str);

	/// Get field name
	fn name(&self) -> Option<&str>;

	/// Check if field is a primary key
	fn is_primary_key(&self) -> bool {
		false
	}

	/// Check if field allows null
	fn is_null(&self) -> bool {
		false
	}

	/// Check if field allows blank
	fn is_blank(&self) -> bool {
		false
	}
}

/// Base field attributes shared by all fields
#[derive(Debug, Clone)]
pub struct BaseField {
	pub name: Option<String>,
	pub null: bool,
	pub blank: bool,
	pub default: Option<FieldKwarg>,
	pub db_default: Option<FieldKwarg>, // Database-level default value
	pub db_column: Option<String>,
	pub db_tablespace: Option<String>,
	pub primary_key: bool,
	pub unique: bool,
	pub editable: bool,
	pub choices: Option<Vec<(String, String)>>,
}

impl BaseField {
	/// Creates a new BaseField with default values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::BaseField;
	///
	/// let field = BaseField::new();
	/// assert!(!field.null);
	/// assert!(!field.blank);
	/// assert!(!field.primary_key);
	/// assert!(!field.unique);
	/// assert!(field.editable);
	/// ```
	pub fn new() -> Self {
		Self {
			name: None,
			null: false,
			blank: false,
			default: None,
			db_default: None,
			db_column: None,
			db_tablespace: None,
			primary_key: false,
			unique: false,
			editable: true,
			choices: None,
		}
	}
	/// Extract non-default kwargs for deconstruction
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{BaseField, FieldKwarg};
	/// use std::collections::HashMap;
	///
	/// let mut field = BaseField::new();
	/// field.null = true;
	/// field.blank = true;
	/// field.primary_key = true;
	///
	/// let kwargs = field.get_kwargs();
	/// assert_eq!(kwargs.get("null"), Some(&FieldKwarg::Bool(true)));
	/// assert_eq!(kwargs.get("blank"), Some(&FieldKwarg::Bool(true)));
	/// assert_eq!(kwargs.get("primary_key"), Some(&FieldKwarg::Bool(true)));
	/// ```
	pub fn get_kwargs(&self) -> HashMap<String, FieldKwarg> {
		let mut kwargs = HashMap::new();

		if self.null {
			kwargs.insert("null".to_string(), FieldKwarg::Bool(true));
		}
		if self.blank {
			kwargs.insert("blank".to_string(), FieldKwarg::Bool(true));
		}
		if let Some(ref default) = self.default {
			kwargs.insert("default".to_string(), default.clone());
		}
		if let Some(ref db_default) = self.db_default {
			kwargs.insert("db_default".to_string(), db_default.clone());
		}
		if let Some(ref db_column) = self.db_column {
			kwargs.insert(
				"db_column".to_string(),
				FieldKwarg::String(db_column.clone()),
			);
		}
		if let Some(ref db_tablespace) = self.db_tablespace {
			kwargs.insert(
				"db_tablespace".to_string(),
				FieldKwarg::String(db_tablespace.clone()),
			);
		}
		if self.primary_key {
			kwargs.insert("primary_key".to_string(), FieldKwarg::Bool(true));
		}
		if self.unique {
			kwargs.insert("unique".to_string(), FieldKwarg::Bool(true));
		}
		if !self.editable {
			kwargs.insert("editable".to_string(), FieldKwarg::Bool(false));
		}
		if let Some(ref choices) = self.choices {
			kwargs.insert("choices".to_string(), FieldKwarg::Choices(choices.clone()));
		}

		kwargs
	}
}

impl Default for BaseField {
	fn default() -> Self {
		Self::new()
	}
}

/// AutoField - auto-incrementing integer primary key
#[derive(Debug, Clone)]
pub struct AutoField {
	pub base: BaseField,
}

impl Default for AutoField {
	fn default() -> Self {
		Self::new()
	}
}

impl AutoField {
	/// Create a new auto-incrementing primary key field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{AutoField, Field};
	///
	/// let mut id_field = AutoField::new();
	/// id_field.set_attributes_from_name("id");
	/// assert!(id_field.is_primary_key());
	/// assert_eq!(id_field.name(), Some("id"));
	/// ```
	pub fn new() -> Self {
		let mut base = BaseField::new();
		base.primary_key = true;
		Self { base }
	}
}

impl Field for AutoField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let kwargs = self.base.get_kwargs();

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.AutoField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}

	fn is_primary_key(&self) -> bool {
		self.base.primary_key
	}
}

/// BigIntegerField
#[derive(Debug, Clone)]
pub struct BigIntegerField {
	pub base: BaseField,
}

impl Default for BigIntegerField {
	fn default() -> Self {
		Self::new()
	}
}

impl BigIntegerField {
	/// Create a new big integer field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{BigIntegerField, Field};
	///
	/// let mut population_field = BigIntegerField::new();
	/// population_field.set_attributes_from_name("population");
	/// let deconstruction = population_field.deconstruct();
	/// assert_eq!(deconstruction.path, "reinhardt.orm.models.BigIntegerField");
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
		}
	}
}

impl Field for BigIntegerField {
	fn deconstruct(&self) -> FieldDeconstruction {
		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.BigIntegerField".to_string(),
			args: vec![],
			kwargs: self.base.get_kwargs(),
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// BooleanField
#[derive(Debug, Clone)]
pub struct BooleanField {
	pub base: BaseField,
}

impl Default for BooleanField {
	fn default() -> Self {
		Self::new()
	}
}

impl BooleanField {
	/// Create a new boolean field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::BooleanField;
	///
	/// let is_active = BooleanField::new();
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
		}
	}
	/// Create a boolean field with a default value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{BooleanField, Field, FieldKwarg};
	///
	/// let is_active = BooleanField::with_default(true);
	/// let dec = is_active.deconstruct();
	/// assert_eq!(dec.kwargs.get("default"), Some(&FieldKwarg::Bool(true)));
	/// ```
	pub fn with_default(default: bool) -> Self {
		let mut field = Self::new();
		field.base.default = Some(FieldKwarg::Bool(default));
		field
	}
}

impl Field for BooleanField {
	fn deconstruct(&self) -> FieldDeconstruction {
		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.BooleanField".to_string(),
			args: vec![],
			kwargs: self.base.get_kwargs(),
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// CharField - text field with max_length
#[derive(Debug, Clone)]
pub struct CharField {
	pub base: BaseField,
	pub max_length: u64,
}

impl CharField {
	/// Create a new character field with maximum length
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field, FieldKwarg};
	///
	/// let username_field = CharField::new(150);
	/// let dec = username_field.deconstruct();
	/// assert_eq!(dec.kwargs.get("max_length"), Some(&FieldKwarg::Uint(150)));
	/// ```
	pub fn new(max_length: u64) -> Self {
		Self {
			base: BaseField::new(),
			max_length,
		}
	}
	/// Create a character field that allows NULL and blank values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field, FieldKwarg};
	///
	/// let middle_name = CharField::with_null_blank(100);
	/// let dec = middle_name.deconstruct();
	/// assert_eq!(dec.kwargs.get("null"), Some(&FieldKwarg::Bool(true)));
	/// assert_eq!(dec.kwargs.get("blank"), Some(&FieldKwarg::Bool(true)));
	/// ```
	pub fn with_null_blank(max_length: u64) -> Self {
		let mut base = BaseField::new();
		base.null = true;
		base.blank = true;
		Self { base, max_length }
	}
	/// Create a character field with predefined choices
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::CharField;
	///
	/// let status = CharField::with_choices(
	///     10,
	///     vec![
	///         ("draft".to_string(), "Draft".to_string()),
	///         ("published".to_string(), "Published".to_string()),
	///     ],
	/// );
	/// ```
	pub fn with_choices(max_length: u64, choices: Vec<(String, String)>) -> Self {
		let mut base = BaseField::new();
		base.choices = Some(choices);
		Self { base, max_length }
	}
}

impl Field for CharField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();
		kwargs.insert("max_length".to_string(), FieldKwarg::Uint(self.max_length));

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.CharField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// IntegerField
#[derive(Debug, Clone)]
pub struct IntegerField {
	pub base: BaseField,
}

impl Default for IntegerField {
	fn default() -> Self {
		Self::new()
	}
}

impl IntegerField {
	/// Create a new IntegerField
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::IntegerField;
	///
	/// let field = IntegerField::new();
	/// assert!(field.base.choices.is_none());
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
		}
	}
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::IntegerField;
	///
	/// let choices = vec![
	///     ("1".to_string(), "Option 1".to_string()),
	///     ("2".to_string(), "Option 2".to_string()),
	/// ];
	/// let field = IntegerField::with_choices(choices.clone());
	/// assert_eq!(field.base.choices.unwrap().len(), 2);
	/// ```
	pub fn with_choices(choices: Vec<(String, String)>) -> Self {
		let mut base = BaseField::new();
		base.choices = Some(choices);
		Self { base }
	}
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::IntegerField;
	///
	/// let field = IntegerField::with_callable_choices("get_status_choices");
	/// // The callable name would be used to dynamically generate choices at runtime
	/// assert!(field.base.choices.is_none()); // Callable choices are handled separately
	/// ```
	pub fn with_callable_choices(_callable_name: &str) -> Self {
		// Store callable as a special marker in base

		// We'll handle callable differently in deconstruct
		Self::new()
	}
}

impl Field for IntegerField {
	fn deconstruct(&self) -> FieldDeconstruction {
		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.IntegerField".to_string(),
			args: vec![],
			kwargs: self.base.get_kwargs(),
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// DateField
#[derive(Debug, Clone)]
pub struct DateField {
	pub base: BaseField,
	pub auto_now: bool,
	pub auto_now_add: bool,
}

impl Default for DateField {
	fn default() -> Self {
		Self::new()
	}
}

impl DateField {
	/// Create a new DateField without auto timestamps
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::DateField;
	///
	/// let field = DateField::new();
	/// assert!(!field.auto_now);
	/// assert!(!field.auto_now_add);
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
			auto_now: false,
			auto_now_add: false,
		}
	}
	/// Create DateField that auto-updates on every save (like Django's auto_now)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::DateField;
	///
	/// let field = DateField::with_auto_now();
	/// assert!(field.auto_now);
	/// assert!(!field.auto_now_add);
	/// ```
	pub fn with_auto_now() -> Self {
		Self {
			base: BaseField::new(),
			auto_now: true,
			auto_now_add: false,
		}
	}
}

impl Field for DateField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();
		if self.auto_now {
			kwargs.insert("auto_now".to_string(), FieldKwarg::Bool(true));
		}
		if self.auto_now_add {
			kwargs.insert("auto_now_add".to_string(), FieldKwarg::Bool(true));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.DateField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// DateTimeField
#[derive(Debug, Clone)]
pub struct DateTimeField {
	pub base: BaseField,
	pub auto_now: bool,
	pub auto_now_add: bool,
}

impl Default for DateTimeField {
	fn default() -> Self {
		Self::new()
	}
}

impl DateTimeField {
	/// Create a new DateTimeField without auto timestamps
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::DateTimeField;
	///
	/// let field = DateTimeField::new();
	/// assert!(!field.auto_now);
	/// assert!(!field.auto_now_add);
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
			auto_now: false,
			auto_now_add: false,
		}
	}
	/// Create DateTimeField that auto-sets on creation (like Django's auto_now_add)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::DateTimeField;
	///
	/// let field = DateTimeField::with_auto_now_add();
	/// assert!(!field.auto_now);
	/// assert!(field.auto_now_add);
	/// ```
	pub fn with_auto_now_add() -> Self {
		Self {
			base: BaseField::new(),
			auto_now: false,
			auto_now_add: true,
		}
	}
	/// Create DateTimeField with both auto_now and auto_now_add enabled
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::DateTimeField;
	///
	/// let field = DateTimeField::with_both();
	/// assert!(field.auto_now);
	/// assert!(field.auto_now_add);
	/// ```
	pub fn with_both() -> Self {
		Self {
			base: BaseField::new(),
			auto_now: true,
			auto_now_add: true,
		}
	}
}

impl Field for DateTimeField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();
		if self.auto_now {
			kwargs.insert("auto_now".to_string(), FieldKwarg::Bool(true));
		}
		if self.auto_now_add {
			kwargs.insert("auto_now_add".to_string(), FieldKwarg::Bool(true));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.DateTimeField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// DecimalField
#[derive(Debug, Clone)]
pub struct DecimalField {
	pub base: BaseField,
	pub max_digits: u32,
	pub decimal_places: u32,
}

impl DecimalField {
	/// Create a new DecimalField with precision settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::DecimalField;
	///
	/// // For monetary values: max 10 digits, 2 decimal places
	/// let price_field = DecimalField::new(10, 2);
	/// assert_eq!(price_field.max_digits, 10);
	/// assert_eq!(price_field.decimal_places, 2);
	/// ```
	pub fn new(max_digits: u32, decimal_places: u32) -> Self {
		Self {
			base: BaseField::new(),
			max_digits,
			decimal_places,
		}
	}
}

impl Field for DecimalField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();
		kwargs.insert(
			"max_digits".to_string(),
			FieldKwarg::Uint(self.max_digits as u64),
		);
		kwargs.insert(
			"decimal_places".to_string(),
			FieldKwarg::Uint(self.decimal_places as u64),
		);

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.DecimalField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// EmailField
#[derive(Debug, Clone)]
pub struct EmailField {
	pub base: BaseField,
	pub max_length: u64,
}

impl Default for EmailField {
	fn default() -> Self {
		Self::new()
	}
}

impl EmailField {
	/// Create a new EmailField with Django's default max_length (254)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::EmailField;
	///
	/// let field = EmailField::new();
	/// assert_eq!(field.max_length, 254);
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
			max_length: 254, // Django default
		}
	}
	/// Create EmailField with custom max_length
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::EmailField;
	///
	/// let field = EmailField::with_max_length(100);
	/// assert_eq!(field.max_length, 100);
	/// ```
	pub fn with_max_length(max_length: u64) -> Self {
		Self {
			base: BaseField::new(),
			max_length,
		}
	}
}

impl Field for EmailField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();
		kwargs.insert("max_length".to_string(), FieldKwarg::Uint(self.max_length));

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.EmailField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// FloatField
#[derive(Debug, Clone)]
pub struct FloatField {
	pub base: BaseField,
}

impl Default for FloatField {
	fn default() -> Self {
		Self::new()
	}
}

impl FloatField {
	/// Create a new FloatField for storing floating-point numbers
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::FloatField;
	///
	/// let field = FloatField::new();
	/// assert!(field.base.choices.is_none());
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
		}
	}
}

impl Field for FloatField {
	fn deconstruct(&self) -> FieldDeconstruction {
		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.FloatField".to_string(),
			args: vec![],
			kwargs: self.base.get_kwargs(),
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// TextField
#[derive(Debug, Clone)]
pub struct TextField {
	pub base: BaseField,
}

impl Default for TextField {
	fn default() -> Self {
		Self::new()
	}
}

impl TextField {
	/// Create a new TextField for storing large text
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::TextField;
	///
	/// let field = TextField::new();
	/// assert!(field.base.name.is_none());
	/// assert!(!field.base.null);
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
		}
	}
}

impl Field for TextField {
	fn deconstruct(&self) -> FieldDeconstruction {
		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.TextField".to_string(),
			args: vec![],
			kwargs: self.base.get_kwargs(),
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// TimeField
#[derive(Debug, Clone)]
pub struct TimeField {
	pub base: BaseField,
	pub auto_now: bool,
	pub auto_now_add: bool,
}

impl Default for TimeField {
	fn default() -> Self {
		Self::new()
	}
}

impl TimeField {
	/// Create a new TimeField for storing time values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::TimeField;
	///
	/// let field = TimeField::new();
	/// assert!(!field.auto_now);
	/// assert!(!field.auto_now_add);
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
			auto_now: false,
			auto_now_add: false,
		}
	}
	/// Documentation for `with_auto_now`
	pub fn with_auto_now() -> Self {
		Self {
			base: BaseField::new(),
			auto_now: true,
			auto_now_add: false,
		}
	}
	/// Documentation for `with_auto_now_add`
	pub fn with_auto_now_add() -> Self {
		Self {
			base: BaseField::new(),
			auto_now: false,
			auto_now_add: true,
		}
	}
}

impl Field for TimeField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();
		if self.auto_now {
			kwargs.insert("auto_now".to_string(), FieldKwarg::Bool(true));
		}
		if self.auto_now_add {
			kwargs.insert("auto_now_add".to_string(), FieldKwarg::Bool(true));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.TimeField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// URLField
#[derive(Debug, Clone)]
pub struct URLField {
	pub base: BaseField,
	pub max_length: u64,
}

impl Default for URLField {
	fn default() -> Self {
		Self::new()
	}
}

impl URLField {
	/// Create a new URLField for storing and validating URLs
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::URLField;
	///
	/// let field = URLField::new();
	/// assert_eq!(field.max_length, 200); // Django's default
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
			max_length: 200, // Django default
		}
	}
	/// Documentation for `with_max_length`
	pub fn with_max_length(max_length: u64) -> Self {
		Self {
			base: BaseField::new(),
			max_length,
		}
	}
}

impl Field for URLField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();
		// Only include max_length if it's not the default
		if self.max_length != 200 {
			kwargs.insert("max_length".to_string(), FieldKwarg::Uint(self.max_length));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.URLField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// BinaryField
#[derive(Debug, Clone)]
pub struct BinaryField {
	pub base: BaseField,
}

impl Default for BinaryField {
	fn default() -> Self {
		Self::new()
	}
}

impl BinaryField {
	/// Create a new BinaryField for storing raw binary data (not editable by default)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::BinaryField;
	///
	/// let field = BinaryField::new();
	/// assert!(!field.base.editable); // Binary fields are not editable by default
	/// ```
	pub fn new() -> Self {
		let mut base = BaseField::new();
		base.editable = false; // Django default
		Self { base }
	}
	/// Create BinaryField that is editable in forms
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::BinaryField;
	///
	/// let field = BinaryField::with_editable();
	/// assert!(field.base.editable);
	/// ```
	pub fn with_editable() -> Self {
		let mut base = BaseField::new();
		base.editable = true;
		Self { base }
	}
}

impl Field for BinaryField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();
		// BinaryField default is editable=false, so remove it from kwargs if false
		// but add it if true (non-default)
		kwargs.remove("editable");
		if self.base.editable {
			kwargs.insert("editable".to_string(), FieldKwarg::Bool(true));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.BinaryField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// SlugField
#[derive(Debug, Clone)]
pub struct SlugField {
	pub base: BaseField,
	pub max_length: u64,
	pub db_index: bool,
}

impl Default for SlugField {
	fn default() -> Self {
		Self::new()
	}
}

impl SlugField {
	/// Create a new SlugField for URL-friendly strings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::SlugField;
	///
	/// let field = SlugField::new();
	/// assert_eq!(field.max_length, 50); // Django's default
	/// assert!(field.db_index); // Automatically indexed
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
			max_length: 50, // Django default
			db_index: true, // Django default
		}
	}
	/// Documentation for `with_options`
	pub fn with_options(max_length: u64, db_index: bool) -> Self {
		Self {
			base: BaseField::new(),
			max_length,
			db_index,
		}
	}
}

impl Field for SlugField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();
		// Only include non-default values
		if self.max_length != 50 {
			kwargs.insert("max_length".to_string(), FieldKwarg::Uint(self.max_length));
		}
		if !self.db_index {
			kwargs.insert("db_index".to_string(), FieldKwarg::Bool(false));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.SlugField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// SmallIntegerField
#[derive(Debug, Clone)]
pub struct SmallIntegerField {
	pub base: BaseField,
}

impl Default for SmallIntegerField {
	fn default() -> Self {
		Self::new()
	}
}

impl SmallIntegerField {
	/// Create a new SmallIntegerField for small integers (-32768 to 32767)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::SmallIntegerField;
	///
	/// let field = SmallIntegerField::new();
	/// assert!(field.base.name.is_none());
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
		}
	}
}

impl Field for SmallIntegerField {
	fn deconstruct(&self) -> FieldDeconstruction {
		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.SmallIntegerField".to_string(),
			args: vec![],
			kwargs: self.base.get_kwargs(),
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// PositiveIntegerField
#[derive(Debug, Clone)]
pub struct PositiveIntegerField {
	pub base: BaseField,
}

impl Default for PositiveIntegerField {
	fn default() -> Self {
		Self::new()
	}
}

impl PositiveIntegerField {
	/// Create a new PositiveIntegerField for positive integers (0 to 2147483647)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::PositiveIntegerField;
	///
	/// let field = PositiveIntegerField::new();
	/// assert!(field.base.name.is_none());
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
		}
	}
}

impl Field for PositiveIntegerField {
	fn deconstruct(&self) -> FieldDeconstruction {
		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.PositiveIntegerField".to_string(),
			args: vec![],
			kwargs: self.base.get_kwargs(),
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// PositiveSmallIntegerField
#[derive(Debug, Clone)]
pub struct PositiveSmallIntegerField {
	pub base: BaseField,
}

impl Default for PositiveSmallIntegerField {
	fn default() -> Self {
		Self::new()
	}
}

impl PositiveSmallIntegerField {
	/// Create a new PositiveSmallIntegerField for small positive integers (0 to 32767)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::PositiveSmallIntegerField;
	///
	/// let field = PositiveSmallIntegerField::new();
	/// assert!(field.base.name.is_none());
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
		}
	}
}

impl Field for PositiveSmallIntegerField {
	fn deconstruct(&self) -> FieldDeconstruction {
		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.PositiveSmallIntegerField".to_string(),
			args: vec![],
			kwargs: self.base.get_kwargs(),
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// PositiveBigIntegerField
#[derive(Debug, Clone)]
pub struct PositiveBigIntegerField {
	pub base: BaseField,
}

impl Default for PositiveBigIntegerField {
	fn default() -> Self {
		Self::new()
	}
}

impl PositiveBigIntegerField {
	/// Create a new PositiveBigIntegerField for large positive integers (0 to 9223372036854775807)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::PositiveBigIntegerField;
	///
	/// let field = PositiveBigIntegerField::new();
	/// assert!(field.base.name.is_none());
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
		}
	}
}

impl Field for PositiveBigIntegerField {
	fn deconstruct(&self) -> FieldDeconstruction {
		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.PositiveBigIntegerField".to_string(),
			args: vec![],
			kwargs: self.base.get_kwargs(),
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// GenericIPAddressField - IPv4 or IPv6 address field
#[derive(Debug, Clone)]
pub struct GenericIPAddressField {
	pub base: BaseField,
	pub protocol: String, // "both", "IPv4", "IPv6"
	pub unpack_ipv4: bool,
}

impl Default for GenericIPAddressField {
	fn default() -> Self {
		Self::new()
	}
}

impl GenericIPAddressField {
	/// Create a new GenericIPAddressField for storing IP addresses (IPv4 and/or IPv6)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::GenericIPAddressField;
	///
	/// let field = GenericIPAddressField::new();
	/// assert_eq!(field.protocol, "both"); // Accepts both IPv4 and IPv6
	/// assert!(!field.unpack_ipv4); // Don't unpack IPv4-mapped IPv6 addresses
	/// ```
	pub fn new() -> Self {
		Self {
			base: BaseField::new(),
			protocol: "both".to_string(),
			unpack_ipv4: false,
		}
	}
	/// Documentation for `ipv4_only`
	///
	pub fn ipv4_only() -> Self {
		Self {
			base: BaseField::new(),
			protocol: "IPv4".to_string(),
			unpack_ipv4: false,
		}
	}
	/// Documentation for `ipv6_only`
	///
	pub fn ipv6_only() -> Self {
		Self {
			base: BaseField::new(),
			protocol: "IPv6".to_string(),
			unpack_ipv4: false,
		}
	}
}

impl Field for GenericIPAddressField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();

		// Only include non-default values
		if self.protocol != "both" {
			kwargs.insert(
				"protocol".to_string(),
				FieldKwarg::String(self.protocol.clone()),
			);
		}
		if self.unpack_ipv4 {
			kwargs.insert("unpack_ipv4".to_string(), FieldKwarg::Bool(true));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.GenericIPAddressField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// FilePathField - field for selecting file paths
#[derive(Debug, Clone)]
pub struct FilePathField {
	pub base: BaseField,
	pub path: String,
	pub match_pattern: Option<String>,
	pub recursive: bool,
	pub allow_files: bool,
	pub allow_folders: bool,
	pub max_length: u64,
}

impl FilePathField {
	/// Create a new FilePathField for selecting filesystem paths
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::FilePathField;
	///
	/// let field = FilePathField::new("/var/www/uploads".to_string());
	/// assert_eq!(field.path, "/var/www/uploads");
	/// assert!(field.allow_files); // Files are allowed by default
	/// assert!(!field.allow_folders); // Folders are not allowed by default
	/// assert!(!field.recursive); // Non-recursive by default
	/// assert_eq!(field.max_length, 100); // Django's default
	/// ```
	pub fn new(path: String) -> Self {
		Self {
			base: BaseField::new(),
			path,
			match_pattern: None,
			recursive: false,
			allow_files: true,
			allow_folders: false,
			max_length: 100,
		}
	}
}

impl Field for FilePathField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();

		kwargs.insert("path".to_string(), FieldKwarg::String(self.path.clone()));

		if let Some(ref pattern) = self.match_pattern {
			kwargs.insert("match".to_string(), FieldKwarg::String(pattern.clone()));
		}
		if self.recursive {
			kwargs.insert("recursive".to_string(), FieldKwarg::Bool(true));
		}
		if !self.allow_files {
			kwargs.insert("allow_files".to_string(), FieldKwarg::Bool(false));
		}
		if self.allow_folders {
			kwargs.insert("allow_folders".to_string(), FieldKwarg::Bool(true));
		}
		if self.max_length != 100 {
			kwargs.insert("max_length".to_string(), FieldKwarg::Uint(self.max_length));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.FilePathField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// ForeignKey - Many-to-one relationship field
#[derive(Debug, Clone)]
pub struct ForeignKey {
	pub base: BaseField,
	pub to: String,        // Related model name (e.g., "auth.Permission")
	pub on_delete: String, // CASCADE, SET_NULL, etc.
	pub related_name: Option<String>,
}

impl ForeignKey {
	/// Create a new ForeignKey for many-to-one relationships
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::ForeignKey;
	///
	/// let field = ForeignKey::new("auth.User".to_string(), "CASCADE".to_string());
	/// assert_eq!(field.to, "auth.User");
	/// assert_eq!(field.on_delete, "CASCADE");
	/// assert!(field.related_name.is_none());
	/// ```
	pub fn new(to: String, on_delete: String) -> Self {
		Self {
			base: BaseField::new(),
			to,
			on_delete,
			related_name: None,
		}
	}
}

impl Field for ForeignKey {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();

		// Convert to lowercase for consistency with Django
		kwargs.insert("to".to_string(), FieldKwarg::String(self.to.to_lowercase()));
		kwargs.insert(
			"on_delete".to_string(),
			FieldKwarg::String(self.on_delete.clone()),
		);

		if let Some(ref name) = self.related_name {
			kwargs.insert("related_name".to_string(), FieldKwarg::String(name.clone()));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.ForeignKey".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// OneToOneField - One-to-one relationship field
#[derive(Debug, Clone)]
pub struct OneToOneField {
	pub base: BaseField,
	pub to: String,
	pub on_delete: String,
	pub related_name: Option<String>,
}

impl OneToOneField {
	/// Create a new OneToOneField for one-to-one relationships
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::OneToOneField;
	///
	/// let field = OneToOneField::new("auth.User".to_string(), "CASCADE".to_string());
	/// assert_eq!(field.to, "auth.User");
	/// assert_eq!(field.on_delete, "CASCADE");
	/// assert!(field.related_name.is_none());
	/// ```
	pub fn new(to: String, on_delete: String) -> Self {
		Self {
			base: BaseField::new(),
			to,
			on_delete,
			related_name: None,
		}
	}
}

impl Field for OneToOneField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();

		kwargs.insert("to".to_string(), FieldKwarg::String(self.to.to_lowercase()));
		kwargs.insert(
			"on_delete".to_string(),
			FieldKwarg::String(self.on_delete.clone()),
		);

		if let Some(ref name) = self.related_name {
			kwargs.insert("related_name".to_string(), FieldKwarg::String(name.clone()));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.OneToOneField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

/// ManyToManyField - Many-to-many relationship field
#[derive(Debug, Clone)]
pub struct ManyToManyField {
	pub base: BaseField,
	pub to: String,
	pub related_name: Option<String>,
	pub through: Option<String>,
}

impl ManyToManyField {
	/// Create a new ManyToManyField for many-to-many relationships
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::ManyToManyField;
	///
	/// let field = ManyToManyField::new("auth.Permission".to_string());
	/// assert_eq!(field.to, "auth.Permission");
	/// assert!(field.related_name.is_none());
	/// assert!(field.through.is_none());
	/// ```
	pub fn new(to: String) -> Self {
		Self {
			base: BaseField::new(),
			to,
			related_name: None,
			through: None,
		}
	}
	/// Documentation for `with_related_name`
	pub fn with_related_name(to: String, related_name: String) -> Self {
		Self {
			base: BaseField::new(),
			to,
			related_name: Some(related_name),
			through: None,
		}
	}
}

impl Field for ManyToManyField {
	fn deconstruct(&self) -> FieldDeconstruction {
		let mut kwargs = self.base.get_kwargs();

		kwargs.insert("to".to_string(), FieldKwarg::String(self.to.to_lowercase()));

		if let Some(ref name) = self.related_name {
			kwargs.insert("related_name".to_string(), FieldKwarg::String(name.clone()));
		}

		if let Some(ref through) = self.through {
			kwargs.insert("through".to_string(), FieldKwarg::String(through.clone()));
		}

		FieldDeconstruction {
			name: self.base.name.clone(),
			path: "reinhardt.orm.models.ManyToManyField".to_string(),
			args: vec![],
			kwargs,
		}
	}

	fn set_attributes_from_name(&mut self, name: &str) {
		self.base.name = Some(name.to_string());
	}

	fn name(&self) -> Option<&str> {
		self.base.name.as_deref()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_auto_field_deconstruct() {
		let mut field = AutoField::new();
		field.set_attributes_from_name("id");
		let dec = field.deconstruct();

		assert_eq!(dec.name, Some("id".to_string()));
		assert_eq!(dec.path, "reinhardt.orm.models.AutoField");
		assert_eq!(dec.args.len(), 0);
		assert_eq!(dec.kwargs.get("primary_key"), Some(&FieldKwarg::Bool(true)));
	}

	#[test]
	fn test_big_integer_field_deconstruct() {
		let field = BigIntegerField::new();
		let dec = field.deconstruct();

		assert_eq!(dec.path, "reinhardt.orm.models.BigIntegerField");
		assert_eq!(dec.args.len(), 0);
		assert!(dec.kwargs.is_empty());
	}

	#[test]
	fn test_boolean_field_deconstruct() {
		let field = BooleanField::new();
		let dec = field.deconstruct();

		assert_eq!(dec.path, "reinhardt.orm.models.BooleanField");
		assert!(dec.kwargs.is_empty());

		let field_with_default = BooleanField::with_default(true);
		let dec2 = field_with_default.deconstruct();
		assert_eq!(dec2.kwargs.get("default"), Some(&FieldKwarg::Bool(true)));
	}

	#[test]
	fn test_char_field_deconstruct() {
		let field = CharField::new(65);
		let dec = field.deconstruct();

		assert_eq!(dec.path, "reinhardt.orm.models.CharField");
		assert_eq!(dec.kwargs.get("max_length"), Some(&FieldKwarg::Uint(65)));

		let field2 = CharField::with_null_blank(65);
		let dec2 = field2.deconstruct();
		assert_eq!(dec2.kwargs.get("null"), Some(&FieldKwarg::Bool(true)));
		assert_eq!(dec2.kwargs.get("blank"), Some(&FieldKwarg::Bool(true)));
	}

	#[test]
	fn test_char_field_choices() {
		let choices = vec![
			("A".to_string(), "One".to_string()),
			("B".to_string(), "Two".to_string()),
		];
		let field = CharField::with_choices(1, choices.clone());
		let dec = field.deconstruct();

		assert_eq!(
			dec.kwargs.get("choices"),
			Some(&FieldKwarg::Choices(choices))
		);
		assert_eq!(dec.kwargs.get("max_length"), Some(&FieldKwarg::Uint(1)));
	}

	#[test]
	fn test_date_field_deconstruct() {
		let field = DateField::new();
		let dec = field.deconstruct();
		assert_eq!(dec.path, "reinhardt.orm.models.DateField");
		assert!(dec.kwargs.is_empty());

		let field2 = DateField::with_auto_now();
		let dec2 = field2.deconstruct();
		assert_eq!(dec2.kwargs.get("auto_now"), Some(&FieldKwarg::Bool(true)));
	}

	#[test]
	fn test_datetime_field_deconstruct() {
		let field = DateTimeField::new();
		let dec = field.deconstruct();
		assert!(dec.kwargs.is_empty());

		let field2 = DateTimeField::with_auto_now_add();
		let dec2 = field2.deconstruct();
		assert_eq!(
			dec2.kwargs.get("auto_now_add"),
			Some(&FieldKwarg::Bool(true))
		);

		let field3 = DateTimeField::with_both();
		let dec3 = field3.deconstruct();
		assert_eq!(dec3.kwargs.get("auto_now"), Some(&FieldKwarg::Bool(true)));
		assert_eq!(
			dec3.kwargs.get("auto_now_add"),
			Some(&FieldKwarg::Bool(true))
		);
	}

	#[test]
	fn test_decimal_field_deconstruct() {
		let field = DecimalField::new(5, 2);
		let dec = field.deconstruct();

		assert_eq!(dec.path, "reinhardt.orm.models.DecimalField");
		assert_eq!(dec.kwargs.get("max_digits"), Some(&FieldKwarg::Uint(5)));
		assert_eq!(dec.kwargs.get("decimal_places"), Some(&FieldKwarg::Uint(2)));
	}

	#[test]
	fn test_email_field_deconstruct() {
		let field = EmailField::new();
		let dec = field.deconstruct();

		assert_eq!(dec.path, "reinhardt.orm.models.EmailField");
		assert_eq!(dec.kwargs.get("max_length"), Some(&FieldKwarg::Uint(254)));

		let field2 = EmailField::with_max_length(255);
		let dec2 = field2.deconstruct();
		assert_eq!(dec2.kwargs.get("max_length"), Some(&FieldKwarg::Uint(255)));
	}
}
