//! Configuration encryption/decryption

use aes_gcm::{
	Aes256Gcm, Nonce,
	aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};
use serde::{Deserialize, Serialize};

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedConfig {
	pub data: Vec<u8>,
	pub nonce: Vec<u8>,
}

impl EncryptedConfig {
	pub fn new(data: Vec<u8>, nonce: Vec<u8>) -> Self {
		Self { data, nonce }
	}
}

pub struct ConfigEncryptor {
	cipher: Aes256Gcm,
}

impl std::fmt::Debug for ConfigEncryptor {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ConfigEncryptor")
			.field("cipher", &"<cipher>")
			.finish()
	}
}

impl ConfigEncryptor {
	/// Create a new ConfigEncryptor with a 32-byte (256-bit) key
	///
	/// # Arguments
	///
	/// * `key` - A 32-byte key for AES-256-GCM encryption
	///
	/// # Errors
	///
	/// Returns an error if the key length is not exactly 32 bytes
	pub fn new(key: Vec<u8>) -> Result<Self, String> {
		if key.len() != 32 {
			return Err(format!(
				"Encryption key must be exactly 32 bytes (256 bits), got {} bytes",
				key.len()
			));
		}
		let cipher = Aes256Gcm::new_from_slice(&key)
			.map_err(|e| format!("Failed to initialize cipher: {}", e))?;
		Ok(Self { cipher })
	}

	/// Encrypt data using AES-256-GCM
	///
	/// Generates a random 12-byte nonce and encrypts the data with authentication.
	///
	/// # Arguments
	///
	/// * `data` - The plaintext data to encrypt
	///
	/// # Returns
	///
	/// An `EncryptedConfig` containing the ciphertext and nonce
	///
	/// # Errors
	///
	/// Returns an error if encryption fails
	///
	/// # Examples
	///
	/// ```ignore
	/// let key = vec![0u8; 32]; // Use a proper random key in production
	/// let encryptor = ConfigEncryptor::new(key)?;
	/// let encrypted = encryptor.encrypt(b"secret data")?;
	/// ```
	pub fn encrypt(&self, data: &[u8]) -> Result<EncryptedConfig, String> {
		// Generate a random 12-byte nonce
		let mut nonce_bytes = [0u8; 12];
		OsRng.fill_bytes(&mut nonce_bytes);
		let nonce = Nonce::from(nonce_bytes);

		// Encrypt the data
		let ciphertext = self
			.cipher
			.encrypt(&nonce, data)
			.map_err(|e| format!("Encryption failed: {}", e))?;

		Ok(EncryptedConfig {
			data: ciphertext,
			nonce: nonce_bytes.to_vec(),
		})
	}

	/// Decrypt data using AES-256-GCM
	///
	/// Decrypts the ciphertext using the provided nonce and verifies the authentication tag.
	///
	/// # Arguments
	///
	/// * `encrypted` - The encrypted config containing ciphertext and nonce
	///
	/// # Returns
	///
	/// The decrypted plaintext data
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The nonce length is not 12 bytes
	/// - Decryption fails (invalid key or corrupted data)
	/// - Authentication tag verification fails (data has been tampered with)
	///
	/// # Examples
	///
	/// ```ignore
	/// let key = vec![0u8; 32]; // Use the same key as encryption
	/// let encryptor = ConfigEncryptor::new(key)?;
	/// let encrypted = encryptor.encrypt(b"secret data")?;
	/// let decrypted = encryptor.decrypt(&encrypted)?;
	/// assert_eq!(decrypted, b"secret data");
	/// ```
	pub fn decrypt(&self, encrypted: &EncryptedConfig) -> Result<Vec<u8>, String> {
		// Validate nonce length
		if encrypted.nonce.len() != 12 {
			return Err(format!(
				"Invalid nonce length: expected 12 bytes, got {}",
				encrypted.nonce.len()
			));
		}

		let nonce_array: [u8; 12] = encrypted.nonce[..12]
			.try_into()
			.map_err(|_| "Failed to convert nonce to array".to_string())?;
		let nonce = Nonce::from(nonce_array);

		// Decrypt and verify authentication tag
		let plaintext = self
			.cipher
			.decrypt(&nonce, encrypted.data.as_ref())
			.map_err(|e| format!("Decryption failed: {}", e))?;

		Ok(plaintext)
	}
}
