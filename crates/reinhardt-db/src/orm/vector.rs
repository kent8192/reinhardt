use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

/// Maximum number of dimensions supported by pgvector dense vectors.
pub const MAX_DENSE_VECTOR_DIMENSIONS: usize = 2_000;

/// Errors returned when constructing a validated dense vector.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum VectorError {
	/// The supplied element count does not match the const dimension.
	#[error("vector dimension mismatch: expected {expected}, got {actual}")]
	InvalidDimensions {
		/// The required number of dimensions.
		expected: usize,
		/// The supplied number of dimensions.
		actual: usize,
	},
	/// The const dimension exceeds pgvector's dense-vector limit.
	#[error("vector dimension {dimensions} exceeds maximum {max}")]
	UnsupportedDimensions {
		/// The requested number of dimensions.
		dimensions: usize,
		/// The maximum supported number of dimensions.
		max: usize,
	},
	/// An element is not finite.
	#[error("vector element at index {index} is not finite")]
	NonFiniteElement {
		/// The zero-based index of the invalid element.
		index: usize,
	},
}

/// A fixed-dimension dense vector validated for pgvector storage.
#[derive(Debug, Clone, PartialEq)]
pub struct Vector<const N: usize> {
	values: Vec<f32>,
}

impl<const N: usize> Vector<N> {
	/// Creates a vector after validating its dimension and elements.
	pub fn try_from_slice(values: &[f32]) -> Result<Self, VectorError> {
		Self::validate(values)?;
		Ok(Self {
			values: values.to_vec(),
		})
	}

	/// Returns the vector elements.
	pub fn as_slice(&self) -> &[f32] {
		&self.values
	}

	/// Consumes the vector and returns its elements.
	pub fn into_vec(self) -> Vec<f32> {
		self.values
	}

	fn validate(values: &[f32]) -> Result<(), VectorError> {
		if N == 0 || N > MAX_DENSE_VECTOR_DIMENSIONS {
			return Err(VectorError::UnsupportedDimensions {
				dimensions: N,
				max: MAX_DENSE_VECTOR_DIMENSIONS,
			});
		}
		if values.len() != N {
			return Err(VectorError::InvalidDimensions {
				expected: N,
				actual: values.len(),
			});
		}
		if let Some((index, _)) = values
			.iter()
			.enumerate()
			.find(|(_, value)| !value.is_finite())
		{
			return Err(VectorError::NonFiniteElement { index });
		}
		Ok(())
	}
}

impl<const N: usize> TryFrom<Vec<f32>> for Vector<N> {
	type Error = VectorError;

	fn try_from(values: Vec<f32>) -> Result<Self, Self::Error> {
		Self::validate(&values)?;
		Ok(Self { values })
	}
}

impl<const N: usize> Serialize for Vector<N> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		self.values.serialize(serializer)
	}
}

impl<'de, const N: usize> Deserialize<'de> for Vector<N> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		Vec::<f32>::deserialize(deserializer)?
			.try_into()
			.map_err(D::Error::custom)
	}
}

impl<const N: usize> From<Vector<N>> for pgvector::Vector {
	fn from(value: Vector<N>) -> Self {
		Self::from(value.values)
	}
}

impl<const N: usize> TryFrom<pgvector::Vector> for Vector<N> {
	type Error = VectorError;

	fn try_from(value: pgvector::Vector) -> Result<Self, Self::Error> {
		value.to_vec().try_into()
	}
}

#[cfg(test)]
mod tests {
	use super::{Vector, VectorError};

	#[test]
	fn accepts_a_vector_with_the_declared_dimension() {
		let vector = Vector::<3>::try_from(vec![1.0, 2.0, 3.0]).unwrap();

		assert_eq!(vector.as_slice(), &[1.0, 2.0, 3.0]);
	}

	#[test]
	fn rejects_a_vector_with_a_different_dimension() {
		assert!(matches!(
			Vector::<3>::try_from(vec![1.0, 2.0]),
			Err(VectorError::InvalidDimensions {
				expected: 3,
				actual: 2
			})
		));
	}

	#[test]
	fn rejects_a_vector_over_the_dense_dimension_limit() {
		assert!(matches!(
			Vector::<2001>::try_from(vec![0.0; 2001]),
			Err(VectorError::UnsupportedDimensions {
				dimensions: 2001,
				max: 2000
			})
		));
	}

	#[test]
	fn rejects_a_vector_with_zero_dimensions() {
		assert!(matches!(
			Vector::<0>::try_from(Vec::new()),
			Err(VectorError::UnsupportedDimensions {
				dimensions: 0,
				max: 2000
			})
		));
	}

	#[test]
	fn rejects_a_vector_with_a_non_finite_element() {
		assert!(matches!(
			Vector::<3>::try_from(vec![1.0, f32::NAN, 3.0]),
			Err(VectorError::NonFiniteElement { index: 1 })
		));
	}

	#[test]
	fn deserialization_applies_vector_validation() {
		let error = serde_json::from_str::<Vector<3>>("[1.0,2.0]").unwrap_err();

		assert!(error.to_string().contains("dimension mismatch"));
	}

	#[test]
	fn pgvector_round_trip_preserves_all_elements() {
		let vector = Vector::<3>::try_from(vec![1.0, 2.0, 3.0]).unwrap();
		let pgvector: pgvector::Vector = vector.clone().into();
		let round_trip = Vector::<3>::try_from(pgvector).unwrap();

		assert_eq!(round_trip, vector);
	}
}
