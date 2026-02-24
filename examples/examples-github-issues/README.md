# examples-github-issues

A Reinhardt project.

## Getting Started

### Using cargo-make (Recommended)

Install cargo-make:
```bash
cargo install cargo-make
```

Run the development server:
```bash
cargo make runserver
```

### Using manage command

```bash
# Run the development server
cargo run --bin manage runserver

# Run migrations
cargo run --bin manage migrate

# Create a new app
cargo run --bin manage startapp myapp
```

## Common Tasks

### Development

```bash
cargo make dev              # Run checks + build + start server
cargo make runserver-watch  # Start server with auto-reload
```

### Database

```bash
cargo make makemigrations   # Create new migrations
cargo make migrate          # Apply migrations
```

### Testing

```bash
cargo make test             # Run all tests
cargo make test-watch       # Run tests with auto-reload
```

### Code Quality

```bash
cargo make quality          # Run all checks (format + lint)
cargo make quality-fix      # Fix all issues automatically
```

### Help

```bash
cargo make help             # Show all available tasks
```

## Generated with

This project was created using `reinhardt-admin startproject`.
