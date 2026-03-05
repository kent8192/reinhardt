# reinhardt-auth

Authentication and authorization system for Reinhardt framework.

## Overview

Comprehensive authentication and authorization system inspired by Django and
Django REST Framework. Provides JWT tokens, permission classes, user models, and
password hashing with Argon2.

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["auth"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

Then import authentication features:

```rust
use reinhardt::auth::{User, SimpleUser, AnonymousUser};
use reinhardt::auth::{JwtAuth, HttpBasicAuth, AuthenticationBackend};
use reinhardt::auth::{AllowAny, IsAuthenticated, IsAuthenticatedOrReadOnly};
```

**Note:** Authentication features are included in the `standard` and `full` feature presets.

## Implemented ✓

### Core Authentication

#### JWT (JSON Web Token) Authentication

- **Claims Management**: `Claims` struct with user identification, expiration,
  and issue times
- **Token Generation**: Automatic 24-hour expiration by default
- **Token Verification**: Built-in expiration checking and signature validation
- **Encode/Decode**: Full JWT token encoding and decoding support

```rust
use reinhardt::auth::jwt::{JwtAuth, Claims};
use chrono::Duration;

let jwt_auth = JwtAuth::new(b"my-secret-key");
let token = jwt_auth.generate_token("user123".to_string(), "john_doe".to_string()).unwrap();
let claims = jwt_auth.verify_token(&token).unwrap();
```

#### HTTP Basic Authentication

- **BasicAuthentication**: HTTP Basic auth backend with user management
- **Base64 Encoding/Decoding**: Standard HTTP Basic auth header parsing
- **User Registration**: Add users with username/password pairs
- **Request Authentication**: Extract and verify credentials from Authorization
  headers

```rust
use reinhardt::auth::{HttpBasicAuth, AuthenticationBackend};

let mut auth = HttpBasicAuth::new();
auth.add_user("alice", "secret123");

// Request with Basic auth header will be authenticated
let result = auth.authenticate(&request).unwrap();
```

### User Management

#### User Trait

- **Core User Interface**: Unified trait for authenticated and anonymous users
- **User Identification**: `id()`, `username()`, `get_username()` methods
- **Authentication Status**: `is_authenticated()`, `is_active()`, `is_admin()`
  checks
- **Django Compatibility**: Methods compatible with Django's user interface

#### User Implementations

- **SimpleUser**: Fully-featured user with UUID, username, email, active/admin
  flags
- **AnonymousUser**: Zero-sized type representing unauthenticated visitors
- **Serialization Support**: Serde integration for SimpleUser

```rust
use reinhardt::auth::{User, SimpleUser, AnonymousUser};
use uuid::Uuid;

let user = SimpleUser {
    id: Uuid::new_v4(),
    username: "john".to_string(),
    email: "john@example.com".to_string(),
    is_active: true,
    is_admin: false,
};

assert!(user.is_authenticated());
assert!(!user.is_admin());
```

### Django-Style User Models

#### BaseUser Trait (AbstractBaseUser equivalent)

- **Minimal Authentication Interface**: Minimal set of fields for user
  authentication
- **Automatic Password Hashing**: Argon2id hashing by default, fully
  customizable
- **Associated Type Default**:
  `type Hasher: PasswordHasher + Default = Argon2Hasher`
- **Password Management**:
  - `set_password()`: Automatically hashes with configured hasher
  - `check_password()`: Verifies password against hash
  - `set_unusable_password()`: Marks password as unusable (for OAuth-only
    accounts)
  - `has_usable_password()`: Checks if user can log in with password
- **Session Authentication**: `get_session_auth_hash()` for session invalidation
  on password change
- **Username Normalization**: NFKC Unicode normalization to prevent homograph
  attacks
- **Django Compatibility**: Method names and behavior match Django's
  AbstractBaseUser

```rust
use reinhardt::auth::BaseUser;
use uuid::Uuid;
use chrono::Utc;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct MyUser {
    id: Uuid,
    email: String,
    password_hash: Option<String>,
    last_login: Option<chrono::DateTime<Utc>>,
    is_active: bool,
}

impl BaseUser for MyUser {
    type PrimaryKey = Uuid;
    // type Hasher = Argon2Hasher; // Default, can be omitted or customized

    fn get_username_field() -> &'static str { "email" }
    fn get_username(&self) -> &str { &self.email }
    fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
    fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
    fn last_login(&self) -> Option<chrono::DateTime<Utc>> { self.last_login }
    fn set_last_login(&mut self, time: chrono::DateTime<Utc>) { self.last_login = Some(time); }
    fn is_active(&self) -> bool { self.is_active }
}

let mut user = MyUser {
    id: Uuid::new_v4(),
    email: "alice@example.com".to_string(),
    password_hash: None,
    last_login: None,
    is_active: true,
};

// Password is automatically hashed with Argon2id
user.set_password("securepass123").unwrap();
assert!(user.check_password("securepass123").unwrap());
```

#### FullUser Trait (AbstractUser equivalent)

- **Complete User Model**: Extends BaseUser with additional fields
- **User Profile Fields**:
  - `username()`: Unique username for login
  - `email()`: Email address
  - `first_name()` / `last_name()`: User's name
  - `is_staff()`: Can access admin interface
  - `is_superuser()`: Has all permissions
  - `date_joined()`: Account creation timestamp
- **Helper Methods**:
  - `get_full_name()`: Combines first and last name
  - `get_short_name()`: Returns first name only
- **Django Compatibility**: Matches Django's AbstractUser interface

```rust
use reinhardt::auth::{BaseUser, FullUser, DefaultUser};
use uuid::Uuid;
use chrono::Utc;

let mut user = DefaultUser {
    id: Uuid::new_v4(),
    username: "alice".to_string(),
    email: "alice@example.com".to_string(),
    first_name: "Alice".to_string(),
    last_name: "Smith".to_string(),
    password_hash: None,
    last_login: None,
    is_active: true,
    is_staff: false,
    is_superuser: false,
    date_joined: Utc::now(),
    user_permissions: Vec::new(),
    groups: Vec::new(),
};

assert_eq!(user.get_full_name(), "Alice Smith");
assert_eq!(user.get_short_name(), "Alice");
```

#### PermissionsMixin Trait

- **Authorization Interface**: Permission and group management
- **Permission Checking**:
  - `has_perm(perm)`: Check if user has specific permission
  - `has_module_perms(app_label)`: Check if user has any permission for app
  - `get_all_permissions()`: Get all user and group permissions
- **Superuser Bypass**: Superusers automatically pass all permission checks
- **Group Support**: Users can belong to multiple groups
- **Django Compatibility**: Permission format `"app_label.permission_name"`

```rust
use reinhardt::auth::{DefaultUser, PermissionsMixin};
use uuid::Uuid;
use chrono::Utc;

let user = DefaultUser {
    id: Uuid::new_v4(),
    username: "bob".to_string(),
    email: "bob@example.com".to_string(),
    first_name: "Bob".to_string(),
    last_name: "Johnson".to_string(),
    password_hash: None,
    last_login: None,
    is_active: true,
    is_staff: true,
    is_superuser: false,
    date_joined: Utc::now(),
    user_permissions: vec![
        "blog.add_post".to_string(),
        "blog.change_post".to_string(),
    ],
    groups: vec!["editors".to_string()],
};

assert!(user.has_perm("blog.add_post"));
assert!(user.has_module_perms("blog"));
```

#### DefaultUser Struct

- **Ready-to-Use Implementation**: Combines BaseUser, FullUser, and
  PermissionsMixin
- **Database Model**: `#[derive(Model)]` for ORM integration
- **Table Name**: `auth_user` (Django-compatible)
- **All Fields Included**: Username, email, names, passwords, permissions,
  groups, flags, timestamps
- **Zero Configuration**: Works out of the box with automatic Argon2id hashing

```rust
use reinhardt::auth::{BaseUser, DefaultUser, DefaultUserManager};
use std::collections::HashMap;

# tokio_test::block_on(async {
let mut manager = DefaultUserManager::new();

// Create a regular user
let user = manager.create_user(
    "alice",
    Some("securepass123"),
    HashMap::new()
).await.unwrap();

assert_eq!(user.username, "alice");
assert!(user.is_active);
assert!(!user.is_staff);

// Create a superuser
let admin = manager.create_superuser(
    "admin",
    Some("adminsecret"),
    HashMap::new()
).await.unwrap();

assert!(admin.is_staff);
assert!(admin.is_superuser);
# })
```

#### BaseUserManager Trait

- **User Creation Interface**: `create_user()` and `create_superuser()` methods
- **Async Support**: Full async/await integration
- **Extra Fields**: Accept arbitrary extra data via HashMap
- **Email Normalization**: `normalize_email()` static method
- **Django Compatibility**: Matches Django's UserManager interface

#### DefaultUserManager

- **In-Memory Implementation**: Built-in manager for DefaultUser
- **Thread-Safe**: Uses `Arc<RwLock<HashMap>>` for concurrent access
- **User Lookup**: `get_by_id()` and `get_by_username()` methods
- **Demonstration Purpose**: For testing and prototyping (use ORM-based manager
  in production)

### Password Security

#### Password Hashing

- **PasswordHasher Trait**: Composable password hashing interface
- **Argon2Hasher**: Production-ready Argon2id implementation (recommended)
- **Hash Generation**: Secure salt generation using OS random number generator
- **Password Verification**: Constant-time comparison for security

```rust
use reinhardt::auth::{Argon2Hasher, PasswordHasher};

let hasher = Argon2Hasher::new();
let hash = hasher.hash("my_password").unwrap();
assert!(hasher.verify("my_password", &hash).unwrap());
```

### Authentication Backends

#### AuthBackend Trait

- **Composable Architecture**: Support for multiple authentication strategies
- **Async Support**: Full async/await integration with `async_trait`
- **User Authentication**: `authenticate(username, password)` method
- **User Lookup**: `get_user(user_id)` for session restoration

#### Composite Authentication

- **CompositeAuthBackend**: Chain multiple authentication backends
- **Fallback Support**: Try backends in order until one succeeds
- **Flexible Configuration**: Add backends dynamically at runtime

```rust
use reinhardt::auth::CompositeAuthBackend;

let mut composite = CompositeAuthBackend::new();
composite.add_backend(Box::new(database_backend));
composite.add_backend(Box::new(ldap_backend));

// Will try database first, then LDAP
let user = composite.authenticate("alice", "password").await;
```

### Permission System

#### Permission Trait

- **Permission Interface**: Async `has_permission()` method with context
- **PermissionContext**: Request-aware context with authentication flags
- **Composable Permissions**: Build complex permission logic

#### Built-in Permission Classes

- **AllowAny**: Allow all requests without authentication
- **IsAuthenticated**: Require authenticated user
- **IsAdminUser**: Require authenticated admin user
- **IsActiveUser**: Require authenticated and active user
- **IsAuthenticatedOrReadOnly**: Authenticated for write, read-only for
  anonymous users

```rust
use reinhardt::auth::{Permission, IsAuthenticated, PermissionContext};

let permission = IsAuthenticated;
let context = PermissionContext {
    request: &request,
    is_authenticated: true,
    is_admin: false,
    is_active: true,
};

assert!(permission.has_permission(&context).await);
```

### Error Handling

#### AuthenticationError

- **InvalidCredentials**: Wrong username or password
- **UserNotFound**: User does not exist
- **SessionExpired**: Session has expired
- **InvalidToken**: Token is malformed or invalid
- **Unknown**: Generic error with custom message

#### AuthenticationBackend Trait

- **Unified Error Handling**: All backends use `AuthenticationError`
- **Standard Error Trait**: Implements `std::error::Error`
- **Display Implementation**: User-friendly error messages

### Session-Based Authentication

#### SessionAuthentication

- **Session Management**: `Session` struct with HashMap-based data storage
- **SessionStore Trait**: Async interface for session persistence
  - `load()`: Retrieve session by ID
  - `save()`: Persist session data
  - `delete()`: Remove session
- **InMemorySessionStore**: Built-in in-memory session storage
- **SessionId**: Type-safe session identifier wrapper
- **Cookie Integration**: Secure session cookie handling

```rust
use reinhardt::auth::{SessionAuthentication, Authentication};
use reinhardt::auth::sessions::backends::InMemorySessionBackend;

let session_backend = InMemorySessionBackend::new();
let auth = SessionAuthentication::new(session_backend);

// Authenticate user from request (checks session cookie)
let user = auth.authenticate(&request).await?;

// Get user by ID
if let Some(user) = auth.get_user("user_id").await? {
    println!("User: {}", user.get_username());
}
```

### Multi-Factor Authentication (MFA)

#### TOTP-Based MFA

- **MFAAuthentication**: Time-based one-time password (TOTP) authentication
- **Secret Management**: Secure per-user secret storage
- **QR Code Generation**: Generate TOTP URLs for authenticator apps (Google
  Authenticator, Authy)
- **Code Verification**: Verify TOTP codes with configurable time window
- **Registration Flow**: User enrollment with secret generation
- **Time Window**: Configurable tolerance for time skew (default: 1 time step)

```rust
use reinhardt::auth::MFAAuthentication;

let mfa = MFAAuthentication::new("MyApp");

// Register user for MFA
let totp_url = mfa.register_user("alice").await?;
// User scans QR code generated from totp_url

// Verify code during login
let code = "123456"; // from user's authenticator app
assert!(mfa.verify_code("alice", code).await?);
```

### OAuth2 Support

#### OAuth2 Authentication

- **OAuth2Authentication**: Full OAuth2 provider implementation
- **Grant Types**: Authorization Code, Client Credentials, Refresh Token,
  Implicit
- **Application Management**: `OAuth2Application` with client credentials
- **Token Management**: `OAuth2Token` with access and refresh tokens
- **Authorization Flow**:
  - Authorization code generation and validation
  - Token exchange (code → access token)
  - Token refresh with refresh tokens
- **OAuth2TokenStore Trait**: Persistent token storage interface
- **InMemoryTokenStore**: Built-in in-memory token storage

```rust
use reinhardt::auth::{OAuth2Authentication, GrantType, InMemoryOAuth2Store};

let store = InMemoryOAuth2Store::new();
let oauth2 = OAuth2Authentication::new(store);

// Register OAuth2 application
oauth2.register_application(
    "client123",
    "secret456",
    "https://example.com/callback",
    vec![GrantType::AuthorizationCode]
).await?;

// Authorization code flow
let code = oauth2.generate_authorization_code("client123", "user123", vec!["read", "write"]).await?;
let token = oauth2.exchange_code(&code, "client123").await?;

// Use access token
let claims = oauth2.verify_token(&token.access_token).await?;
```

### Token Blacklist & Rotation

#### Token Blacklist

- **TokenBlacklist Trait**: Interface for token invalidation
- **BlacklistReason**: Categorized revocation reasons
  - `Logout`: User-initiated logout
  - `Compromised`: Security incident
  - `ManualRevoke`: Admin revocation
  - `Rotated`: Automatic token rotation
- **InMemoryBlacklist**: Built-in in-memory blacklist storage
- **Cleanup**: Automatic removal of expired blacklist entries
- **Statistics**: Usage tracking and monitoring

#### Token Rotation

- **TokenRotationManager**: Automatic refresh token rotation
- **RefreshTokenStore Trait**: Persistent refresh token storage
- **Rotation Flow**: Invalidate old token when issuing new one
- **Security**: Prevents refresh token reuse attacks
- **InMemoryRefreshStore**: Built-in in-memory refresh token storage

```rust
use reinhardt::auth::{
    TokenBlacklist, InMemoryBlacklist, BlacklistReason,
    TokenRotationManager, InMemoryRefreshStore
};

// Token blacklist
let blacklist = InMemoryBlacklist::new();
use chrono::{Utc, Duration};
let expires_at = Utc::now() + Duration::hours(24);
blacklist.blacklist("old_token", expires_at, BlacklistReason::Logout).await?;
assert!(blacklist.is_blacklisted("old_token").await?);

// Token rotation
let refresh_store = InMemoryRefreshStore::new();
let rotation_manager = TokenRotationManager::new(blacklist, refresh_store);

let new_token = rotation_manager.rotate_token("old_refresh_token", "user123").await?;
```

### Remote User Authentication

#### Header-Based Authentication

- **RemoteUserAuthentication**: Authenticate via trusted HTTP headers
- **Reverse Proxy Integration**: Support for authentication proxies (nginx,
  Apache, etc.)
- **Header Configuration**: Configurable header name (default: `REMOTE_USER`)
- **Header Validation**: Verify header presence and format
- **Automatic Logout**: Optional force logout when header is missing
- **SSO Support**: Single sign-on integration

```rust
use reinhardt::auth::RemoteUserAuthentication;

// Standard configuration
let auth = RemoteUserAuthentication::new("REMOTE_USER");

// With force logout
let auth = RemoteUserAuthentication::new("REMOTE_USER").force_logout_if_no_header(true);

// Authenticate from request
let user = auth.authenticate(&request).await?;
```

## Usage Examples

### Complete Authentication Flow

```rust
use reinhardt::auth::{
    JwtAuth, HttpBasicAuth, AuthBackend,
    SimpleUser, User, Argon2Hasher, PasswordHasher,
    Permission, IsAuthenticated, PermissionContext
};

// 1. Set up JWT authentication
let jwt_auth = JwtAuth::new(b"secret-key");

// 2. Set up Basic authentication with a user
let mut basic_auth = HttpBasicAuth::new();
basic_auth.add_user("alice", "password123");

// 3. Authenticate user and generate JWT
let user = basic_auth.authenticate(&request).unwrap().unwrap();
let token = jwt_auth.generate_token(
    user.id(),
    user.username().to_string()
).unwrap();

// 4. Verify token on subsequent requests
let claims = jwt_auth.verify_token(&token).unwrap();

// 5. Check permissions
let permission = IsAuthenticated;
let context = PermissionContext {
    request: &request,
    is_authenticated: true,
    is_admin: user.is_admin(),
    is_active: user.is_active(),
};

if permission.has_permission(&context).await {
    // Grant access
}
```

### Custom Authentication Backend

```rust
use reinhardt::auth::{AuthBackend, SimpleUser, Argon2Hasher, PasswordHasher};
use async_trait::async_trait;
use std::collections::HashMap;

struct MyAuthBackend {
    users: HashMap<String, (String, SimpleUser)>,
    hasher: Argon2Hasher,
}

#[async_trait]
impl AuthBackend for MyAuthBackend {
    type User = SimpleUser;

    async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<Self::User>, reinhardt_exception::Error> {
        if let Some((hash, user)) = self.users.get(username) {
            if self.hasher.verify(password, hash)? {
                return Ok(Some(user.clone()));
            }
        }
        Ok(None)
    }

    async fn get_user(&self, user_id: &str)
        -> Result<Option<Self::User>, reinhardt_exception::Error> {
        Ok(self.users.values()
            .find(|(_, u)| u.id.to_string() == user_id)
            .map(|(_, u)| u.clone()))
    }
}
```


## sessions

### Features

### Implemented ✓

#### Core Session Backend

- **SessionBackend Trait** - Async trait defining session storage operations (load, save, delete, exists)
- **SessionError** - Error types for session operations (cache errors, serialization errors)
- **Generic Session Storage** - Type-safe session data storage with `serde` support

#### Cache-Based Backends

- **InMemorySessionBackend** - In-memory session storage using `InMemoryCache`
  - Fast, volatile storage (sessions lost on restart)
  - TTL (Time-To-Live) support for automatic expiration
  - Suitable for development and single-instance deployments
- **CacheSessionBackend** - Generic cache-based session backend
  - Works with any `Cache` trait implementation
  - Supports external cache systems (Redis, Memcached, etc.)
  - Configurable TTL for session expiration
  - Horizontal scalability for distributed systems

#### Dependency Injection Support

- Integration with `reinhardt-di` for dependency injection
- Session backend registration and resolution

#### High-Level Session API

- **Session<B>** struct - Django-style session object with dictionary-like interface
  - Type-safe with generic backend parameter `B: SessionBackend`
  - Dictionary-like methods: `get()`, `set()`, `delete()`, `contains_key()`
  - Session iteration methods: `keys()`, `values()`, `items()`
  - Manual session clearing: `clear()`
  - Manual modification tracking: `mark_modified()`, `mark_unmodified()`
  - Session modification tracking: `is_modified()`, `is_accessed()`
  - Session key management: `get_or_create_key()`, `generate_key()`
  - Session lifecycle: `flush()` (clear and new key), `cycle_key()` (keep data, new key)
  - Automatic persistence: `save()` method with TTL support (default: 3600 seconds)
  - Comprehensive doctests and unit tests (36 total tests)

#### Storage Backends

- **DatabaseSessionBackend** (feature: `database`) - Persistent session storage in database
  - Session model with expiration timestamps
  - Automatic session cleanup with `cleanup_expired()`
  - SQLite, PostgreSQL, and MySQL support via sqlx
  - Table creation with `create_table()`
  - Indexed expiration dates for efficient cleanup
  - 9 comprehensive tests
- **FileSessionBackend** (feature: `file`) - File-based session storage
  - Session files stored in configurable directory (default: `/tmp/reinhardt_sessions`)
  - File locking using `fs2` for concurrent access safety
  - JSON serialization with TTL support
  - Automatic expired session cleanup on access
  - 11 comprehensive tests
- **CookieSessionBackend** (feature: `cookie`) - Encrypted session data in cookies
  - AES-256-GCM encryption for session data
  - HMAC-SHA256 signing for tamper detection
  - Automatic size limitation checking (4KB max)
  - Secure client-side storage
  - 11 comprehensive tests

#### HTTP Middleware

- **SessionMiddleware** (feature: `middleware`) - HTTP middleware for session management
  - Automatic session loading from cookies
  - Automatic session saving on response
  - Cookie configuration: name, path, domain
  - Security settings: secure, httponly, samesite
  - TTL and max-age support
- **HttpSessionConfig** - Comprehensive middleware configuration
- **SameSite** enum - Cookie SameSite attribute (Strict, Lax, None)

#### Session Management Features

- **Session expiration and cleanup** - Implemented via `cleanup_expired()` in DatabaseSessionBackend
- **Session key rotation** - Implemented via `cycle_key()` and `flush()` in Session API
- **Cross-site request forgery (CSRF) protection integration** - CSRF module available
- **Session serialization formats** - JSON via serde_json, MessagePack, CBOR, Bincode
- **Session storage migration tools** - Migration module available

#### Session Serialization Formats

- **JSON** (always available) - Human-readable, widely compatible via `serde_json`
- **MessagePack** (feature: `messagepack`) - Compact binary format, cross-platform via `rmp-serde`
- **CBOR** (feature: `cbor`) - RFC 7049 compliant binary format via `ciborium`
- **Bincode** (feature: `bincode`) - Fastest for Rust-to-Rust communication

#### Session Compression

- **CompressedSessionBackend** (feature: `compression`) - Automatic compression wrapper
  - Threshold-based compression (default: 512 bytes, configurable)
  - Only compresses data exceeding threshold to avoid overhead
  - **Zstd compression** (feature: `compression-zstd`) - Best balance of speed and ratio
  - **Gzip compression** (feature: `compression-gzip`) - Wide compatibility
  - **Brotli compression** (feature: `compression-brotli`) - Best compression ratio

#### Session Replication

- **ReplicatedSessionBackend** (feature: `replication`) - High availability with multi-backend replication
  - **AsyncReplication** - Eventual consistency, highest throughput
  - **SyncReplication** - Strong consistency, both backends updated in parallel
  - **AcknowledgedReplication** - Primary first, then secondary with acknowledgment
  - Configurable retry attempts and delays for failure handling

#### Session Analytics

- **InstrumentedSessionBackend** - Automatic session event tracking wrapper
- **LoggerAnalytics** - Tracing-based logging (always available)
- **PrometheusAnalytics** (feature: `analytics-prometheus`) - Prometheus metrics export
  - `session_created_total` - Total sessions created
  - `session_accessed_total` - Total session accesses
  - `session_access_latency_seconds` - Access latency histogram
  - `session_size_bytes` - Session data size histogram
  - `session_deleted_total` - Deletions by reason (explicit, expired, flushed)
  - `session_expired_total` - Total expired sessions

#### Multi-Tenant Session Isolation

- **TenantSessionBackend** (feature: `tenant`) - Tenant-specific session namespacing
  - Prefix-based keying: `tenant:{tenant_id}:session:{session_id}`
  - Configurable key prefix pattern
  - Maximum sessions per tenant limit
  - Strict isolation mode for security
  - **TenantSessionOperations** trait: `list_sessions()`, `count_sessions()`, `delete_all_sessions()`

## License

Licensed under the BSD 3-Clause License.
