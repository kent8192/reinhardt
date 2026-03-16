//! Webhook event handling and signature verification.

use std::collections::HashMap;

/// Webhook event from payment provider.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum WebhookEvent {
	/// Payment succeeded
	#[serde(rename = "payment_succeeded")]
	PaymentSucceeded {
		/// Payment intent ID
		payment_intent_id: String,
		/// Amount in smallest currency unit
		amount: u64,
		/// Custom metadata
		metadata: HashMap<String, String>,
	},

	/// Payment failed
	#[serde(rename = "payment_failed")]
	PaymentFailed {
		/// Payment intent ID
		payment_intent_id: String,
		/// Error message
		error: Option<String>,
	},

	/// Checkout session completed
	#[serde(rename = "checkout_completed")]
	CheckoutCompleted {
		/// Session ID
		session_id: String,
		/// Customer ID
		customer_id: Option<String>,
		/// Payment status
		payment_status: String,
	},

	/// Subscription updated
	#[serde(rename = "subscription_updated")]
	SubscriptionUpdated {
		/// Subscription ID
		subscription_id: String,
		/// Subscription status
		status: String,
	},

	/// Other event type
	#[serde(rename = "other")]
	Other {
		/// Event type name
		#[serde(rename = "event_type")]
		type_: String,
	},
}

pub mod events;
pub mod signature;
