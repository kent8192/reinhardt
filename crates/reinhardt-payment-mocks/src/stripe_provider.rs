//! Mock Stripe provider for testing PaymentProvider trait.

use async_trait::async_trait;
use reinhardt_payment::{
	CheckoutParams, CheckoutSession, PaymentError, PaymentIntent, PaymentIntentParams,
	PaymentIntentStatus, PaymentProvider, Subscription, SubscriptionParams, SubscriptionStatus,
	SubscriptionUpdateParams, WebhookEvent,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Mock Stripe provider for testing.
///
/// This provider stores all data in memory and can be configured
/// to fail operations for testing error paths.
pub struct MockStripeProvider {
	_api_key: String,
	payment_intents: Arc<RwLock<HashMap<String, PaymentIntent>>>,
	checkout_sessions: Arc<RwLock<HashMap<String, CheckoutSession>>>,
	subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
	fail_next: Arc<RwLock<bool>>,
}

impl MockStripeProvider {
	/// Creates a new mock provider.
	///
	/// # Arguments
	///
	/// * `api_key` - Mock API key (not used for actual API calls)
	pub fn new(api_key: impl Into<String>) -> Self {
		Self {
			_api_key: api_key.into(),
			payment_intents: Arc::new(RwLock::new(HashMap::new())),
			checkout_sessions: Arc::new(RwLock::new(HashMap::new())),
			subscriptions: Arc::new(RwLock::new(HashMap::new())),
			fail_next: Arc::new(RwLock::new(false)),
		}
	}

	/// Configures whether the next operation should fail.
	///
	/// # Arguments
	///
	/// * `fail` - If true, the next operation will return an error
	pub async fn set_fail_next(&self, fail: bool) {
		*self.fail_next.write().await = fail;
	}

	/// Clears all stored data.
	pub async fn clear(&self) {
		self.payment_intents.write().await.clear();
		self.checkout_sessions.write().await.clear();
		self.subscriptions.write().await.clear();
	}

	/// Gets the number of stored payment intents.
	pub async fn payment_intent_count(&self) -> usize {
		self.payment_intents.read().await.len()
	}

	/// Gets the number of stored checkout sessions.
	pub async fn checkout_session_count(&self) -> usize {
		self.checkout_sessions.read().await.len()
	}

	/// Gets the number of stored subscriptions.
	pub async fn subscription_count(&self) -> usize {
		self.subscriptions.read().await.len()
	}
}

#[async_trait]
impl PaymentProvider for MockStripeProvider {
	async fn create_payment_intent(
		&self,
		params: PaymentIntentParams,
	) -> Result<PaymentIntent, PaymentError> {
		if *self.fail_next.read().await {
			return Err(PaymentError::ProviderError(
				"Mock configured to fail".to_string(),
			));
		}

		// Validate amount
		if params.amount == 0 {
			return Err(PaymentError::InvalidParameters(
				"Amount must be greater than zero".to_string(),
			));
		}

		let id = format!("pi_mock_{}", Uuid::new_v4());
		let client_secret = Some(format!("pi_mock_secret_{}", Uuid::new_v4()));

		let intent = PaymentIntent {
			id: id.clone(),
			amount: params.amount,
			currency: params.currency,
			status: if params.confirm {
				PaymentIntentStatus::RequiresConfirmation
			} else {
				PaymentIntentStatus::RequiresPaymentMethod
			},
			client_secret,
			metadata: params.metadata.unwrap_or_default(),
			created_at: chrono::Utc::now(),
		};

		self.payment_intents
			.write()
			.await
			.insert(id, intent.clone());
		Ok(intent)
	}

	async fn confirm_payment(&self, id: &str) -> Result<PaymentIntent, PaymentError> {
		if *self.fail_next.read().await {
			return Err(PaymentError::ProviderError(
				"Mock configured to fail".to_string(),
			));
		}

		let mut intents = self.payment_intents.write().await;
		let intent = intents
			.get_mut(id)
			.ok_or_else(|| PaymentError::NotFound(id.to_string()))?;

		match intent.status {
			PaymentIntentStatus::Canceled => {
				return Err(PaymentError::InvalidParameters(
					"Cannot confirm canceled payment intent".to_string(),
				));
			}
			PaymentIntentStatus::Succeeded => {
				return Err(PaymentError::AlreadyProcessed(id.to_string()));
			}
			_ => {}
		}

		intent.status = PaymentIntentStatus::Succeeded;
		Ok(intent.clone())
	}

	async fn capture_payment(
		&self,
		id: &str,
		amount: Option<u64>,
	) -> Result<PaymentIntent, PaymentError> {
		if *self.fail_next.read().await {
			return Err(PaymentError::ProviderError(
				"Mock configured to fail".to_string(),
			));
		}

		let mut intents = self.payment_intents.write().await;
		let intent = intents
			.get_mut(id)
			.ok_or_else(|| PaymentError::NotFound(id.to_string()))?;

		if intent.status == PaymentIntentStatus::Succeeded {
			return Err(PaymentError::AlreadyProcessed(id.to_string()));
		}

		if let Some(capture_amount) = amount {
			intent.amount = capture_amount;
		}

		intent.status = PaymentIntentStatus::Succeeded;
		Ok(intent.clone())
	}

	async fn cancel_payment(&self, id: &str) -> Result<PaymentIntent, PaymentError> {
		if *self.fail_next.read().await {
			return Err(PaymentError::ProviderError(
				"Mock configured to fail".to_string(),
			));
		}

		let mut intents = self.payment_intents.write().await;
		let intent = intents
			.get_mut(id)
			.ok_or_else(|| PaymentError::NotFound(id.to_string()))?;

		if intent.status == PaymentIntentStatus::Succeeded {
			return Err(PaymentError::InvalidParameters(
				"Cannot cancel succeeded payment intent".to_string(),
			));
		}

		intent.status = PaymentIntentStatus::Canceled;
		Ok(intent.clone())
	}

	async fn create_checkout_session(
		&self,
		params: CheckoutParams,
	) -> Result<CheckoutSession, PaymentError> {
		if *self.fail_next.read().await {
			return Err(PaymentError::ProviderError(
				"Mock configured to fail".to_string(),
			));
		}

		let id = format!("cs_mock_{}", Uuid::new_v4());
		let url = format!("https://checkout.stripe.com/mock/{}", id);

		let session = CheckoutSession {
			id: id.clone(),
			url,
			customer: params.customer_id,
			payment_status: "unpaid".to_string(),
			status: "open".to_string(),
		};

		self.checkout_sessions
			.write()
			.await
			.insert(id, session.clone());
		Ok(session)
	}

	async fn create_subscription(
		&self,
		params: SubscriptionParams,
	) -> Result<Subscription, PaymentError> {
		if *self.fail_next.read().await {
			return Err(PaymentError::ProviderError(
				"Mock configured to fail".to_string(),
			));
		}

		let id = format!("sub_mock_{}", Uuid::new_v4());
		let now = chrono::Utc::now();

		let subscription = Subscription {
			id: id.clone(),
			customer: params.customer_id,
			status: if params.trial_period_days.is_some() {
				SubscriptionStatus::Trialing
			} else {
				SubscriptionStatus::Active
			},
			current_period_start: now,
			current_period_end: now + chrono::Duration::days(30),
			cancel_at_period_end: false,
		};

		self.subscriptions
			.write()
			.await
			.insert(id, subscription.clone());
		Ok(subscription)
	}

	async fn update_subscription(
		&self,
		id: &str,
		params: SubscriptionUpdateParams,
	) -> Result<Subscription, PaymentError> {
		if *self.fail_next.read().await {
			return Err(PaymentError::ProviderError(
				"Mock configured to fail".to_string(),
			));
		}

		let mut subscriptions = self.subscriptions.write().await;
		let subscription = subscriptions
			.get_mut(id)
			.ok_or_else(|| PaymentError::NotFound(id.to_string()))?;

		if let Some(cancel_at_end) = params.cancel_at_period_end {
			subscription.cancel_at_period_end = cancel_at_end;
		}

		Ok(subscription.clone())
	}

	async fn cancel_subscription(&self, id: &str) -> Result<Subscription, PaymentError> {
		if *self.fail_next.read().await {
			return Err(PaymentError::ProviderError(
				"Mock configured to fail".to_string(),
			));
		}

		let mut subscriptions = self.subscriptions.write().await;
		let subscription = subscriptions
			.get_mut(id)
			.ok_or_else(|| PaymentError::NotFound(id.to_string()))?;

		subscription.status = SubscriptionStatus::Canceled;
		Ok(subscription.clone())
	}

	async fn handle_webhook(
		&self,
		payload: &[u8],
		_signature: &str,
	) -> Result<WebhookEvent, PaymentError> {
		// In mock mode, we parse the payload directly without signature verification
		let event: WebhookEvent =
			serde_json::from_slice(payload).map_err(PaymentError::SerializationError)?;

		Ok(event)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_create_mock_provider() {
		let provider = MockStripeProvider::new("sk_test_mock_key");
		assert_eq!(provider.payment_intent_count().await, 0);
	}

	#[tokio::test]
	async fn test_create_payment_intent_succeeds() {
		let provider = MockStripeProvider::new("sk_test_mock_key");
		let params = PaymentIntentParams {
			amount: 1000,
			currency: "usd".to_string(),
			confirm: false,
			..Default::default()
		};

		let result = provider.create_payment_intent(params).await;
		assert!(result.is_ok());
		let intent = result.unwrap();
		assert_eq!(intent.amount, 1000);
		assert_eq!(intent.currency, "usd");
	}

	#[tokio::test]
	async fn test_create_payment_intent_with_zero_amount_fails() {
		let provider = MockStripeProvider::new("sk_test_mock_key");
		let params = PaymentIntentParams {
			amount: 0,
			currency: "usd".to_string(),
			confirm: false,
			..Default::default()
		};

		let result = provider.create_payment_intent(params).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_set_fail_next() {
		let provider = MockStripeProvider::new("sk_test_mock_key");
		provider.set_fail_next(true).await;

		let params = PaymentIntentParams {
			amount: 1000,
			currency: "usd".to_string(),
			confirm: false,
			..Default::default()
		};

		let result = provider.create_payment_intent(params).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_clear_removes_all_data() {
		let provider = MockStripeProvider::new("sk_test_mock_key");
		let params = PaymentIntentParams {
			amount: 1000,
			currency: "usd".to_string(),
			confirm: false,
			..Default::default()
		};

		let _ = provider.create_payment_intent(params).await;
		assert_eq!(provider.payment_intent_count().await, 1);

		provider.clear().await;
		assert_eq!(provider.payment_intent_count().await, 0);
	}
}
