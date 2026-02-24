# examples-twitter

A comprehensive Twitter-like social networking application demonstrating Reinhardt framework features.

## Overview

This example showcases a production-ready social networking platform built with Reinhardt, featuring authentication, user profiles, direct messaging, and relationship management (follow/block).

**Scale**: 9,243 lines of code across 73 files

**Implementation Status**: ✅ Fully functional with comprehensive test coverage

## Features

### Authentication System (`apps/auth`)

Comprehensive authentication with JWT token management:

- **User Registration**: Create new accounts with email validation
- **Sign In/Sign Out**: JWT-based session management
- **Password Management**:
  - Password verification endpoint
  - Change password for authenticated users
  - Password reset with token-based recovery
- **Security**: Argon2 password hashing, secure token storage

**API Endpoints**:
- `POST /api/auth/register` - Create new user account
- `POST /api/auth/signin` - Authenticate and receive JWT token
- `POST /api/auth/signout` - Invalidate session
- `POST /api/auth/verify-password` - Verify user password
- `POST /api/auth/change-password` - Change authenticated user's password
- `POST /api/auth/reset-password` - Request password reset
- `POST /api/auth/reset-password/:token` - Complete password reset

### Direct Messaging (`apps/dm`)

Real-time messaging between users:

- **Message Rooms**: Create and manage conversation rooms
- **Message History**: Retrieve and send messages
- **Room Management**: List rooms, participants, and metadata

**Models**:
- `Room`: Conversation container with participants
- `Message`: Individual messages with sender, content, timestamps

**API Endpoints**:
- `GET /api/dm/rooms` - List user's conversation rooms
- `POST /api/dm/rooms` - Create new conversation room
- `GET /api/dm/rooms/{id}` - Get room details
- `GET /api/dm/rooms/{id}/messages` - Retrieve message history
- `POST /api/dm/rooms/{id}/messages` - Send message to room

### User Profiles (`apps/profile`)

User profile management with rich metadata:

- **Profile Creation**: Auto-create profile on user registration
- **Profile Updates**: Partial updates (PATCH) for flexible editing
- **Profile Retrieval**: Fetch user profiles by user ID

**Profile Fields**:
- `bio`: User biography/description
- `location`: Geographic location
- `website`: Personal website URL
- `avatar_url`: Profile picture URL
- `banner_url`: Profile banner image URL
- `birth_date`: Date of birth
- `joined_date`: Account creation timestamp

**API Endpoints**:
- `GET /api/profile/:user_id` - Get user profile
- `POST /api/profile` - Create profile (auto-created on registration)
- `PATCH /api/profile` - Update authenticated user's profile

### Relationship Management (`apps/relationship`)

Social graph functionality (follow/block):

- **Follow System**: Follow/unfollow users
- **Block System**: Block/unblock users
- **Relationship Queries**: List followers, following, blocked users

**API Endpoints**:
- `POST /api/relationship/follow/:user_id` - Follow a user
- `DELETE /api/relationship/follow/:user_id` - Unfollow a user
- `POST /api/relationship/block/:user_id` - Block a user
- `DELETE /api/relationship/block/:user_id` - Unblock a user
- `GET /api/relationship/followers` - List followers
- `GET /api/relationship/following` - List users you follow
- `GET /api/relationship/blocked` - List blocked users

## Project Structure

```
examples-twitter/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Library exports
│   ├── config.rs           # Configuration module
│   ├── apps.rs             # App registry
│   ├── migrations.rs       # Database migrations
│   ├── test_utils.rs       # Testing utilities
│   ├── config/
│   │   ├── apps.rs         # Installed apps configuration
│   │   ├── settings.rs     # Settings loader
│   │   ├── urls.rs         # URL routing configuration
│   │   └── views.rs        # Root view handlers
│   ├── apps/
│   │   ├── auth/           # Authentication app (75 KB, ~2,400 lines)
│   │   │   ├── lib.rs
│   │   │   ├── models.rs   # User, PasswordResetToken
│   │   │   ├── serializers.rs
│   │   │   ├── views.rs    # Registration, signin, password management
│   │   │   ├── admin.rs
│   │   │   ├── urls.rs
│   │   │   └── tests.rs
│   │   ├── dm/             # Direct messaging app (56 KB, ~1,800 lines)
│   │   │   ├── lib.rs
│   │   │   ├── models.rs   # Room, Message
│   │   │   ├── serializers.rs
│   │   │   ├── views.rs    # Room and message management
│   │   │   ├── admin.rs
│   │   │   ├── urls.rs
│   │   │   └── tests.rs
│   │   ├── profile/        # User profiles app (37 KB, ~1,200 lines)
│   │   │   ├── lib.rs
│   │   │   ├── models.rs   # Profile
│   │   │   ├── serializers.rs
│   │   │   ├── views.rs    # Profile CRUD operations
│   │   │   ├── admin.rs
│   │   │   ├── urls.rs
│   │   │   └── tests.rs
│   │   └── relationship/   # Follow/block app (64 KB, ~2,000 lines)
│   │       ├── lib.rs
│   │       ├── serializers.rs
│   │       ├── views.rs    # Follow, block, list operations
│   │       ├── admin.rs
│   │       ├── urls.rs
│   │       └── tests.rs
│   ├── test_utils/
│   │   ├── fixtures.rs     # Test fixtures
│   │   │   ├── database.rs
│   │   │   ├── users.rs
│   │   │   ├── auth.rs
│   │   │   └── server.rs
│   │   └── helpers.rs      # Test helpers
│   │       ├── assertions.rs
│   │       └── builders.rs
│   └── bin/
│       └── manage.rs       # Management CLI
└── README.md
```

## Setup

### Prerequisites

- Rust 1.91.1+ (2024 Edition required)
- Docker (required for TestContainers-based integration tests)
- PostgreSQL (optional - can use TestContainers instead)

### Installation

```bash
# From project root
cd examples/examples-twitter

# Build the project
cargo build

# Run migrations
cargo run --bin manage migrate

# Start development server
cargo run
```

## Configuration

### Environment Variables

```bash
# Database connection
export DATABASE_URL="postgres://postgres:password@localhost:5432/twitter_dev"

# JWT secret (for token signing)
export JWT_SECRET="your-secret-key-here"

# Environment profile (local, staging, production)
export REINHARDT_ENV="local"
```

### Settings Files

Project uses TOML-based configuration in `settings/` directory:

- `base.toml`: Common settings for all environments
- `local.toml`: Local development settings
- `staging.toml`: Staging environment settings
- `production.toml`: Production environment settings

Settings are loaded based on `REINHARDT_ENV` environment variable (defaults to `local`).

## Running the Application

### Development Server

```bash
# Run with default settings (local environment)
cargo run

# Run with specific environment
REINHARDT_ENV=staging cargo run

# Run with custom port
cargo run -- --port 3000
```

### Management Commands

```bash
# Database migrations
cargo run --bin manage makemigrations
cargo run --bin manage migrate

# Create new app
cargo run --bin manage startapp myapp --template-type restful

# Development server
cargo run --bin manage runserver
```

## Testing

### Prerequisites

Tests require **Docker** for TestContainers integration:

```bash
# Verify Docker is running
docker version
docker ps
```

**CRITICAL**: This project uses Docker for TestContainers integration, NOT Podman.

### Running Tests

```bash
# Run all tests (requires Docker)
cargo test

# Run tests for specific app
cargo test --package examples-twitter --test auth_integration
cargo test --package examples-twitter --test dm_integration
cargo test --package examples-twitter --test profile_integration
cargo test --package examples-twitter --test relationship_integration

# Run with test output
cargo test -- --nocapture
```

### Test Coverage

**Authentication Tests** (`apps/auth/tests/`):
- ✅ User registration with validation
- ✅ Sign in with JWT token generation
- ✅ Sign out with token invalidation
- ✅ Password verification
- ✅ Password change for authenticated users
- ✅ Password reset flow with tokens

**Direct Messaging Tests** (`apps/dm/tests/`):
- ✅ Room creation and retrieval
- ✅ Message sending and history
- ✅ Multi-user conversations
- ✅ Room participant management

**Profile Tests** (`apps/profile/tests/`):
- ✅ Profile creation on user registration
- ✅ Profile retrieval by user ID
- ✅ Partial profile updates (PATCH)
- ✅ Profile validation

**Relationship Tests** (`apps/relationship/tests/`):
- ✅ Follow/unfollow operations
- ✅ Block/unblock operations
- ✅ List followers, following, blocked users
- ✅ Relationship state consistency

**Test Infrastructure** (`test_utils/`):
- Standard fixtures from `reinhardt-test`
- PostgreSQL TestContainers integration
- Test database with migrations
- User and authentication fixtures
- Test server with request helpers

### Standard Fixtures Used

This example uses standard fixtures from `reinhardt-test`:

```rust
use reinhardt::test::fixtures::postgres_with_migrations_from;
use rstest::*;

#[rstest]
#[tokio::test]
async fn test_with_database(
    #[future] postgres_with_migrations_from: (
        ContainerAsync<GenericImage>,
        Arc<PgPool>,
        u16,
        String,
        Vec<Box<dyn Migration>>,
    ),
) {
    let (_container, pool, _port, _database_url, _migrations) =
        postgres_with_migrations_from.await;

    // Use pool for database operations
    // Migrations are already applied

    // Container is automatically cleaned up when dropped
}
```

For comprehensive testing standards, see:
- [Testing Standards](../../../docs/TESTING_STANDARDS.md)
- [Examples Database Integration](../examples-database-integration/README.md)

## API Reference

### Authentication API

#### POST /api/auth/register

Create new user account.

**Request Body**:
```json
{
  "username": "john_doe",
  "email": "john@example.com",
  "password": "secure_password123",
  "password_confirm": "secure_password123"
}
```

**Response** (201 Created):
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "username": "john_doe",
  "email": "john@example.com",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

#### POST /api/auth/signin

Authenticate user and receive JWT token.

**Request Body**:
```json
{
  "username": "john_doe",
  "password": "secure_password123"
}
```

**Response** (200 OK):
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "user_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

#### POST /api/auth/change-password

Change authenticated user's password.

**Request Headers**:
```
Authorization: Bearer <jwt_token>
```

**Request Body**:
```json
{
  "old_password": "current_password",
  "new_password": "new_secure_password",
  "new_password_confirm": "new_secure_password"
}
```

**Response** (200 OK):
```json
{
  "message": "Password changed successfully"
}
```

### Direct Messaging API

#### POST /api/dm/rooms

Create new conversation room.

**Request Headers**:
```
Authorization: Bearer <jwt_token>
```

**Request Body**:
```json
{
  "participant_ids": [
    "550e8400-e29b-41d4-a716-446655440000",
    "650e8400-e29b-41d4-a716-446655440001"
  ],
  "name": "Project Discussion"
}
```

**Response** (201 Created):
```json
{
  "id": "750e8400-e29b-41d4-a716-446655440002",
  "name": "Project Discussion",
  "created_at": "2025-12-10T12:00:00Z",
  "participants": [...]
}
```

#### POST /api/dm/rooms/{id}/messages

Send message to room.

**Request Headers**:
```
Authorization: Bearer <jwt_token>
```

**Request Body**:
```json
{
  "content": "Hello, team!"
}
```

**Response** (201 Created):
```json
{
  "id": "850e8400-e29b-41d4-a716-446655440003",
  "room_id": "750e8400-e29b-41d4-a716-446655440002",
  "sender_id": "550e8400-e29b-41d4-a716-446655440000",
  "content": "Hello, team!",
  "sent_at": "2025-12-10T12:05:00Z"
}
```

### Profile API

#### PATCH /api/profile

Update authenticated user's profile (partial update).

**Request Headers**:
```
Authorization: Bearer <jwt_token>
```

**Request Body** (all fields optional):
```json
{
  "bio": "Software engineer and open source enthusiast",
  "location": "San Francisco, CA",
  "website": "https://example.com",
  "avatar_url": "https://example.com/avatar.jpg"
}
```

**Response** (200 OK):
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "bio": "Software engineer and open source enthusiast",
  "location": "San Francisco, CA",
  "website": "https://example.com",
  "avatar_url": "https://example.com/avatar.jpg",
  "updated_at": "2025-12-10T12:10:00Z"
}
```

### Relationship API

#### POST /api/relationship/follow/:user_id

Follow a user.

**Request Headers**:
```
Authorization: Bearer <jwt_token>
```

**Response** (201 Created):
```json
{
  "follower_id": "550e8400-e29b-41d4-a716-446655440000",
  "following_id": "650e8400-e29b-41d4-a716-446655440001",
  "created_at": "2025-12-10T12:15:00Z"
}
```

#### GET /api/relationship/followers

List users who follow the authenticated user.

**Request Headers**:
```
Authorization: Bearer <jwt_token>
```

**Response** (200 OK):
```json
{
  "followers": [
    {
      "user_id": "750e8400-e29b-41d4-a716-446655440002",
      "username": "alice",
      "followed_at": "2025-12-09T10:00:00Z"
    }
  ],
  "count": 1
}
```

## Database Schema

### Users (auth app)

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(254) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    is_staff BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### Profiles (profile app)

```sql
CREATE TABLE profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID UNIQUE NOT NULL REFERENCES users(id),
    bio TEXT,
    location VARCHAR(100),
    website VARCHAR(200),
    avatar_url VARCHAR(500),
    banner_url VARCHAR(500),
    birth_date DATE,
    joined_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### Rooms (dm app)

```sql
CREATE TABLE dm_rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE dm_room_participants (
    room_id UUID REFERENCES dm_rooms(id),
    user_id UUID REFERENCES users(id),
    joined_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (room_id, user_id)
);
```

### Messages (dm app)

```sql
CREATE TABLE dm_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id UUID NOT NULL REFERENCES dm_rooms(id),
    sender_id UUID NOT NULL REFERENCES users(id),
    content TEXT NOT NULL,
    sent_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    read_at TIMESTAMP
);
```

### Relationships (relationship app)

```sql
CREATE TABLE relationships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    follower_id UUID NOT NULL REFERENCES users(id),
    following_id UUID NOT NULL REFERENCES users(id),
    relationship_type VARCHAR(20) NOT NULL, -- 'follow' or 'block'
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (follower_id, following_id, relationship_type)
);

CREATE INDEX idx_relationships_follower ON relationships(follower_id);
CREATE INDEX idx_relationships_following ON relationships(following_id);
```

## Key Implementation Details

### JWT Authentication

Uses `reinhardt-auth` with JWT token authentication:

```rust
use reinhardt::auth::{JwtAuth, BaseUser};

// Token generation on signin
let jwt_auth = JwtAuth::new(secret_key.as_bytes());
let token = jwt_auth.generate_token(&user)?;

// Token verification in protected routes
let claims = jwt_auth.verify_token(&token)?;
let user = User::find_by_id(&db, &claims.user_id).await?;
```

### Password Hashing

Uses Argon2 for secure password hashing:

```rust
use reinhardt::auth::hasher::Argon2Hasher;

let hasher = Argon2Hasher::new();
let password_hash = hasher.hash_password("user_password")?;
let is_valid = hasher.verify_password("user_password", &password_hash)?;
```

### Database Transactions

Uses `reinhardt-db` transaction support:

```rust
use reinhardt::db::transaction;

transaction(&db, |_tx| async move {
    // Create user
    let user = User::create(user_data).await?;

    // Auto-create profile
    let profile = Profile::create_for_user(&user).await?;

    Ok((user, profile))
}).await?;
```

### Test Fixtures

Uses `reinhardt-test` standard fixtures:

```rust
use reinhardt::test::fixtures::postgres_with_migrations_from;

#[rstest]
#[tokio::test]
async fn test_feature(
    #[future] postgres_with_migrations_from: DatabaseFixture,
) {
    let (_container, pool, _port, _url, _migrations) =
        postgres_with_migrations_from.await;

    // Test with real database and migrations
}
```

## Troubleshooting

### Docker Connection Issues

```bash
# 1. Check Docker is running
docker ps

# 2. Check DOCKER_HOST environment variable
echo $DOCKER_HOST

# 3. If DOCKER_HOST points to Podman, unset it
unset DOCKER_HOST

# 4. Verify .testcontainers.properties exists in project root
cat ../../../.testcontainers.properties
```

### Database Migration Errors

```bash
# Reset database and rerun migrations
cargo run --bin manage migrate --reset

# Create fresh migration
cargo run --bin manage makemigrations --name initial_schema
```

### Test Failures

```bash
# Run tests with verbose output
cargo test -- --nocapture --test-threads=1

# Run specific test
cargo test --package examples-twitter test_user_registration -- --exact --nocapture
```

### Static Files Not Found (404 Errors)

**Symptom**: Browser shows 404 errors when accessing `http://localhost:8000/`, WASM application doesn't load.

**Cause**: When the server is started from the workspace root, the relative path `"dist"` resolves incorrectly.

**Solution 1**: Run from project directory (Recommended)

```bash
cd examples/examples-twitter
cargo run -- runserver --with-pages
```

**Solution 2**: Specify absolute path

```bash
# When running from workspace root
cargo run --bin examples-twitter -- runserver \
  --with-pages \
  --static-dir /Users/kent8192/Projects/reinhardt/examples/examples-twitter/dist
```

**Solution 3**: Use environment variable (Future support planned)

```bash
CARGO_MANIFEST_DIR=$(pwd)/examples/examples-twitter \
  cargo run --bin examples-twitter -- runserver --with-pages
```

## References

- [Reinhardt Framework Documentation](https://docs.rs/reinhardt)
- [Getting Started Guide](../../../docs/GETTING_STARTED.md)
- [Testing Standards](../../../docs/TESTING_STANDARDS.md)
- [Database Integration Example](../examples-database-integration/README.md)
- [REST API Example](../examples-rest-api/README.md)

## License

This example is provided as part of the Reinhardt project under the BSD 3-Clause License.
