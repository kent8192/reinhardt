//! Test fixtures using rstest framework.

use reinhardt_payment::vault::CardData;
use reinhardt_payment::{CheckoutMode, CheckoutParams, PaymentIntentParams, SubscriptionParams};
use reinhardt_payment_mocks::{MockBasisTheoryVault, MockStripeProvider};

use super::constants::*;

/// Mock Stripe provider fixture.
pub fn mock_provider() -> MockStripeProvider {
	MockStripeProvider::new("sk_test_mock_key")
}

/// Mock BasisTheory vault fixture.
pub fn mock_vault() -> MockBasisTheoryVault {
	MockBasisTheoryVault::new("bt_test_mock_key")
}

/// Standard payment intent parameters.
pub fn payment_intent_params() -> PaymentIntentParams {
	PaymentIntentParams {
		amount: TEST_AMOUNT_STANDARD,
		currency: TEST_CURRENCY_USD.to_string(),
		confirm: false,
		..Default::default()
	}
}

/// Payment intent parameters with idempotency key.
pub fn payment_intent_params_with_idempotency() -> PaymentIntentParams {
	PaymentIntentParams {
		amount: TEST_AMOUNT_STANDARD,
		currency: TEST_CURRENCY_USD.to_string(),
		confirm: false,
		idempotency_key: Some(reinhardt_payment::idempotency::IdempotencyKeyGenerator::generate()),
		..Default::default()
	}
}

/// Checkout session parameters for payment mode.
pub fn checkout_params_payment() -> CheckoutParams {
	CheckoutParams {
		mode: CheckoutMode::Payment,
		success_url: "https://example.com/success".to_string(),
		cancel_url: "https://example.com/cancel".to_string(),
		price_id: TEST_PRICE_ID.to_string(),
		customer_id: None,
	}
}

/// Checkout session parameters for subscription mode.
pub fn checkout_params_subscription() -> CheckoutParams {
	CheckoutParams {
		mode: CheckoutMode::Subscription,
		success_url: "https://example.com/success".to_string(),
		cancel_url: "https://example.com/cancel".to_string(),
		price_id: TEST_PRICE_ID.to_string(),
		customer_id: None,
	}
}

/// Subscription parameters.
pub fn subscription_params() -> SubscriptionParams {
	SubscriptionParams {
		customer_id: TEST_CUSTOMER_ID.to_string(),
		price_id: TEST_PRICE_ID.to_string(),
		trial_period_days: None,
		default_payment_method: None,
	}
}

/// Subscription parameters with trial period.
pub fn subscription_params_with_trial() -> SubscriptionParams {
	SubscriptionParams {
		customer_id: TEST_CUSTOMER_ID.to_string(),
		price_id: TEST_PRICE_ID.to_string(),
		trial_period_days: Some(14),
		default_payment_method: None,
	}
}

/// Valid test card data.
pub fn test_card_data() -> CardData {
	CardData {
		number: TEST_CARD_VISA.to_string(),
		exp_month: 12,
		exp_year: valid_test_exp_year(),
		cvc: "123".to_string(),
	}
}

/// Invalid card data (too short).
pub fn invalid_card_data_short() -> CardData {
	CardData {
		number: "123".to_string(),
		exp_month: 12,
		exp_year: valid_test_exp_year(),
		cvc: "123".to_string(),
	}
}

/// Expired card data.
pub fn expired_card_data() -> CardData {
	CardData {
		number: TEST_CARD_VISA.to_string(),
		exp_month: 1,
		exp_year: 2020, // Expired
		cvc: "123".to_string(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_fixture_constants_exist() {
		assert_eq!(TEST_AMOUNT_STANDARD, 1000);
		assert_eq!(TEST_CURRENCY_USD, "usd");
		assert_eq!(TEST_CARD_VISA, "4242424242424242");
	}

	#[test]
	fn test_payment_intent_params_fixture() {
		let params = payment_intent_params();
		assert_eq!(params.amount, 1000);
		assert_eq!(params.currency, "usd");
		assert!(!params.confirm);
	}

	#[test]
	fn test_checkout_params_fixture() {
		let params = checkout_params_payment();
		assert_eq!(params.mode, CheckoutMode::Payment);
	}

	#[test]
	fn test_subscription_params_fixture() {
		let params = subscription_params();
		assert_eq!(params.customer_id, "cus_test_123");
		assert_eq!(params.price_id, "price_test_123");
		assert!(params.trial_period_days.is_none());
	}

	#[test]
	fn test_card_data_fixture() {
		let card = test_card_data();
		assert_eq!(card.number, "4242424242424242");
		assert_eq!(card.exp_month, 12);
		assert_eq!(card.exp_year, valid_test_exp_year());
		assert_eq!(card.cvc, "123");
	}
}
