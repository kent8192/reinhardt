//! Test data constants.

/// Valid test card numbers.
pub const TEST_CARD_VISA: &str = "4242424242424242";
pub const TEST_CARD_MASTERCARD: &str = "5555555555554444";
pub const TEST_CARD_DECLINED: &str = "4000000000000002";
pub const TEST_CARD_INSUFFICIENT: &str = "4000000000009995";

/// Test amounts (in cents).
pub const TEST_AMOUNT_MIN: u64 = 100; // $1.00
pub const TEST_AMOUNT_STANDARD: u64 = 1000; // $10.00
pub const TEST_AMOUNT_LARGE: u64 = 10000; // $100.00
pub const INVALID_AMOUNT_ZERO: u64 = 0;

/// Test currencies.
pub const TEST_CURRENCY_USD: &str = "usd";
pub const TEST_CURRENCY_EUR: &str = "eur";
pub const TEST_CURRENCY_JPY: &str = "jpy";
pub const INVALID_CURRENCY: &str = "xxx";

/// Webhook test secret.
pub const TEST_WEBHOOK_SECRET: &str = "whsec_test_secret_key_12345";

/// Test customer IDs.
pub const TEST_CUSTOMER_ID: &str = "cus_test_123";

/// Test price IDs.
pub const TEST_PRICE_ID: &str = "price_test_123";

/// Returns a valid future expiration year for test card data.
///
/// Dynamically computed to prevent tests from breaking due to time-based expiration.
pub fn valid_test_exp_year() -> u16 {
	chrono::Datelike::year(&chrono::Utc::now()) as u16 + 5
}
