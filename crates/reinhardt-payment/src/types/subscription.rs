//! Subscription types for recurring payments.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Subscription for recurring payments.
///
/// Manages recurring billing cycles with automatic payment collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
	/// Unique identifier
	pub id: String,
	/// Customer ID
	pub customer: String,
	/// Subscription status
	pub status: SubscriptionStatus,
	/// Current billing period start
	pub current_period_start: DateTime<Utc>,
	/// Current billing period end
	pub current_period_end: DateTime<Utc>,
	/// Cancel at period end flag
	pub cancel_at_period_end: bool,
}

/// Subscription lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
	/// Active and paid
	Active,
	/// Payment failed, retry in progress
	PastDue,
	/// Payment failed after retries
	Unpaid,
	/// Subscription canceled
	Canceled,
	/// Initial payment incomplete
	Incomplete,
	/// Initial payment expired
	IncompleteExpired,
	/// In trial period
	Trialing,
}

/// Parameters for creating a subscription.
#[derive(Debug, Clone)]
pub struct SubscriptionParams {
	/// Customer ID
	pub customer_id: String,
	/// Price ID
	pub price_id: String,
	/// Trial period in days
	pub trial_period_days: Option<u32>,
	/// Default payment method
	pub default_payment_method: Option<String>,
}

/// Parameters for updating a subscription.
#[derive(Debug, Clone, Default)]
pub struct SubscriptionUpdateParams {
	/// New price ID
	pub price_id: Option<String>,
	/// Cancel at period end
	pub cancel_at_period_end: Option<bool>,
}
