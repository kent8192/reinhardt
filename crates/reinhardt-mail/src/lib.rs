//! # Reinhardt Email
//!
//! Django-style email sending for Reinhardt with comprehensive features for production use.
//!
//! ## Features
//!
//! ### Core Message Building
//! - **EmailMessage**: Flexible email message builder with fluent API
//! - **Alternative Content**: Support for multiple content representations (HTML, plain text)
//! - **Attachments**: File attachments with automatic MIME type detection
//! - **Inline Images**: Embed images in HTML emails using Content-ID
//! - **CC/BCC/Reply-To**: Full support for email headers
//! - **Custom Headers**: Add custom email headers
//!
//! ### Multiple Backends
//! - **SMTP Backend**: Production-ready SMTP with TLS/SSL support
//!   - STARTTLS and direct TLS/SSL connections
//!   - Multiple authentication mechanisms (PLAIN, LOGIN, Auto)
//!   - Configurable connection timeout
//! - **Console Backend**: Development backend that prints to console
//! - **File Backend**: Save emails to files for testing
//! - **Memory Backend**: In-memory storage for unit tests
//!
//! ### Template System
//! - **Template Integration**: Simple template rendering with context
//! - **Dynamic Content**: Generate emails from templates with variable substitution
//! - **HTML and Text**: Support for both HTML and plain text templates
//!
//! ### Email Validation
//! - **RFC 5321/5322 Compliance**: Validate email addresses
//! - **Header Injection Protection**: Prevent email header injection attacks
//! - **Domain Validation**: IDNA support for international domains
//! - **Sanitization**: Normalize and clean email addresses
//!
//! ### Bulk Operations
//! - **Connection Pooling**: Efficient connection management for bulk sending
//! - **Batch Sending**: Send emails in batches with rate limiting
//! - **Concurrent Sending**: Parallel email delivery with configurable concurrency
//! - **Mass Mail**: Send multiple emails efficiently
//!
//! ### Async Support
//! - **Fully Async**: All operations use async/await
//! - **Tokio Integration**: Built on Tokio runtime
//! - **Non-blocking**: No blocking operations in the async path
//!
//! ## Examples
//!
//! ### Simple Email
//!
//! ```rust,no_run
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use reinhardt_mail::send_mail;
//! use reinhardt_conf::settings::EmailSettings;
//!
//! let mut settings = EmailSettings::default();
//! settings.backend = "console".to_string();
//! settings.from_email = "noreply@example.com".to_string();
//!
//! send_mail(
//!     &settings,
//!     "Welcome!",
//!     "Welcome to our service",
//!     vec!["user@example.com"],
//!     None,
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Email with Attachments
//!
//! ```rust,no_run
//! use reinhardt_mail::{EmailMessage, Attachment};
//!
//! let pdf_data = b"PDF content".to_vec();
//! let attachment = Attachment::new("report.pdf", pdf_data);
//!
//! let email = EmailMessage::builder()
//!     .from("reports@example.com")
//!     .to(vec!["user@example.com".to_string()])
//!     .subject("Monthly Report")
//!     .body("Please find attached your monthly report.")
//!     .attachment(attachment)
//!     .build()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ### HTML Email with Inline Images
//!
//! ```rust,no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use reinhardt_mail::{EmailMessage, Attachment};
//!
//! let logo_data = b"PNG content".to_vec();
//! let logo = Attachment::inline("logo.png", logo_data, "logo-cid");
//!
//! let email = EmailMessage::builder()
//!     .from("marketing@example.com")
//!     .to(vec!["customer@example.com".to_string()])
//!     .subject("Newsletter")
//!     .body("Newsletter content")
//!     .html(r#"<html><body><img src="cid:logo-cid"/><h1>Newsletter</h1></body></html>"#)
//!     .attachment(logo)
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Template-based Emails
//!
//! ```rust,no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use reinhardt_mail::templates::{TemplateEmailBuilder, TemplateContext};
//!
//! let mut context = TemplateContext::new();
//! context.insert("name".to_string(), "Alice".into());
//! context.insert("order_id".to_string(), "12345".into());
//!
//! let email = TemplateEmailBuilder::new()
//!     .from("orders@example.com")
//!     .to(vec!["customer@example.com".to_string()])
//!     .subject_template("Order {{order_id}} Confirmation")
//!     .body_template("Hello {{name}}, your order {{order_id}} is confirmed.")
//!     .html_template("<h1>Hello {{name}}</h1><p>Order {{order_id}} confirmed.</p>")
//!     .context(context)
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### SMTP with TLS
//!
//! ```rust,no_run
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use reinhardt_mail::{SmtpBackend, SmtpConfig, SmtpSecurity, EmailMessage};
//! use std::time::Duration;
//!
//! let config = SmtpConfig::new("smtp.gmail.com", 587)
//!     .with_credentials("user@gmail.com".to_string(), "password".to_string())
//!     .with_security(SmtpSecurity::StartTls)
//!     .with_timeout(Duration::from_secs(30));
//!
//! let backend = SmtpBackend::new(config)?;
//!
//! let email = EmailMessage::builder()
//!     .from("sender@gmail.com")
//!     .to(vec!["recipient@example.com".to_string()])
//!     .subject("Test")
//!     .body("Test message")
//!     .build()?;
//!
//! email.send(&backend).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Bulk Sending with Connection Pool
//!
//! ```rust,no_run
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use reinhardt_mail::pooling::{EmailPool, PoolConfig};
//! use reinhardt_mail::{SmtpConfig, EmailMessage};
//!
//! let smtp_config = SmtpConfig::new("smtp.example.com", 587);
//! let pool_config = PoolConfig::new().with_max_connections(5);
//!
//! let pool = EmailPool::new(smtp_config, pool_config)?;
//!
//! let messages = vec![
//!     EmailMessage::builder()
//!         .from("sender@example.com")
//!         .to(vec!["user1@example.com".to_string()])
//!         .subject("Newsletter")
//!         .body("Content")
//!         .build()?,
//!     // ... more messages
//! ];
//!
//! let sent_count = pool.send_bulk(messages).await?;
//! # Ok(())
//! # }

pub mod backends;
pub mod headers;
pub mod message;
pub mod pooling;
pub mod templates;
pub mod utils;
pub mod validation;

use thiserror::Error;

pub use backends::{
	ConsoleBackend, EmailBackend, FileBackend, MemoryBackend, SmtpAuthMechanism, SmtpBackend,
	SmtpConfig, SmtpSecurity, backend_from_settings,
};
pub use message::{Alternative, Attachment, EmailMessage, EmailMessageBuilder};
pub use utils::{mail_admins, mail_managers, send_mail, send_mail_with_backend, send_mass_mail};
pub use validation::MAX_EMAIL_LENGTH;

#[derive(Debug, Error)]
pub enum EmailError {
	#[error("Invalid email address: {0}")]
	InvalidAddress(String),

	#[error("Missing required field: {0}")]
	MissingField(String),

	#[error("Backend error: {0}")]
	BackendError(String),

	#[error("SMTP error: {0}")]
	SmtpError(String),

	#[error("IO error: {0}")]
	IoError(#[from] std::io::Error),

	#[error("Template error: {0}")]
	TemplateError(String),

	#[error("Attachment error: {0}")]
	AttachmentError(String),

	#[error("Invalid header: {0}")]
	InvalidHeader(String),

	#[error("Header injection attempt detected: {0}")]
	HeaderInjection(String),
}

impl EmailError {
	/// Returns true if this is a transient error that can be safely suppressed
	/// by fail_silently mode.
	///
	/// Configuration errors, authentication failures, and security errors
	/// are never considered transient and will always propagate even when
	/// fail_silently is enabled.
	pub fn is_transient(&self) -> bool {
		matches!(self, EmailError::IoError(_) | EmailError::SmtpError(_))
	}
}

pub type EmailResult<T> = std::result::Result<T, EmailError>;
