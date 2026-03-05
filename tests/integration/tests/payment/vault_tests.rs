//! TokenVault integration tests.

use reinhardt_payment::vault::TokenVault;
use reinhardt_payment_mocks::MockBasisTheoryVault;

use super::common::*;

/// Test tokenizing a valid card succeeds.
#[tokio::test]
async fn test_tokenize_card_with_valid_data_succeeds() {
	let vault = mock_vault();
	let card = test_card_data();

	let result = vault.tokenize_card(card).await;

	assert!(result.is_ok());
	let token = result.unwrap();
	assert!(!token.id.is_empty());
	assert!(!token.fingerprint.is_empty());
	assert_eq!(vault.token_count().await, 1);
}

/// Test tokenizing a card with invalid number fails.
#[tokio::test]
async fn test_tokenize_card_with_invalid_number_fails() {
	let vault = mock_vault();
	let card = invalid_card_data_short();

	let result = vault.tokenize_card(card).await;

	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.to_string()
			.contains("Invalid card number")
	);
}

/// Test tokenizing an expired card fails.
#[tokio::test]
async fn test_tokenize_card_expired_card_fails() {
	let vault = mock_vault();
	let card = expired_card_data();

	let result = vault.tokenize_card(card).await;

	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("expired"));
}

/// Test that tokenized cards generate unique tokens.
#[tokio::test]
async fn test_tokenize_card_generates_unique_tokens() {
	let vault = mock_vault();
	let card = test_card_data();

	let token1 = vault.tokenize_card(card.clone()).await.unwrap();
	let token2 = vault.tokenize_card(card).await.unwrap();

	assert_ne!(token1.id, token2.id);
	assert_eq!(vault.token_count().await, 2);
}

/// Test processing payment with a valid token succeeds.
#[tokio::test]
async fn test_process_payment_with_token_succeeds() {
	let vault = mock_vault();
	let card = test_card_data();

	let token = vault.tokenize_card(card).await.unwrap();

	let result = vault
		.process_payment_with_token(&token.id, TEST_AMOUNT_STANDARD, TEST_CURRENCY_USD)
		.await;

	assert!(result.is_ok());
	let payment = result.unwrap();
	assert_eq!(payment.status, "succeeded");
	assert!(!payment.payment_intent_id.is_empty());
}

/// Test processing payment with invalid token fails.
#[tokio::test]
async fn test_process_payment_with_invalid_token_fails() {
	let vault = mock_vault();

	let result = vault
		.process_payment_with_token("invalid_token", TEST_AMOUNT_STANDARD, TEST_CURRENCY_USD)
		.await;

	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("not found"));
}

/// Test getting token info succeeds.
#[tokio::test]
async fn test_get_token_with_valid_id_succeeds() {
	let vault = mock_vault();
	let card = test_card_data();

	let token = vault.tokenize_card(card).await.unwrap();

	let result = vault.get_token(&token.id).await;

	assert!(result.is_ok());
	let info = result.unwrap();
	assert_eq!(info.id, token.id);
	assert_eq!(info.type_, "card");
	assert!(info.mask.contains("XXXX"));
}

/// Test getting token info with invalid ID fails.
#[tokio::test]
async fn test_get_token_with_invalid_id_fails() {
	let vault = mock_vault();

	let result = vault.get_token("nonexistent_token").await;

	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("not found"));
}

/// Test deleting a token succeeds.
#[tokio::test]
async fn test_delete_token_with_valid_id_succeeds() {
	let vault = mock_vault();
	let card = test_card_data();

	let token = vault.tokenize_card(card).await.unwrap();
	assert_eq!(vault.token_count().await, 1);

	let result = vault.delete_token(&token.id).await;

	assert!(result.is_ok());
	assert_eq!(vault.token_count().await, 1); // Token still counted, just marked as deleted
}

/// Test deleting an already deleted token fails.
#[tokio::test]
async fn test_delete_token_already_deleted_fails() {
	let vault = mock_vault();
	let card = test_card_data();

	let token = vault.tokenize_card(card).await.unwrap();
	let _ = vault.delete_token(&token.id).await;

	let result = vault.delete_token(&token.id).await;

	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("not found"));
}

/// Test that deleted tokens cannot be used for payment.
#[tokio::test]
async fn test_deleted_token_cannot_be_used() {
	let vault = mock_vault();
	let card = test_card_data();

	let token = vault.tokenize_card(card).await.unwrap();
	let _ = vault.delete_token(&token.id).await;

	let result = vault
		.process_payment_with_token(&token.id, TEST_AMOUNT_STANDARD, TEST_CURRENCY_USD)
		.await;

	assert!(result.is_err());
	assert!(result.unwrap_err().to_string().contains("not found"));
}

/// Test clearing tokens from the mock vault.
#[tokio::test]
async fn test_clear_tokens_succeeds() {
	let vault = mock_vault();
	let card = test_card_data();

	let _ = vault.tokenize_card(card).await;
	assert_eq!(vault.token_count().await, 1);

	vault.clear().await;
	assert_eq!(vault.token_count().await, 0);
}

/// Test fail_next mode causes operations to fail.
#[tokio::test]
async fn test_fail_next_causes_operation_to_fail() {
	let vault = mock_vault();
	vault.set_fail_next(true).await;

	let card = test_card_data();
	let result = vault.tokenize_card(card).await;

	assert!(result.is_err());
}

/// Test card data validation: card number too long.
#[tokio::test]
async fn test_tokenize_card_number_too_long_fails() {
	let vault = mock_vault();
	let mut card = test_card_data();
	card.number = "1".repeat(20); // 20 digits, too long

	let result = vault.tokenize_card(card).await;

	assert!(result.is_err());
}

/// Test card data validation: invalid expiration month.
#[tokio::test]
async fn test_tokenize_card_invalid_month_fails() {
	let vault = mock_vault();
	let mut card = test_card_data();
	card.exp_month = 13; // Invalid month

	let result = vault.tokenize_card(card).await;

	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.to_string()
			.contains("Invalid expiration month")
	);
}

/// Test tokenization preserves fingerprint.
#[tokio::test]
async fn test_tokenize_card_preserves_fingerprint() {
	let vault = mock_vault();
	let card = test_card_data();

	let token = vault.tokenize_card(card).await.unwrap();

	assert!(!token.fingerprint.is_empty());
	assert!(token.fingerprint.starts_with("fp_"));
}
