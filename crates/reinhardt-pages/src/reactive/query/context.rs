use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use super::client::QueryClient;

thread_local! {
	static ACTIVE_QUERY_CLIENTS: RefCell<Vec<ActiveQueryClient>> =
		const { RefCell::new(Vec::new()) };
	static NEXT_QUERY_CLIENT_REGISTRATION_ID: Cell<u64> = const { Cell::new(0) };
}

struct ActiveQueryClient {
	registration_id: u64,
	client: QueryClient,
}

pub(crate) struct QueryClientGuard {
	registration_id: u64,
	client: QueryClient,
}

pub(crate) fn provide_query_client(client: QueryClient) -> QueryClientGuard {
	let registration_id = NEXT_QUERY_CLIENT_REGISTRATION_ID.with(|next| {
		let registration_id = next.get();
		next.set(
			registration_id
				.checked_add(1)
				.expect("QueryClient context registration IDs are exhausted"),
		);
		registration_id
	});
	ACTIVE_QUERY_CLIENTS.with(|clients| {
		clients.borrow_mut().push(ActiveQueryClient {
			registration_id,
			client: client.clone(),
		});
	});
	QueryClientGuard {
		registration_id,
		client,
	}
}

impl Drop for QueryClientGuard {
	fn drop(&mut self) {
		ACTIVE_QUERY_CLIENTS.with(|clients| {
			let mut clients = clients.borrow_mut();
			let position = clients
				.iter()
				.position(|entry| entry.registration_id == self.registration_id)
				.expect("active QueryClient guard is missing from its context stack");
			let removed = clients.remove(position);
			debug_assert!(removed.client.same_instance(&self.client));
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
			.map(|entry| entry.client.clone())
			.expect("use_query requires an active QueryClient")
	})
}
