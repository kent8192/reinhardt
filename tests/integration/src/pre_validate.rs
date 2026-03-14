//! Integration tests for pre_validate parameter

#[cfg(test)]
mod tests {
	use reinhardt::post;
	use reinhardt_core::endpoint::EndpointInfo;
	use reinhardt_core::exception::Error;
	use reinhardt_core::validators::Validate;
	use reinhardt_http::Response;
	use rstest::rstest;
	use serde::Deserialize;

	// A validatable request struct
	#[derive(Deserialize, Validate)]
	struct CreateUserRequest {
		#[validate(length(min = 1, max = 100))]
		pub name: String,
		#[validate(email)]
		pub email: String,
	}

	// Handler with pre_validate = true
	#[post("/users", pre_validate = true)]
	async fn create_user_validated(
		body: reinhardt::Json<CreateUserRequest>,
	) -> Result<Response, Error> {
		let _ = body;
		Ok(Response::new(hyper::StatusCode::CREATED))
	}

	// Handler without pre_validate (default = false)
	#[post("/users-no-validate")]
	async fn create_user_no_validate(
		body: reinhardt::Json<CreateUserRequest>,
	) -> Result<Response, Error> {
		let _ = body;
		Ok(Response::new(hyper::StatusCode::CREATED))
	}

	// Handler with pre_validate = true and use_inject = true
	#[post("/users-inject", pre_validate = true, use_inject = true)]
	async fn create_user_with_inject(
		body: reinhardt::Json<CreateUserRequest>,
	) -> Result<Response, Error> {
		let _ = body;
		Ok(Response::new(hyper::StatusCode::CREATED))
	}

	#[rstest]
	fn test_pre_validate_view_has_correct_path() {
		// Arrange & Act
		let path = CreateUserValidatedView::path();

		// Assert
		assert_eq!(path, "/users");
	}

	#[rstest]
	fn test_pre_validate_view_has_correct_method() {
		// Arrange & Act
		let method = CreateUserValidatedView::method();

		// Assert
		assert_eq!(method, reinhardt::Method::POST);
	}

	#[rstest]
	fn test_no_validate_view_has_correct_path() {
		// Arrange & Act
		let path = CreateUserNoValidateView::path();

		// Assert
		assert_eq!(path, "/users-no-validate");
	}

	#[rstest]
	fn test_inject_variant_view_has_correct_path() {
		// Arrange & Act
		let path = CreateUserWithInjectView::path();

		// Assert
		assert_eq!(path, "/users-inject");
	}

	#[rstest]
	fn test_factory_function_returns_view_instance() {
		// Arrange & Act
		let _view: CreateUserValidatedView = create_user_validated();

		// Assert - verify the factory function returns the correct View type
		assert_eq!(CreateUserValidatedView::path(), "/users");
	}

	#[rstest]
	fn test_validator_rejects_empty_name() {
		// Arrange
		let request = CreateUserRequest {
			name: String::new(),
			email: "user@example.com".to_string(),
		};

		// Act
		let result = request.validate();

		// Assert
		assert!(result.is_err());
		let errors = result.unwrap_err();
		assert!(errors.field_errors().contains_key("name"));
	}

	#[rstest]
	fn test_validator_rejects_invalid_email() {
		// Arrange
		let request = CreateUserRequest {
			name: "Alice".to_string(),
			email: "not-an-email".to_string(),
		};

		// Act
		let result = request.validate();

		// Assert
		assert!(result.is_err());
		let errors = result.unwrap_err();
		assert!(errors.field_errors().contains_key("email"));
	}

	#[rstest]
	fn test_validator_accepts_valid_request() {
		// Arrange
		let request = CreateUserRequest {
			name: "Alice".to_string(),
			email: "alice@example.com".to_string(),
		};

		// Act
		let result = request.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_validation_error_converts_to_reinhardt_error() {
		// Arrange
		let request = CreateUserRequest {
			name: String::new(),
			email: "invalid".to_string(),
		};
		let validation_errors = request.validate().unwrap_err();

		// Act
		let error: Error = validation_errors.into();

		// Assert
		assert_eq!(error.status_code(), 400);
		assert_eq!(
			error.kind(),
			reinhardt_core::exception::ErrorKind::Validation
		);
	}
}
