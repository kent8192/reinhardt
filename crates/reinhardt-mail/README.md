# reinhardt-mail

Email sending and templating

## Overview

Email framework for sending emails with support for HTML and plain text, attachments, inline images, and template-based emails. Supports multiple email backends including SMTP and development console backend.

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["mail"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

Then import mail features:

```rust
use reinhardt::mail::{EmailMessage, EmailBackend, send_mail};
use reinhardt::mail::backends::{SmtpBackend, ConsoleBackend};
```

**Note:** Mail features are included in the `standard` and `full` feature presets.

## Features

### Implemented ✓

#### Core Message Building

- **EmailMessage**: Flexible email message builder with builder pattern
  - Subject, body, from address, and recipient list configuration
  - HTML content support with plain text fallback
  - Mutable message modification methods
- **EmailMessageBuilder**: Type-safe builder for constructing email messages

#### Email Backends

- **ConsoleBackend**: Development backend that prints emails to console
- **FileBackend**: Backend for saving emails to files
- **MemoryBackend**: In-memory storage backend for testing
  - Thread-safe message storage using Arc<Mutex<>>
  - Message counting and retrieval capabilities
- **SmtpBackend**: SMTP protocol backend for sending emails
- **EmailBackend trait**: Extensible trait for implementing custom backends

#### Utility Functions

- **send_mail**: Simple convenience function for sending basic emails
- **send_mail_with_backend**: Send emails using a specific backend
- **send_mass_mail**: Bulk email sending for efficient multi-message delivery
- **mail_admins**: Send emails to administrators with subject prefix support
- **mail_managers**: Send emails to managers with subject prefix support

#### Settings Integration

- **backend_from_settings**: Backend initialization from configuration
- Integration with reinhardt-conf for email configuration
  - Admin and manager email lists
  - Subject prefix configuration
  - Server email and from email settings

#### Error Handling

- **EmailError**: Comprehensive error types for email operations
  - InvalidAddress: Email address validation errors
  - MissingField: Required field validation
  - BackendError: Backend-specific errors
  - SmtpError: SMTP protocol errors
  - IoError: File system and I/O errors
  - TemplateError: Template rendering errors
  - AttachmentError: File attachment errors
  - InvalidHeader: Email header validation errors
  - HeaderInjection: Security validation for header injection attacks