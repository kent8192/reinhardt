//! Token vault trait for secure card data storage.

use async_trait::async_trait;

// Re-export VaultError from types module (canonical definition)
pub use crate::types::error::VaultError;

/// Card data for tokenization.
///
/// **Security Note**: This type does not implement `Debug` or `Display`
/// to prevent accidental logging of sensitive card data.
#[derive(Clone)]
pub struct CardData {
	/// Card number
	pub number: String,
	/// Expiration month (1-12)
	pub exp_month: u8,
	/// Expiration year (4 digits)
	pub exp_year: u16,
	/// CVC/CVV code
	pub cvc: String,
}

/// Card token.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Token {
	/// Unique token ID
	pub id: String,
	/// Creation timestamp
	pub created_at: chrono::DateTime<chrono::Utc>,
	/// Card fingerprint
	pub fingerprint: String,
}

/// Token information with masked display.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenInfo {
	/// Token ID
	pub id: String,
	/// Token type (e.g., "card")
	#[serde(rename = "type")]
	pub type_: String,
	/// Masked display (e.g., "XXXX-XXXX-XXXX-1234")
	pub mask: String,
	/// Custom metadata
	pub metadata: std::collections::HashMap<String, String>,
}

/// Payment result from tokenized payment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaymentResult {
	/// Payment status
	pub status: String,
	/// Payment intent ID
	pub payment_intent_id: String,
}

/// Token vault abstraction for secure card data storage.
///
/// This trait provides PCI-compliant tokenization for sensitive
/// payment method data.
#[async_trait]
pub trait TokenVault: Send + Sync {
	/// Tokenizes card data.
	async fn tokenize_card(&self, card: CardData) -> Result<Token, VaultError>;

	/// Processes payment using a token.
	async fn process_payment_with_token(
		&self,
		token_id: &str,
		amount: u64,
		currency: &str,
	) -> Result<PaymentResult, VaultError>;

	/// Retrieves token metadata.
	async fn get_token(&self, token_id: &str) -> Result<TokenInfo, VaultError>;

	/// Deletes a token.
	async fn delete_token(&self, token_id: &str) -> Result<(), VaultError>;
}
