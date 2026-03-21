use crate::orm::annotation::AnnotationValue;
use serde::{Deserialize, Serialize};

/// Base database function trait
pub trait DatabaseFunction {
	/// Converts the function call to its SQL representation.
	fn to_sql(&self) -> String;
	/// Returns the SQL function name.
	fn function_name(&self) -> &'static str;
}

/// Cast expression - convert a value to a specific type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cast {
	expression: Box<AnnotationValue>,
	/// The target type.
	pub target_type: SqlType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Defines possible sql type values.
pub enum SqlType {
	/// Integer variant.
	Integer,
	/// BigInt variant.
	BigInt,
	/// SmallInt variant.
	SmallInt,
	/// Float variant.
	Float,
	/// Real variant.
	Real,
	/// Double variant.
	Double,
	/// Decimal variant.
	Decimal {
		/// The precision.
		precision: Option<u8>,
		/// The scale.
		scale: Option<u8>,
	},
	/// Text variant.
	Text,
	/// Varchar variant.
	Varchar {
		/// The length.
		length: Option<usize>,
	},
	/// Char variant.
	Char {
		/// The length.
		length: usize,
	},
	/// Boolean variant.
	Boolean,
	/// Date variant.
	Date,
	/// Time variant.
	Time,
	/// Timestamp variant.
	Timestamp,
	/// Json variant.
	Json,
}

impl SqlType {
	/// Generate SQL type representation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::SqlType;
	///
	/// assert_eq!(SqlType::Integer.to_sql(), "INTEGER");
	/// assert_eq!(SqlType::Varchar { length: Some(255) }.to_sql(), "VARCHAR(255)");
	/// assert_eq!(SqlType::Decimal { precision: Some(10), scale: Some(2) }.to_sql(), "DECIMAL(10, 2)");
	/// ```
	pub fn to_sql(&self) -> String {
		match self {
			SqlType::Integer => "INTEGER".to_string(),
			SqlType::BigInt => "BIGINT".to_string(),
			SqlType::SmallInt => "SMALLINT".to_string(),
			SqlType::Float => "FLOAT".to_string(),
			SqlType::Real => "REAL".to_string(),
			SqlType::Double => "DOUBLE PRECISION".to_string(),
			SqlType::Decimal { precision, scale } => match (precision, scale) {
				(Some(p), Some(s)) => format!("DECIMAL({}, {})", p, s),
				(Some(p), None) => format!("DECIMAL({})", p),
				_ => "DECIMAL".to_string(),
			},
			SqlType::Text => "TEXT".to_string(),
			SqlType::Varchar { length } => {
				if let Some(len) = length {
					format!("VARCHAR({})", len)
				} else {
					"VARCHAR".to_string()
				}
			}
			SqlType::Char { length } => format!("CHAR({})", length),
			SqlType::Boolean => "BOOLEAN".to_string(),
			SqlType::Date => "DATE".to_string(),
			SqlType::Time => "TIME".to_string(),
			SqlType::Timestamp => "TIMESTAMP".to_string(),
			SqlType::Json => "JSON".to_string(),
		}
	}
}

impl Cast {
	/// Cast an expression to a specific SQL type
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::{Cast, SqlType};
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let cast = Cast::new(
	///     AnnotationValue::Field(F::new("price")),
	///     SqlType::Integer
	/// );
	/// assert_eq!(cast.to_sql(), "CAST(\"price\" AS INTEGER)");
	/// ```
	pub fn new(expression: AnnotationValue, target_type: SqlType) -> Self {
		Self {
			expression: Box::new(expression),
			target_type,
		}
	}

	/// Get a reference to the expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::{Cast, SqlType};
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let cast = Cast::new(
	///     AnnotationValue::Field(F::new("price")),
	///     SqlType::Integer
	/// );
	/// let expr = cast.expression();
	/// ```
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the inner expression, consuming self
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::{Cast, SqlType};
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let cast = Cast::new(
	///     AnnotationValue::Field(F::new("price")),
	///     SqlType::Integer
	/// );
	/// let expr = cast.into_expression();
	/// ```
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
	/// Generate SQL for CAST expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::{Cast, SqlType};
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let cast = Cast::new(
	///     AnnotationValue::Field(F::new("id")),
	///     SqlType::Varchar { length: Some(50) }
	/// );
	/// assert_eq!(cast.to_sql(), "CAST(\"id\" AS VARCHAR(50))");
	/// ```
	pub fn to_sql(&self) -> String {
		format!(
			"CAST({} AS {})",
			self.expression.to_sql(),
			self.target_type.to_sql()
		)
	}
}

/// Greatest - return the maximum value among expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greatest {
	/// The expressions.
	pub expressions: Vec<AnnotationValue>,
}

impl Greatest {
	/// Create a new Greatest function to return the maximum value from multiple expressions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Greatest;
	/// use reinhardt_db::orm::annotation::{AnnotationValue, Value};
	///
	/// let expr1 = AnnotationValue::Value(Value::Int(10));
	/// let expr2 = AnnotationValue::Value(Value::Int(20));
	/// let greatest = Greatest::new(vec![expr1, expr2]).unwrap();
	/// assert_eq!(greatest.expressions.len(), 2);
	/// ```
	pub fn new(expressions: Vec<AnnotationValue>) -> Result<Self, String> {
		if expressions.len() < 2 {
			return Err("Greatest must take at least two expressions".to_string());
		}
		Ok(Self { expressions })
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		let exprs: Vec<String> = self.expressions.iter().map(|e| e.to_sql()).collect();
		format!("GREATEST({})", exprs.join(", "))
	}
}

/// Least - return the minimum value among expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Least {
	/// The expressions.
	pub expressions: Vec<AnnotationValue>,
}

impl Least {
	/// Create a new Least function to return the minimum value from multiple expressions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Least;
	/// use reinhardt_db::orm::annotation::{AnnotationValue, Value};
	///
	/// let expr1 = AnnotationValue::Value(Value::Int(10));
	/// let expr2 = AnnotationValue::Value(Value::Int(20));
	/// let least = Least::new(vec![expr1, expr2]).unwrap();
	/// assert_eq!(least.expressions.len(), 2);
	/// ```
	pub fn new(expressions: Vec<AnnotationValue>) -> Result<Self, String> {
		if expressions.len() < 2 {
			return Err("Least must take at least two expressions".to_string());
		}
		Ok(Self { expressions })
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		let exprs: Vec<String> = self.expressions.iter().map(|e| e.to_sql()).collect();
		format!("LEAST({})", exprs.join(", "))
	}
}

/// NullIf - return NULL if two expressions are equal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NullIf {
	expr1: Box<AnnotationValue>,
	expr2: Box<AnnotationValue>,
}

impl NullIf {
	/// Create a NULLIF function that returns NULL if two expressions are equal
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::NullIf;
	/// use reinhardt_db::orm::annotation::{AnnotationValue, Value};
	///
	/// let nullif = NullIf::new(
	///     AnnotationValue::Value(Value::Int(0)),
	///     AnnotationValue::Value(Value::Int(0))
	/// );
	/// assert_eq!(nullif.to_sql(), "NULLIF(0, 0)");
	/// ```
	pub fn new(expr1: AnnotationValue, expr2: AnnotationValue) -> Self {
		Self {
			expr1: Box::new(expr1),
			expr2: Box::new(expr2),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!("NULLIF({}, {})", self.expr1.to_sql(), self.expr2.to_sql())
	}

	/// Get a reference to the first expression
	pub fn expr1(&self) -> &AnnotationValue {
		&self.expr1
	}

	/// Get a reference to the second expression
	pub fn expr2(&self) -> &AnnotationValue {
		&self.expr2
	}

	/// Convert into the first expression, consuming self
	pub fn into_expr1(self) -> AnnotationValue {
		*self.expr1
	}

	/// Convert into the second expression, consuming self
	pub fn into_expr2(self) -> AnnotationValue {
		*self.expr2
	}
}

// Text functions

/// Concat - concatenate multiple strings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concat {
	/// The expressions.
	pub expressions: Vec<AnnotationValue>,
}

impl Concat {
	/// Concatenate multiple string expressions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Concat;
	/// use reinhardt_db::orm::annotation::{AnnotationValue, Value};
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let concat = Concat::new(vec![
	///     AnnotationValue::Field(F::new("first_name")),
	///     AnnotationValue::Value(Value::String(" ".into())),
	///     AnnotationValue::Field(F::new("last_name")),
	/// ]).unwrap();
	/// assert_eq!(concat.to_sql(), "CONCAT(\"first_name\", ' ', \"last_name\")");
	/// ```
	pub fn new(expressions: Vec<AnnotationValue>) -> Result<Self, String> {
		if expressions.len() < 2 {
			return Err("Concat must take at least two expressions".to_string());
		}
		Ok(Self { expressions })
	}
	/// Generate CONCAT SQL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Concat;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let concat = Concat::new(vec![
	///     AnnotationValue::Field(F::new("city")),
	///     AnnotationValue::Field(F::new("country")),
	/// ]).unwrap();
	/// assert!(concat.to_sql().starts_with("CONCAT("));
	/// ```
	pub fn to_sql(&self) -> String {
		let exprs: Vec<String> = self.expressions.iter().map(|e| e.to_sql()).collect();
		format!("CONCAT({})", exprs.join(", "))
	}
}

/// Upper - convert string to uppercase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upper {
	expression: Box<AnnotationValue>,
}

impl Upper {
	/// Convert string to uppercase
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Upper;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let upper = Upper::new(AnnotationValue::Field(F::new("email")));
	/// assert_eq!(upper.to_sql(), "UPPER(\"email\")");
	/// ```
	pub fn new(expression: AnnotationValue) -> Self {
		Self {
			expression: Box::new(expression),
		}
	}
	/// Generate UPPER SQL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Upper;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let upper = Upper::new(AnnotationValue::Field(F::new("name")));
	/// assert_eq!(upper.to_sql(), "UPPER(\"name\")");
	/// ```
	pub fn to_sql(&self) -> String {
		format!("UPPER({})", self.expression.to_sql())
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
}

/// Lower - convert string to lowercase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lower {
	expression: Box<AnnotationValue>,
}

impl Lower {
	/// Create a new Lower function to convert string to lowercase
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Lower;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let lower = Lower::new(AnnotationValue::Field(F::new("name")));
	/// assert_eq!(lower.to_sql(), "LOWER(\"name\")");
	/// ```
	pub fn new(expression: AnnotationValue) -> Self {
		Self {
			expression: Box::new(expression),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!("LOWER({})", self.expression.to_sql())
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
}

/// Length - return the length of a string
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Length {
	expression: Box<AnnotationValue>,
}

impl Length {
	/// Create a new Length function to return the length of a string
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Length;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let length = Length::new(AnnotationValue::Field(F::new("name")));
	/// assert_eq!(length.to_sql(), "LENGTH(\"name\")");
	/// ```
	pub fn new(expression: AnnotationValue) -> Self {
		Self {
			expression: Box::new(expression),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!("LENGTH({})", self.expression.to_sql())
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
}

/// Trim - remove leading and trailing whitespace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trim {
	expression: Box<AnnotationValue>,
	/// The trim type.
	pub trim_type: TrimType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Defines possible trim type values.
pub enum TrimType {
	/// Both variant.
	Both,
	/// Leading variant.
	Leading,
	/// Trailing variant.
	Trailing,
}

impl Trim {
	/// Create a Trim function to remove leading and trailing whitespace
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Trim;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let trim = Trim::new(AnnotationValue::Field(F::new("name")));
	/// assert_eq!(trim.to_sql(), "TRIM(\"name\")");
	/// ```
	pub fn new(expression: AnnotationValue) -> Self {
		Self {
			expression: Box::new(expression),
			trim_type: TrimType::Both,
		}
	}
	/// Documentation for `leading`
	///
	pub fn leading(mut self) -> Self {
		self.trim_type = TrimType::Leading;
		self
	}
	/// Documentation for `trailing`
	///
	pub fn trailing(mut self) -> Self {
		self.trim_type = TrimType::Trailing;
		self
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		match self.trim_type {
			TrimType::Both => format!("TRIM({})", self.expression.to_sql()),
			TrimType::Leading => format!("LTRIM({})", self.expression.to_sql()),
			TrimType::Trailing => format!("RTRIM({})", self.expression.to_sql()),
		}
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
}

/// Substr - extract a substring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Substr {
	expression: Box<AnnotationValue>,
	start: Box<AnnotationValue>,
	length: Option<Box<AnnotationValue>>,
}

impl Substr {
	/// Create a new Substr function to extract a substring
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Substr;
	/// use reinhardt_db::orm::annotation::{AnnotationValue, Value};
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let substr = Substr::new(
	///     AnnotationValue::Field(F::new("name")),
	///     AnnotationValue::Value(Value::Int(1)),
	///     Some(AnnotationValue::Value(Value::Int(5))),
	/// );
	/// assert_eq!(substr.to_sql(), "SUBSTR(\"name\", 1, 5)");
	/// ```
	pub fn new(
		expression: AnnotationValue,
		start: AnnotationValue,
		length: Option<AnnotationValue>,
	) -> Self {
		Self {
			expression: Box::new(expression),
			start: Box::new(start),
			length: length.map(Box::new),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		if let Some(len) = &self.length {
			format!(
				"SUBSTR({}, {}, {})",
				self.expression.to_sql(),
				self.start.to_sql(),
				len.to_sql()
			)
		} else {
			format!(
				"SUBSTR({}, {})",
				self.expression.to_sql(),
				self.start.to_sql()
			)
		}
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Get a reference to the start position
	pub fn start(&self) -> &AnnotationValue {
		&self.start
	}

	/// Get a reference to the length (if provided)
	pub fn length(&self) -> Option<&AnnotationValue> {
		self.length.as_deref()
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}

	/// Convert into the start position, consuming self
	pub fn into_start(self) -> AnnotationValue {
		*self.start
	}

	/// Convert into the length (if provided), consuming self
	pub fn into_length(self) -> Option<AnnotationValue> {
		self.length.map(|b| *b)
	}
}

// Math functions

/// Abs - absolute value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abs {
	expression: Box<AnnotationValue>,
}

impl Abs {
	/// Create a new Abs function to return the absolute value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Abs;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let abs = Abs::new(AnnotationValue::Field(F::new("temperature")));
	/// assert_eq!(abs.to_sql(), "ABS(\"temperature\")");
	/// ```
	pub fn new(expression: AnnotationValue) -> Self {
		Self {
			expression: Box::new(expression),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!("ABS({})", self.expression.to_sql())
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
}

/// Ceil - round up to nearest integer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ceil {
	expression: Box<AnnotationValue>,
}

impl Ceil {
	/// Create a new Ceil function to round up to the nearest integer
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Ceil;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let ceil = Ceil::new(AnnotationValue::Field(F::new("price")));
	/// assert_eq!(ceil.to_sql(), "CEIL(\"price\")");
	/// ```
	pub fn new(expression: AnnotationValue) -> Self {
		Self {
			expression: Box::new(expression),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!("CEIL({})", self.expression.to_sql())
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
}

/// Floor - round down to nearest integer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Floor {
	expression: Box<AnnotationValue>,
}

impl Floor {
	/// Create a new Floor function to round down to the nearest integer
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Floor;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let floor = Floor::new(AnnotationValue::Field(F::new("price")));
	/// assert_eq!(floor.to_sql(), "FLOOR(\"price\")");
	/// ```
	pub fn new(expression: AnnotationValue) -> Self {
		Self {
			expression: Box::new(expression),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!("FLOOR({})", self.expression.to_sql())
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
}

/// Round - round to specified decimal places
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Round {
	expression: Box<AnnotationValue>,
	/// The decimals.
	pub decimals: Option<i32>,
}

impl Round {
	/// Create a new Round function to round to specified decimal places
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Round;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let round = Round::new(AnnotationValue::Field(F::new("price")), Some(2));
	/// assert_eq!(round.to_sql(), "ROUND(\"price\", 2)");
	/// ```
	pub fn new(expression: AnnotationValue, decimals: Option<i32>) -> Self {
		Self {
			expression: Box::new(expression),
			decimals,
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		if let Some(d) = self.decimals {
			format!("ROUND({}, {})", self.expression.to_sql(), d)
		} else {
			format!("ROUND({})", self.expression.to_sql())
		}
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
}

/// Mod - modulo operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mod {
	dividend: Box<AnnotationValue>,
	divisor: Box<AnnotationValue>,
}

impl Mod {
	/// Create a Mod function for modulo operation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Mod;
	/// use reinhardt_db::orm::annotation::{AnnotationValue, Value};
	///
	/// let mod_op = Mod::new(
	///     AnnotationValue::Value(Value::Int(10)),
	///     AnnotationValue::Value(Value::Int(3))
	/// );
	/// assert_eq!(mod_op.to_sql(), "MOD(10, 3)");
	/// ```
	pub fn new(dividend: AnnotationValue, divisor: AnnotationValue) -> Self {
		Self {
			dividend: Box::new(dividend),
			divisor: Box::new(divisor),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!("MOD({}, {})", self.dividend.to_sql(), self.divisor.to_sql())
	}

	/// Get a reference to the dividend
	pub fn dividend(&self) -> &AnnotationValue {
		&self.dividend
	}

	/// Get a reference to the divisor
	pub fn divisor(&self) -> &AnnotationValue {
		&self.divisor
	}

	/// Convert into the dividend, consuming self
	pub fn into_dividend(self) -> AnnotationValue {
		*self.dividend
	}

	/// Convert into the divisor, consuming self
	pub fn into_divisor(self) -> AnnotationValue {
		*self.divisor
	}
}

/// Power - raise to a power
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Power {
	base: Box<AnnotationValue>,
	exponent: Box<AnnotationValue>,
}

impl Power {
	/// Create a Power function to raise to a power
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Power;
	/// use reinhardt_db::orm::annotation::{AnnotationValue, Value};
	///
	/// let power = Power::new(
	///     AnnotationValue::Value(Value::Int(2)),
	///     AnnotationValue::Value(Value::Int(3))
	/// );
	/// assert_eq!(power.to_sql(), "POWER(2, 3)"); // 2^3 = 8
	/// ```
	pub fn new(base: AnnotationValue, exponent: AnnotationValue) -> Self {
		Self {
			base: Box::new(base),
			exponent: Box::new(exponent),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!("POWER({}, {})", self.base.to_sql(), self.exponent.to_sql())
	}

	/// Get a reference to the base
	pub fn base(&self) -> &AnnotationValue {
		&self.base
	}

	/// Get a reference to the exponent
	pub fn exponent(&self) -> &AnnotationValue {
		&self.exponent
	}

	/// Convert into the base, consuming self
	pub fn into_base(self) -> AnnotationValue {
		*self.base
	}

	/// Convert into the exponent, consuming self
	pub fn into_exponent(self) -> AnnotationValue {
		*self.exponent
	}
}

/// Sqrt - square root
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sqrt {
	expression: Box<AnnotationValue>,
}

impl Sqrt {
	/// Create a new Sqrt function to return the square root
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Sqrt;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let sqrt = Sqrt::new(AnnotationValue::Field(F::new("area")));
	/// assert_eq!(sqrt.to_sql(), "SQRT(\"area\")");
	/// ```
	pub fn new(expression: AnnotationValue) -> Self {
		Self {
			expression: Box::new(expression),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		format!("SQRT({})", self.expression.to_sql())
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
}

// Date/Time functions

/// Extract component from date/time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extract {
	expression: Box<AnnotationValue>,
	/// The component.
	pub component: ExtractComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Defines possible extract component values.
pub enum ExtractComponent {
	/// Year variant.
	Year,
	/// Month variant.
	Month,
	/// Day variant.
	Day,
	/// Hour variant.
	Hour,
	/// Minute variant.
	Minute,
	/// Second variant.
	Second,
	/// Week variant.
	Week,
	/// Quarter variant.
	Quarter,
	/// WeekDay variant.
	WeekDay,
	/// IsoWeekDay variant.
	IsoWeekDay,
	/// IsoYear variant.
	IsoYear,
}

impl ExtractComponent {
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> &'static str {
		match self {
			ExtractComponent::Year => "YEAR",
			ExtractComponent::Month => "MONTH",
			ExtractComponent::Day => "DAY",
			ExtractComponent::Hour => "HOUR",
			ExtractComponent::Minute => "MINUTE",
			ExtractComponent::Second => "SECOND",
			ExtractComponent::Week => "WEEK",
			ExtractComponent::Quarter => "QUARTER",
			ExtractComponent::WeekDay => "DOW",
			ExtractComponent::IsoWeekDay => "ISODOW",
			ExtractComponent::IsoYear => "ISOYEAR",
		}
	}
}

impl Extract {
	/// Extract a component from a date/time value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::{Extract, ExtractComponent};
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let extract = Extract::new(
	///     AnnotationValue::Field(F::new("created_at")),
	///     ExtractComponent::Year
	/// );
	/// assert_eq!(extract.to_sql(), "EXTRACT(YEAR FROM \"created_at\")");
	/// ```
	pub fn new(expression: AnnotationValue, component: ExtractComponent) -> Self {
		Self {
			expression: Box::new(expression),
			component,
		}
	}
	/// Extract year from a date/time
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Extract;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let year_extract = Extract::year(AnnotationValue::Field(F::new("birth_date")));
	/// assert_eq!(year_extract.to_sql(), "EXTRACT(YEAR FROM \"birth_date\")");
	/// ```
	pub fn year(expression: AnnotationValue) -> Self {
		Self::new(expression, ExtractComponent::Year)
	}
	/// Extract month from a date/time
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Extract;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let month_extract = Extract::month(AnnotationValue::Field(F::new("order_date")));
	/// assert_eq!(month_extract.to_sql(), "EXTRACT(MONTH FROM \"order_date\")");
	/// ```
	pub fn month(expression: AnnotationValue) -> Self {
		Self::new(expression, ExtractComponent::Month)
	}
	/// Extract day from a date/time
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Extract;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let day_extract = Extract::day(AnnotationValue::Field(F::new("timestamp")));
	/// assert_eq!(day_extract.to_sql(), "EXTRACT(DAY FROM \"timestamp\")");
	/// ```
	pub fn day(expression: AnnotationValue) -> Self {
		Self::new(expression, ExtractComponent::Day)
	}
	/// Extract hour from a date/time
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Extract;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let hour_extract = Extract::hour(AnnotationValue::Field(F::new("event_time")));
	/// assert_eq!(hour_extract.to_sql(), "EXTRACT(HOUR FROM \"event_time\")");
	/// ```
	pub fn hour(expression: AnnotationValue) -> Self {
		Self::new(expression, ExtractComponent::Hour)
	}
	/// Extract minute from a date/time
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Extract;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let minute_extract = Extract::minute(AnnotationValue::Field(F::new("timestamp")));
	/// assert_eq!(minute_extract.to_sql(), "EXTRACT(MINUTE FROM \"timestamp\")");
	/// ```
	pub fn minute(expression: AnnotationValue) -> Self {
		Self::new(expression, ExtractComponent::Minute)
	}
	/// Extract second from a date/time
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Extract;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let second_extract = Extract::second(AnnotationValue::Field(F::new("timestamp")));
	/// assert_eq!(second_extract.to_sql(), "EXTRACT(SECOND FROM \"timestamp\")");
	/// ```
	pub fn second(expression: AnnotationValue) -> Self {
		Self::new(expression, ExtractComponent::Second)
	}
	/// Generate EXTRACT SQL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Extract;
	/// use reinhardt_db::orm::annotation::AnnotationValue;
	/// use reinhardt_db::orm::expressions::F;
	///
	/// let extract = Extract::year(AnnotationValue::Field(F::new("created_at")));
	/// assert!(extract.to_sql().starts_with("EXTRACT("));
	/// ```
	pub fn to_sql(&self) -> String {
		format!(
			"EXTRACT({} FROM {})",
			self.component.to_sql(),
			self.expression.to_sql()
		)
	}

	/// Get a reference to the expression
	pub fn expression(&self) -> &AnnotationValue {
		&self.expression
	}

	/// Convert into the expression, consuming self
	pub fn into_expression(self) -> AnnotationValue {
		*self.expression
	}
}

/// Now - current timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Now;

impl Now {
	/// Create a new Now function to get the current timestamp
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::Now;
	///
	/// let now = Now::new();
	/// assert_eq!(now.to_sql(), "CURRENT_TIMESTAMP");
	/// ```
	pub fn new() -> Self {
		Self
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		"CURRENT_TIMESTAMP".to_string()
	}
}

impl Default for Now {
	fn default() -> Self {
		Self::new()
	}
}

/// CurrentDate - current date
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentDate;

impl CurrentDate {
	/// Create a CurrentDate function to get the current date
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::CurrentDate;
	///
	/// let current_date = CurrentDate::new();
	/// assert_eq!(current_date.to_sql(), "CURRENT_DATE");
	/// ```
	pub fn new() -> Self {
		Self
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		"CURRENT_DATE".to_string()
	}
}

impl Default for CurrentDate {
	fn default() -> Self {
		Self::new()
	}
}

/// CurrentTime - current time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentTime;

impl CurrentTime {
	/// Create a CurrentTime function to get the current time
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::functions::CurrentTime;
	///
	/// let current_time = CurrentTime::new();
	/// assert_eq!(current_time.to_sql(), "CURRENT_TIME");
	/// ```
	pub fn new() -> Self {
		Self
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self) -> String {
		"CURRENT_TIME".to_string()
	}
}

impl Default for CurrentTime {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::annotation::Value;
	use crate::orm::expressions::F;

	#[test]
	fn test_cast_to_integer() {
		let cast = Cast::new(AnnotationValue::Field(F::new("price")), SqlType::Integer);
		assert_eq!(cast.to_sql(), "CAST(\"price\" AS INTEGER)");
	}

	#[test]
	fn test_cast_to_varchar() {
		let cast = Cast::new(
			AnnotationValue::Field(F::new("id")),
			SqlType::Varchar { length: Some(50) },
		);
		assert_eq!(cast.to_sql(), "CAST(\"id\" AS VARCHAR(50))");
	}

	#[test]
	fn test_greatest() {
		let greatest = Greatest::new(vec![
			AnnotationValue::Field(F::new("price1")),
			AnnotationValue::Field(F::new("price2")),
			AnnotationValue::Value(Value::Int(100)),
		])
		.unwrap();
		assert_eq!(greatest.to_sql(), "GREATEST(\"price1\", \"price2\", 100)");
	}

	#[test]
	fn test_least() {
		let least = Least::new(vec![
			AnnotationValue::Field(F::new("score1")),
			AnnotationValue::Field(F::new("score2")),
		])
		.unwrap();
		assert_eq!(least.to_sql(), "LEAST(\"score1\", \"score2\")");
	}

	#[test]
	fn test_nullif() {
		let nullif = NullIf::new(
			AnnotationValue::Field(F::new("status")),
			AnnotationValue::Value(Value::String("inactive".into())),
		);
		assert_eq!(nullif.to_sql(), "NULLIF(\"status\", 'inactive')");
	}

	#[test]
	fn test_concat() {
		let concat = Concat::new(vec![
			AnnotationValue::Field(F::new("first_name")),
			AnnotationValue::Value(Value::String(" ".into())),
			AnnotationValue::Field(F::new("last_name")),
		])
		.unwrap();
		assert_eq!(
			concat.to_sql(),
			"CONCAT(\"first_name\", ' ', \"last_name\")"
		);
	}

	#[test]
	fn test_upper() {
		let upper = Upper::new(AnnotationValue::Field(F::new("name")));
		assert_eq!(upper.to_sql(), "UPPER(\"name\")");
	}

	#[test]
	fn test_lower() {
		let lower = Lower::new(AnnotationValue::Field(F::new("email")));
		assert_eq!(lower.to_sql(), "LOWER(\"email\")");
	}

	#[test]
	fn test_length() {
		let length = Length::new(AnnotationValue::Field(F::new("description")));
		assert_eq!(length.to_sql(), "LENGTH(\"description\")");
	}

	#[test]
	fn test_trim() {
		let trim = Trim::new(AnnotationValue::Field(F::new("name")));
		assert_eq!(trim.to_sql(), "TRIM(\"name\")");
	}

	#[test]
	fn test_trim_leading() {
		let trim = Trim::new(AnnotationValue::Field(F::new("name"))).leading();
		assert_eq!(trim.to_sql(), "LTRIM(\"name\")");
	}

	#[test]
	fn test_substr() {
		let substr = Substr::new(
			AnnotationValue::Field(F::new("description")),
			AnnotationValue::Value(Value::Int(1)),
			Some(AnnotationValue::Value(Value::Int(100))),
		);
		assert_eq!(substr.to_sql(), "SUBSTR(\"description\", 1, 100)");
	}

	#[test]
	fn test_abs() {
		let abs = Abs::new(AnnotationValue::Field(F::new("balance")));
		assert_eq!(abs.to_sql(), "ABS(\"balance\")");
	}

	#[test]
	fn test_ceil() {
		let ceil = Ceil::new(AnnotationValue::Field(F::new("price")));
		assert_eq!(ceil.to_sql(), "CEIL(\"price\")");
	}

	#[test]
	fn test_floor() {
		let floor = Floor::new(AnnotationValue::Field(F::new("score")));
		assert_eq!(floor.to_sql(), "FLOOR(\"score\")");
	}

	#[test]
	fn test_round() {
		let round = Round::new(AnnotationValue::Field(F::new("price")), Some(2));
		assert_eq!(round.to_sql(), "ROUND(\"price\", 2)");
	}

	#[test]
	fn test_mod() {
		let mod_op = Mod::new(
			AnnotationValue::Field(F::new("value")),
			AnnotationValue::Value(Value::Int(10)),
		);
		assert_eq!(mod_op.to_sql(), "MOD(\"value\", 10)");
	}

	#[test]
	fn test_power() {
		let power = Power::new(
			AnnotationValue::Field(F::new("base")),
			AnnotationValue::Value(Value::Int(2)),
		);
		assert_eq!(power.to_sql(), "POWER(\"base\", 2)");
	}

	#[test]
	fn test_sqrt() {
		let sqrt = Sqrt::new(AnnotationValue::Field(F::new("area")));
		assert_eq!(sqrt.to_sql(), "SQRT(\"area\")");
	}

	#[test]
	fn test_greatest_minimum_expressions() {
		let result = Greatest::new(vec![AnnotationValue::Field(F::new("x"))]);
		assert!(result.is_err());
	}

	#[test]
	fn test_least_minimum_expressions() {
		let result = Least::new(vec![AnnotationValue::Field(F::new("x"))]);
		assert!(result.is_err());
	}

	#[test]
	fn test_concat_minimum_expressions() {
		let result = Concat::new(vec![AnnotationValue::Field(F::new("x"))]);
		assert!(result.is_err());
	}

	#[test]
	fn test_extract_year() {
		let extract = Extract::year(AnnotationValue::Field(F::new("created_at")));
		assert_eq!(extract.to_sql(), "EXTRACT(YEAR FROM \"created_at\")");
	}

	#[test]
	fn test_extract_month() {
		let extract = Extract::month(AnnotationValue::Field(F::new("order_date")));
		assert_eq!(extract.to_sql(), "EXTRACT(MONTH FROM \"order_date\")");
	}

	#[test]
	fn test_extract_day() {
		let extract = Extract::day(AnnotationValue::Field(F::new("timestamp")));
		assert_eq!(extract.to_sql(), "EXTRACT(DAY FROM \"timestamp\")");
	}

	#[test]
	fn test_extract_hour() {
		let extract = Extract::hour(AnnotationValue::Field(F::new("event_time")));
		assert_eq!(extract.to_sql(), "EXTRACT(HOUR FROM \"event_time\")");
	}

	#[test]
	fn test_orm_functions_now() {
		let now = Now::new();
		assert_eq!(now.to_sql(), "CURRENT_TIMESTAMP");
	}

	#[test]
	fn test_current_date() {
		let date = CurrentDate::new();
		assert_eq!(date.to_sql(), "CURRENT_DATE");
	}

	#[test]
	fn test_current_time() {
		let time = CurrentTime::new();
		assert_eq!(time.to_sql(), "CURRENT_TIME");
	}
}
