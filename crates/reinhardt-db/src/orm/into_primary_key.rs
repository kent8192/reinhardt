/// Trait for types that can be converted into a model's primary key.
///
/// This trait enables flexible function signatures in generated `new()` methods,
/// allowing both model instances and raw primary key values to be passed.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt::db::orm::{Model, IntoPrimaryKey};
/// use uuid::Uuid;
///
/// // Pass a primary key value directly
/// let user_id = Uuid::now_v7();
/// let message = DMMessage::new(room_id, user_id, "Hello".to_string());
///
/// // Pass a model instance
/// let user = User::new(...);
/// let message = DMMessage::new(room_id, &user, "Hello".to_string());
/// ```
pub trait IntoPrimaryKey<T: super::Model> {
	/// Convert this value into the primary key of model `T`.
	fn into_primary_key(self) -> T::PrimaryKey;
}

// Implementation for model references (borrows the model)
impl<T: super::Model> IntoPrimaryKey<T> for &T {
	fn into_primary_key(self) -> T::PrimaryKey {
		self.primary_key().expect(
			"Model instance passed to new() must have a primary key set. \
			         Ensure the model was created with new() or loaded from database.",
		)
	}
}

// Implementations for common primary key types
// This avoids conflicts with the blanket impl for T: Model

// UUID (most common in examples)
impl<T: super::Model<PrimaryKey = uuid::Uuid>> IntoPrimaryKey<T> for uuid::Uuid {
	fn into_primary_key(self) -> T::PrimaryKey {
		self
	}
}

// i32
impl<T: super::Model<PrimaryKey = i32>> IntoPrimaryKey<T> for i32 {
	fn into_primary_key(self) -> T::PrimaryKey {
		self
	}
}

// i64
impl<T: super::Model<PrimaryKey = i64>> IntoPrimaryKey<T> for i64 {
	fn into_primary_key(self) -> T::PrimaryKey {
		self
	}
}

// Option<Uuid>
impl<T: super::Model<PrimaryKey = uuid::Uuid>> IntoPrimaryKey<T> for Option<uuid::Uuid> {
	fn into_primary_key(self) -> T::PrimaryKey {
		self.expect("Primary key value must be provided")
	}
}

// Option<i32>
impl<T: super::Model<PrimaryKey = i32>> IntoPrimaryKey<T> for Option<i32> {
	fn into_primary_key(self) -> T::PrimaryKey {
		self.expect("Primary key value must be provided")
	}
}

// Option<i64>
impl<T: super::Model<PrimaryKey = i64>> IntoPrimaryKey<T> for Option<i64> {
	fn into_primary_key(self) -> T::PrimaryKey {
		self.expect("Primary key value must be provided")
	}
}
