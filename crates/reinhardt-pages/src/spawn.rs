//! Task spawning utilities for WASM and non-WASM environments.

use std::future::Future;

/// Spawns a task on the current runtime.
///
/// On WASM, this uses `wasm_bindgen_futures::spawn_local` to schedule
/// the task on the browser's event loop. On non-WASM targets, this is
/// a no-op stub.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::spawn::spawn_task;
///
/// spawn_task(async move {
///     let data = fetch_data().await;
///     process(data);
/// });
/// ```
#[cfg(wasm)]
pub fn spawn_task<F>(fut: F)
where
	F: Future<Output = ()> + 'static,
{
	wasm_bindgen_futures::spawn_local(fut);
}

/// No-op stub for non-WASM targets.
#[cfg(native)]
pub fn spawn_task<F>(_fut: F)
where
	F: Future<Output = ()> + 'static,
{
	// Non-WASM: just drop the future
}

/// Yields to the event loop by queuing a microtask.
///
/// On WASM, this resolves a `JsFuture` wrapping a `Promise.resolve()`,
/// giving the browser event loop a chance to tick. This is necessary when
/// spawning async work during initialization (e.g. inside `main()`),
/// where `JsFuture`s from fetch would otherwise hang.
#[cfg(wasm)]
pub async fn defer_yield() {
	use wasm_bindgen::JsValue;
	let promise = js_sys::Promise::resolve(&JsValue::UNDEFINED);
	let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// No-op stub for non-WASM targets.
#[cfg(native)]
pub async fn defer_yield() {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_spawn_task_compiles() {
		spawn_task(async {
			// テスト処理
		});
	}
}
