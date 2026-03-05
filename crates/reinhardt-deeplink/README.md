# reinhardt-deeplink

Mobile deep linking support for the Reinhardt web framework.

## Features

- **iOS Universal Links**: Automatically generates Apple App Site Association (AASA) files
- **Android App Links**: Automatically generates Digital Asset Links (assetlinks.json) files
- **Custom URL Schemes**: Configuration helpers for custom URL schemes (e.g., `myapp://`)
- **Router Integration**: Seamless integration with `UnifiedRouter` and `ServerRouter`
- **Type-Safe Builders**: Fluent API for building configurations

## Quick Start

```rust
use reinhardt_deeplink::{DeeplinkConfig, IosConfig, AndroidConfig, DeeplinkRouterExt};
use reinhardt_urls::routers::UnifiedRouter;

// Configure deep links
let config = DeeplinkConfig::builder()
    .ios(
        IosConfig::builder()
            .app_id("TEAM123456.com.example.app")
            .paths(&["/products/*", "/users/*"])
            .exclude_paths(&["/api/*"])
            .build()
    )
    .android(
        AndroidConfig::builder()
            .package_name("com.example.app")
            .sha256_fingerprint("FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C")
            .build()
            .unwrap()
    )
    .build();

// Add to router
let router = UnifiedRouter::new()
    .with_deeplinks(config)
    .unwrap();
```

## Endpoints

This crate automatically registers the following endpoints:

| Endpoint | Description |
|----------|-------------|
| `GET /.well-known/apple-app-site-association` | iOS Universal Links |
| `GET /.well-known/apple-app-site-association.json` | iOS Universal Links (alternative) |
| `GET /.well-known/assetlinks.json` | Android App Links |

## iOS Configuration

### Basic Usage

```rust
use reinhardt_deeplink::IosConfig;

let config = IosConfig::builder()
    .app_id("TEAM123456.com.example.app")
    .paths(&["/products/*", "/users/*"])
    .build();
```

### With Web Credentials

```rust
use reinhardt_deeplink::IosConfig;

let config = IosConfig::builder()
    .app_id("TEAM123456.com.example.app")
    .paths(&["/"])
    .with_web_credentials()
    .build();
```

### With App Clips

```rust
use reinhardt_deeplink::IosConfig;

let config = IosConfig::builder()
    .app_id("TEAM123456.com.example.app")
    .paths(&["/"])
    .app_clip("TEAM123456.com.example.app.Clip")
    .build();
```

### iOS 13+ Component Matching

```rust
use reinhardt_deeplink::{IosConfig, AppLinkComponent};

let config = IosConfig::builder()
    .app_id("TEAM123456.com.example.app")
    .component(AppLinkComponent {
        path: "/products/*".to_string(),
        query: Some("ref=*".to_string()),
        fragment: None,
        exclude: None,
        comment: Some("Product pages with referral".to_string()),
    })
    .build();
```

## Android Configuration

### Basic Usage

```rust
use reinhardt_deeplink::AndroidConfig;

let config = AndroidConfig::builder()
    .package_name("com.example.app")
    .sha256_fingerprint("FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C")
    .build()
    .unwrap();
```

### Multiple Fingerprints

```rust
use reinhardt_deeplink::AndroidConfig;

let config = AndroidConfig::builder()
    .package_name("com.example.app")
    .sha256_fingerprints(&[
        "FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C",
        "00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00",
    ])
    .build()
    .unwrap();
```

## Custom URL Schemes

```rust
use reinhardt_deeplink::CustomSchemeConfig;

let config = CustomSchemeConfig::builder()
    .scheme("myapp")
    .host("open")
    .paths(&["/products/*", "/users/*"])
    .build();

// Generates URL template: myapp://open/products/*
```

## License

Licensed under the BSD 3-Clause License.
