//! Views module for {{ app_name }} app (RESTful)
// Add your view submodules here. Each `pub mod` declaration
// corresponds to a file under the `views/` directory.
//
// For multi-file views that need re-exports for discovery, use:
// flatten_imports! {
//     pub mod example;
// }
//
// Example of a JWT-protected endpoint using typed `JwtError` (rc.15+):
//
// use reinhardt::{get, JwtAuth, JwtError, Response, StatusCode};
// use reinhardt::http::ViewResult;
// use axum::extract::Query;
// use std::collections::HashMap;
//
// #[get("/protected/", name = "{{ app_name }}_protected")]
// pub async fn protected(
//     Query(params): Query<HashMap<String, String>>,
// ) -> ViewResult<Response> {
//     let token = params.get("token").ok_or("missing token")?;
//     let jwt = JwtAuth::new(b"your_secret"); // load from settings in practice
//     match jwt.verify_token(token) {
//         Ok(claims) => Ok(Response::new(StatusCode::OK).with_body(claims.username)),
//         Err(JwtError::TokenExpired) => {
//             Ok(Response::new(StatusCode::UNAUTHORIZED).with_body("Token expired"))
//         }
//         Err(JwtError::InvalidSignature(_)) => {
//             Ok(Response::new(StatusCode::UNAUTHORIZED).with_body("Invalid signature"))
//         }
//         Err(e) => Ok(Response::new(StatusCode::INTERNAL_SERVER_ERROR).with_body(e.to_string())),
//     }
// }
