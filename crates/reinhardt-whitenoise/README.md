# reinhardt-whitenoise

WhiteNoise-style static file optimization for Reinhardt web framework.

## Features

- **Compression**: Automatic gzip and brotli compression
- **Content Hashing**: MD5-based filename hashing (Django compatible)
- **Cache Control**: Intelligent caching with immutable detection
- **ETag Support**: Conditional requests with 304 Not Modified
- **Content Negotiation**: Accept-Encoding based variant selection

## Quick Start

```rust
use reinhardt_whitenoise::{WhiteNoiseConfig, WhiteNoiseMiddleware};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure WhiteNoise
    let config = WhiteNoiseConfig::new(
        PathBuf::from("static"),
        "/static/".to_string(),
    )
    .with_compression(true, true)
    .with_max_age_immutable(31536000);

    // Initialize middleware
    let middleware = WhiteNoiseMiddleware::new(config).await?;

    Ok(())
}
```

## Documentation

See [documentation](https://docs.rs/reinhardt-whitenoise) for more details.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE))
- MIT License ([LICENSE-MIT](../../LICENSE-MIT))

at your option.
