//! Backend-neutral values returned by terminal aggregate queries.

/// A timezone-aware or timezone-naive timestamp returned by an aggregate.
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateDateTime {
	/// A timestamp normalized to UTC by the database driver.
	Utc(chrono::DateTime<chrono::Utc>),
	/// A timestamp whose database type does not carry timezone information.
	Naive(chrono::NaiveDateTime),
}

/// A backend-neutral value returned by a terminal aggregate query.
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateValue {
	/// A signed integer value.
	Integer(i64),
	/// A floating-point value.
	Float(f64),
	/// A fixed-precision decimal value.
	Decimal(rust_decimal::Decimal),
	/// A UTF-8 string value.
	String(String),
	/// A boolean value.
	Bool(bool),
	/// A binary value.
	Bytes(Vec<u8>),
	/// A native JSON value.
	Json(serde_json::Value),
	/// A UUID value.
	Uuid(uuid::Uuid),
	/// A calendar date.
	Date(chrono::NaiveDate),
	/// A time of day.
	Time(chrono::NaiveTime),
	/// A timestamp value.
	DateTime(AggregateDateTime),
	/// A SQL NULL value.
	Null,
}

impl AggregateValue {
	/// Returns the stable name used in checked-accessor diagnostics.
	pub(crate) fn variant_name(&self) -> &'static str {
		match self {
			Self::Integer(_) => "Integer",
			Self::Float(_) => "Float",
			Self::Decimal(_) => "Decimal",
			Self::String(_) => "String",
			Self::Bool(_) => "Bool",
			Self::Bytes(_) => "Bytes",
			Self::Json(_) => "Json",
			Self::Uuid(_) => "Uuid",
			Self::Date(_) => "Date",
			Self::Time(_) => "Time",
			Self::DateTime(_) => "DateTime",
			Self::Null => "Null",
		}
	}
}

/// Values returned by a terminal aggregate query in deterministic label order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AggregateResult {
	values: std::collections::BTreeMap<String, AggregateValue>,
}

impl AggregateResult {
	/// Creates an empty result container.
	pub fn new() -> Self {
		Self::default()
	}

	/// Inserts a decoded aggregate value under a validated label.
	pub fn insert(&mut self, label: impl Into<String>, value: AggregateValue) {
		self.values.insert(label.into(), value);
	}

	/// Returns a checked value by label.
	pub fn get(&self, label: &str) -> reinhardt_core::exception::Result<&AggregateValue> {
		self.values.get(label).ok_or_else(|| {
			reinhardt_core::exception::Error::Serialization(format!(
				"aggregate result does not contain label '{label}'"
			))
		})
	}

	/// Iterates values in lexical label order.
	pub fn iter(&self) -> impl Iterator<Item = (&str, &AggregateValue)> {
		self.values
			.iter()
			.map(|(label, value)| (label.as_str(), value))
	}

	/// Returns an integer value, failing if the label is absent or has another type.
	pub fn get_i64(&self, label: &str) -> reinhardt_core::exception::Result<i64> {
		match self.get(label)? {
			AggregateValue::Integer(value) => Ok(*value),
			value => Err(accessor_type_error(label, value, "Integer")),
		}
	}

	/// Returns a floating-point value, failing if the label is absent or has another type.
	pub fn get_f64(&self, label: &str) -> reinhardt_core::exception::Result<f64> {
		match self.get(label)? {
			AggregateValue::Float(value) => Ok(*value),
			value => Err(accessor_type_error(label, value, "Float")),
		}
	}

	/// Returns a decimal value, failing if the label is absent or has another type.
	pub fn get_decimal(
		&self,
		label: &str,
	) -> reinhardt_core::exception::Result<rust_decimal::Decimal> {
		match self.get(label)? {
			AggregateValue::Decimal(value) => Ok(*value),
			value => Err(accessor_type_error(label, value, "Decimal")),
		}
	}

	/// Returns the number of decoded labels.
	pub fn len(&self) -> usize {
		self.values.len()
	}

	/// Returns whether no aggregate values were decoded.
	pub fn is_empty(&self) -> bool {
		self.values.is_empty()
	}
}

fn accessor_type_error(
	label: &str,
	value: &AggregateValue,
	expected: &str,
) -> reinhardt_core::exception::Error {
	reinhardt_core::exception::Error::Serialization(format!(
		"aggregate label '{label}' contains {}, expected {expected}",
		value.variant_name()
	))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn aggregate_result_iterates_labels_lexically() {
		let mut result = AggregateResult::new();
		result.insert("zeta", AggregateValue::String("z".to_owned()));
		result.insert("alpha", AggregateValue::Integer(1));

		let labels = result.iter().map(|(label, _)| label).collect::<Vec<_>>();
		assert_eq!(labels, vec!["alpha", "zeta"]);
	}

	#[test]
	fn aggregate_result_reports_missing_label_and_checked_types() {
		let mut result = AggregateResult::new();
		result.insert(
			"total",
			AggregateValue::Decimal(rust_decimal::Decimal::new(425, 2)),
		);
		assert_eq!(
			result.get_i64("missing").unwrap_err().to_string(),
			"Serialization error: aggregate result does not contain label 'missing'"
		);
		assert_eq!(
			result.get_i64("total").unwrap_err().to_string(),
			"Serialization error: aggregate label 'total' contains Decimal, expected Integer"
		);
	}

	#[test]
	fn aggregate_result_checked_numeric_accessors_return_values() {
		let mut result = AggregateResult::new();
		result.insert("count", AggregateValue::Integer(4));
		result.insert("average", AggregateValue::Float(4.25));
		result.insert(
			"total",
			AggregateValue::Decimal(rust_decimal::Decimal::new(425, 2)),
		);
		assert_eq!(result.get_i64("count").unwrap(), 4);
		assert_eq!(result.get_f64("average").unwrap(), 4.25);
		assert_eq!(
			result.get_decimal("total").unwrap(),
			rust_decimal::Decimal::new(425, 2)
		);
	}
}
