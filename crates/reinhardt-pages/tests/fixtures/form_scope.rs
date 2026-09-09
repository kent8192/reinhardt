//! Keeps an owning reactive scope active whenever an asynchronous test is polled.

pub(crate) async fn run<F: std::future::Future>(future: F) -> F::Output {
	let scope = reinhardt_pages::reactive::ReactiveScope::new();
	let mut future = std::pin::pin!(future);
	std::future::poll_fn(|context| scope.enter(|| future.as_mut().poll(context))).await
}
