//! Payment provider trait and implementations.

use crate::types::{
	CheckoutParams, CheckoutSession, PaymentError, PaymentIntent, PaymentIntentParams,
	Subscription, SubscriptionParams, SubscriptionUpdateParams,
};
use crate::webhook::WebhookEvent;
use async_trait::async_trait;

/// Payment provider abstraction for multiple payment processors.
///
/// This trait defines the interface for payment operations including
/// one-time payments, checkout sessions, and subscriptions.
#[async_trait]
pub trait PaymentProvider: Send + Sync {
	/// Creates a new payment intent for one-time payments.
	async fn create_payment_intent(
		&self,
		params: PaymentIntentParams,
	) -> Result<PaymentIntent, PaymentError>;

	/// Confirms a payment intent.
	async fn confirm_payment(&self, id: &str) -> Result<PaymentIntent, PaymentError>;

	/// Captures a payment (for manual capture mode).
	async fn capture_payment(
		&self,
		id: &str,
		amount: Option<u64>,
	) -> Result<PaymentIntent, PaymentError>;

	/// Cancels a payment intent.
	async fn cancel_payment(&self, id: &str) -> Result<PaymentIntent, PaymentError>;

	/// Creates a checkout session for hosted payment pages.
	async fn create_checkout_session(
		&self,
		params: CheckoutParams,
	) -> Result<CheckoutSession, PaymentError>;

	/// Creates a new subscription.
	async fn create_subscription(
		&self,
		params: SubscriptionParams,
	) -> Result<Subscription, PaymentError>;

	/// Updates an existing subscription.
	async fn update_subscription(
		&self,
		id: &str,
		params: SubscriptionUpdateParams,
	) -> Result<Subscription, PaymentError>;

	/// Cancels a subscription.
	async fn cancel_subscription(&self, id: &str) -> Result<Subscription, PaymentError>;

	/// Handles incoming webhook events.
	async fn handle_webhook(
		&self,
		payload: &[u8],
		signature: &str,
	) -> Result<WebhookEvent, PaymentError>;
}
