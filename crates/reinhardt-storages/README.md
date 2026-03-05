# reinhardt-storages

Cloud storage backend abstraction for the Reinhardt framework, inspired by [django-storages](https://django-storages.readthedocs.io/).

## Features

- **Unified API**: Single `` `StorageBackend` `` trait for all storage providers
- **Async I/O**: All operations are asynchronous using Tokio
- **Feature Flags**: Enable only the backends you need
- **Presigned URLs**: Generate temporary access URLs for secure file sharing
- **Multiple Backends**:
  - Amazon S3 (✅ Implemented)
  - Google Cloud Storage (🚧 Planned)
  - Azure Blob Storage (🚧 Planned)
  - Local File System (✅ Implemented)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
reinhardt-storages = "0.1.0"
```

### Feature Flags

By default, `reinhardt-storages` enables `s3` and `local` backends. You can customize this:

```toml
[dependencies]
# Only local storage
reinhardt-storages = { version = "0.1.0", default-features = false, features = ["local"] }

# S3 only
reinhardt-storages = { version = "0.1.0", default-features = false, features = ["s3"] }

# All available backends
reinhardt-storages = { version = "0.1.0", features = ["all"] }
```

Available features:
- `default`: `["s3", "local"]`
- `s3`: Amazon S3 support
- `gcs`: Google Cloud Storage support (not yet implemented)
- `azure`: Azure Blob Storage support (not yet implemented)
- `local`: Local file system support
- `all`: All backends

## Usage

### Basic Example

```rust
use reinhardt_storages::{StorageBackend, create_storage, StorageConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment
    let config = StorageConfig::from_env()?;

    // Create storage backend
    let storage = create_storage(config).await?;

    // Save a file
    let data = b"Hello, world!";
    let path = storage.save("example.txt", data).await?;
    println!("File saved to: {}", path);

    // Read a file
    let content = storage.open("example.txt").await?;
    println!("File content: {}", String::from_utf8_lossy(&content));

    // Check if file exists
    if storage.exists("example.txt").await? {
        // Get file size
        let size = storage.size("example.txt").await?;
        println!("File size: {} bytes", size);

        // Get presigned URL (valid for 1 hour)
        let url = storage.url("example.txt", 3600).await?;
        println!("Presigned URL: {}", url);

        // Delete the file
        storage.delete("example.txt").await?;
    }

    Ok(())
}
```

### Using Local Storage

```rust
use reinhardt_storages::{StorageBackend, StorageConfig};
use reinhardt_storages::config::LocalConfig;
use reinhardt_storages::backends::local::LocalStorage;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create local storage backend
    let config = LocalConfig {
        base_path: "/tmp/storage".to_string(),
    };
    let storage = LocalStorage::new(config)?;

    // Use the storage
    storage.save("test.txt", b"Hello!").await?;

    Ok(())
}
```

### Using S3 Storage

```rust
use reinhardt_storages::{StorageBackend, StorageConfig};
use reinhardt_storages::config::S3Config;
use reinhardt_storages::backends::s3::S3Storage;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create S3 storage backend
    let config = S3Config {
        bucket: "my-bucket".to_string(),
        region: Some("us-east-1".to_string()),
        endpoint: None, // Use default AWS endpoint
        prefix: Some("uploads/".to_string()), // Optional path prefix
    };
    let storage = S3Storage::new(config).await?;

    // Use the storage
    storage.save("file.txt", b"Hello from S3!").await?;

    // Generate presigned URL (valid for 1 hour)
    let url = storage.url("file.txt", 3600).await?;
    println!("Download URL: {}", url);

    Ok(())
}
```

## Configuration

### Environment Variables

`reinhardt-storages` supports loading configuration from environment variables:

```bash
# Backend selection
export STORAGE_BACKEND=s3  # or: local, gcs, azure

# S3 Configuration
export S3_BUCKET=my-bucket
export S3_REGION=us-east-1
export S3_ENDPOINT=http://localhost:4566  # Optional (for LocalStack)
export S3_PREFIX=uploads/                  # Optional

# Local Configuration
export LOCAL_BASE_PATH=/var/storage

# GCS Configuration (not yet implemented)
export GCS_BUCKET=my-bucket
export GCS_PREFIX=uploads/

# Azure Configuration (not yet implemented)
export AZURE_ACCOUNT=myaccount
export AZURE_CONTAINER=mycontainer
export AZURE_PREFIX=uploads/
```

Then load the configuration:

```rust
use reinhardt_storages::{StorageConfig, create_storage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = StorageConfig::from_env()?;
    let storage = create_storage(config).await?;
    Ok(())
}
```

## API Reference

### `StorageBackend` Trait

All storage backends implement this trait:

```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Save a file to storage
    async fn save(&self, name: &str, content: &[u8]) -> Result<String>;

    /// Open (read) a file from storage
    async fn open(&self, name: &str) -> Result<Vec<u8>>;

    /// Delete a file from storage
    async fn delete(&self, name: &str) -> Result<()>;

    /// Check if a file exists
    async fn exists(&self, name: &str) -> Result<bool>;

    /// Generate a URL for accessing the file
    async fn url(&self, name: &str, expiry_secs: u64) -> Result<String>;

    /// Get the file size in bytes
    async fn size(&self, name: &str) -> Result<u64>;

    /// Get the file's last modified timestamp
    async fn get_modified_time(&self, name: &str) -> Result<DateTime<Utc>>;
}
```

## Development Status

- ✅ **Local Storage**: Fully implemented
- ✅ **S3 Storage**: Fully implemented
- 🚧 **Google Cloud Storage**: Planned for Phase 2
- 🚧 **Azure Blob Storage**: Planned for Phase 2

## Testing

Run tests with:

```bash
# All tests
cargo test

# Local storage tests only
cargo test --test local_storage

# S3 integration tests (requires Docker for LocalStack)
cargo test --test s3_storage
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../../LICENSE-MIT))

at your option.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.
