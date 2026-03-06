//! Integration tests for pre_validate parameter

#[cfg(test)]
mod tests {
	use reinhardt::post;
	use reinhardt_core::exception::Error;
	use reinhardt_http::Response;
	use rstest::rstest;
	use serde::Deserialize;
	use validator::Validate;

	// A validatable request struct
	#[derive(Deserialize, Validate)]
	struct CreateUserRequest {
		#[validate(length(min = 1, max = 100))]
		pub name: String,
		#[validate(email)]
		pub email: String,
	}

	// Test that pre_validate = true compiles correctly with extractor
	// Note: This test verifies macro expansion, not runtime behavior.
	// Runtime behavior requires a full HTTP pipeline which is tested via E2E tests.
	#[post("/users", pre_validate = true)]
	async fn create_user_validated(
		body: reinhardt::Json<CreateUserRequest>,
	) -> Result<Response, Error> {
		let _ = body;
		Ok(Response::new(hyper::StatusCode::CREATED))
	}

	// Test that pre_validate = false (default) compiles correctly
	#[post("/users-no-validate")]
	async fn create_user_no_validate(
		body: reinhardt::Json<CreateUserRequest>,
	) -> Result<Response, Error> {
		let _ = body;
		Ok(Response::new(hyper::StatusCode::CREATED))
	}

	// Test that pre_validate = true compiles with use_inject
	#[post("/users-inject", pre_validate = true, use_inject = true)]
	async fn create_user_with_inject(
		body: reinhardt::Json<CreateUserRequest>,
	) -> Result<Response, Error> {
		let _ = body;
		Ok(Response::new(hyper::StatusCode::CREATED))
	}

	#[rstest]
	fn test_pre_validate_macro_generates_view_types() {
		// Arrange & Act
		// Verify that the macro generates the expected View types
		let _validated = create_user_validated();
		let _no_validate = create_user_no_validate();
		let _with_inject = create_user_with_inject();

		// Assert - compilation success proves macro works correctly
		assert!(
			true,
			"All pre_validate macro variants compiled successfully"
		);
	}
}
