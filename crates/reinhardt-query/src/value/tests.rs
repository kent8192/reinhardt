//! Tests for the value module.

use super::*;
use rstest::rstest;

mod value_tests {
	use super::*;
	use pretty_assertions::assert_eq;

	#[rstest]
	fn test_value_is_null() {
		// Null values
		assert!(Value::Int(None).is_null());
		assert!(Value::String(None).is_null());
		assert!(Value::Bool(None).is_null());
		assert!(Value::Bytes(None).is_null());

		// Non-null values
		assert!(!Value::Int(Some(42)).is_null());
		assert!(!Value::String(Some(Box::new("hello".to_string()))).is_null());
		assert!(!Value::Bool(Some(true)).is_null());
	}

	#[rstest]
	fn test_value_default() {
		let default = Value::default();
		assert_eq!(default, Value::String(None));
		assert!(default.is_null());
	}

	#[rstest]
	fn test_value_clone() {
		let original = Value::String(Some(Box::new("hello".to_string())));
		let cloned = original.clone();
		assert_eq!(original, cloned);
	}

	#[rstest]
	fn test_value_debug() {
		let value = Value::Int(Some(42));
		let debug = format!("{:?}", value);
		assert!(debug.contains("Int"));
		assert!(debug.contains("42"));
	}
}

mod into_value_tests {
	use super::*;
	use pretty_assertions::assert_eq;

	#[rstest]
	#[case::bool_true(true, Value::Bool(Some(true)))]
	#[case::bool_false(false, Value::Bool(Some(false)))]
	fn test_bool_into_value(#[case] input: bool, #[case] expected: Value) {
		assert_eq!(input.into_value(), expected);
		assert_eq!(Value::from(input), expected);
	}

	#[rstest]
	fn test_option_bool_into_value() {
		assert_eq!(Some(true).into_value(), Value::Bool(Some(true)));
		assert_eq!(Option::<bool>::None.into_value(), Value::Bool(None));
	}

	#[rstest]
	#[case::i8(42i8, Value::TinyInt(Some(42)))]
	#[case::i16(42i16, Value::SmallInt(Some(42)))]
	#[case::i32(42i32, Value::Int(Some(42)))]
	#[case::i64(42i64, Value::BigInt(Some(42)))]
	fn test_signed_int_into_value(#[case] input: impl IntoValue, #[case] expected: Value) {
		assert_eq!(input.into_value(), expected);
	}

	#[rstest]
	#[case::u8(42u8, Value::TinyUnsigned(Some(42)))]
	#[case::u16(42u16, Value::SmallUnsigned(Some(42)))]
	#[case::u32(42u32, Value::Unsigned(Some(42)))]
	#[case::u64(42u64, Value::BigUnsigned(Some(42)))]
	fn test_unsigned_int_into_value(#[case] input: impl IntoValue, #[case] expected: Value) {
		assert_eq!(input.into_value(), expected);
	}

	#[rstest]
	fn test_float_into_value() {
		let f32_val: Value = 3.14f32.into_value();
		assert!(matches!(f32_val, Value::Float(Some(v)) if (v - 3.14).abs() < 0.001));

		let f64_val: Value = 3.14159f64.into_value();
		assert!(matches!(f64_val, Value::Double(Some(v)) if (v - 3.14159).abs() < 0.00001));
	}

	#[rstest]
	fn test_char_into_value() {
		assert_eq!('a'.into_value(), Value::Char(Some('a')));
		assert_eq!(Option::<char>::None.into_value(), Value::Char(None));
	}

	#[rstest]
	fn test_string_into_value() {
		let owned = "hello".to_string();
		assert_eq!(
			owned.into_value(),
			Value::String(Some(Box::new("hello".to_string())))
		);

		let str_ref: &str = "world";
		assert_eq!(
			str_ref.into_value(),
			Value::String(Some(Box::new("world".to_string())))
		);
	}

	#[rstest]
	fn test_option_string_into_value() {
		assert_eq!(
			Some("hello".to_string()).into_value(),
			Value::String(Some(Box::new("hello".to_string())))
		);
		assert_eq!(Option::<String>::None.into_value(), Value::String(None));
	}

	#[rstest]
	fn test_bytes_into_value() {
		let bytes = vec![1u8, 2, 3, 4];
		assert_eq!(
			bytes.clone().into_value(),
			Value::Bytes(Some(Box::new(vec![1, 2, 3, 4])))
		);

		let slice: &[u8] = &[5, 6, 7];
		assert_eq!(
			slice.into_value(),
			Value::Bytes(Some(Box::new(vec![5, 6, 7])))
		);
	}

	#[rstest]
	fn test_value_into_value_identity() {
		let value = Value::Int(Some(42));
		assert_eq!(value.clone().into_value(), value);
	}

	#[rstest]
	fn test_cow_string_into_value() {
		use std::borrow::Cow;

		let borrowed: Cow<'_, str> = Cow::Borrowed("hello");
		assert_eq!(
			borrowed.into_value(),
			Value::String(Some(Box::new("hello".to_string())))
		);

		let owned: Cow<'_, str> = Cow::Owned("world".to_string());
		assert_eq!(
			owned.into_value(),
			Value::String(Some(Box::new("world".to_string())))
		);
	}
}

mod value_tuple_tests {
	use super::*;
	use pretty_assertions::assert_eq;

	#[rstest]
	fn test_value_tuple_len() {
		assert_eq!(ValueTuple::One(Value::Int(Some(1))).len(), 1);
		assert_eq!(
			ValueTuple::Two(Value::Int(Some(1)), Value::Int(Some(2))).len(),
			2
		);
		assert_eq!(
			ValueTuple::Three(
				Value::Int(Some(1)),
				Value::Int(Some(2)),
				Value::Int(Some(3))
			)
			.len(),
			3
		);
		assert_eq!(
			ValueTuple::Many(vec![
				Value::Int(Some(1)),
				Value::Int(Some(2)),
				Value::Int(Some(3)),
				Value::Int(Some(4))
			])
			.len(),
			4
		);
	}

	#[rstest]
	fn test_value_tuple_is_empty() {
		assert!(!ValueTuple::One(Value::Int(Some(1))).is_empty());
		assert!(!ValueTuple::Two(Value::Int(Some(1)), Value::Int(Some(2))).is_empty());
		assert!(ValueTuple::Many(vec![]).is_empty());
	}

	#[rstest]
	fn test_value_tuple_into_vec() {
		let tuple = ValueTuple::Two(Value::Int(Some(1)), Value::Int(Some(2)));
		let vec = tuple.into_vec();
		assert_eq!(vec.len(), 2);
		assert_eq!(vec[0], Value::Int(Some(1)));
		assert_eq!(vec[1], Value::Int(Some(2)));
	}

	#[rstest]
	fn test_value_tuple_iter() {
		let tuple = ValueTuple::Three(
			Value::Int(Some(1)),
			Value::Int(Some(2)),
			Value::Int(Some(3)),
		);

		let collected: Vec<_> = tuple.iter().collect();
		assert_eq!(collected.len(), 3);
		assert_eq!(*collected[0], Value::Int(Some(1)));
		assert_eq!(*collected[1], Value::Int(Some(2)));
		assert_eq!(*collected[2], Value::Int(Some(3)));
	}

	#[rstest]
	fn test_value_tuple_into_iter() {
		let tuple = ValueTuple::Two(Value::Int(Some(1)), Value::Int(Some(2)));
		let collected: Vec<_> = tuple.into_iter().collect();
		assert_eq!(collected.len(), 2);
	}

	#[rstest]
	fn test_value_tuple_from_single() {
		let tuple: ValueTuple = 42i32.into();
		assert_eq!(tuple.len(), 1);
		assert!(matches!(tuple, ValueTuple::One(Value::Int(Some(42)))));
	}

	#[rstest]
	fn test_value_tuple_from_pair() {
		let tuple: ValueTuple = (1i32, "hello").into();
		assert_eq!(tuple.len(), 2);
	}

	#[rstest]
	fn test_value_tuple_from_triple() {
		let tuple: ValueTuple = (1i32, 2i32, 3i32).into();
		assert_eq!(tuple.len(), 3);
	}
}

mod values_tests {
	use super::*;
	use pretty_assertions::assert_eq;

	#[rstest]
	fn test_values_new() {
		let values = Values::new();
		assert!(values.is_empty());
		assert_eq!(values.len(), 0);
	}

	#[rstest]
	fn test_values_with_capacity() {
		let values = Values::with_capacity(10);
		assert!(values.is_empty());
	}

	#[rstest]
	fn test_values_push() {
		let mut values = Values::new();
		let idx1 = values.push(Value::Int(Some(1)));
		let idx2 = values.push(Value::Int(Some(2)));

		assert_eq!(idx1, 1);
		assert_eq!(idx2, 2);
		assert_eq!(values.len(), 2);
	}

	#[rstest]
	fn test_values_iter() {
		let values = Values(vec![Value::Int(Some(1)), Value::Int(Some(2))]);
		let collected: Vec<_> = values.iter().collect();
		assert_eq!(collected.len(), 2);
	}

	#[rstest]
	fn test_values_into_inner() {
		let values = Values(vec![Value::Int(Some(1))]);
		let inner = values.into_inner();
		assert_eq!(inner.len(), 1);
	}

	#[rstest]
	fn test_values_into_iter() {
		let values = Values(vec![Value::Int(Some(1)), Value::Int(Some(2))]);
		let collected: Vec<_> = values.into_iter().collect();
		assert_eq!(collected.len(), 2);
	}

	#[rstest]
	fn test_values_from_vec() {
		let vec = vec![Value::Int(Some(1))];
		let values: Values = vec.into();
		assert_eq!(values.len(), 1);
	}

	#[rstest]
	fn test_values_into_vec() {
		let values = Values(vec![Value::Int(Some(1))]);
		let vec: Vec<Value> = values.into();
		assert_eq!(vec.len(), 1);
	}

	#[rstest]
	fn test_values_deref() {
		let values = Values(vec![Value::Int(Some(1)), Value::Int(Some(2))]);
		assert_eq!(values.len(), 2);
		assert_eq!(values[0], Value::Int(Some(1)));
	}

	#[rstest]
	fn test_values_index() {
		let values = Values(vec![Value::Int(Some(1)), Value::Int(Some(2))]);
		assert_eq!(values[0], Value::Int(Some(1)));
		assert_eq!(values[1], Value::Int(Some(2)));
	}
}

mod array_type_tests {
	use super::*;
	use pretty_assertions::assert_eq;

	#[rstest]
	fn test_array_type_clone() {
		let arr = ArrayType::Int;
		let cloned = arr.clone();
		assert_eq!(arr, cloned);
	}

	#[rstest]
	fn test_array_type_debug() {
		let arr = ArrayType::String;
		let debug = format!("{:?}", arr);
		assert_eq!(debug, "String");
	}

	#[rstest]
	fn test_array_type_hash() {
		use std::collections::HashSet;

		let mut set = HashSet::new();
		set.insert(ArrayType::Int);
		set.insert(ArrayType::String);
		set.insert(ArrayType::Int); // duplicate

		assert_eq!(set.len(), 2);
	}
}

mod value_size_tests {
	use super::*;

	#[rstest]
	fn test_value_size_is_reasonable() {
		// Value should be reasonably sized (boxed types keep size down)
		// The exact size depends on platform, but should be <= 32 bytes
		let size = std::mem::size_of::<Value>();
		assert!(size <= 32, "Value size is {} bytes, expected <= 32", size);
	}

	#[rstest]
	fn test_value_array_size() {
		// Array variant should not significantly increase enum size
		let value_size = std::mem::size_of::<Value>();
		let array_type_size = std::mem::size_of::<ArrayType>();

		// ArrayType is small enough that it shouldn't bloat the enum
		assert!(
			array_type_size <= 8,
			"ArrayType size is {} bytes",
			array_type_size
		);

		// Value should still be reasonably sized
		assert!(value_size <= 32, "Value size is {} bytes", value_size);
	}
}
