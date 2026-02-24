//! Email validation and sanitization utilities

use crate::{EmailError, EmailResult};
use idna::domain_to_ascii;

/// Maximum email address length per RFC 5321 Section 4.5.3.1.3.
///
/// The total path length including angle brackets must not exceed 256 octets,
/// so the address itself is limited to 254 characters.
pub const MAX_EMAIL_LENGTH: usize = 254;

/// Validates an email address according to RFC 5321/5322 standards
///
/// # Examples
///
/// ```
/// use reinhardt_mail::validation::validate_email;
///
/// assert!(validate_email("user@example.com").is_ok());
/// assert!(validate_email("user+tag@example.com").is_ok());
/// assert!(validate_email("invalid@").is_err());
/// assert!(validate_email("@example.com").is_err());
/// ```
pub fn validate_email(email: &str) -> EmailResult<()> {
	if email.is_empty() {
		return Err(EmailError::InvalidAddress(
			"Email cannot be empty".to_string(),
		));
	}

	// RFC 5321 Section 4.5.3.1.3: maximum total length is 254 characters
	// (256 octets including angle brackets in SMTP envelope)
	if email.len() > MAX_EMAIL_LENGTH {
		return Err(EmailError::InvalidAddress(format!(
			"Email address exceeds maximum length of {} characters (got {})",
			MAX_EMAIL_LENGTH,
			email.len()
		)));
	}

	// Check for header injection attempts
	if email.contains('\n') || email.contains('\r') {
		return Err(EmailError::HeaderInjection(
			"Email address contains newline characters".to_string(),
		));
	}

	// Count @ symbols
	let at_count = email.chars().filter(|&c| c == '@').count();
	if at_count != 1 {
		return Err(EmailError::InvalidAddress(format!(
			"Email must contain exactly one @ symbol, found {}",
			at_count
		)));
	}

	// Split into local and domain parts
	let parts: Vec<&str> = email.split('@').collect();
	let local = parts[0];
	let domain = parts[1];

	// Validate local part
	validate_local_part(local)?;

	// Validate domain part
	validate_domain(domain)?;

	Ok(())
}

/// Validates the local part of an email address (before @)
fn validate_local_part(local: &str) -> EmailResult<()> {
	if local.is_empty() {
		return Err(EmailError::InvalidAddress(
			"Local part cannot be empty".to_string(),
		));
	}

	if local.len() > 64 {
		return Err(EmailError::InvalidAddress(
			"Local part is too long (max 64 characters)".to_string(),
		));
	}

	// Check for invalid characters
	if local.starts_with('.') || local.ends_with('.') {
		return Err(EmailError::InvalidAddress(
			"Local part cannot start or end with a dot".to_string(),
		));
	}

	if local.contains("..") {
		return Err(EmailError::InvalidAddress(
			"Local part cannot contain consecutive dots".to_string(),
		));
	}

	Ok(())
}

/// Validates the domain part of an email address (after @)
fn validate_domain(domain: &str) -> EmailResult<()> {
	if domain.is_empty() {
		return Err(EmailError::InvalidAddress(
			"Domain cannot be empty".to_string(),
		));
	}

	if domain.len() > 253 {
		return Err(EmailError::InvalidAddress(
			"Domain is too long (max 253 characters)".to_string(),
		));
	}

	// Check for invalid characters
	if domain.starts_with('.') || domain.ends_with('.') {
		return Err(EmailError::InvalidAddress(
			"Domain cannot start or end with a dot".to_string(),
		));
	}

	if domain.starts_with('-') || domain.ends_with('-') {
		return Err(EmailError::InvalidAddress(
			"Domain cannot start or end with a hyphen".to_string(),
		));
	}

	// Try to convert to ASCII using IDNA (for international domains)
	if domain_to_ascii(domain).is_err() {
		return Err(EmailError::InvalidAddress(
			"Invalid domain name".to_string(),
		));
	}

	Ok(())
}

/// Validates a list of email addresses
///
/// # Examples
///
/// ```
/// use reinhardt_mail::validation::validate_email_list;
///
/// let emails = vec!["user1@example.com", "user2@example.com"];
/// assert!(validate_email_list(&emails).is_ok());
///
/// let invalid_emails = vec!["valid@example.com", "invalid@"];
/// assert!(validate_email_list(&invalid_emails).is_err());
/// ```
pub fn validate_email_list(emails: &[impl AsRef<str>]) -> EmailResult<()> {
	for email in emails {
		validate_email(email.as_ref())?;
	}
	Ok(())
}

/// Sanitizes an email address by trimming whitespace and lowercasing the domain.
///
/// Per RFC 5321 Section 2.4, the local part of an email address is case-sensitive,
/// while the domain part is case-insensitive. This function only lowercases the
/// domain part, preserving the original case of the local part.
///
/// # Examples
///
/// ```
/// use reinhardt_mail::validation::sanitize_email;
///
/// assert_eq!(sanitize_email("  User@Example.COM  ").unwrap(), "User@example.com");
/// assert_eq!(sanitize_email("user+tag@EXAMPLE.com").unwrap(), "user+tag@example.com");
/// ```
pub fn sanitize_email(email: &str) -> EmailResult<String> {
	let trimmed = email.trim();
	validate_email(trimmed)?;

	// RFC 5321: local part is case-sensitive, domain is case-insensitive
	let (local, domain) = trimmed.rsplit_once('@').ok_or_else(|| {
		EmailError::InvalidAddress("Email must contain exactly one @ symbol, found 0".to_string())
	})?;
	Ok(format!("{}@{}", local, domain.to_lowercase()))
}

/// Sanitizes a list of email addresses
///
/// Preserves case of local parts and lowercases domain parts per RFC 5321.
///
/// # Examples
///
/// ```
/// use reinhardt_mail::validation::sanitize_email_list;
///
/// let emails = vec!["  User1@Example.COM  ", "User2@EXAMPLE.com"];
/// let sanitized = sanitize_email_list(&emails).unwrap();
/// assert_eq!(sanitized, vec!["User1@example.com", "User2@example.com"]);
/// ```
pub fn sanitize_email_list(emails: &[impl AsRef<str>]) -> EmailResult<Vec<String>> {
	emails.iter().map(|e| sanitize_email(e.as_ref())).collect()
}

/// Validates a custom header name against RFC 2822 field name rules.
///
/// RFC 2822 Section 2.2 defines a field name as one or more printable US-ASCII
/// characters (values 33-126) except the colon (`:`, value 58).
///
/// # Examples
///
/// ```
/// use reinhardt_mail::validation::validate_header_name;
///
/// assert!(validate_header_name("X-Custom-Header").is_ok());
/// assert!(validate_header_name("").is_err());
/// assert!(validate_header_name("Invalid Header").is_err());   // space
/// assert!(validate_header_name("Invalid:Header").is_err());   // colon
/// ```
pub fn validate_header_name(name: &str) -> EmailResult<()> {
	if name.is_empty() {
		return Err(EmailError::InvalidHeader(
			"Header name cannot be empty".to_string(),
		));
	}
	for ch in name.chars() {
		// RFC 2822: printable ASCII (33-126) except colon
		if !ch.is_ascii() || ch.is_ascii_control() || ch == ':' || ch == ' ' {
			return Err(EmailError::InvalidHeader(format!(
				"Invalid character '{}' (U+{:04X}) in header name",
				ch.escape_debug(),
				ch as u32
			)));
		}
	}
	Ok(())
}

/// Checks if a string contains potential header injection attempts
///
/// # Examples
///
/// ```
/// use reinhardt_mail::validation::check_header_injection;
///
/// assert!(check_header_injection("Normal subject").is_ok());
/// assert!(check_header_injection("Subject\nBcc: attacker@evil.com").is_err());
/// assert!(check_header_injection("Subject\rCc: attacker@evil.com").is_err());
/// ```
pub fn check_header_injection(value: &str) -> EmailResult<()> {
	if value.contains('\n') || value.contains('\r') {
		return Err(EmailError::HeaderInjection(
			"Value contains newline characters".to_string(),
		));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("user@example.com")]
	#[case("user+tag@example.com")]
	#[case("user.name@example.com")]
	#[case("user_name@example.com")]
	#[case("user123@example.co.uk")]
	fn test_validate_email_valid(#[case] email: &str) {
		// Arrange
		// (input provided by case parameter)

		// Act
		let result = validate_email(email);

		// Assert
		assert!(result.is_ok(), "Expected valid email: {}", email);
	}

	#[rstest]
	#[case("")]
	#[case("invalid")]
	#[case("@example.com")]
	#[case("user@")]
	#[case("user@@example.com")]
	#[case("user@example@com")]
	fn test_validate_email_invalid(#[case] email: &str) {
		// Arrange
		// (input provided by case parameter)

		// Act
		let result = validate_email(email);

		// Assert
		assert!(result.is_err(), "Expected invalid email: {}", email);
	}

	#[rstest]
	#[case("user@example.com\nBcc: attacker@evil.com")]
	#[case("user@example.com\rCc: attacker@evil.com")]
	fn test_validate_email_header_injection(#[case] email: &str) {
		// Arrange
		// (input provided by case parameter)

		// Act
		let result = validate_email(email);

		// Assert
		assert!(
			result.is_err(),
			"Expected header injection rejection for: {:?}",
			email
		);
	}

	#[rstest]
	fn test_validate_email_max_length_boundary() {
		// Arrange
		// 254 characters is the maximum allowed (local@domain)
		let local = "a".repeat(64);
		let domain_label = "b".repeat(63);
		// Build domain to reach exactly 254 total: local(64) + @(1) + domain(189) = 254
		let domain_part_len = MAX_EMAIL_LENGTH - local.len() - 1; // subtract local and @
		let domain = format!("{}.{}", "b".repeat(domain_part_len - 4), "com");
		let email_at_limit = format!("{}@{}", local, domain);
		assert_eq!(email_at_limit.len(), MAX_EMAIL_LENGTH);

		// Act
		let result = validate_email(&email_at_limit);

		// Assert
		assert!(
			result.is_ok(),
			"Email at exactly {} chars should be valid",
			MAX_EMAIL_LENGTH
		);
	}

	#[rstest]
	fn test_validate_email_exceeds_max_length() {
		// Arrange
		// 255 characters exceeds the maximum
		let local = "a".repeat(64);
		let domain_part_len = MAX_EMAIL_LENGTH - local.len(); // one more than allowed
		let domain = format!("{}.{}", "b".repeat(domain_part_len - 4), "com");
		let email_over_limit = format!("{}@{}", local, domain);
		assert!(email_over_limit.len() > MAX_EMAIL_LENGTH);

		// Act
		let result = validate_email(&email_over_limit);

		// Assert
		assert!(
			result.is_err(),
			"Email over {} chars should be rejected",
			MAX_EMAIL_LENGTH
		);
		let err_msg = result.unwrap_err().to_string();
		assert!(
			err_msg.contains("maximum length"),
			"Error should mention maximum length, got: {}",
			err_msg
		);
	}

	#[rstest]
	#[case("user", true)]
	#[case("user.name", true)]
	#[case("user+tag", true)]
	#[case("", false)]
	#[case(".user", false)]
	#[case("user.", false)]
	#[case("user..name", false)]
	fn test_validate_local_part(#[case] local: &str, #[case] expected_valid: bool) {
		// Arrange
		// (input provided by case parameters)

		// Act
		let result = validate_local_part(local);

		// Assert
		assert_eq!(
			result.is_ok(),
			expected_valid,
			"Local part '{}': expected valid={}, got {:?}",
			local,
			expected_valid,
			result
		);
	}

	#[rstest]
	#[case("example.com", true)]
	#[case("mail.example.com", true)]
	#[case("example.co.uk", true)]
	#[case("", false)]
	#[case(".example.com", false)]
	#[case("example.com.", false)]
	#[case("-example.com", false)]
	#[case("example.com-", false)]
	fn test_validate_domain(#[case] domain: &str, #[case] expected_valid: bool) {
		// Arrange
		// (input provided by case parameters)

		// Act
		let result = validate_domain(domain);

		// Assert
		assert_eq!(
			result.is_ok(),
			expected_valid,
			"Domain '{}': expected valid={}, got {:?}",
			domain,
			expected_valid,
			result
		);
	}

	#[rstest]
	#[case("  User@Example.COM  ", "User@example.com")]
	#[case("USER+TAG@EXAMPLE.COM", "USER+TAG@example.com")]
	#[case("john.smith@example.com", "john.smith@example.com")]
	fn test_sanitize_email(#[case] input: &str, #[case] expected: &str) {
		// Arrange
		// (inputs provided by case parameters)

		// Act
		let result = sanitize_email(input).unwrap();

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	fn test_sanitize_email_list() {
		// Arrange
		let emails = vec!["  User1@Example.COM  ", "User2@EXAMPLE.com"];

		// Act
		let sanitized = sanitize_email_list(&emails).unwrap();

		// Assert
		assert_eq!(sanitized, vec!["User1@example.com", "User2@example.com"]);
	}

	#[rstest]
	#[case("Normal subject", true)]
	#[case("Subject with spaces", true)]
	#[case("Subject\nBcc: attacker@evil.com", false)]
	#[case("Subject\rCc: attacker@evil.com", false)]
	#[case("Subject\r\nTo: attacker@evil.com", false)]
	fn test_check_header_injection(#[case] value: &str, #[case] expected_ok: bool) {
		// Arrange
		// (input provided by case parameters)

		// Act
		let result = check_header_injection(value);

		// Assert
		assert_eq!(
			result.is_ok(),
			expected_ok,
			"Header injection check for {:?}: expected ok={}, got {:?}",
			value,
			expected_ok,
			result
		);
	}

	#[rstest]
	fn test_validate_email_list_valid() {
		// Arrange
		let valid_emails = vec!["user1@example.com", "user2@example.com"];

		// Act
		let result = validate_email_list(&valid_emails);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validate_email_list_invalid() {
		// Arrange
		let invalid_emails = vec!["valid@example.com", "invalid@"];

		// Act
		let result = validate_email_list(&invalid_emails);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_max_email_length_constant() {
		// Assert
		assert_eq!(
			MAX_EMAIL_LENGTH, 254,
			"RFC 5321 max email length should be 254"
		);
	}

	#[rstest]
	#[case("X-Custom-Header", true)]
	#[case("Content-Type", true)]
	#[case("X-Mailer", true)]
	#[case("Accept", true)]
	fn test_validate_header_name_valid(#[case] name: &str, #[case] expected_valid: bool) {
		// Arrange
		// (input provided by case parameters)

		// Act
		let result = validate_header_name(name);

		// Assert
		assert_eq!(
			result.is_ok(),
			expected_valid,
			"Header name '{}': expected valid={}, got {:?}",
			name,
			expected_valid,
			result
		);
	}

	#[rstest]
	#[case("", "empty")]
	#[case("Invalid Header", "space")]
	#[case("Invalid:Header", "colon")]
	#[case("Invalid\tHeader", "control character")]
	#[case("Invalid\x00Header", "null byte")]
	#[case("H\u{00e9}ader", "non-ASCII character")]
	fn test_validate_header_name_invalid(#[case] name: &str, #[case] reason: &str) {
		// Arrange
		// (input provided by case parameters)

		// Act
		let result = validate_header_name(name);

		// Assert
		assert!(
			result.is_err(),
			"Header name {:?} should be rejected ({})",
			name,
			reason
		);
	}
}
