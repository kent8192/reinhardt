//! PaymentIntent integration tests.

use reinhardt_payment::{PaymentIntentStatus, PaymentProvider};
use reinhardt_payment_mocks::MockStripeProvider;

use super::common::*;

/// Test creating a payment intent with valid parameters succeeds.
#[tokio::test]
async fn test_create_payment_intent_with_valid_params_succeeds() {
	let provider = mock_provider();
	let mut params = payment_intent_params();
	params.amount = TEST_AMOUNT_STANDARD;
	params.currency = TEST_CURRENCY_USD.to_string();

	let result = provider.create_payment_intent(params).await;

	assert!(result.is_ok());
	let intent = result.unwrap();
	assert_eq!(intent.amount, TEST_AMOUNT_STANDARD);
	assert_eq!(intent.currency, TEST_CURRENCY_USD);
	assert!(matches!(
		intent.status,
		PaymentIntentStatus::RequiresPaymentMethod
	));
	assert!(intent.client_secret.is_some());
}

/// Test creating a payment intent with zero amount fails.
#[tokio::test]
async fn test_create_payment_intent_with_zero_amount_fails() {
	let provider = mock_provider();
	let mut params = payment_intent_params();
	params.amount = INVALID_AMOUNT_ZERO;

	let result = provider.create_payment_intent(params).await;

	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.to_string()
			.contains("must be greater than zero")
	);
}

/// Test creating a payment intent with confirm flag.
#[tokio::test]
async fn test_create_payment_intent_with_confirm_flag_succeeds() {
	let provider = mock_provider();
	let mut params = payment_intent_params();
	params.confirm = true;

	let result = provider.create_payment_intent(params).await;

	assert!(result.is_ok());
	let intent = result.unwrap();
	assert_eq!(intent.status, PaymentIntentStatus::RequiresConfirmation);
}

/// Test confirming a payment intent succeeds.
#[tokio::test]
async fn test_confirm_payment_with_valid_intent_succeeds() {
	let provider = mock_provider();
	let params = payment_intent_params();

	let intent = provider.create_payment_intent(params).await.unwrap();

	let result = provider.confirm_payment(&intent.id).await;

	assert!(result.is_ok());
	let confirmed = result.unwrap();
	assert_eq!(confirmed.status, PaymentIntentStatus::Succeeded);
}

/// Test confirming an already confirmed payment intent fails.
#[tokio::test]
async fn test_confirm_payment_already_confirmed_fails() {
	let provider = mock_provider();
	let params = payment_intent_params();

	let intent = provider.create_payment_intent(params).await.unwrap();
	let _ = provider.confirm_payment(&intent.id).await;

	let result = provider.confirm_payment(&intent.id).await;

	assert!(result.is_err());
}

/// Test capturing a payment intent succeeds.
#[tokio::test]
async fn test_capture_payment_with_valid_intent_succeeds() {
	let provider = mock_provider();
	let params = payment_intent_params();

	let intent = provider.create_payment_intent(params).await.unwrap();

	let result = provider.capture_payment(&intent.id, None).await;

	assert!(result.is_ok());
	let captured = result.unwrap();
	assert_eq!(captured.status, PaymentIntentStatus::Succeeded);
}

/// Test capturing a payment with partial amount succeeds.
#[tokio::test]
async fn test_capture_payment_partial_amount_succeeds() {
	let provider = mock_provider();
	let params = payment_intent_params();

	let intent = provider.create_payment_intent(params).await.unwrap();

	let partial_amount = TEST_AMOUNT_STANDARD / 2;
	let result = provider
		.capture_payment(&intent.id, Some(partial_amount))
		.await;

	assert!(result.is_ok());
	let captured = result.unwrap();
	assert_eq!(captured.amount, partial_amount);
	assert_eq!(captured.status, PaymentIntentStatus::Succeeded);
}

/// Test canceling a payment intent succeeds.
#[tokio::test]
async fn test_cancel_payment_with_valid_intent_succeeds() {
	let provider = mock_provider();
	let params = payment_intent_params();

	let intent = provider.create_payment_intent(params).await.unwrap();

	let result = provider.cancel_payment(&intent.id).await;

	assert!(result.is_ok());
	let canceled = result.unwrap();
	assert_eq!(canceled.status, PaymentIntentStatus::Canceled);
}

/// Test canceling a succeeded payment intent fails.
#[tokio::test]
async fn test_cancel_payment_succeeded_intent_fails() {
	let provider = mock_provider();
	let params = payment_intent_params();

	let intent = provider.create_payment_intent(params).await.unwrap();
	let _ = provider.capture_payment(&intent.id, None).await;

	let result = provider.cancel_payment(&intent.id).await;

	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.to_string()
			.contains("Cannot cancel succeeded")
	);
}

/// Test payment intent lifecycle: create -> confirm -> succeed.
#[tokio::test]
async fn test_payment_lifecycle_automatic_capture_succeeds() {
	let provider = mock_provider();
	let params = payment_intent_params();

	// Create
	let intent = provider.create_payment_intent(params).await.unwrap();
	assert!(matches!(
		intent.status,
		PaymentIntentStatus::RequiresPaymentMethod
	));

	// Confirm
	let confirmed = provider.confirm_payment(&intent.id).await.unwrap();
	assert_eq!(confirmed.status, PaymentIntentStatus::Succeeded);
}

/// Test payment intent lifecycle: create -> cancel.
#[tokio::test]
async fn test_payment_lifecycle_cancellation_succeeds() {
	let provider = mock_provider();
	let params = payment_intent_params();

	// Create
	let intent = provider.create_payment_intent(params).await.unwrap();

	// Cancel
	let canceled = provider.cancel_payment(&intent.id).await.unwrap();
	assert_eq!(canceled.status, PaymentIntentStatus::Canceled);
}

/// Test that payment intents are stored in the mock provider.
#[tokio::test]
async fn test_payment_intents_are_stored_in_mock() {
	let provider = mock_provider();

	let params = payment_intent_params();
	let _ = provider.create_payment_intent(params).await;

	assert_eq!(provider.payment_intent_count().await, 1);

	let params2 = payment_intent_params();
	let _ = provider.create_payment_intent(params2).await;

	assert_eq!(provider.payment_intent_count().await, 2);
}

/// Test clearing payment intents from mock provider.
#[tokio::test]
async fn test_clear_payment_intents_succeeds() {
	let provider = mock_provider();

	let params = payment_intent_params();
	let _ = provider.create_payment_intent(params).await;
	assert_eq!(provider.payment_intent_count().await, 1);

	provider.clear().await;
	assert_eq!(provider.payment_intent_count().await, 0);
}

/// Test fail_next mode causes operations to fail.
#[tokio::test]
async fn test_fail_next_causes_operation_to_fail() {
	let provider = mock_provider();
	provider.set_fail_next(true).await;

	let params = payment_intent_params();
	let result = provider.create_payment_intent(params).await;

	assert!(result.is_err());
}
