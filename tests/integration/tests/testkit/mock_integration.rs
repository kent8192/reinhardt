use bytes::Bytes;
use http::StatusCode;
use reinhardt_http::Handler;
use reinhardt_test::SimpleHandler;
use rstest::rstest;

#[rstest]
#[tokio::test]
async fn simple_handler_delegates_real_request_to_closure() {
	// Arrange
	let handler = SimpleHandler::new(|request: reinhardt_http::Request| {
		Ok(reinhardt_http::Response::ok().with_body(request.uri.path().to_string()))
	});
	let request = reinhardt_http::Request::builder()
		.uri("/mocked")
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let response = handler.handle(request).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	assert_eq!(response.body, Bytes::from_static(b"/mocked"));
}
