# Reinhardt Basis Tutorial Example - Polling Application

This example demonstrates the concepts covered in the [Reinhardt Basis Tutorial](../../../docs/tutorials/en/basis/). It implements a complete polling application with questions and choices.

## What This Example Covers

This example corresponds to the basis tutorial parts 1-7:

- **Part 1: Project Setup** - Project structure, development server, first views
- **Part 2: Models and Database** - Database configuration, ORM models, admin panel
- **Part 3: Views and URLs** - View functions, URL routing, templates
- **Part 4: Forms and Generic Views** - HTML forms, form processing, generic views
- **Part 5: Testing** - Automated testing, model and view tests
- **Part 6: Static Files** - CSS, images, static file management
- **Part 7: Admin Customization** - Admin interface customization

## Features

### Models

- **Question**: Represents a poll question with publication date
- **Choice**: Represents an answer option with vote count

### Views

- **Index**: List all available polls
- **Detail**: Display a specific poll with voting form
- **Results**: Show poll results with vote counts
- **Vote**: Handle vote submission

### URL Structure

```
/polls/                      - List all polls (index)
/polls/<question_id>/        - Poll detail with voting form
/polls/<question_id>/results/  - Poll results
/polls/<question_id>/vote/     - Vote submission (POST)
```

## Setup

### Prerequisites

- Rust 1.75 or later
- PostgreSQL (optional, for database features)
- Docker (optional, for TestContainers in tests)

### Installation

```bash
# From the project root
cd examples/examples-tutorial-basis

# Build the project
cargo build

# Run tests
cargo test
```

## Usage

### Run the Development Server

```bash
cargo run --bin manage runserver
```

The server will start at `http://127.0.0.1:8000/`.

### Available Endpoints

```bash
# List all polls
curl http://127.0.0.1:8000/polls/

# Get specific poll
curl http://127.0.0.1:8000/polls/1/

# Get poll results
curl http://127.0.0.1:8000/polls/1/results/

# Submit a vote
curl -X POST http://127.0.0.1:8000/polls/1/vote/ \
  -H "Content-Type: application/json" \
  -d '{"choice_id": 1}'
```

## Project Structure

```
examples-tutorial-basis/
├── Cargo.toml                 # Project configuration
├── build.rs                   # Build script
├── README.md                  # This file
├── src/
│   ├── lib.rs                # Library entry point
│   ├── config.rs             # Config module
│   ├── apps.rs               # Apps module
│   ├── bin/
│   │   └── manage.rs         # Management command
│   ├── config/
│   │   ├── settings.rs       # Settings configuration
│   │   ├── urls.rs           # URL routing
│   │   └── apps.rs           # Installed apps
│   └── apps/
│       └── polls/
│           ├── lib.rs        # App module
│           ├── models.rs     # Question and Choice models
│           ├── views.rs      # View handlers
│           └── urls.rs       # URL patterns
└── tests/
    ├── integration.rs        # Integration tests
    └── availability.rs       # Availability tests
```

## Learning Path

This example is designed to be studied alongside the basis tutorial:

1. **Start with the tutorial**: Read [Part 1](../../../docs/tutorials/en/basis/1-project-setup.md)
2. **Examine the code**: Look at how concepts are implemented in this example
3. **Run the tests**: `cargo test` to see the functionality in action
4. **Experiment**: Modify the code and see what happens

## Key Concepts Demonstrated

### 1. Models (models.rs)

```rust
pub struct Question {
    pub id: Option<i64>,
    pub question_text: String,
    pub pub_date: DateTime<Utc>,
}

pub struct Choice {
    pub id: Option<i64>,
    pub question_id: i64,
    pub choice_text: String,
    pub votes: i32,
}
```

### 2. Views (views.rs)

```rust
pub async fn index(_req: Request) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
    // List all polls
}

pub async fn detail(req: Request) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
    // Show specific poll
}
```

### 3. URL Routing (urls.rs)

```rust
UnifiedRouter::new()
    .function("/", Method::GET, super::views::index)
    .function("/{question_id}/", Method::GET, super::views::detail)
    .function("/{question_id}/results/", Method::GET, super::views::results)
    .function("/{question_id}/vote/", Method::POST, super::views::vote)
```

### 4. Configuration (config/)

- **settings.rs**: Application settings
- **apps.rs**: Installed apps registry
- **urls.rs**: Top-level URL configuration

## Testing

Run the test suite:

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_question_model

# Run with output
cargo test -- --nocapture
```

## Next Steps

After understanding this example:

1. **Extend the models**: Add user authentication, comments, or tags
2. **Add database integration**: Implement actual database storage
3. **Create templates**: Add HTML templates for views
4. **Customize admin**: Create custom admin interface
5. **Add static files**: Include CSS and JavaScript

## Related Documentation

- [Basis Tutorial](../../../docs/tutorials/en/basis/) - Step-by-step guide
- [Feature Flags Guide](../../../docs/FEATURE_FLAGS.md) - Available features
- [Getting Started](../../../docs/GETTING_STARTED.md) - Quick start guide
- [API Documentation](https://docs.rs/reinhardt) - Complete API reference

## License

This example is part of the Reinhardt project and is licensed under the BSD 3-Clause License.
