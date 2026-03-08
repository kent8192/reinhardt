//! Query filtering for proxies

use crate::proxy::ScalarValue;
use serde::{Deserialize, Serialize};

/// Filter comparison operators for proxy queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterOp {
	/// Equal comparison.
	Eq,
	/// Not equal comparison.
	Ne,
	/// Less than comparison.
	Lt,
	/// Less than or equal comparison.
	Le,
	/// Greater than comparison.
	Gt,
	/// Greater than or equal comparison.
	Ge,
	/// Membership test (value is contained in the target set).
	In,
	/// Negative membership test (value is not in the target set).
	NotIn,
	/// Substring containment check.
	Contains,
	/// Prefix match check.
	StartsWith,
	/// Suffix match check.
	EndsWith,
}

/// A single filter condition combining a field, operator, and value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterCondition {
	/// The field name to filter on.
	pub field: String,
	/// The comparison operator.
	pub op: FilterOp,
	/// The value to compare against (as a string).
	pub value: String,
}

impl FilterCondition {
	/// Create a new filter condition with the given field, operator, and value.
	pub fn new(field: String, op: FilterOp, value: String) -> Self {
		Self { field, op, value }
	}

	/// Check if a `ScalarValue` matches this filter condition.
	pub fn matches(&self, scalar: &ScalarValue) -> bool {
		let scalar_str = match scalar {
			ScalarValue::String(s) => s.clone(),
			ScalarValue::Integer(i) => i.to_string(),
			ScalarValue::Float(f) => f.to_string(),
			ScalarValue::Boolean(b) => b.to_string(),
			ScalarValue::Null => "null".to_string(),
		};

		match self.op {
			FilterOp::Eq => scalar_str == self.value,
			FilterOp::Ne => scalar_str != self.value,
			FilterOp::Lt => scalar_str < self.value,
			FilterOp::Le => scalar_str <= self.value,
			FilterOp::Gt => scalar_str > self.value,
			FilterOp::Ge => scalar_str >= self.value,
			FilterOp::In => self.value.contains(&scalar_str),
			FilterOp::NotIn => !self.value.contains(&scalar_str),
			FilterOp::Contains => scalar_str.contains(&self.value),
			FilterOp::StartsWith => scalar_str.starts_with(&self.value),
			FilterOp::EndsWith => scalar_str.ends_with(&self.value),
		}
	}
}

/// A collection of filter conditions applied together as a conjunction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFilter {
	/// The list of filter conditions.
	pub conditions: Vec<FilterCondition>,
}

impl QueryFilter {
	/// Create a new empty query filter.
	pub fn new() -> Self {
		Self {
			conditions: Vec::new(),
		}
	}

	/// Add a filter condition to this query filter.
	pub fn add_condition(&mut self, condition: FilterCondition) {
		self.conditions.push(condition);
	}
}

impl Default for QueryFilter {
	fn default() -> Self {
		Self::new()
	}
}
