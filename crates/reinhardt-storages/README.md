# reinhardt-storages

Cloud storage backend abstraction for the Reinhardt framework, inspired by
[django-storages](https://django-storages.readthedocs.io/).

## Features

- **Unified API**: single `StorageBackend` trait for all providers
- **Settings-first configuration**: `StorageSettings` composes with the
  Reinhardt `#[settings]` macro
- **Async I/O**: all storage operations are asynchronous
- **Feature flags**: enable only the providers your application uses
- **Temporary URLs**: presigned URLs for S3, V4 signed URLs for GCS, and SAS
  URLs for Azure Blob Storage
- **Provider boundary**: S3 uses `reinhardt-providers` for the minimal HTTP and
  SigV4 operations required by this crate
- **Backends**:
  - Amazon S3
  - Google Cloud Storage
  - Azure Blob Storage
  - Local file system

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
reinhardt-storages = "0.4.0-alpha.6"
```

### Feature Flags

The compatibility default enables the `s3` and `local` backends. For an
application build, prefer disabling defaults and selecting the one provider
that the application uses. This keeps the dependency graph and the runtime
configuration aligned.

```toml
[dependencies]
# Only local storage
reinhardt-storages = { version = "0.4.0-alpha.6", default-features = false, features = ["local"] }

# S3 only
reinhardt-storages = { version = "0.4.0-alpha.6", default-features = false, features = ["s3"] }
```

Available features:

- `default`: `["s3", "local"]`
- `s3`: Amazon S3 support
- `gcs`: Google Cloud Storage support
- `azure`: Azure Blob Storage support
- `local`: local file system support
- `all`: all backends (use for provider-matrix or compatibility tests, not an
  application default)

Each `StorageSettings` entry selects exactly one backend with its `backend`
value. Enabling several provider features does not make one entry multi-cloud;
it only makes those implementations available for settings validation. The
facade's `file-storage-local`, `file-storage-s3`, `file-storage-gcs`, and
`file-storage-azure` features make this one-provider choice explicit for root
applications. None is included in the `standard` or `full` presets.

## Usage

### Settings-First Example

```rust
use reinhardt_storages::{StorageSettings, create_storage_from_settings};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings: StorageSettings = toml::from_str(r#"
backend = "local"

[local]
base_path = "media"
"#)?;

    let storage = create_storage_from_settings(&settings).await?;

    storage.save("example.txt", b"Hello, world!").await?;
    let content = storage.open("example.txt").await?;
    println!("File content: {}", String::from_utf8_lossy(&content));

    if storage.exists("example.txt").await? {
        let size = storage.size("example.txt").await?;
        println!("File size: {size} bytes");

        let url = storage.url("example.txt", 3600).await?;
        println!("Temporary URL: {url}");

        storage.delete("example.txt").await?;
    }

    Ok(())
}
```

### Composed Application Settings

`StorageSettings` is a fragment for the `[storage]` section. Applications can
compose it with other settings fragments through `#[settings]`.

```rust
use reinhardt_core::macros::settings;
use reinhardt_storages::StorageSettings;

#[settings(storage: StorageSettings)]
pub struct AppSettings {}
```

### Default and named storage entries

The existing `[storage]` section is preserved as the `default` alias. Add
named aliases under `[storage.named.<alias>]`; each alias has an independent
backend and URL expiry. Omitted `url_expiry_secs` defaults to 3,600 seconds.
Named entries are intentionally not recursive.

```toml
[storage]
backend = "local"
url_expiry_secs = 3600

[storage.local]
base_path = "media"

[storage.named.private_uploads]
backend = "local"
url_expiry_secs = 900

[storage.named.private_uploads.local]
base_path = "private-media"
```

The same shape works for a cloud alias. For example, compile only the S3
provider with `default-features = false, features = ["s3"]` and configure the
default backend as follows:

```toml
[storage]
backend = "s3"
url_expiry_secs = 3600

[storage.s3]
bucket = "my-bucket"
region = "us-east-1"
prefix = "uploads/"
```

The default alias and every alias referenced by a model `FileField` must point
to a backend with atomic exclusive creation. Startup validation rejects a
missing alias or a backend whose `StorageCapabilities::exclusive_create` is
false. `save_if_absent` is the collision-safe operation; `save` keeps its
existing overwrite semantics. Local, S3, GCS, and Azure backends implement the
exclusive operation.

Example TOML for Google Cloud Storage:

```toml
[storage]
backend = "gcs"

[storage.gcs]
bucket = "my-bucket"
prefix = "uploads/"
service_account_json = { secret = "{\"client_email\":\"storage@example.com\"}" }
```

Example TOML for Azure Blob Storage:

```toml
[storage]
backend = "azure"

[storage.azure]
account = "myaccount"
container = "media"
prefix = "uploads/"
access_key = { secret = "base64-account-key" }
```

Example TOML for local storage:

```toml
[storage]
backend = "local"

[storage.local]
base_path = "media"
```

## Backend Settings

### S3

```toml
[storage]
backend = "s3"

[storage.s3]
bucket = "my-bucket"
region = "us-east-1"
endpoint = "http://localhost:4566"
prefix = "uploads/"
```

`endpoint` and `prefix` are optional. S3 credentials are resolved through the
AWS SDK default provider chain. The signing region uses `[storage.s3].region`
when configured and falls back to the default provider chain only when omitted.
Object operations are sent through the minimal `reinhardt-providers` S3 client
instead of `aws-sdk-s3`.

### Google Cloud Storage

```toml
[storage]
backend = "gcs"

[storage.gcs]
bucket = "my-bucket"
prefix = "uploads/"
service_account_json = { secret = "{\"type\":\"service_account\"}" }
```

`endpoint` is available for emulators such as fake-gcs-server. Without
`endpoint`, the Google Cloud SDK client is used. `service_account_json` is
optional and can provide explicit credentials and local signing material for
V4 signed URLs; otherwise Application Default Credentials are used.

### Azure Blob Storage

```toml
[storage]
backend = "azure"

[storage.azure]
account = "myaccount"
container = "media"
prefix = "uploads/"
access_key = { secret = "base64-account-key" }
```

`endpoint` is available for Azurite or custom endpoints. A configured
`sas_token` is used only as a backend operation credential and is never returned
from temporary URL generation. Temporary URLs require `access_key` or
`connection_string` so Reinhardt can generate a per-blob read-only service SAS
URL with the requested expiry.

### Local

```toml
[storage]
backend = "local"

[storage.local]
base_path = "/var/storage"
```

## Compatibility API

`StorageConfig`, `S3Config`, `GcsConfig`, `AzureConfig`, `LocalConfig`, and
`StorageConfig::from_env()` are deprecated in favor of `StorageSettings`.
They remain available during the compatibility window so existing applications
can migrate incrementally.

```rust
use reinhardt_storages::{StorageSettings, create_storage_from_settings};

async fn build_storage(settings: &StorageSettings) -> reinhardt_storages::Result<()> {
    let storage = create_storage_from_settings(settings).await?;
    storage.save("example.txt", b"content").await?;
    Ok(())
}
```

## Lower-level model upload API

The storage crate supplies the backend, registry, and collision-safe upload
primitive used by the opt-in ORM file-field integration. `store_uploaded_file`
writes one object eagerly and returns its logical path plus storage alias; it
does not know about a database transaction, an old committed value, or image
validation.

```rust,no_run
use reinhardt_core::parsers::UploadedFile;
use reinhardt_storages::{store_uploaded_file, StorageRegistry, UploadPolicy};

async fn store_avatar(
    registry: &StorageRegistry,
) -> Result<(), reinhardt_storages::FileStorageError> {
    let upload = UploadedFile::new("avatar".to_owned(), b"bytes".to_vec().into())
        .with_filename("avatar.txt".to_owned());
    let stored = store_uploaded_file(
        registry,
        UploadPolicy {
            model: "Profile",
            field: "avatar",
            upload_to: "avatars/%Y/%m/%d",
            storage_alias: "default",
            max_length: 255,
        },
        upload,
    )
    .await?;
    assert_eq!(stored.storage_alias, "default");
    Ok(())
}
```

For storage-backed model mutations, use `ModelFileField` or `ModelImageField`
from `reinhardt-db`. Their lifecycle methods compensate new files when
validation, storage, or the caller-owned database closure fails, then perform
best-effort cleanup of old committed files after database success. Cleanup
failures are logged and do not replace the database result. `ImageField`
validation is owned by the database layer: it requires a matching supported
raster filename/format, rejects corrupt, unknown, and SVG uploads, applies
inclusive dimension limits, and preserves original bytes without transforms.
Multipart parsing, forms, and admin integration are owned by their respective
layers rather than this lower-level storage API.

## API Reference

All storage backends implement `StorageBackend`:

```rust
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn save(&self, name: &str, content: &[u8]) -> Result<String>;
    async fn open(&self, name: &str) -> Result<Vec<u8>>;
    async fn delete(&self, name: &str) -> Result<()>;
    async fn exists(&self, name: &str) -> Result<bool>;
    async fn url(&self, name: &str, expiry_secs: u64) -> Result<String>;
    async fn size(&self, name: &str) -> Result<u64>;
    async fn get_modified_time(&self, name: &str) -> Result<DateTime<Utc>>;
}
```

## Testing

Run tests with:

```bash
# All storage tests
cargo test -p reinhardt-storages --all-features

# GCS emulator tests with fake-gcs-server
cargo test -p reinhardt-storages --features gcs,local --test gcs_tests -- --nocapture

# Azure emulator tests with Azurite
cargo test -p reinhardt-storages --features azure,local --test azure_tests -- --nocapture
```

GCS and Azure emulator tests use TestContainers and require Docker.

## License

MIT OR Apache-2.0
