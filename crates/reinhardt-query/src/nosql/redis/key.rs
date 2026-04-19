//! Redis key type and conversion trait.

use crate::types::{Alias, DynIden, IntoIden};

/// A Redis key, built from any type implementing [`IntoRedisKey`].
///
/// Wraps a [`DynIden`] but serializes without SQL quoting,
/// producing raw UTF-8 bytes for use in Redis commands.
#[derive(Debug, Clone)]
pub struct RedisKey(pub(crate) DynIden);

impl RedisKey {
    /// Serialize this key to unquoted UTF-8 bytes for use in a Redis command.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut s = String::new();
        self.0.unquoted(&mut s);
        s.into_bytes()
    }
}

/// Conversion trait for types that can be used as Redis keys.
pub trait IntoRedisKey {
    /// Convert this type into a [`RedisKey`].
    fn into_redis_key(self) -> RedisKey;
}

impl IntoRedisKey for &str {
    fn into_redis_key(self) -> RedisKey {
        RedisKey(Alias::new(self).into_iden())
    }
}

impl IntoRedisKey for String {
    fn into_redis_key(self) -> RedisKey {
        RedisKey(Alias::new(self).into_iden())
    }
}

impl IntoRedisKey for DynIden {
    fn into_redis_key(self) -> RedisKey {
        RedisKey(self)
    }
}

impl IntoRedisKey for RedisKey {
    fn into_redis_key(self) -> RedisKey {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn test_key_from_str() {
        // Arrange + Act
        let key = "session:123".into_redis_key();

        // Assert
        assert_eq!(key.to_bytes(), b"session:123");
    }

    #[rstest]
    fn test_key_from_string() {
        // Arrange + Act
        let key = String::from("user:456").into_redis_key();

        // Assert
        assert_eq!(key.to_bytes(), b"user:456");
    }

    #[rstest]
    fn test_redis_key_passthrough() {
        // Arrange
        let original = "mykey".into_redis_key();

        // Act
        let passthrough = original.into_redis_key();

        // Assert
        assert_eq!(passthrough.to_bytes(), b"mykey");
    }
}
