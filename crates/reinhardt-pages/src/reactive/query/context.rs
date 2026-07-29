use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use super::client::QueryClient;

thread_local! {
	static ACTIVE_QUERY_CLIENTS: RefCell<Vec<QueryClient>> =
		const { RefCell::new(Vec::new()) };
}

pub(crate) struct QueryClientGuard {
	client: QueryClient,
}

pub(crate) fn provide_query_client(client: QueryClient) -> QueryClientGuard {
	ACTIVE_QUERY_CLIENTS.with(|clients| clients.borrow_mut().push(client.clone()));
	QueryClientGuard { client }
}

impl Drop for QueryClientGuard {
	fn drop(&mut self) {
		ACTIVE_QUERY_CLIENTS.with(|clients| {
			let mut clients = clients.borrow_mut();
			let position = clients
				.iter()
				.rposition(|client| client.same_instance(&self.client))
				.expect("active QueryClient guard is missing from its context stack");
			clients.remove(position);
		});
	}
}

pub(crate) fn with_query_client<R>(client: &QueryClient, f: impl FnOnce() -> R) -> R {
	let _guard = provide_query_client(client.clone());
	f()
}

pub(crate) fn with_query_client_async<Fut>(
	client: QueryClient,
	future: Fut,
) -> impl Future<Output = Fut::Output>
where
	Fut: Future,
{
	QueryClientFuture {
		client,
		future: Box::pin(future),
	}
}

struct QueryClientFuture<Fut> {
	client: QueryClient,
	future: Pin<Box<Fut>>,
}

impl<Fut> Future for QueryClientFuture<Fut>
where
	Fut: Future,
{
	type Output = Fut::Output;

	fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		with_query_client(&this.client, || this.future.as_mut().poll(context))
	}
}

/// Returns the active application query client.
///
/// # Panics
///
/// Panics when called outside an application, SSR request, or component-test
/// query client context.
pub fn queries() -> QueryClient {
	ACTIVE_QUERY_CLIENTS.with(|clients| {
		clients
			.borrow()
			.last()
			.cloned()
			.expect("use_query requires an active QueryClient")
	})
}
