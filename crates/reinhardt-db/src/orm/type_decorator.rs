use aes_gcm::{
	Aes256Gcm, Nonce,
	aead::{Aead, KeyInit},
};
use rand::RngCore;
/// TypeDecorator - Custom type wrapper for database columns
/// Based on SQLAlchemy's TypeDecorator
use serde::{Deserialize, Serialize};
use std::fmt;

/// Base trait for type decoration
pub trait TypeDecorator: Send + Sync {
	/// The underlying SQL type
	type ImplType;

	/// Convert Rust value to database value (before saving)
	fn process_bind_param(&self, value: &Self::ImplType) -> Result<Vec<u8>, TypeError>;

	/// Convert database value to Rust value (after loading)
	fn process_result_value(&self, value: &[u8]) -> Result<Self::ImplType, TypeError>;

	/// Get the SQL type name
	fn sql_type_name(&self) -> &str;
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum TypeError {
	SerializationError(String),
	DeserializationError(String),
	ValidationError(String),
	EncryptionError(String),
	DecryptionError(String),
}

impl fmt::Display for TypeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			TypeError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
			TypeError::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
			TypeError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
			TypeError::EncryptionError(msg) => write!(f, "Encryption error: {}", msg),
			TypeError::DecryptionError(msg) => write!(f, "Decryption error: {}", msg),
		}
	}
}

impl std::error::Error for TypeError {}

/// Encrypted string type decorator using AES-256-GCM
pub struct EncryptedString {
	encryption_key: [u8; 32], // AES-256 requires 32-byte key
}

impl EncryptedString {
	/// Create new EncryptedString with a 32-byte key
	pub fn new(key: [u8; 32]) -> Self {
		Self {
			encryption_key: key,
		}
	}
	/// Create from a `Vec<u8>`, padding or truncating as needed
	///
	pub fn from_key_bytes(key: Vec<u8>) -> Result<Self, TypeError> {
		if key.len() < 32 {
			return Err(TypeError::EncryptionError(
				"Encryption key must be at least 32 bytes".to_string(),
			));
		}
		let mut key_array = [0u8; 32];
		key_array.copy_from_slice(&key[..32]);
		Ok(Self::new(key_array))
	}

	fn encrypt(&self, data: &str) -> Result<Vec<u8>, TypeError> {
		let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
			.map_err(|e| TypeError::EncryptionError(e.to_string()))?;

		// Generate a random 12-byte nonce
		let mut nonce_bytes = [0u8; 12];
		rand::rng().fill_bytes(&mut nonce_bytes);
		let nonce = Nonce::from(nonce_bytes);

		// Encrypt the data
		let ciphertext = cipher
			.encrypt(&nonce, data.as_bytes())
			.map_err(|e| TypeError::EncryptionError(e.to_string()))?;

		// Prepend nonce to ciphertext for storage
		let mut result = nonce_bytes.to_vec();
		result.extend_from_slice(&ciphertext);

		Ok(result)
	}

	fn decrypt(&self, data: &[u8]) -> Result<String, TypeError> {
		if data.len() < 12 {
			return Err(TypeError::DecryptionError(
				"Invalid encrypted data: too short".to_string(),
			));
		}

		let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
			.map_err(|e| TypeError::DecryptionError(e.to_string()))?;

		// Extract nonce from the first 12 bytes
		let (nonce_bytes, ciphertext) = data.split_at(12);
		let nonce_array: [u8; 12] = nonce_bytes
			.try_into()
			.map_err(|_| TypeError::DecryptionError("Invalid nonce length".to_string()))?;
		let nonce = Nonce::from(nonce_array);

		// Decrypt
		let plaintext = cipher
			.decrypt(&nonce, ciphertext)
			.map_err(|e| TypeError::DecryptionError(e.to_string()))?;

		String::from_utf8(plaintext).map_err(|e| TypeError::DecryptionError(e.to_string()))
	}
}

impl TypeDecorator for EncryptedString {
	type ImplType = String;

	fn process_bind_param(&self, value: &Self::ImplType) -> Result<Vec<u8>, TypeError> {
		self.encrypt(value)
	}

	fn process_result_value(&self, value: &[u8]) -> Result<Self::ImplType, TypeError> {
		self.decrypt(value)
	}

	fn sql_type_name(&self) -> &str {
		"BLOB"
	}
}

/// JSON type decorator with custom serialization
pub struct JsonType<T: Serialize + for<'de> Deserialize<'de>> {
	_phantom: std::marker::PhantomData<T>,
}

impl<T: Serialize + for<'de> Deserialize<'de>> JsonType<T> {
	/// Create a new JsonType decorator for JSON serialization
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::type_decorator::JsonType;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Serialize, Deserialize)]
	/// struct Config {
	///     key: String,
	/// }
	///
	/// let json_type = JsonType::<Config>::new();
	/// // Verify type is created successfully
	/// let _: JsonType<Config> = json_type;
	/// ```
	pub fn new() -> Self {
		Self {
			_phantom: std::marker::PhantomData,
		}
	}
}

impl<T: Serialize + for<'de> Deserialize<'de> + Send + Sync> TypeDecorator for JsonType<T> {
	type ImplType = T;

	fn process_bind_param(&self, value: &Self::ImplType) -> Result<Vec<u8>, TypeError> {
		serde_json::to_vec(value).map_err(|e| TypeError::SerializationError(e.to_string()))
	}

	fn process_result_value(&self, value: &[u8]) -> Result<Self::ImplType, TypeError> {
		serde_json::from_slice(value).map_err(|e| TypeError::DeserializationError(e.to_string()))
	}

	fn sql_type_name(&self) -> &str {
		"TEXT"
	}
}

impl<T: Serialize + for<'de> Deserialize<'de>> Default for JsonType<T> {
	fn default() -> Self {
		Self::new()
	}
}

/// Phone number type with validation and formatting using phonenumber crate
pub struct PhoneNumberType {
	default_country: phonenumber::country::Id,
}

impl PhoneNumberType {
	/// Create a new PhoneNumberType with validation for a specific country
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::type_decorator::PhoneNumberType;
	///
	/// let us_phone = PhoneNumberType::new("US");
	/// let jp_phone = PhoneNumberType::new("JP");
	/// // Verify types are created successfully
	/// let _: PhoneNumberType = us_phone;
	/// let _: PhoneNumberType = jp_phone;
	/// ```
	pub fn new(country_code: impl Into<String>) -> Self {
		let code = country_code.into();
		let country = match code.as_str() {
			"US" => phonenumber::country::Id::US,
			"GB" => phonenumber::country::Id::GB,
			"JP" => phonenumber::country::Id::JP,
			"DE" => phonenumber::country::Id::DE,
			"FR" => phonenumber::country::Id::FR,
			_ => phonenumber::country::Id::US, // Default to US
		};

		Self {
			default_country: country,
		}
	}

	fn validate(&self, number: &str) -> Result<(), TypeError> {
		phonenumber::parse(Some(self.default_country), number)
			.map_err(|e| TypeError::ValidationError(format!("Invalid phone number: {}", e)))?;
		Ok(())
	}

	fn format(&self, number: &str) -> String {
		match phonenumber::parse(Some(self.default_country), number) {
			Ok(parsed) => {
				// Format as E.164 (international format)
				parsed.format().mode(phonenumber::Mode::E164).to_string()
			}
			Err(_) => {
				// Fallback: Remove non-numeric characters
				number.chars().filter(|c| c.is_numeric()).collect()
			}
		}
	}

	fn unformat(&self, formatted: &str) -> String {
		formatted.chars().filter(|c| c.is_numeric()).collect()
	}
}

impl TypeDecorator for PhoneNumberType {
	type ImplType = String;

	fn process_bind_param(&self, value: &Self::ImplType) -> Result<Vec<u8>, TypeError> {
		self.validate(value)?;
		let formatted = self.format(value);
		Ok(formatted.into_bytes())
	}

	fn process_result_value(&self, value: &[u8]) -> Result<Self::ImplType, TypeError> {
		let formatted = String::from_utf8(value.to_vec())
			.map_err(|e| TypeError::DeserializationError(e.to_string()))?;
		Ok(self.unformat(&formatted))
	}

	fn sql_type_name(&self) -> &str {
		"VARCHAR(20)"
	}
}

/// Email type with validation and normalization
pub struct EmailType;

impl EmailType {
	/// Create a new EmailType with validation and normalization
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::type_decorator::EmailType;
	///
	/// let email_type = EmailType::new();
	/// // Verify the email type is created successfully
	/// let _: EmailType = email_type;
	/// ```
	pub fn new() -> Self {
		Self
	}

	fn validate(&self, email: &str) -> Result<(), TypeError> {
		if !email.contains('@') || !email.contains('.') {
			return Err(TypeError::ValidationError(
				"Invalid email format".to_string(),
			));
		}
		Ok(())
	}

	fn normalize(&self, email: &str) -> String {
		email.to_lowercase().trim().to_string()
	}
}

impl Default for EmailType {
	fn default() -> Self {
		Self::new()
	}
}

impl TypeDecorator for EmailType {
	type ImplType = String;

	fn process_bind_param(&self, value: &Self::ImplType) -> Result<Vec<u8>, TypeError> {
		self.validate(value)?;
		let normalized = self.normalize(value);
		Ok(normalized.into_bytes())
	}

	fn process_result_value(&self, value: &[u8]) -> Result<Self::ImplType, TypeError> {
		String::from_utf8(value.to_vec())
			.map_err(|e| TypeError::DeserializationError(e.to_string()))
	}

	fn sql_type_name(&self) -> &str {
		"VARCHAR(255)"
	}
}

/// URL type with validation
pub struct UrlType;

impl UrlType {
	/// Create a new UrlType with URL validation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::type_decorator::UrlType;
	///
	/// let url_type = UrlType::new();
	/// // Verify the URL type is created successfully
	/// let _: UrlType = url_type;
	/// ```
	pub fn new() -> Self {
		Self
	}

	fn validate(&self, url: &str) -> Result<(), TypeError> {
		if !url.starts_with("http://") && !url.starts_with("https://") {
			return Err(TypeError::ValidationError(
				"URL must start with http:// or https://".to_string(),
			));
		}
		Ok(())
	}
}

impl Default for UrlType {
	fn default() -> Self {
		Self::new()
	}
}

impl TypeDecorator for UrlType {
	type ImplType = String;

	fn process_bind_param(&self, value: &Self::ImplType) -> Result<Vec<u8>, TypeError> {
		self.validate(value)?;
		Ok(value.as_bytes().to_vec())
	}

	fn process_result_value(&self, value: &[u8]) -> Result<Self::ImplType, TypeError> {
		String::from_utf8(value.to_vec())
			.map_err(|e| TypeError::DeserializationError(e.to_string()))
	}

	fn sql_type_name(&self) -> &str {
		"TEXT"
	}
}

/// Compressed text type using gzip compression
pub struct CompressedTextType {
	compression_level: flate2::Compression,
}

impl CompressedTextType {
	/// Create a new CompressedTextType with default compression level
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::type_decorator::CompressedTextType;
	///
	/// let compressed = CompressedTextType::new();
	/// // Verify the compressed text type is created successfully
	/// let _: CompressedTextType = compressed;
	/// ```
	pub fn new() -> Self {
		Self {
			compression_level: flate2::Compression::default(),
		}
	}

	/// Create a new CompressedTextType with custom compression level (0-9)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::type_decorator::CompressedTextType;
	///
	/// let compressed = CompressedTextType::with_level(9);
	/// // Verify the compressed text type with custom level is created successfully
	/// let _: CompressedTextType = compressed;
	/// ```
	pub fn with_level(level: u32) -> Self {
		Self {
			compression_level: flate2::Compression::new(level),
		}
	}

	fn compress(&self, text: &str) -> Result<Vec<u8>, TypeError> {
		use flate2::write::GzEncoder;
		use std::io::Write;

		let mut encoder = GzEncoder::new(Vec::new(), self.compression_level);
		encoder
			.write_all(text.as_bytes())
			.map_err(|e| TypeError::ValidationError(format!("Compression write error: {}", e)))?;
		encoder
			.finish()
			.map_err(|e| TypeError::ValidationError(format!("Compression error: {}", e)))
	}

	fn decompress(&self, data: &[u8]) -> Result<String, TypeError> {
		use flate2::read::GzDecoder;
		use std::io::Read;

		let mut decoder = GzDecoder::new(data);
		let mut decompressed = String::new();
		decoder
			.read_to_string(&mut decompressed)
			.map_err(|e| TypeError::DeserializationError(format!("Decompression error: {}", e)))?;
		Ok(decompressed)
	}
}

impl Default for CompressedTextType {
	fn default() -> Self {
		Self::new()
	}
}

impl TypeDecorator for CompressedTextType {
	type ImplType = String;

	fn process_bind_param(&self, value: &Self::ImplType) -> Result<Vec<u8>, TypeError> {
		self.compress(value)
	}

	fn process_result_value(&self, value: &[u8]) -> Result<Self::ImplType, TypeError> {
		self.decompress(value)
	}

	fn sql_type_name(&self) -> &str {
		"BLOB"
	}
}

/// Enum type decorator
pub struct EnumType<T> {
	variants: Vec<(T, String)>,
}

impl<T: Clone + PartialEq> EnumType<T> {
	/// Create a new EnumType with custom enum-to-string mappings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::type_decorator::EnumType;
	///
	/// #[derive(Clone, PartialEq)]
	/// enum Status { Active, Inactive }
	///
	/// let enum_type = EnumType::new(vec![
	///     (Status::Active, "active".to_string()),
	///     (Status::Inactive, "inactive".to_string()),
	/// ]);
	/// // Verify the enum type is created successfully
	/// let _: EnumType<Status> = enum_type;
	/// ```
	pub fn new(variants: Vec<(T, String)>) -> Self {
		Self { variants }
	}
	fn to_string(&self, value: &T) -> Result<String, TypeError> {
		self.variants
			.iter()
			.find(|(v, _)| v == value)
			.map(|(_, s)| s.clone())
			.ok_or_else(|| TypeError::SerializationError("Unknown enum variant".to_string()))
	}

	fn parse_string(&self, s: &str) -> Result<T, TypeError> {
		self.variants
			.iter()
			.find(|(_, name)| name == s)
			.map(|(v, _)| v.clone())
			.ok_or_else(|| TypeError::DeserializationError("Unknown enum value".to_string()))
	}
}

impl<T: Clone + PartialEq + Send + Sync> TypeDecorator for EnumType<T> {
	type ImplType = T;

	fn process_bind_param(&self, value: &Self::ImplType) -> Result<Vec<u8>, TypeError> {
		let s = self.to_string(value)?;
		Ok(s.into_bytes())
	}

	fn process_result_value(&self, value: &[u8]) -> Result<Self::ImplType, TypeError> {
		let s = String::from_utf8(value.to_vec())
			.map_err(|e| TypeError::DeserializationError(e.to_string()))?;
		self.parse_string(&s)
	}

	fn sql_type_name(&self) -> &str {
		"VARCHAR(50)"
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_encrypted_string() {
		let key = [42u8; 32];
		let decorator = EncryptedString::new(key);

		let original = "secret data".to_string();
		let encrypted = decorator.process_bind_param(&original).unwrap();
		let decrypted = decorator.process_result_value(&encrypted).unwrap();

		assert_eq!(original, decrypted);
		assert_ne!(original.as_bytes(), encrypted.as_slice());
	}

	#[test]
	fn test_phone_number_validation() {
		let decorator = PhoneNumberType::new("US");

		let valid = "555-123-4567".to_string();
		assert!(decorator.process_bind_param(&valid).is_ok());

		// Test empty string
		let invalid = "".to_string();
		assert!(decorator.process_bind_param(&invalid).is_err());
	}

	#[test]
	fn test_phone_number_formatting() {
		let decorator = PhoneNumberType::new("US");

		let number = "555-123-4567".to_string();
		let stored = decorator.process_bind_param(&number).unwrap();
		let retrieved = decorator.process_result_value(&stored).unwrap();

		assert_eq!(retrieved, "15551234567");
	}

	#[test]
	fn test_email_validation() {
		let decorator = EmailType::new();

		let valid = "test@example.com".to_string();
		assert!(decorator.process_bind_param(&valid).is_ok());

		let invalid = "notanemail".to_string();
		assert!(decorator.process_bind_param(&invalid).is_err());
	}

	#[test]
	fn test_email_normalization() {
		let decorator = EmailType::new();

		let email = "  Test@Example.COM  ".to_string();
		let stored = decorator.process_bind_param(&email).unwrap();
		let retrieved = decorator.process_result_value(&stored).unwrap();

		assert_eq!(retrieved, "test@example.com");
	}

	#[test]
	fn test_url_validation() {
		let decorator = UrlType::new();

		let valid = "https://example.com".to_string();
		assert!(decorator.process_bind_param(&valid).is_ok());

		let invalid = "not-a-url".to_string();
		assert!(decorator.process_bind_param(&invalid).is_err());
	}

	#[test]
	fn test_compressed_text() {
		let decorator = CompressedTextType::new();

		let text = "This is some text to compress".to_string();
		let compressed = decorator.process_bind_param(&text).unwrap();
		let decompressed = decorator.process_result_value(&compressed).unwrap();

		assert_eq!(text, decompressed);
	}

	#[derive(Debug, Clone, PartialEq)]
	enum Status {
		Active,
		Inactive,
		Pending,
	}

	#[test]
	fn test_enum_type() {
		let decorator = EnumType::new(vec![
			(Status::Active, "active".to_string()),
			(Status::Inactive, "inactive".to_string()),
			(Status::Pending, "pending".to_string()),
		]);

		let status = Status::Active;
		let stored = decorator.process_bind_param(&status).unwrap();
		let retrieved = decorator.process_result_value(&stored).unwrap();

		assert_eq!(status, retrieved);
		assert_eq!(String::from_utf8(stored).unwrap(), "active");
	}

	#[test]
	fn test_enum_type_invalid() {
		let decorator = EnumType::new(vec![(Status::Active, "active".to_string())]);

		let invalid = b"unknown";
		assert!(decorator.process_result_value(invalid).is_err());
	}

	#[derive(Serialize, Deserialize, Debug, PartialEq)]
	struct TestData {
		name: String,
		value: i32,
	}

	#[test]
	fn test_type_decorator_json() {
		let decorator = JsonType::<TestData>::new();

		let data = TestData {
			name: "test".to_string(),
			value: 42,
		};

		let stored = decorator.process_bind_param(&data).unwrap();
		let retrieved = decorator.process_result_value(&stored).unwrap();

		assert_eq!(data, retrieved);
	}

	#[test]
	fn test_type_sql_names() {
		assert_eq!(EncryptedString::new([0u8; 32]).sql_type_name(), "BLOB");
		assert_eq!(EmailType::new().sql_type_name(), "VARCHAR(255)");
		assert_eq!(UrlType::new().sql_type_name(), "TEXT");
		assert_eq!(CompressedTextType::new().sql_type_name(), "BLOB");
	}
}
