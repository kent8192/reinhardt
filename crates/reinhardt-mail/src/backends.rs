use crate::headers::{
	ListUnsubscribe, ListUnsubscribePost, Precedence, XEntityRefId, XMailer, XPriority,
};
use crate::message::EmailMessage;
use crate::{EmailError, EmailResult};
use lettre::message::header::{HeaderName, HeaderValue};
use lettre::message::{Mailbox, MultiPart, SinglePart, header};
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::time::Duration;
use zeroize::Zeroize;

/// Trait for email backends
#[async_trait::async_trait]
pub trait EmailBackend: Send + Sync {
	async fn send_messages(&self, messages: &[EmailMessage]) -> EmailResult<usize>;
}

/// Creates an email backend from settings configuration.
///
/// # Arguments
/// * `settings` - Email configuration settings
///
/// # Returns
/// A boxed EmailBackend trait object based on settings.backend field
///
/// # Errors
/// Returns EmailError if:
/// - Unknown backend type
/// - Missing required fields (e.g., file_path for FileBackend)
pub fn backend_from_settings(
	settings: &reinhardt_conf::settings::EmailSettings,
) -> crate::EmailResult<Box<dyn EmailBackend>> {
	// Validate from_email if configured
	if !settings.from_email.is_empty() {
		crate::validation::validate_email(&settings.from_email)?;
	}

	match settings.backend.to_lowercase().as_str() {
		"smtp" => {
			let security = match (settings.use_tls, settings.use_ssl) {
				(true, _) => SmtpSecurity::StartTls,
				(_, true) => SmtpSecurity::Tls,
				_ => SmtpSecurity::None,
			};

			let timeout = settings
				.timeout
				.map(std::time::Duration::from_secs)
				.unwrap_or(std::time::Duration::from_secs(60));

			let mut config = SmtpConfig::new(&settings.host, settings.port)
				.with_security(security)
				.with_timeout(timeout);

			if let (Some(username), Some(password)) = (&settings.username, &settings.password) {
				config = config.with_credentials(username.clone(), password.clone());
			}

			let backend = SmtpBackend::new(config)?;
			Ok(Box::new(backend))
		}
		"console" => Ok(Box::new(ConsoleBackend)),
		"file" => {
			let directory = settings
				.file_path
				.clone()
				.ok_or_else(|| crate::EmailError::MissingField("file_path".to_string()))?;
			Ok(Box::new(FileBackend::new(directory)))
		}
		"memory" => Ok(Box::new(MemoryBackend::new())),
		unknown => Err(crate::EmailError::BackendError(format!(
			"Unknown email backend type: '{}'. Valid options: smtp, console, file, memory",
			unknown
		))),
	}
}

/// Console backend for development
///
/// Prints email messages to the console instead of sending them.
pub struct ConsoleBackend;

#[async_trait::async_trait]
impl EmailBackend for ConsoleBackend {
	async fn send_messages(&self, messages: &[EmailMessage]) -> EmailResult<usize> {
		for (i, msg) in messages.iter().enumerate() {
			println!("========== Email {} ==========", i + 1);
			println!("From: {}", msg.from_email());
			println!("To: {}", msg.to().join(", "));
			if !msg.cc().is_empty() {
				println!("Cc: {}", msg.cc().join(", "));
			}
			if !msg.bcc().is_empty() {
				println!("Bcc: {}", msg.bcc().join(", "));
			}
			println!("Subject: {}", msg.subject());
			for (name, value) in msg.headers() {
				println!("{}: {}", name, value);
			}
			println!("\n{}", msg.body());
			if let Some(html) = msg.html_body() {
				println!("\n--- HTML ---\n{}", html);
			}
			for attachment in msg.attachments() {
				println!(
					"\n--- Attachment: {} (Content-Type: {}, {} bytes) ---",
					attachment.filename(),
					attachment.mime_type(),
					attachment.content().len()
				);
			}
			println!("==============================\n");
		}
		Ok(messages.len())
	}
}

/// File backend for saving emails to files
pub struct FileBackend {
	directory: std::path::PathBuf,
}

impl FileBackend {
	pub fn new(directory: impl Into<std::path::PathBuf>) -> Self {
		Self {
			directory: directory.into(),
		}
	}
}

#[async_trait::async_trait]
impl EmailBackend for FileBackend {
	async fn send_messages(&self, messages: &[EmailMessage]) -> EmailResult<usize> {
		std::fs::create_dir_all(&self.directory)?;

		for msg in messages.iter() {
			let filename = format!(
				"email_{}.eml",
				chrono::Utc::now().format("%Y%m%d_%H%M%S_%f")
			);
			let path = self.directory.join(filename);

			let mut content = format!(
				"From: {}\nTo: {}\nSubject: {}",
				msg.from_email(),
				msg.to().join(", "),
				msg.subject()
			);

			// Include custom headers
			for (name, value) in msg.headers() {
				content.push_str(&format!("\n{}: {}", name, value));
			}

			content.push_str(&format!("\n\n{}", msg.body()));

			// Include HTML body if present
			if let Some(html) = msg.html_body() {
				content.push_str("\n\n--- HTML Body ---\n");
				content.push_str(html);
			}

			// Include attachment metadata
			for attachment in msg.attachments() {
				content.push_str(&format!(
					"\n\n--- Attachment: {} ---\nContent-Type: {}\nSize: {} bytes\n",
					attachment.filename(),
					attachment.mime_type(),
					attachment.content().len()
				));
			}

			tokio::fs::write(path, content).await?;
		}

		Ok(messages.len())
	}
}

/// Memory backend for testing
pub struct MemoryBackend {
	messages: std::sync::Arc<tokio::sync::Mutex<Vec<EmailMessage>>>,
}

impl MemoryBackend {
	pub fn new() -> Self {
		Self {
			messages: std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new())),
		}
	}

	pub async fn count(&self) -> usize {
		self.messages.lock().await.len()
	}

	pub async fn get_messages(&self) -> Vec<EmailMessage> {
		self.messages.lock().await.clone()
	}

	pub async fn clear(&self) {
		self.messages.lock().await.clear();
	}
}

impl Default for MemoryBackend {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl EmailBackend for MemoryBackend {
	async fn send_messages(&self, messages: &[EmailMessage]) -> EmailResult<usize> {
		let mut stored = self.messages.lock().await;
		stored.extend_from_slice(messages);
		Ok(messages.len())
	}
}

/// SMTP connection security mode
#[derive(Debug, Clone)]
pub enum SmtpSecurity {
	/// No encryption
	None,
	/// STARTTLS (upgrade to TLS)
	StartTls,
	/// Direct TLS/SSL connection
	Tls,
}

/// SMTP authentication mechanism
#[derive(Debug, Clone)]
pub enum SmtpAuthMechanism {
	/// PLAIN authentication
	Plain,
	/// LOGIN authentication
	Login,
	/// Any supported mechanism
	Auto,
}

/// Configuration for SMTP backend
#[derive(Debug, Clone)]
pub struct SmtpConfig {
	pub host: String,
	pub port: u16,
	pub username: Option<String>,
	pub password: Option<String>,
	pub security: SmtpSecurity,
	pub auth_mechanism: SmtpAuthMechanism,
	pub timeout: Duration,
}

impl Default for SmtpConfig {
	fn default() -> Self {
		Self {
			host: "localhost".to_string(),
			port: 25,
			username: None,
			password: None,
			security: SmtpSecurity::None,
			auth_mechanism: SmtpAuthMechanism::Auto,
			timeout: Duration::from_secs(30),
		}
	}
}

impl SmtpConfig {
	pub fn new(host: impl Into<String>, port: u16) -> Self {
		Self {
			host: host.into(),
			port,
			username: None,
			password: None,
			security: SmtpSecurity::None,
			auth_mechanism: SmtpAuthMechanism::Auto,
			timeout: Duration::from_secs(30),
		}
	}

	pub fn with_credentials(mut self, username: String, password: String) -> Self {
		self.username = Some(username);
		self.password = Some(password);
		self
	}

	pub fn with_security(mut self, security: SmtpSecurity) -> Self {
		self.security = security;
		self
	}

	pub fn with_auth_mechanism(mut self, mechanism: SmtpAuthMechanism) -> Self {
		self.auth_mechanism = mechanism;
		self
	}

	pub fn with_timeout(mut self, timeout: Duration) -> Self {
		self.timeout = timeout;
		self
	}

	/// Validate the SMTP configuration
	///
	/// Checks that email-formatted usernames (containing `@`) are valid email addresses.
	pub fn validate(&self) -> EmailResult<()> {
		// Validate username if it looks like an email address
		if let Some(username) = &self.username
			&& username.contains('@')
		{
			crate::validation::validate_email(username)?;
		}
		Ok(())
	}
}

/// Zeroize SMTP credentials on drop to prevent sensitive data from lingering in memory.
///
/// This ensures that username and password fields are securely erased when
/// the `SmtpConfig` is no longer needed, reducing the risk of credential
/// exposure through memory inspection or core dumps.
impl Drop for SmtpConfig {
	fn drop(&mut self) {
		if let Some(ref mut username) = self.username {
			username.zeroize();
		}
		if let Some(ref mut password) = self.password {
			password.zeroize();
		}
	}
}

/// SMTP backend for sending emails
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_mail::{SmtpBackend, SmtpConfig, SmtpSecurity};
/// # use std::time::Duration;
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = SmtpConfig::new("smtp.gmail.com", 587)
///     .with_credentials("user@gmail.com".to_string(), "password".to_string())
///     .with_security(SmtpSecurity::StartTls)
///     .with_timeout(Duration::from_secs(30));
///
/// let backend = SmtpBackend::new(config)?;
/// # Ok(())
/// # }
/// ```
pub struct SmtpBackend {
	config: SmtpConfig,
}

impl SmtpBackend {
	pub fn new(config: SmtpConfig) -> EmailResult<Self> {
		config.validate()?;
		Ok(Self { config })
	}

	fn build_transport(&self) -> EmailResult<AsyncSmtpTransport<Tokio1Executor>> {
		// Use lettre's recommended secure APIs for standard ports
		// This ensures proper TLS hostname verification by default
		match (&self.config.security, self.config.port) {
			// Port 465 with TLS: use relay() for secure SMTPS
			(SmtpSecurity::Tls, 465) => {
				let builder = AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.host)
					.map_err(|e| EmailError::SmtpError(format!("TLS relay error: {}", e)))?
					.timeout(Some(self.config.timeout));
				let builder = self.configure_auth(builder);
				Ok(builder.build())
			}
			// Port 587 with STARTTLS: use starttls_relay() for secure STARTTLS
			(SmtpSecurity::StartTls, 587) => {
				let builder =
					AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)
						.map_err(|e| EmailError::SmtpError(format!("STARTTLS relay error: {}", e)))?
						.timeout(Some(self.config.timeout));
				let builder = self.configure_auth(builder);
				Ok(builder.build())
			}
			// Custom port or no TLS: use builder_dangerous with manual TLS configuration
			// This is needed for test environments and non-standard SMTP configurations
			_ => self.build_transport_with_custom_port(),
		}
	}

	/// Configure authentication on the transport builder
	fn configure_auth(
		&self,
		mut builder: lettre::transport::smtp::AsyncSmtpTransportBuilder,
	) -> lettre::transport::smtp::AsyncSmtpTransportBuilder {
		if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
			let credentials = Credentials::new(username.clone(), password.clone());

			builder = match &self.config.auth_mechanism {
				SmtpAuthMechanism::Plain => builder
					.credentials(credentials)
					.authentication(vec![Mechanism::Plain]),
				SmtpAuthMechanism::Login => builder
					.credentials(credentials)
					.authentication(vec![Mechanism::Login]),
				SmtpAuthMechanism::Auto => builder.credentials(credentials),
			};
		}
		builder
	}

	/// Build transport with custom port using builder_dangerous
	///
	/// This method is used for non-standard ports or when TLS is disabled.
	/// For standard ports (465/587), prefer `relay()` or `starttls_relay()` instead.
	fn build_transport_with_custom_port(&self) -> EmailResult<AsyncSmtpTransport<Tokio1Executor>> {
		let mut builder =
			AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.host)
				.port(self.config.port)
				.timeout(Some(self.config.timeout));

		// Configure TLS/SSL
		match &self.config.security {
			SmtpSecurity::None => {
				// No encryption - intended for test environments only
			}
			SmtpSecurity::StartTls => {
				let tls_params = TlsParameters::builder(self.config.host.clone())
					.build()
					.map_err(|e| EmailError::SmtpError(format!("TLS error: {}", e)))?;
				builder = builder.tls(Tls::Required(tls_params));
			}
			SmtpSecurity::Tls => {
				let tls_params = TlsParameters::builder(self.config.host.clone())
					.build()
					.map_err(|e| EmailError::SmtpError(format!("TLS error: {}", e)))?;
				builder = builder.tls(Tls::Wrapper(tls_params));
			}
		}

		builder = self.configure_auth(builder);

		Ok(builder.build())
	}

	fn build_message(&self, email: &EmailMessage) -> EmailResult<Message> {
		// Parse from address
		let from = email
			.from_email()
			.parse::<Mailbox>()
			.map_err(|e| EmailError::InvalidAddress(format!("Invalid from address: {}", e)))?;

		// Start building the message
		let mut builder = Message::builder().from(from).subject(email.subject());

		// Add recipients
		for to in email.to() {
			let mailbox = to
				.parse::<Mailbox>()
				.map_err(|e| EmailError::InvalidAddress(format!("Invalid to address: {}", e)))?;
			builder = builder.to(mailbox);
		}

		// Add CC recipients
		for cc in email.cc() {
			let mailbox = cc
				.parse::<Mailbox>()
				.map_err(|e| EmailError::InvalidAddress(format!("Invalid cc address: {}", e)))?;
			builder = builder.cc(mailbox);
		}

		// Add BCC recipients
		for bcc in email.bcc() {
			let mailbox = bcc
				.parse::<Mailbox>()
				.map_err(|e| EmailError::InvalidAddress(format!("Invalid bcc address: {}", e)))?;
			builder = builder.bcc(mailbox);
		}

		// Add Reply-To
		for reply_to in email.reply_to() {
			let mailbox = reply_to.parse::<Mailbox>().map_err(|e| {
				EmailError::InvalidAddress(format!("Invalid reply-to address: {}", e))
			})?;
			builder = builder.reply_to(mailbox);
		}

		// Add custom headers
		// Known headers are added via typed lettre Header implementations.
		// Unknown/arbitrary headers are injected via raw header insertion after message build.
		let mut deferred_headers: Vec<(String, String)> = Vec::new();
		for (name, value) in email.headers() {
			let name_lower = name.to_lowercase();
			match name_lower.as_str() {
				"x-mailer" => {
					builder = builder.header(XMailer::new(value));
				}
				"x-priority" => {
					builder = builder.header(XPriority::new(value));
				}
				"list-unsubscribe" => {
					builder = builder.header(ListUnsubscribe::new(value));
				}
				"list-unsubscribe-post" => {
					builder = builder.header(ListUnsubscribePost::new(value));
				}
				"x-entity-ref-id" => {
					builder = builder.header(XEntityRefId::new(value));
				}
				"precedence" => {
					builder = builder.header(Precedence::new(value));
				}
				_ => {
					// Defer arbitrary headers for raw insertion after build
					deferred_headers.push((name.clone(), value.clone()));
				}
			}
		}

		// Build the body - convert body to String once to avoid repeated allocation
		let has_html = email.html_body().is_some();
		let has_attachments = !email.attachments().is_empty();
		let body = email.body().to_string();

		let message = if has_html && has_attachments {
			// HTML with plain text alternative AND attachments
			// Structure: mixed( alternative(text, html), attachment1, attachment2, ... )
			let alternative = MultiPart::alternative()
				.singlepart(SinglePart::plain(body))
				.singlepart(SinglePart::html(email.html_body().unwrap().to_string()));

			let mut mixed = MultiPart::mixed().multipart(alternative);

			for attachment in email.attachments() {
				let content_type = header::ContentType::parse(attachment.mime_type())
					.unwrap_or(header::ContentType::parse("application/octet-stream").unwrap());

				let part = if let Some(cid) = attachment.content_id() {
					SinglePart::builder()
						.header(content_type)
						.header(header::ContentDisposition::inline())
						.header(header::ContentId::from(cid.to_string()))
						.body(attachment.content().to_vec())
				} else {
					SinglePart::builder()
						.header(content_type)
						.header(header::ContentDisposition::attachment(
							attachment.filename(),
						))
						.body(attachment.content().to_vec())
				};

				mixed = mixed.singlepart(part);
			}

			builder
				.multipart(mixed)
				.map_err(|e| EmailError::BackendError(format!("Failed to build message: {}", e)))?
		} else if has_html {
			// HTML with plain text alternative (no attachments)
			let multipart = MultiPart::alternative()
				.singlepart(SinglePart::plain(body))
				.singlepart(SinglePart::html(email.html_body().unwrap().to_string()));

			builder
				.multipart(multipart)
				.map_err(|e| EmailError::BackendError(format!("Failed to build message: {}", e)))?
		} else if has_attachments {
			// Plain text with attachments
			let mut multipart = MultiPart::mixed().singlepart(SinglePart::plain(body));

			for attachment in email.attachments() {
				let content_type = header::ContentType::parse(attachment.mime_type())
					.unwrap_or(header::ContentType::parse("application/octet-stream").unwrap());

				let part = if let Some(cid) = attachment.content_id() {
					// Inline attachment with content ID and Content-Type
					SinglePart::builder()
						.header(content_type)
						.header(header::ContentDisposition::inline())
						.header(header::ContentId::from(cid.to_string()))
						.body(attachment.content().to_vec())
				} else {
					// Regular attachment with Content-Type
					SinglePart::builder()
						.header(content_type)
						.header(header::ContentDisposition::attachment(
							attachment.filename(),
						))
						.body(attachment.content().to_vec())
				};

				multipart = multipart.singlepart(part);
			}

			builder
				.multipart(multipart)
				.map_err(|e| EmailError::BackendError(format!("Failed to build message: {}", e)))?
		} else {
			// Plain text only
			builder
				.body(body)
				.map_err(|e| EmailError::BackendError(format!("Failed to build message: {}", e)))?
		};

		// Inject deferred arbitrary headers via raw insertion
		let mut message = message;
		for (name, value) in deferred_headers {
			match HeaderName::new_from_ascii(name.clone()) {
				Ok(header_name) => {
					let header_value = HeaderValue::new(header_name, value);
					message.headers_mut().insert_raw(header_value);
				}
				Err(_) => {
					return Err(EmailError::InvalidHeader(format!(
						"Invalid header name: '{}'",
						name
					)));
				}
			}
		}

		Ok(message)
	}
}

#[async_trait::async_trait]
impl EmailBackend for SmtpBackend {
	async fn send_messages(&self, messages: &[EmailMessage]) -> EmailResult<usize> {
		let transport = self.build_transport()?;

		let mut sent_count = 0;
		for email in messages {
			let message = self.build_message(email)?;

			transport
				.send(message)
				.await
				.map_err(|e| EmailError::SmtpError(format!("Failed to send email: {}", e)))?;

			sent_count += 1;
		}

		Ok(sent_count)
	}
}
