//! Value serialization to Redis byte representation.

use crate::value::Value;

/// Trait for converting Rust values to Redis byte representation.
///
/// Independent from [`IntoValue`](crate::value::IntoValue), which targets SQL.
/// Redis treats all values as byte strings, so this trait converts directly to bytes.
pub trait ToRedisBytes {
    /// Convert this value to a Redis byte string.
    fn to_redis_bytes(self) -> Vec<u8>;
}

impl ToRedisBytes for &str {
    fn to_redis_bytes(self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl ToRedisBytes for String {
    fn to_redis_bytes(self) -> Vec<u8> {
        self.into_bytes()
    }
}

impl ToRedisBytes for i64 {
    fn to_redis_bytes(self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl ToRedisBytes for u64 {
    fn to_redis_bytes(self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl ToRedisBytes for i32 {
    fn to_redis_bytes(self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl ToRedisBytes for u32 {
    fn to_redis_bytes(self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl ToRedisBytes for f64 {
    fn to_redis_bytes(self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl ToRedisBytes for bool {
    fn to_redis_bytes(self) -> Vec<u8> {
        if self { b"1".to_vec() } else { b"0".to_vec() }
    }
}

impl ToRedisBytes for Vec<u8> {
    fn to_redis_bytes(self) -> Vec<u8> {
        self
    }
}

impl ToRedisBytes for &[u8] {
    fn to_redis_bytes(self) -> Vec<u8> {
        self.to_vec()
    }
}

impl ToRedisBytes for Value {
    fn to_redis_bytes(self) -> Vec<u8> {
        match self {
            Value::Bool(Some(b)) => if b { b"1".to_vec() } else { b"0".to_vec() },
            Value::TinyInt(Some(n)) => n.to_string().into_bytes(),
            Value::SmallInt(Some(n)) => n.to_string().into_bytes(),
            Value::Int(Some(n)) => n.to_string().into_bytes(),
            Value::BigInt(Some(n)) => n.to_string().into_bytes(),
            Value::TinyUnsigned(Some(n)) => n.to_string().into_bytes(),
            Value::SmallUnsigned(Some(n)) => n.to_string().into_bytes(),
            Value::Unsigned(Some(n)) => n.to_string().into_bytes(),
            Value::BigUnsigned(Some(n)) => n.to_string().into_bytes(),
            Value::Float(Some(f)) => f.to_string().into_bytes(),
            Value::Double(Some(f)) => f.to_string().into_bytes(),
            Value::String(Some(s)) => s.into_bytes(),
            Value::Bytes(Some(b)) => *b,
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn test_str_to_redis_bytes() {
        assert_eq!("hello".to_redis_bytes(), b"hello");
    }

    #[rstest]
    fn test_i64_to_redis_bytes() {
        assert_eq!(42i64.to_redis_bytes(), b"42");
    }

    #[rstest]
    fn test_negative_i64() {
        assert_eq!((-1i64).to_redis_bytes(), b"-1");
    }

    #[rstest]
    fn test_u64_to_redis_bytes() {
        assert_eq!(100u64.to_redis_bytes(), b"100");
    }

    #[rstest]
    fn test_bool_true() {
        assert_eq!(true.to_redis_bytes(), b"1");
    }

    #[rstest]
    fn test_bool_false() {
        assert_eq!(false.to_redis_bytes(), b"0");
    }

    #[rstest]
    fn test_string_to_redis_bytes() {
        assert_eq!(String::from("world").to_redis_bytes(), b"world");
    }

    #[rstest]
    fn test_bytes_passthrough() {
        assert_eq!(vec![1u8, 2, 3].to_redis_bytes(), vec![1, 2, 3]);
    }

    #[rstest]
    fn test_value_big_int() {
        // Arrange
        let v = Value::BigInt(Some(99));

        // Act
        let bytes = v.to_redis_bytes();

        // Assert
        assert_eq!(bytes, b"99");
    }

    #[rstest]
    fn test_value_string() {
        // Arrange
        let v = Value::String(Some(Box::new("hello".to_string())));

        // Act
        let bytes = v.to_redis_bytes();

        // Assert
        assert_eq!(bytes, b"hello");
    }
}
