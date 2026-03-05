//! Payment abstraction layer with Stripe and BasisTheory integration.
//!
//! `reinhardt-payment` provides a unified interface for payment processing,
//! supporting one-time payments (PaymentIntent), hosted payment pages
//! (Checkout Sessions), and recurring payments (Subscriptions).
//!
//! # Features
//!
//! - **PaymentProvider Trait**: Abstract interface for multiple payment processors
//! - **Stripe Integration**: Full support for Stripe PaymentIntent, Checkout, and Subscriptions
//! - **BasisTheory TokenVault**: PCI-compliant card tokenization
//! - **Webhook Processing**: Signature verification and event handling
//! - **Idempotency**: Safe payment retry with UUID-based keys
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_payment::{PaymentProvider, PaymentIntentParams};
//! use reinhardt_payment_mocks::MockStripeProvider;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize mock provider (for testing)
//! let provider = MockStripeProvider::new("sk_test_...".to_string());
//!
//! // Create payment intent
//! let params = PaymentIntentParams {
//!     amount: 1000, // $10.00
//!     currency: "usd".to_string(),
//!     confirm: false,
//!     ..Default::default()
//! };
//!
//! let intent = provider.create_payment_intent(params).await?;
//! println!("Payment intent created: {}", intent.id);
//! # Ok(())
//! # }
//! ```
//!
//! # Security
//!
//! - Webhook signature verification prevents replay attacks
//! - BasisTheory tokenization keeps card data out of your application
//! - Constant-time comparison for cryptographic operations
//!
//! # Architecture
//!
//! ```mermaid
//! graph TB
//!     App[Your Application]
//!     PP[PaymentProvider Trait]
//!     TV[TokenVault Trait]
//!     SP[StripeProvider]
//!     BTV[BasisTheoryVault]
//!     Stripe[Stripe API]
//!     BT[BasisTheory API]
//!
//!     App --> PP
//!     App --> TV
//!     PP --> SP
//!     TV --> BTV
//!     SP --> Stripe
//!     BTV --> BT
//!     BTV -.Proxy.-> Stripe
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod idempotency;
pub mod provider;
pub mod retry;
pub mod types;
pub mod vault;
pub mod webhook;

// Re-export core types and traits
pub use provider::PaymentProvider;
pub use types::{
	checkout::{CheckoutMode, CheckoutParams, CheckoutSession},
	error::{PaymentError, VaultError},
	payment_intent::{CaptureMethod, PaymentIntent, PaymentIntentParams, PaymentIntentStatus},
	subscription::{
		Subscription, SubscriptionParams, SubscriptionStatus, SubscriptionUpdateParams,
	},
};
pub use vault::TokenVault;
pub use webhook::WebhookEvent;
