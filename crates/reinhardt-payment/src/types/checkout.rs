//! Checkout Session types for hosted payment pages.

use serde::{Deserialize, Serialize};

/// Checkout session for hosted payment pages.
///
/// Stripe-hosted payment page that handles the entire checkout flow
/// including payment method collection and confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
	/// Unique identifier
	pub id: String,
	/// Redirect URL for customer
	pub url: String,
	/// Associated customer ID
	pub customer: Option<String>,
	/// Payment status (paid, unpaid, no_payment_required)
	pub payment_status: String,
	/// Session status (open, complete, expired)
	pub status: String,
}

/// Parameters for creating a checkout session.
#[derive(Debug, Clone)]
pub struct CheckoutParams {
	/// Success redirect URL
	pub success_url: String,
	/// Cancel redirect URL
	pub cancel_url: String,
	/// Price ID for line item
	pub price_id: String,
	/// Optional customer ID
	pub customer_id: Option<String>,
	/// Session mode (payment or subscription)
	pub mode: CheckoutMode,
}

/// Checkout session mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckoutMode {
	/// One-time payment
	Payment,
	/// Recurring subscription
	Subscription,
}
