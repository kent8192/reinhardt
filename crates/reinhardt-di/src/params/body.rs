//! Raw body extraction

use async_trait::async_trait;
use reinhardt_http::Request;

use super::{ParamContext, ParamError, ParamResult, extract::FromRequest};

/// Default maximum body size: 2 MiB
const DEFAULT_MAX_BODY_SIZE: usize = 2 * 1024 * 1024;

/// Extract the raw request body as bytes
///
/// Enforces a maximum body size of 2 MiB to prevent memory exhaustion.
/// Requests exceeding this limit are rejected with `PayloadTooLarge`.
///
/// # Examples
///
/// ```
/// use reinhardt_di::params::Body;
///
/// let body = Body(vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]); // "Hello"
/// assert_eq!(body.0, vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]);
/// ```
pub struct Body(pub Vec<u8>);

#[async_trait]
impl FromRequest for Body {
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		// Extract raw body bytes from request
		let body_bytes = req
			.read_body()
			.map_err(|e| ParamError::BodyError(format!("Failed to read body: {}", e)))?;

		// Enforce body size limit to prevent memory exhaustion
		if body_bytes.len() > DEFAULT_MAX_BODY_SIZE {
			return Err(ParamError::PayloadTooLarge(format!(
				"Request body size {} bytes exceeds maximum allowed size of {} bytes",
				body_bytes.len(),
				DEFAULT_MAX_BODY_SIZE
			)));
		}

		Ok(Body(body_bytes.to_vec()))
	}
}
