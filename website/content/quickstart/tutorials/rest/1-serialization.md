+++
title = "Tutorial 1: Serialization"
weight = 20

[extra]
sidebar_weight = 30
+++

# Tutorial 1: Serialization

Learn how to serialize and deserialize data in Reinhardt.

## Setup

First, add Reinhardt to your project's `Cargo.toml`:

{% versioned_code(lang="toml") %}
[dependencies]
reinhardt = { version = "LATEST_VERSION", package = "reinhardt-web", features = ["standard"] }
# Or for minimal setup: reinhardt = { version = "LATEST_VERSION", package = "reinhardt-web", default-features = false, features = ["minimal"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
{% end %}

**Note:** Reinhardt uses feature flags to control which components are included in your build. The `standard` feature includes serializers, ORM, authentication, and other common API development tools. For more granular control, see the [Feature Flags Guide](/docs/feature-flags/).

## Defining the Snippet Model

First, define the Snippet model using Reinhardt's `#[model(...)]` attribute. This automatically implements the `Model` trait, generates type-safe field accessors, and registers the model globally:

```rust
use chrono::{DateTime, Utc};
use reinhardt::prelude::*;
use serde::{Serialize, Deserialize};

/// Snippet model representing a code snippet
#[model(app_label = "snippets", table_name = "snippets")]
#[derive(Serialize, Deserialize)]
pub struct Snippet {
	#[field(primary_key = true)]
	pub id: i64,

	#[field(max_length = 100)]
	pub title: String,

	#[field(max_length = 10000)]
	pub code: String,

	#[field(max_length = 50)]
	pub language: String,

	#[field(auto_now_add = true)]
	pub created_at: DateTime<Utc>,
}
```

The `#[model(...)]` attribute automatically derives the `Model` trait -- you do not need `#[derive(Model)]` separately. Field attributes like `primary_key`, `max_length`, and `auto_now_add` define database schema constraints.

## Basic Serialization with Serde

Reinhardt primarily uses [serde](https://serde.rs/) for serialization. For simple cases, derive macros are sufficient:

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: i64,
    pub title: String,
    pub code: String,
    pub language: String,
}
```

## Custom Serialization with the Serializer Trait

For custom serialization logic, implement the `Serializer` trait. Validation can be handled by a separate function:

```rust
use reinhardt::prelude::*;

pub struct SnippetSerializer;

impl Serializer for SnippetSerializer {
    type Input = Snippet;
    type Output = Vec<u8>;

    fn serialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError> {
        serde_json::to_vec(input).map_err(|e| SerializerError::SerializeError(e.to_string()))
    }

    fn deserialize(&self, output: &Self::Output) -> Result<Self::Input, SerializerError> {
        serde_json::from_slice(output).map_err(|e| SerializerError::DeserializeError(e.to_string()))
    }
}

// For custom validation, use a separate validation function:
fn validate_snippet(snippet: &Snippet) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    if snippet.title.is_empty() {
        errors.push(ValidationError::field_error("title", "Title cannot be empty"));
    }

    if snippet.code.is_empty() {
        errors.push(ValidationError::field_error("code", "Code cannot be empty"));
    }

    let valid_languages = ["python", "rust", "javascript"];
    if !valid_languages.contains(&snippet.language.as_str()) {
        errors.push(ValidationError::field_error(
            "language",
            "Language must be python, rust, or javascript"
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
```

## Field-Level Validation

Use field validators for specific field constraints:

```rust
use reinhardt::prelude::*;

// String field with length constraints
let title_field = CharField::new("title".to_string())
    .min_length(1)
    .max_length(100);

// Integer field with range constraints
let age_field = IntegerField::new("age".to_string())
    .min_value(0)
    .max_value(150);

// Email validation
let email_field = EmailField::new("email".to_string());

// Validate values
title_field.validate(&"My Snippet".to_string()).unwrap();
age_field.validate(&25).unwrap();
email_field.validate(&"user@example.com".to_string()).unwrap();
```

## JSON Serialization

Convert data to/from JSON:

```rust
use reinhardt::prelude::*;

let serializer = JsonSerializer::<Snippet>::new();

// Serialize to JSON
// #[model] generates Snippet::new(title, code, language) — primary key and
// auto_now_add fields are populated automatically and omitted from the
// constructor signature.
let snippet = Snippet::new(
    "Hello World".to_string(),
    "print('hello')".to_string(),
    "python".to_string(),
);

let json_bytes = serializer.serialize(&snippet).unwrap();
let json_str = String::from_utf8(json_bytes).unwrap();
// json_str: {"id":1,"title":"Hello World",...}

// Deserialize from JSON
let json = r#"{"id":2,"title":"Test","code":"fn main(){}","language":"rust"}"#;
let snippet = serializer.deserialize(json.as_bytes()).unwrap();
```

## Serialization Patterns in Reinhardt

Reinhardt supports multiple serialization approaches. Choose the pattern that
best fits your use case.

### Pattern 1: Simple Serde (Most Common)

For basic validation with standard rules, use serde with Reinhardt's built-in validation:

```rust
use serde::{Serialize, Deserialize};
use reinhardt::Validate;

#[derive(Serialize, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(email)]
    pub email: String,

    #[validate(length(min = 3, max = 20))]
    pub username: String,

    #[validate(range(min = 18, max = 120))]
    pub age: i32,
}

// Use in view
let request: CreateUserRequest = request.json()?;
request.validate()?;  // Validates all fields
```

**When to use:**
- Standard validation rules (email, length, range, regex)
- REST API with JSON
- No complex business logic needed

**Benefits:**
- Simple and declarative
- Works with `#[derive]` macros
- Well-documented validation system

**Split Serializer Pattern (Input + Output):**

For production APIs, separate input validation from output formatting. This pattern uses `SnippetSerializer` for request validation (with Reinhardt's built-in validation) and `SnippetResponse` for response serialization (with a `from_model()` method):

```rust
use serde::{Serialize, Deserialize};
use reinhardt::Validate;

/// Serializer for creating/updating snippets (input + validation)
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SnippetSerializer {
	#[validate(length(
		min = 1,
		max = 100,
		message = "Title must be between 1 and 100 characters"
	))]
	pub title: String,

	#[validate(length(
		min = 1,
		max = 10000,
		message = "Code must be between 1 and 10000 characters"
	))]
	pub code: String,

	#[validate(length(
		min = 1,
		max = 50,
		message = "Language must be between 1 and 50 characters"
	))]
	pub language: String,
}

/// Response serializer for snippets (output + formatting)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetResponse {
	pub id: i64,
	pub title: String,
	pub code: String,
	pub language: String,
	pub highlighted: String,
}

impl SnippetResponse {
	pub fn from_model(snippet: &Snippet) -> Self {
		Self {
			id: snippet.id,
			title: snippet.title.clone(),
			code: snippet.code.clone(),
			language: snippet.language.clone(),
			highlighted: snippet.highlighted(),
		}
	}
}
```

Key advantages of this split pattern:
- **Input validation** is declarative using `#[validate(...)]` attributes
- **Output formatting** is separate, allowing fields like `highlighted` that are computed from the model
- The `from_model()` method provides a clean conversion from database model to API response

---

### Pattern 2: Custom `Serializer` Trait with Validation

For custom serialization with complex validation logic or business rules:

```rust
use reinhardt::prelude::*;

pub struct UserSerializer;

impl Serializer for UserSerializer {
    type Input = User;
    type Output = Vec<u8>;

    fn serialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError> {
        serde_json::to_vec(input).map_err(|e| SerializerError::SerializeError(e.to_string()))
    }

    fn deserialize(&self, output: &Self::Output) -> Result<Self::Input, SerializerError> {
        serde_json::from_slice(output).map_err(|e| SerializerError::DeserializeError(e.to_string()))
    }
}

// For complex business rules, use a separate validation function:
async fn validate_user(user: &User) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Complex business rule: username must not contain admin keywords
    if user.username.to_lowercase().contains("admin") {
        errors.push(ValidationError::field_error(
            "username",
            "Username cannot contain 'admin'"
        ));
    }

    // Database lookup validation (async)
    // Check if username is already taken
    if User::exists_by_username(&user.username).await {
        errors.push(ValidationError::field_error(
            "username",
            "Username already taken"
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
```

**When to use:**
- Complex business rules
- Database lookups in validation
- Custom error messages
- Need async validation

**Benefits:**
- Full control over validation logic
- Can access database or external services
- Custom error formatting

---

### Pattern 3: GraphQL InputObject

For GraphQL APIs, use `async-graphql`'s `InputObject`:

```rust
use async_graphql::InputObject;

#[derive(InputObject)]
pub struct CreateUserInput {
    /// User's email address
    pub email: String,

    /// Username (3-20 characters)
    pub username: String,

    /// User's age
    pub age: i32,
}

// Use in GraphQL mutation
impl Mutation {
    async fn create_user(&self, input: CreateUserInput) -> Result<User> {
        // GraphQL handles deserialization automatically
        User::objects().create(input.username, input.email, input.age).await
    }
}
```

**When to use:**
- Building GraphQL APIs
- Need GraphQL-specific features (field descriptions, deprecation)
- Want auto-generated GraphQL schema

**Benefits:**
- GraphQL schema auto-generation
- Built-in introspection
- Field-level documentation

**Example:** See [examples/examples-github-issues](../../../../examples/examples-github-issues)
for a complete GraphQL implementation with `InputObject`.

---

### Recommendation Summary

| Use Case | Pattern | Crate |
|----------|---------|-------|
| **REST API with standard validation** | Pattern 1 | `serde` + `reinhardt::Validate` |
| **Complex business rules** | Pattern 2 | `reinhardt::Serializer` |
| **GraphQL API** | Pattern 3 | `async-graphql::InputObject` |

**Quick decision tree:**

1. Are you building a GraphQL API?
   - **Yes** → Use Pattern 3 (`InputObject`)
   - **No** → Continue

2. Do you need complex validation (database lookups, custom logic)?
   - **Yes** → Use Pattern 2 (Custom `Serializer`)
   - **No** → Use Pattern 1 (Simple Serde + built-in validation)

For most REST APIs, **Pattern 1** is the recommended starting point.

## Model Serializers

For database models, use `ModelSerializer` with custom validation:

```rust
use reinhardt::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub age: i32,
}

fn validate_user(user: &User) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    if user.username.len() < 3 {
        errors.push(ValidationError::field_error(
            "username",
            "Username must be at least 3 characters"
        ));
    }

    if user.age < 18 {
        errors.push(ValidationError::field_error(
            "age",
            "User must be at least 18 years old"
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// Valid data
let valid_user = User {
    id: 1,
    username: "alice".to_string(),
    email: "alice@example.com".to_string(),
    age: 25,
};
assert!(validate_user(&valid_user).is_ok());

// Invalid data
let invalid_user = User {
    id: 2,
    username: "ab".to_string(),  // Too short
    email: "bob@example.com".to_string(),
    age: 16,  // Too young
};
assert!(validate_user(&invalid_user).is_err());
```

## Nested Serializers

Handle nested data structures:

```rust
use reinhardt::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: i64,
    pub title: String,
    pub author: Author,  // Nested object
}

let article = Article {
    id: 1,
    title: "Introduction to Reinhardt".to_string(),
    author: Author {
        id: 1,
        name: "Alice".to_string(),
    },
};

// Serialize with nested data
let serializer = JsonSerializer::<Article>::new();
let json = serializer.serialize(&article).unwrap();
```

## Validation Workflow

Typical validation workflow in an API view with Reinhardt:

```rust
use json::json;
use reinhardt::core::serde::json;
use reinhardt::ViewResult;
use reinhardt::{post, Json, Response, StatusCode};
use reinhardt::Validate;

#[post("/snippets/", name = "snippets_create")]
async fn create_snippet(
    Json(serializer): Json<SnippetSerializer>,
) -> ViewResult<Response> {
    // 1. Validate the data
    serializer.validate()?;

    // 2. Save to database (using Reinhardt ORM)
    // let snippet = Manager::<Snippet>::new().create(...).await?;

    // Demo mode: construct a mock snippet via the macro-generated
    // constructor. In production the record would be created through
    // Manager::<Snippet>::new().create(...).
    let snippet = Snippet::new(
        serializer.title.clone(),
        serializer.code.clone(),
        serializer.language.clone(),
    );

    // 3. Return response with created status
    let response_data = json!({
        "message": "Snippet created",
        "snippet": SnippetResponse::from_model(&snippet)
    });
    let json = json::to_string(&response_data)?;
    Ok(Response::new(StatusCode::CREATED)
        .with_header("Content-Type", "application/json")
        .with_body(json))
}
```

**Key Points:**
- **Automatic Parsing**: Reinhardt's HTTP method decorators handle Content-Type checking
- **serde_json**: Use `serde_json::from_slice` for JSON deserialization
- **Validation**: Custom validators return `ValidationResult`
- **Response Builder**: Use `.with_json()` for JSON responses

## Summary

In this tutorial, you learned:

1. Using serde for basic serialization/deserialization
2. Implementing custom validation with the `Serializer` trait
3. Field-level validation with built-in field types
4. JSON serialization with `JsonSerializer`
5. Model serializers with custom validation logic
6. Handling nested data structures
7. Complete validation workflow in API views

Next tutorial: [Tutorial 2: Requests and Responses](../2-requests-and-responses/)
