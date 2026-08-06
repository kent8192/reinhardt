# reinhardt-providers

Cloud provider integrations for the Reinhardt framework.

This crate contains small provider-specific utilities used by higher-level
Reinhardt crates. It is not a replacement for the full AWS or Google Cloud
SDKs. Implementations are added only when Reinhardt needs a narrow provider
operation and the full SDK surface is too heavy or too unstable for that path.

## Features

- `aws`: AWS helpers, currently including a minimal S3 HTTP/SigV4 client

## AWS S3

The S3 client supports the object operations required by
`reinhardt-storages`:

- `PUT Object`
- `GET Object`
- `DELETE Object`
- `HEAD Object`
- presigned `GET` URLs

Credentials and region are loaded through `aws-config`, preserving the AWS SDK
default provider chain without constructing an `aws-sdk-s3` service client.

### Credentials and endpoints

`S3ClientConfig` accepts either static `AwsCredentials` or the AWS SDK default
credential provider chain. Custom S3-compatible endpoints use path-style
addressing and preserve any endpoint base path when constructing object URLs.

Presigned `GET` URLs use SigV4 and accept expirations up to the S3 limit of
seven days. Object operations map missing objects, permission failures, and
other service responses to the corresponding `ProviderError` variants.
