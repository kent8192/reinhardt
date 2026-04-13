use async_trait::async_trait;

use super::error::TestAuthError;
use super::identity::SessionIdentity;
use crate::client::APIClient;

/// Secondary (multi-factor) authentication layer.
///
/// Open trait — implement this for custom secondary auth methods
/// (e.g., PassKey, FIDO2) that compose with a primary auth method.
#[async_trait]
pub trait SecondaryAuth: Send + Sync {
	/// Apply this secondary auth layer to an HTTP test client.
	///
	/// Typically sets additional headers or cookies on the client.
	async fn apply_to_client(
		&self,
		client: &APIClient,
		primary: &SessionIdentity,
	) -> Result<(), TestAuthError>;
}

/// TOTP MFA secondary authentication for tests.
///
/// When applied, sets the `X-MFA-Code` header with either an auto-generated
/// valid TOTP code or a manually specified code.
#[cfg(native)]
pub struct TotpSecondaryAuth {
	manager: reinhardt_auth::mfa::MFAAuthentication,
	code_override: Option<String>,
}

#[cfg(native)]
impl TotpSecondaryAuth {
	/// Create a new TOTP secondary auth using the given MFA manager.
	pub fn new(manager: reinhardt_auth::mfa::MFAAuthentication) -> Self {
		Self {
			manager,
			code_override: None,
		}
	}

	/// Create a TOTP secondary auth with a pre-generated code.
	///
	/// Use this when you don't have access to the `MFAAuthentication` manager,
	/// or when testing with a specific code (including invalid codes).
	pub fn with_code_only(code: impl Into<String>) -> Self {
		Self {
			manager: reinhardt_auth::mfa::MFAAuthentication::new("test"),
			code_override: Some(code.into()),
		}
	}

	/// Override the TOTP code with a specific value.
	///
	/// Useful for testing invalid codes or expired codes.
	pub fn with_code(mut self, code: impl Into<String>) -> Self {
		self.code_override = Some(code.into());
		self
	}
}

#[cfg(native)]
#[async_trait]
impl SecondaryAuth for TotpSecondaryAuth {
	async fn apply_to_client(
		&self,
		client: &APIClient,
		primary: &SessionIdentity,
	) -> Result<(), TestAuthError> {
		let code = match &self.code_override {
			Some(c) => c.clone(),
			None => {
				let secret = self
					.manager
					.get_secret(&primary.user_id)
					.await
					.ok_or_else(|| TestAuthError::MfaUserNotRegistered(primary.user_id.clone()))?;
				generate_totp_code(&secret)
					.map_err(|e| TestAuthError::SecondaryAuthError(e.to_string()))?
			}
		};
		client
			.set_header("X-MFA-Code", &code)
			.await
			.map_err(|e| TestAuthError::ClientError(e.to_string()))?;
		Ok(())
	}
}

/// Generate a TOTP code from a base32-encoded secret using the same
/// algorithm as `MFAAuthentication` (SHA-256, 6 digits, 30s window).
#[cfg(native)]
fn generate_totp_code(secret: &str) -> Result<String, String> {
	use std::time::{SystemTime, UNIX_EPOCH};

	let secret_bytes = base32_decode(secret).ok_or("invalid base32 secret")?;
	let time = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_err(|e| e.to_string())?
		.as_secs();
	let step = time / 30;
	Ok(totp_lite::totp_custom::<totp_lite::Sha256>(
		30,
		6,
		&secret_bytes,
		step,
	))
}

/// Minimal base32 decoder (RFC 4648, no padding required).
#[cfg(native)]
fn base32_decode(input: &str) -> Option<Vec<u8>> {
	const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
	let mut bits = 0u64;
	let mut bit_count = 0u32;
	let mut result = Vec::new();

	for &byte in input.as_bytes() {
		if byte == b'=' {
			break;
		}
		let val = ALPHABET
			.iter()
			.position(|&c| c == byte.to_ascii_uppercase())? as u64;
		bits = (bits << 5) | val;
		bit_count += 5;
		if bit_count >= 8 {
			bit_count -= 8;
			result.push((bits >> bit_count) as u8);
			bits &= (1 << bit_count) - 1;
		}
	}
	Some(result)
}

#[cfg(test)]
mod tests {
	#[cfg(native)]
	mod totp_tests {
		use super::super::*;
		use rstest::*;

		#[rstest]
		fn base32_decode_known_value() {
			let decoded = base32_decode("JBSWY3DPEHPK3PXP");
			assert!(decoded.is_some());
			let bytes = decoded.unwrap();
			assert!(!bytes.is_empty());
		}

		#[rstest]
		fn generate_totp_produces_six_digit_code() {
			let code = generate_totp_code("JBSWY3DPEHPK3PXP").unwrap();
			assert_eq!(code.len(), 6);
			assert!(code.chars().all(|c| c.is_ascii_digit()));
		}
	}
}
