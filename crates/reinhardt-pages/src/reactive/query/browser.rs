use std::rc::Weak;

use super::client::QueryClientInner;

#[cfg(wasm)]
use std::cell::{Cell, RefCell};
#[cfg(wasm)]
use std::rc::Rc;

#[cfg(wasm)]
use wasm_bindgen::JsCast;

#[cfg(wasm)]
#[derive(Default)]
struct BrowserResourceCounts {
	listeners: Cell<usize>,
	timers: Cell<usize>,
}

#[cfg(wasm)]
struct BrowserTimerGuard {
	id: i32,
	deadline_ms: u64,
	fired: Rc<Cell<bool>>,
	counts: Rc<BrowserResourceCounts>,
	_closure: wasm_bindgen::closure::Closure<dyn FnMut()>,
}

#[cfg(wasm)]
impl Drop for BrowserTimerGuard {
	fn drop(&mut self) {
		if let Some(window) = web_sys::window() {
			window.clear_timeout_with_handle(self.id);
		}
		self.counts
			.timers
			.set(self.counts.timers.get().saturating_sub(1));
	}
}

#[cfg(wasm)]
struct VisibilityListenerGuard {
	document: web_sys::Document,
	counts: Rc<BrowserResourceCounts>,
	closure: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
}

#[cfg(wasm)]
impl Drop for VisibilityListenerGuard {
	fn drop(&mut self) {
		let _ = self.document.remove_event_listener_with_callback(
			"visibilitychange",
			self.closure.as_ref().unchecked_ref(),
		);
		self.counts
			.listeners
			.set(self.counts.listeners.get().saturating_sub(1));
	}
}

#[cfg(wasm)]
pub(super) struct QueryBrowser {
	owner: Weak<QueryClientInner>,
	enabled: bool,
	timer: RefCell<Option<BrowserTimerGuard>>,
	_listener: Option<VisibilityListenerGuard>,
	counts: Rc<BrowserResourceCounts>,
	#[cfg(feature = "testing")]
	visibility_override: Cell<Option<bool>>,
}

#[cfg(wasm)]
impl QueryBrowser {
	pub(super) fn initial_visibility(enabled: bool) -> bool {
		!enabled
			|| web_sys::window()
				.and_then(|window| window.document())
				.is_none_or(|document| {
					document.visibility_state() != web_sys::VisibilityState::Hidden
				})
	}

	pub(super) fn new(owner: Weak<QueryClientInner>, enabled: bool) -> Self {
		let counts = Rc::new(BrowserResourceCounts::default());
		let listener = enabled
			.then(|| {
				let document = web_sys::window()?.document()?;
				let callback_owner = owner.clone();
				let closure =
					wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::Event| {
						if let Some(owner) = callback_owner.upgrade() {
							owner.handle_visibility_change();
						}
					}) as Box<dyn FnMut(web_sys::Event)>);
				document
					.add_event_listener_with_callback(
						"visibilitychange",
						closure.as_ref().unchecked_ref(),
					)
					.ok()?;
				counts.listeners.set(counts.listeners.get() + 1);
				Some(VisibilityListenerGuard {
					document,
					counts: Rc::clone(&counts),
					closure,
				})
			})
			.flatten();
		Self {
			owner,
			enabled,
			timer: RefCell::new(None),
			_listener: listener,
			counts,
			#[cfg(feature = "testing")]
			visibility_override: Cell::new(None),
		}
	}

	pub(super) fn schedule(&self, deadline_ms: Option<u64>, now_ms: u64) {
		if !self.enabled {
			return;
		}
		let Some(deadline_ms) = deadline_ms else {
			self.timer.borrow_mut().take();
			return;
		};
		if self
			.timer
			.borrow()
			.as_ref()
			.is_some_and(|timer| timer.deadline_ms == deadline_ms && !timer.fired.get())
		{
			return;
		}
		let Some(window) = web_sys::window() else {
			self.timer.borrow_mut().take();
			return;
		};
		let owner = self.owner.clone();
		let fired = Rc::new(Cell::new(false));
		let callback_fired = Rc::clone(&fired);
		let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
			callback_fired.set(true);
			let owner = owner.clone();
			wasm_bindgen_futures::spawn_local(async move {
				crate::platform::defer_yield().await;
				if let Some(owner) = owner.upgrade() {
					owner.handle_browser_timer();
				}
			});
		}) as Box<dyn FnMut()>);
		let delay_ms = deadline_ms.saturating_sub(now_ms).min(i32::MAX as u64) as i32;
		let Ok(id) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
			closure.as_ref().unchecked_ref(),
			delay_ms,
		) else {
			self.timer.borrow_mut().take();
			return;
		};
		self.counts.timers.set(self.counts.timers.get() + 1);
		self.timer.borrow_mut().replace(BrowserTimerGuard {
			id,
			deadline_ms,
			fired,
			counts: Rc::clone(&self.counts),
			_closure: closure,
		});
	}

	pub(super) fn document_is_visible(&self) -> bool {
		#[cfg(feature = "testing")]
		if let Some(visible) = self.visibility_override.get() {
			return visible;
		}
		web_sys::window()
			.and_then(|window| window.document())
			.is_none_or(|document| document.visibility_state() != web_sys::VisibilityState::Hidden)
	}

	#[cfg(feature = "testing")]
	pub(super) fn set_visibility_for_test(&self, visible: bool) {
		self.visibility_override.set(Some(visible));
	}

	#[cfg(feature = "testing")]
	pub(super) fn resource_counts(&self) -> (usize, usize) {
		(self.counts.listeners.get(), self.counts.timers.get())
	}

	#[cfg(feature = "testing")]
	pub(super) fn resource_probe(&self) -> QueryBrowserResourceProbe {
		QueryBrowserResourceProbe {
			counts: Rc::downgrade(&self.counts),
		}
	}
}

#[cfg(not(wasm))]
pub(super) struct QueryBrowser;

#[cfg(not(wasm))]
impl QueryBrowser {
	pub(super) fn initial_visibility(_enabled: bool) -> bool {
		true
	}

	pub(super) fn new(_owner: Weak<QueryClientInner>, _enabled: bool) -> Self {
		Self
	}

	pub(super) fn schedule(&self, _deadline_ms: Option<u64>, _now_ms: u64) {}

	#[cfg(feature = "testing")]
	pub(super) fn set_visibility_for_test(&self, _visible: bool) {}

	#[cfg(feature = "testing")]
	pub(super) fn resource_counts(&self) -> (usize, usize) {
		(0, 0)
	}

	#[cfg(feature = "testing")]
	pub(super) fn resource_probe(&self) -> QueryBrowserResourceProbe {
		QueryBrowserResourceProbe
	}
}

/// Weak testing view of browser resources owned by a query client.
#[cfg(all(feature = "testing", wasm))]
pub struct QueryBrowserResourceProbe {
	counts: Weak<BrowserResourceCounts>,
}

#[cfg(all(feature = "testing", wasm))]
impl QueryBrowserResourceProbe {
	/// Returns active visibility listeners and query maintenance timers.
	pub fn counts(&self) -> (usize, usize) {
		self.counts.upgrade().map_or((0, 0), |counts| {
			(counts.listeners.get(), counts.timers.get())
		})
	}
}

/// Weak testing view of browser resources owned by a query client.
#[cfg(all(feature = "testing", not(wasm)))]
pub struct QueryBrowserResourceProbe;

#[cfg(all(feature = "testing", not(wasm)))]
impl QueryBrowserResourceProbe {
	/// Returns active visibility listeners and query maintenance timers.
	pub fn counts(&self) -> (usize, usize) {
		(0, 0)
	}
}
