//! PaymentIntent types for one-time payments.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Payment intent for one-time payments.
///
/// # Lifecycle
///
/// ```text
/// requires_payment_method → requires_confirmation → requires_action (3DS)
///                                                 → processing
///                                                 → succeeded ✓
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentIntent {
	/// Unique identifier
	pub id: String,
	/// Amount in smallest currency unit (e.g., cents for USD)
	pub amount: u64,
	/// Currency code (ISO 4217)
	pub currency: String,
	/// Current status
	pub status: PaymentIntentStatus,
	/// Client secret for frontend confirmation
	pub client_secret: Option<String>,
	/// Custom metadata
	pub metadata: HashMap<String, String>,
	/// Creation timestamp
	pub created_at: DateTime<Utc>,
}

/// Payment intent lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentIntentStatus {
	/// Awaiting payment method
	RequiresPaymentMethod,
	/// Awaiting confirmation
	RequiresConfirmation,
	/// Requires additional action (3DS, etc.)
	RequiresAction,
	/// Processing asynchronously
	Processing,
	/// Awaiting manual capture
	RequiresCapture,
	/// Payment succeeded
	Succeeded,
	/// Payment canceled
	Canceled,
}

/// Parameters for creating a payment intent.
#[derive(Debug, Clone)]
pub struct PaymentIntentParams {
	/// Amount in smallest currency unit
	pub amount: u64,
	/// Currency code (e.g., "usd", "jpy")
	pub currency: String,
	/// Optional payment method ID
	pub payment_method: Option<String>,
	/// Confirm immediately
	pub confirm: bool,
	/// Return URL for 3DS redirect
	pub return_url: Option<String>,
	/// Capture method (immediate or manual)
	pub capture_method: Option<CaptureMethod>,
	/// Custom metadata
	pub metadata: Option<HashMap<String, String>>,
	/// Idempotency key for safe retry
	pub idempotency_key: Option<String>,
}

impl Default for PaymentIntentParams {
	fn default() -> Self {
		Self {
			amount: 0,
			currency: "usd".to_string(),
			payment_method: None,
			confirm: false,
			return_url: None,
			capture_method: None,
			metadata: None,
			idempotency_key: None,
		}
	}
}

/// Capture method for payment intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMethod {
	/// Capture immediately on confirmation
	Automatic,
	/// Manual capture required
	Manual,
}
