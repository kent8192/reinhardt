use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use sqlx::{
	Decode, Encode, Postgres, Type, TypeInfo,
	encode::IsNull,
	error::BoxDynError,
	postgres::{PgArgumentBuffer, PgTypeInfo, PgValueFormat, PgValueRef},
};

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

#[derive(Debug, PartialEq, thiserror::Error)]
enum PgVectorCodecError {
	#[error("pgvector dimensions must fit in a signed 16-bit integer, got {0}")]
	DimensionsOutOfRange(usize),
	#[error("invalid pgvector payload: missing the 4-byte header")]
	MissingHeader,
	#[error("invalid pgvector dimension {0}")]
	InvalidWireDimensions(i16),
	#[error("invalid pgvector reserved value: expected 0, got {0}")]
	InvalidReserved(i16),
	#[error("invalid pgvector payload length: expected {expected} bytes, got {actual}")]
	InvalidPayloadLength { expected: usize, actual: usize },
	#[error(transparent)]
	InvalidVector(#[from] VectorError),
}

fn encode_pgvector_binary(values: &[f32]) -> Result<Vec<u8>, PgVectorCodecError> {
	let dimensions = i16::try_from(values.len())
		.map_err(|_| PgVectorCodecError::DimensionsOutOfRange(values.len()))?;
	let mut bytes = Vec::with_capacity(4 + values.len() * size_of::<f32>());
	bytes.extend_from_slice(&dimensions.to_be_bytes());
	bytes.extend_from_slice(&0_i16.to_be_bytes());
	for value in values {
		bytes.extend_from_slice(&value.to_be_bytes());
	}
	Ok(bytes)
}

fn decode_pgvector_values(bytes: &[u8]) -> Result<Vec<f32>, PgVectorCodecError> {
	let header = bytes.get(..4).ok_or(PgVectorCodecError::MissingHeader)?;
	let dimensions = i16::from_be_bytes([header[0], header[1]]);
	if dimensions <= 0 {
		return Err(PgVectorCodecError::InvalidWireDimensions(dimensions));
	}
	let reserved = i16::from_be_bytes([header[2], header[3]]);
	if reserved != 0 {
		return Err(PgVectorCodecError::InvalidReserved(reserved));
	}
	let dimensions = dimensions as usize;
	let expected = 4 + dimensions * size_of::<f32>();
	if bytes.len() != expected {
		return Err(PgVectorCodecError::InvalidPayloadLength {
			expected,
			actual: bytes.len(),
		});
	}
	Ok(bytes[4..]
		.chunks_exact(size_of::<f32>())
		.map(|chunk| f32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
		.collect())
}

fn decode_pgvector_binary<const N: usize>(bytes: &[u8]) -> Result<Vector<N>, PgVectorCodecError> {
	decode_pgvector_values(bytes)?
		.try_into()
		.map_err(Into::into)
}

fn pgvector_type_info() -> PgTypeInfo {
	PgTypeInfo::with_name("vector")
}

fn pgvector_type_compatible(type_info: &PgTypeInfo) -> bool {
	type_info.name().eq_ignore_ascii_case("vector")
}

#[derive(Debug, Clone)]
pub(crate) struct PgVectorValue(Vec<f32>);

impl PgVectorValue {
	pub(crate) fn new(values: Vec<f32>) -> Self {
		Self(values)
	}

	pub(crate) fn into_vec(self) -> Vec<f32> {
		self.0
	}
}

impl Type<Postgres> for PgVectorValue {
	fn type_info() -> PgTypeInfo {
		pgvector_type_info()
	}

	fn compatible(type_info: &PgTypeInfo) -> bool {
		pgvector_type_compatible(type_info)
	}
}

impl<'query> Encode<'query, Postgres> for PgVectorValue {
	fn produces(&self) -> Option<PgTypeInfo> {
		Some(pgvector_type_info())
	}

	fn encode_by_ref(&self, buffer: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
		buffer.extend_from_slice(&encode_pgvector_binary(&self.0)?);
		Ok(IsNull::No)
	}

	fn size_hint(&self) -> usize {
		4 + self.0.len() * size_of::<f32>()
	}
}

impl<'row> Decode<'row, Postgres> for PgVectorValue {
	fn decode(value: PgValueRef<'row>) -> Result<Self, BoxDynError> {
		if value.format() != PgValueFormat::Binary {
			return Err("pgvector values must use PostgreSQL binary format".into());
		}
		Ok(Self(decode_pgvector_values(value.as_bytes()?)?))
	}
}

impl<const N: usize> Type<Postgres> for Vector<N> {
	fn type_info() -> PgTypeInfo {
		pgvector_type_info()
	}

	fn compatible(type_info: &PgTypeInfo) -> bool {
		pgvector_type_compatible(type_info)
	}
}

impl<'query, const N: usize> Encode<'query, Postgres> for Vector<N> {
	fn produces(&self) -> Option<PgTypeInfo> {
		Some(pgvector_type_info())
	}

	fn encode_by_ref(&self, buffer: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
		buffer.extend_from_slice(&encode_pgvector_binary(self.as_slice())?);
		Ok(IsNull::No)
	}

	fn size_hint(&self) -> usize {
		4 + N * size_of::<f32>()
	}
}

impl<'row, const N: usize> Decode<'row, Postgres> for Vector<N> {
	fn decode(value: PgValueRef<'row>) -> Result<Self, BoxDynError> {
		if value.format() != PgValueFormat::Binary {
			return Err("pgvector values must use PostgreSQL binary format".into());
		}
		decode_pgvector_binary(value.as_bytes()?).map_err(Into::into)
	}
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
	use super::{Vector, VectorError, decode_pgvector_binary, encode_pgvector_binary};
	use sqlx::{Postgres, Type, TypeInfo, postgres::PgTypeInfo};

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

	#[test]
	fn pgvector_binary_encoding_uses_the_postgresql_wire_layout() {
		let encoded = encode_pgvector_binary(&[1.0, -2.5, 3.0]).unwrap();

		assert_eq!(
			encoded,
			vec![
				0x00, 0x03, 0x00, 0x00, 0x3f, 0x80, 0x00, 0x00, 0xc0, 0x20, 0x00, 0x00, 0x40, 0x40,
				0x00, 0x00,
			]
		);
	}

	#[test]
	fn pgvector_binary_decoding_rejects_malformed_payload_lengths() {
		let error = decode_pgvector_binary::<3>(&[0x00, 0x03, 0x00, 0x00, 0x3f, 0x80, 0x00, 0x00])
			.unwrap_err();

		assert_eq!(
			error.to_string(),
			"invalid pgvector payload length: expected 16 bytes, got 8"
		);
	}

	#[test]
	fn pgvector_binary_decoding_applies_the_public_dimension_invariant() {
		let encoded = encode_pgvector_binary(&[1.0, 2.0]).unwrap();
		let error = decode_pgvector_binary::<3>(&encoded).unwrap_err();

		assert!(matches!(
			error,
			super::PgVectorCodecError::InvalidVector(VectorError::InvalidDimensions {
				expected: 3,
				actual: 2,
			})
		));
	}

	#[test]
	fn sqlx_type_compatibility_accepts_the_vector_type_name() {
		let type_info = PgTypeInfo::with_name("vector");

		assert_eq!(<Vector<3> as Type<Postgres>>::type_info().name(), "vector");
		assert!(<Vector<3> as Type<Postgres>>::compatible(&type_info));
	}
}
