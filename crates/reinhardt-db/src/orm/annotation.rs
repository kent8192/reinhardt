//! Low-level annotation values retained for expressions and explicit subqueries.

use crate::orm::expressions::{F, Q};
use crate::orm::query::quote_identifier;
use serde::{Deserialize, Serialize};

/// Represents an annotation value that can be added to a legacy expression query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnnotationValue {
	/// A constant value.
	Value(Value),
	/// A field reference (F expression).
	Field(F),
	/// A complex expression combining multiple values.
	Expression(Expression),
	/// A subquery (scalar subquery in SELECT clause).
	Subquery(String),
}

/// Constant value types for annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
	/// String variant.
	String(String),
	/// Int variant.
	Int(i64),
	/// Float variant.
	Float(f64),
	/// Bool variant.
	Bool(bool),
	/// Null variant.
	Null,
}

impl Value {
	/// Convert the value to a SQL literal.
	pub fn to_sql(&self) -> String {
		match self {
			Value::String(s) => format!("'{}'", s.replace('\'', "''")),
			Value::Int(i) => i.to_string(),
			Value::Float(f) => f.to_string(),
			Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
			Value::Null => "NULL".to_string(),
		}
	}
}

/// Expression types for complex annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
	/// Addition.
	Add(Box<AnnotationValue>, Box<AnnotationValue>),
	/// Subtraction.
	Subtract(Box<AnnotationValue>, Box<AnnotationValue>),
	/// Multiplication.
	Multiply(Box<AnnotationValue>, Box<AnnotationValue>),
	/// Division.
	Divide(Box<AnnotationValue>, Box<AnnotationValue>),
	/// CASE WHEN expression.
	Case {
		/// The clauses.
		whens: Vec<When>,
		/// The default.
		default: Option<Box<AnnotationValue>>,
	},
	/// COALESCE expression.
	Coalesce(Vec<AnnotationValue>),
}

/// WHEN clause for CASE expressions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct When {
	/// The condition.
	pub condition: Q,
	/// The then expression.
	pub then: AnnotationValue,
}

impl When {
	/// Creates a CASE WHEN clause.
	pub fn new(condition: Q, then: AnnotationValue) -> Self {
		Self { condition, then }
	}

	/// Convert this clause to SQL.
	pub fn to_sql(&self) -> String {
		format!(
			"WHEN {} THEN {}",
			self.condition.to_sql(),
			self.then.to_sql()
		)
	}
}

/// Represents a low-level annotation on a QuerySet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
	/// The alias.
	pub alias: String,
	/// The value.
	pub value: AnnotationValue,
}

impl Annotation {
	/// Creates a low-level annotation.
	pub fn new(alias: impl Into<String>, value: AnnotationValue) -> Self {
		Self {
			alias: alias.into(),
			value,
		}
	}

	/// Converts to SQL with an alias.
	pub fn to_sql(&self) -> String {
		format!(
			"{} AS {}",
			self.value.to_sql(),
			quote_identifier(&self.alias)
		)
	}

	/// Convenience constructor for field-based annotations.
	pub fn field(alias: impl Into<String>, value: AnnotationValue) -> Self {
		Self::new(alias, value)
	}
}

impl AnnotationValue {
	/// Converts this value to SQL.
	pub fn to_sql(&self) -> String {
		match self {
			Self::Value(value) => value.to_sql(),
			Self::Field(field) => field.to_sql(),
			Self::Expression(expression) => expression.to_sql(),
			Self::Subquery(sql) => sql.clone(),
		}
	}

	/// Converts this value to a SQL expression without an alias.
	pub fn to_sql_expr(&self) -> String {
		self.to_sql()
	}
}

impl Expression {
	/// Converts this expression to SQL.
	pub fn to_sql(&self) -> String {
		match self {
			Self::Add(left, right) => format!("({} + {})", left.to_sql(), right.to_sql()),
			Self::Subtract(left, right) => format!("({} - {})", left.to_sql(), right.to_sql()),
			Self::Multiply(left, right) => format!("({} * {})", left.to_sql(), right.to_sql()),
			Self::Divide(left, right) => format!("({} / {})", left.to_sql(), right.to_sql()),
			Self::Case { whens, default } => {
				let mut sql = String::from("CASE");
				for when in whens {
					sql.push(' ');
					sql.push_str(&when.to_sql());
				}
				if let Some(default) = default {
					sql.push_str(&format!(" ELSE {}", default.to_sql()));
				}
				sql.push_str(" END");
				sql
			}
			Self::Coalesce(values) => format!(
				"COALESCE({})",
				values
					.iter()
					.map(AnnotationValue::to_sql)
					.collect::<Vec<_>>()
					.join(", ")
			),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn scalar_annotation_renders() {
		let annotation = Annotation::new("is_active", AnnotationValue::Value(Value::Bool(true)));
		assert_eq!(annotation.to_sql(), "TRUE AS \"is_active\"");
	}

	#[test]
	fn expression_annotation_renders() {
		let expression = Expression::Add(
			Box::new(AnnotationValue::Field(F::new("price"))),
			Box::new(AnnotationValue::Value(Value::Int(10))),
		);
		assert_eq!(
			Annotation::new("total", AnnotationValue::Expression(expression)).to_sql(),
			"(\"price\" + 10) AS \"total\""
		);
	}
}
