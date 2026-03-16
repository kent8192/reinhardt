//! Mock BasisTheory vault for testing TokenVault trait.

use async_trait::async_trait;
use chrono::Datelike;
use reinhardt_payment::vault::{CardData, PaymentResult, Token, TokenInfo, TokenVault, VaultError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Mock BasisTheory vault for testing.
///
/// This vault stores all tokens in memory and can be configured
/// to fail operations for testing error paths.
pub struct MockBasisTheoryVault {
	_api_key: String,
	tokens: Arc<RwLock<HashMap<String, Token>>>,
	/// Stores the last four digits of the card number per token ID.
	card_last_four: Arc<RwLock<HashMap<String, String>>>,
	deleted_tokens: Arc<RwLock<HashMap<String, ()>>>,
	fail_next: Arc<RwLock<bool>>,
}

impl MockBasisTheoryVault {
	/// Creates a new mock vault.
	///
	/// # Arguments
	///
	/// * `api_key` - Mock API key (not used for actual API calls)
	pub fn new(api_key: impl Into<String>) -> Self {
		Self {
			_api_key: api_key.into(),
			tokens: Arc::new(RwLock::new(HashMap::new())),
			card_last_four: Arc::new(RwLock::new(HashMap::new())),
			deleted_tokens: Arc::new(RwLock::new(HashMap::new())),
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
		self.tokens.write().await.clear();
		self.card_last_four.write().await.clear();
		self.deleted_tokens.write().await.clear();
	}

	/// Gets the number of stored tokens.
	pub async fn token_count(&self) -> usize {
		self.tokens.read().await.len()
	}

	/// Validates card number format.
	fn validate_card_number(number: &str) -> Result<(), VaultError> {
		if number.len() < 13 || number.len() > 19 {
			return Err(VaultError::InvalidCardData(
				"Invalid card number length".to_string(),
			));
		}

		// Check if all characters are digits
		if !number.chars().all(|c| c.is_ascii_digit()) {
			return Err(VaultError::InvalidCardData(
				"Card number must contain only digits".to_string(),
			));
		}

		Ok(())
	}

	/// Extracts the last four digits from a card number.
	fn extract_last_four(number: &str) -> String {
		number
			.chars()
			.rev()
			.take(4)
			.collect::<Vec<_>>()
			.into_iter()
			.rev()
			.collect()
	}

	/// Creates a masked display from the last four digits.
	fn mask_from_last_four(last_four: &str) -> String {
		format!("XXXX-XXXX-XXXX-{}", last_four)
	}
}

#[async_trait]
impl TokenVault for MockBasisTheoryVault {
	async fn tokenize_card(&self, card: CardData) -> Result<Token, VaultError> {
		if *self.fail_next.read().await {
			return Err(VaultError::TokenizationFailed(
				"Mock configured to fail".to_string(),
			));
		}

		// Validate card data
		Self::validate_card_number(&card.number)?;

		if card.exp_month < 1 || card.exp_month > 12 {
			return Err(VaultError::InvalidCardData(
				"Invalid expiration month".to_string(),
			));
		}

		let now = chrono::Utc::now();
		let current_year = now.year() as u16;

		if card.exp_year < current_year {
			return Err(VaultError::InvalidCardData("Card has expired".to_string()));
		}

		let last_four = Self::extract_last_four(&card.number);
		let token = Token {
			id: format!("bt_token_{}", Uuid::new_v4()),
			created_at: now,
			fingerprint: format!("fp_{}", last_four),
		};

		self.card_last_four
			.write()
			.await
			.insert(token.id.clone(), last_four);
		self.tokens
			.write()
			.await
			.insert(token.id.clone(), token.clone());
		Ok(token)
	}

	async fn process_payment_with_token(
		&self,
		token_id: &str,
		_amount: u64,
		_currency: &str,
	) -> Result<PaymentResult, VaultError> {
		if *self.fail_next.read().await {
			return Err(VaultError::ApiError("Mock configured to fail".to_string()));
		}

		// Check if token exists
		let tokens = self.tokens.read().await;
		if !tokens.contains_key(token_id) {
			return Err(VaultError::TokenNotFound(token_id.to_string()));
		}

		// Check if token was deleted
		let deleted = self.deleted_tokens.read().await;
		if deleted.contains_key(token_id) {
			return Err(VaultError::TokenNotFound(token_id.to_string()));
		}

		Ok(PaymentResult {
			status: "succeeded".to_string(),
			payment_intent_id: format!("pi_mock_{}", Uuid::new_v4()),
		})
	}

	async fn get_token(&self, token_id: &str) -> Result<TokenInfo, VaultError> {
		if *self.fail_next.read().await {
			return Err(VaultError::ApiError("Mock configured to fail".to_string()));
		}

		let tokens = self.tokens.read().await;
		let token = tokens
			.get(token_id)
			.ok_or_else(|| VaultError::TokenNotFound(token_id.to_string()))?;

		let last_four_map = self.card_last_four.read().await;
		let last_four = last_four_map
			.get(token_id)
			.map(|s| s.as_str())
			.unwrap_or("0000");

		Ok(TokenInfo {
			id: token.id.clone(),
			type_: "card".to_string(),
			mask: Self::mask_from_last_four(last_four),
			metadata: HashMap::new(),
		})
	}

	async fn delete_token(&self, token_id: &str) -> Result<(), VaultError> {
		if *self.fail_next.read().await {
			return Err(VaultError::ApiError("Mock configured to fail".to_string()));
		}

		let tokens = self.tokens.write().await;
		let mut deleted = self.deleted_tokens.write().await;

		if !tokens.contains_key(token_id) {
			return Err(VaultError::TokenNotFound(token_id.to_string()));
		}

		if deleted.contains_key(token_id) {
			return Err(VaultError::TokenNotFound(token_id.to_string()));
		}

		deleted.insert(token_id.to_string(), ());
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn valid_test_exp_year() -> u16 {
		chrono::Utc::now().year() as u16 + 5
	}

	#[tokio::test]
	async fn test_create_mock_vault() {
		let vault = MockBasisTheoryVault::new("bt_test_mock_key");
		assert_eq!(vault.token_count().await, 0);
	}

	#[tokio::test]
	async fn test_tokenize_valid_card_succeeds() {
		let vault = MockBasisTheoryVault::new("bt_test_mock_key");
		let card = CardData {
			number: "4242424242424242".to_string(),
			exp_month: 12,
			exp_year: valid_test_exp_year(),
			cvc: "123".to_string(),
		};

		let result = vault.tokenize_card(card).await;
		assert!(result.is_ok());
		let token = result.unwrap();
		assert!(!token.id.is_empty());
		assert_eq!(vault.token_count().await, 1);
	}

	#[tokio::test]
	async fn test_tokenize_invalid_card_number_fails() {
		let vault = MockBasisTheoryVault::new("bt_test_mock_key");
		let card = CardData {
			number: "123".to_string(), // Too short
			exp_month: 12,
			exp_year: valid_test_exp_year(),
			cvc: "123".to_string(),
		};

		let result = vault.tokenize_card(card).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_tokenize_expired_card_fails() {
		let vault = MockBasisTheoryVault::new("bt_test_mock_key");
		let card = CardData {
			number: "4242424242424242".to_string(),
			exp_month: 12,
			exp_year: 2020, // Expired
			cvc: "123".to_string(),
		};

		let result = vault.tokenize_card(card).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_process_payment_with_valid_token_succeeds() {
		let vault = MockBasisTheoryVault::new("bt_test_mock_key");
		let card = CardData {
			number: "4242424242424242".to_string(),
			exp_month: 12,
			exp_year: valid_test_exp_year(),
			cvc: "123".to_string(),
		};

		let token = vault.tokenize_card(card).await.unwrap();

		let result = vault
			.process_payment_with_token(&token.id, 1000, "usd")
			.await;
		assert!(result.is_ok());
		let payment = result.unwrap();
		assert_eq!(payment.status, "succeeded");
	}

	#[tokio::test]
	async fn test_process_payment_with_invalid_token_fails() {
		let vault = MockBasisTheoryVault::new("bt_test_mock_key");

		let result = vault
			.process_payment_with_token("invalid_token", 1000, "usd")
			.await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_get_token_succeeds() {
		let vault = MockBasisTheoryVault::new("bt_test_mock_key");
		let card = CardData {
			number: "4242424242424242".to_string(),
			exp_month: 12,
			exp_year: valid_test_exp_year(),
			cvc: "123".to_string(),
		};

		let token = vault.tokenize_card(card).await.unwrap();

		let result = vault.get_token(&token.id).await;
		assert!(result.is_ok());
		let info = result.unwrap();
		assert_eq!(info.id, token.id);
		assert_eq!(info.type_, "card");
	}

	#[tokio::test]
	async fn test_delete_token_succeeds() {
		let vault = MockBasisTheoryVault::new("bt_test_mock_key");
		let card = CardData {
			number: "4242424242424242".to_string(),
			exp_month: 12,
			exp_year: valid_test_exp_year(),
			cvc: "123".to_string(),
		};

		let token = vault.tokenize_card(card).await.unwrap();

		let result = vault.delete_token(&token.id).await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_delete_already_deleted_token_fails() {
		let vault = MockBasisTheoryVault::new("bt_test_mock_key");
		let card = CardData {
			number: "4242424242424242".to_string(),
			exp_month: 12,
			exp_year: valid_test_exp_year(),
			cvc: "123".to_string(),
		};

		let token = vault.tokenize_card(card).await.unwrap();
		let _ = vault.delete_token(&token.id).await;

		let result = vault.delete_token(&token.id).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_deleted_token_cannot_be_used() {
		let vault = MockBasisTheoryVault::new("bt_test_mock_key");
		let card = CardData {
			number: "4242424242424242".to_string(),
			exp_month: 12,
			exp_year: valid_test_exp_year(),
			cvc: "123".to_string(),
		};

		let token = vault.tokenize_card(card).await.unwrap();
		let _ = vault.delete_token(&token.id).await;

		let result = vault
			.process_payment_with_token(&token.id, 1000, "usd")
			.await;
		assert!(result.is_err());
	}
}
