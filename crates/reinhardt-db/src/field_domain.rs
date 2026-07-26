//! Shared database field domain metadata.

/// Maximum number of dimensions supported by pgvector dense vectors.
pub const MAX_DENSE_VECTOR_DIMENSIONS: usize = 2_000;

/// Physical database storage used by a model field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DatabaseStorageKind {
	/// Boolean storage.
	Bool,
	/// 32-bit signed integer storage.
	I32,
	/// 64-bit signed integer storage.
	I64,
	/// 32-bit floating-point storage.
	F32,
	/// 64-bit floating-point storage.
	F64,
	/// Fixed-precision decimal storage.
	Decimal,
	/// UTF-8 string storage.
	String,
	/// Binary byte storage.
	Bytes,
	/// Native JSON storage.
	Json,
	/// Native PostgreSQL dense-vector storage with a fixed dimension.
	#[cfg(feature = "pgvector")]
	Vector(usize),
	/// UUID storage.
	Uuid,
	/// Calendar date storage.
	Date,
	/// Time-of-day storage.
	Time,
	/// UTC timestamp storage.
	DateTime,
}

/// Persistent representation used by a model enum.
#[derive(
	Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum ModelEnumRepr {
	/// String-backed enum values.
	String,
	/// 32-bit signed integer-backed enum values.
	I32,
}

/// Owned persistent value for a model enum variant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum ModelEnumValue {
	/// An owned string enum value.
	String(String),
	/// A 32-bit signed integer enum value.
	I32(i32),
}

/// Additional value constraints associated with a model field.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum FieldDomain {
	/// A finite set of model enum values.
	Enum {
		/// Persistent scalar representation.
		repr: ModelEnumRepr,
		/// Canonical persistent values.
		values: Vec<ModelEnumValue>,
	},
}

impl FieldDomain {
	/// Returns schema metadata in a declaration-order-independent form.
	#[must_use]
	pub fn canonicalized(mut self) -> Self {
		match &mut self {
			Self::Enum { values, .. } => {
				values.sort();
				values.dedup();
			}
		}
		self
	}
}
