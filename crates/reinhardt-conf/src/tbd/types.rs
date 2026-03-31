//! Type system for the TBD DSL.

use std::fmt;

/// Types in the TBD DSL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DslType {
	/// Numeric type (integer or float).
	Number,
	/// String type.
	String,
	/// Boolean type.
	Boolean,
	/// Tuple of types.
	Tuple(Vec<DslType>),
	/// Array of a single element type.
	Array(Box<DslType>),
	/// A generation rule parameterized by output type.
	Rule(Box<DslType>),
}

impl DslType {
	/// Returns `true` if this type is a primitive (Number, String, or Boolean).
	pub fn is_primitive(&self) -> bool {
		matches!(self, Self::Number | Self::String | Self::Boolean)
	}

	/// Returns `true` if this type is a generation rule.
	pub fn is_rule(&self) -> bool {
		matches!(self, Self::Rule(_))
	}

	/// Extracts the inner type from a `Rule`, returning `None` for non-rule types.
	pub fn inner_rule_type(&self) -> Option<&DslType> {
		match self {
			Self::Rule(inner) => Some(inner),
			_ => None,
		}
	}
}

impl fmt::Display for DslType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Number => write!(f, "Number"),
			Self::String => write!(f, "String"),
			Self::Boolean => write!(f, "Boolean"),
			Self::Rule(inner) => write!(f, "Rule<{inner}>"),
			Self::Tuple(types) => {
				write!(f, "(")?;
				for (i, ty) in types.iter().enumerate() {
					if i > 0 {
						write!(f, ", ")?;
					}
					write!(f, "{ty}")?;
				}
				write!(f, ")")
			}
			Self::Array(inner) => write!(f, "[{inner}]"),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case::number(DslType::Number, true)]
	#[case::string(DslType::String, true)]
	#[case::boolean(DslType::Boolean, true)]
	#[case::rule(DslType::Rule(Box::new(DslType::String)), false)]
	#[case::tuple(DslType::Tuple(vec![DslType::Number, DslType::String]), false)]
	#[case::array(DslType::Array(Box::new(DslType::Number)), false)]
	fn test_is_primitive(#[case] ty: DslType, #[case] expected: bool) {
		// Arrange
		// (provided via #[case] parameters)

		// Act
		let result = ty.is_primitive();

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::rule_string(DslType::Rule(Box::new(DslType::String)), true)]
	#[case::rule_boolean(DslType::Rule(Box::new(DslType::Boolean)), true)]
	#[case::number(DslType::Number, false)]
	#[case::string(DslType::String, false)]
	fn test_is_rule(#[case] ty: DslType, #[case] expected: bool) {
		// Arrange
		// (provided via #[case] parameters)

		// Act
		let result = ty.is_rule();

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case::rule_string(DslType::Rule(Box::new(DslType::String)), Some(DslType::String))]
	#[case::rule_number(DslType::Rule(Box::new(DslType::Number)), Some(DslType::Number))]
	#[case::number(DslType::Number, None)]
	fn test_inner_rule_type(#[case] ty: DslType, #[case] expected: Option<DslType>) {
		// Arrange
		// (provided via #[case] parameters)

		// Act
		let result = ty.inner_rule_type();

		// Assert
		assert_eq!(result, expected.as_ref());
	}

	#[rstest]
	#[case::number(DslType::Number, "Number")]
	#[case::string(DslType::String, "String")]
	#[case::boolean(DslType::Boolean, "Boolean")]
	#[case::rule(DslType::Rule(Box::new(DslType::String)), "Rule<String>")]
	#[case::tuple(DslType::Tuple(vec![DslType::Number, DslType::String]), "(Number, String)")]
	#[case::array(DslType::Array(Box::new(DslType::Number)), "[Number]")]
	#[case::nested_rule(DslType::Rule(Box::new(DslType::Array(Box::new(DslType::Boolean)))), "Rule<[Boolean]>")]
	fn test_display(#[case] ty: DslType, #[case] expected: &str) {
		// Arrange
		// (provided via #[case] parameters)

		// Act
		let result = ty.to_string();

		// Assert
		assert_eq!(result, expected);
	}
}
