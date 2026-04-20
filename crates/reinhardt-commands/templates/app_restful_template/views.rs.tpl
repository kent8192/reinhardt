//! Views module for {{ app_name }} app (RESTful)
// Add your view submodules here. Each `pub mod` declaration
// corresponds to a file under the `views/` directory.
//
// For multi-file views that need re-exports for discovery, use:
// flatten_imports! {
//     pub mod example;
// }
//
// Example of a JWT-protected endpoint using `JwtError` (rc.15+):
//
// use reinhardt::{get, JwtAuth, JwtError};
// use reinhardt::http::Request;
//
// #[get("/protected")]
// pub async fn protected_view(req: Request) -> Result<String, String> {
//     let auth = JwtAuth::from_request(&req)?;
//     match auth.verify() {
//         Ok(claims) => Ok(format!("Hello, {}!", claims.username)),
//         Err(JwtError::TokenExpired) => Err("Token has expired".to_string()),
//         Err(JwtError::InvalidSignature(_)) => Err("Invalid token signature".to_string()),
//         Err(e) => Err(format!("Authentication error: {}", e)),
//     }
// }
