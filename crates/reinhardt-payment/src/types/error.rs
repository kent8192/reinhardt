//! Error types for payment operations.

use thiserror::Error;

/// Payment operation errors.
#[derive(Debug, Error)]
pub enum PaymentError {
	/// Invalid parameters
	#[error("Invalid parameters: {0}")]
	InvalidParameters(String),

	/// Payment provider error
	#[error("Payment provider error: {0}")]
	ProviderError(String),

	/// Network error
	#[error("Network error: {0}")]
	NetworkError(#[from] reqwest::Error),

	/// Serialization error
	#[error("Serialization error: {0}")]
	SerializationError(#[from] serde_json::Error),

	/// Invalid webhook signature
	#[error("Invalid webhook signature")]
	InvalidSignature,

	/// Payment not found
	#[error("Payment not found: {0}")]
	NotFound(String),

	/// Payment already processed
	#[error("Payment already processed: {0}")]
	AlreadyProcessed(String),

	/// Stripe API error
	#[error("Stripe API error: {0}")]
	StripeError(String),
}

/// Token vault operation errors.
#[derive(Debug, Error)]
pub enum VaultError {
	/// Invalid card data
	#[error("Invalid card data: {0}")]
	InvalidCardData(String),

	/// Tokenization failed
	#[error("Tokenization failed: {0}")]
	TokenizationFailed(String),

	/// Token not found
	#[error("Token not found: {0}")]
	TokenNotFound(String),

	/// Network error
	#[error("Network error: {0}")]
	NetworkError(#[from] reqwest::Error),

	/// API error
	#[error("API error: {0}")]
	ApiError(String),
}
