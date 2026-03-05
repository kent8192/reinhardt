//! Mock implementations for reinhardt-payment testing.
//!
//! This crate provides mock implementations of `PaymentProvider` and `TokenVault` traits
//! for testing payment integration without requiring actual API calls to Stripe or BasisTheory.
//!
//! # Features
//!
//! - **MockStripeProvider**: In-memory payment provider for testing
//! - **MockBasisTheoryVault**: In-memory token vault for testing
//! - **Configurable behavior**: Control success/failure modes
//! - **No network required**: All operations use in-memory storage
//!
//! # Example
//!
//! ```rust,no_run
//! use reinhardt_payment::{PaymentProvider, PaymentIntentParams};
//! use reinhardt_payment_mocks::MockStripeProvider;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let provider = MockStripeProvider::new("sk_test_mock_key");
//!
//! let params = PaymentIntentParams {
//!     amount: 1000,
//!     currency: "usd".to_string(),
//!     confirm: false,
//!     ..Default::default()
//! };
//!
//! let intent = provider.create_payment_intent(params).await?;
//! assert_eq!(intent.amount, 1000);
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod basistheory_vault;
pub mod stripe_provider;

// Re-export mock implementations
pub use basistheory_vault::MockBasisTheoryVault;
pub use stripe_provider::MockStripeProvider;
