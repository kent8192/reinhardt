use std::rc::Rc;
use std::time::Duration;

use super::client::QueryEntry;
#[cfg(wasm)]
use super::runtime::duration_ms;

#[cfg(wasm)]
pub(super) struct QueryGuard {
	interval_id: i32,
	_closure: wasm_bindgen::closure::Closure<dyn FnMut()>,
}

#[cfg(wasm)]
impl QueryGuard {
	pub(super) fn poll<T, E>(interval: Duration, entry: Rc<QueryEntry<T, E>>) -> Self
	where
		T: Clone + 'static,
		E: Clone + 'static,
	{
		use wasm_bindgen::JsCast;

		let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
			entry.start_fetch(true);
		}) as Box<dyn FnMut()>);

		let interval_ms = duration_ms(interval).min(i32::MAX as u64) as i32;
		let interval_id = web_sys::window()
			.and_then(|window| {
				window
					.set_interval_with_callback_and_timeout_and_arguments_0(
						closure.as_ref().unchecked_ref(),
						interval_ms,
					)
					.ok()
			})
			.unwrap_or(-1);

		Self {
			interval_id,
			_closure: closure,
		}
	}
}

#[cfg(wasm)]
impl Drop for QueryGuard {
	fn drop(&mut self) {
		if self.interval_id >= 0
			&& let Some(window) = web_sys::window()
		{
			window.clear_interval_with_handle(self.interval_id);
		}
	}
}

#[cfg(not(wasm))]
pub(super) struct QueryGuard;

#[cfg(not(wasm))]
impl QueryGuard {
	pub(super) fn poll<T, E>(_interval: Duration, _entry: Rc<QueryEntry<T, E>>) -> Self
	where
		T: Clone + 'static,
		E: Clone + 'static,
	{
		Self
	}
}
