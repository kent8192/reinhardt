#[cfg(wasm)]
use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::component::Head;

#[cfg(wasm)]
mod dom;
pub(crate) mod registry;

#[cfg(wasm)]
use dom::BrowserDocumentHead;
use registry::{HeadRegistry, HeadSlotId, HeadSlotKind, ResolvedHeadEntry};

thread_local! {
	static CURRENT_DOCUMENT_HEAD_MANAGER: RefCell<Option<DocumentHeadManager>> =
		const { RefCell::new(None) };
	#[cfg(wasm)]
	static BROWSER_DOCUMENT_HEAD_MANAGER: RefCell<Option<DocumentHeadManager>> =
		const { RefCell::new(None) };
}

#[derive(Clone)]
pub(crate) struct DocumentHeadManager {
	registry: Rc<RefCell<HeadRegistry>>,
	#[cfg(wasm)]
	browser: Option<Rc<RefCell<BrowserDocumentHead>>>,
	#[cfg(wasm)]
	batch_depth: Rc<Cell<usize>>,
}

pub(crate) struct DocumentHeadRegistration {
	manager: DocumentHeadManager,
	id: HeadSlotId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DocumentHeadError {
	NoActiveManager,
	DomOperation(String),
}

struct DocumentHeadManagerGuard {
	previous: Option<DocumentHeadManager>,
}

impl DocumentHeadManager {
	pub(crate) fn new(default_head: Head) -> Self {
		Self {
			registry: Rc::new(RefCell::new(HeadRegistry::new(default_head))),
			#[cfg(wasm)]
			browser: None,
			#[cfg(wasm)]
			batch_depth: Rc::new(Cell::new(0)),
		}
	}

	#[cfg(wasm)]
	fn new_browser(default_head: Head) -> Result<Self, DocumentHeadError> {
		let mut browser = BrowserDocumentHead::new()?;
		browser.adopt_marked_nodes()?;
		Ok(Self {
			registry: Rc::new(RefCell::new(HeadRegistry::new(default_head))),
			browser: Some(Rc::new(RefCell::new(browser))),
			batch_depth: Rc::new(Cell::new(0)),
		})
	}

	pub(crate) fn register_static_page(
		&self,
		head: Head,
	) -> Result<DocumentHeadRegistration, DocumentHeadError> {
		self.register(HeadSlotKind::StaticPage, head)
	}

	pub(crate) fn register_hook(
		&self,
		head: Head,
	) -> Result<DocumentHeadRegistration, DocumentHeadError> {
		self.register(HeadSlotKind::RetainedHook, head)
	}

	pub(crate) fn reconcile(&self) -> Result<(), DocumentHeadError> {
		let _entries = self.registry.borrow().resolved_entries();
		#[cfg(wasm)]
		if let Some(browser) = self.browser.as_ref() {
			browser.borrow_mut().reconcile(&_entries)?;
		}
		Ok(())
	}

	#[cfg(wasm)]
	pub(crate) fn begin_batch(&self) {
		self.batch_depth.set(
			self.batch_depth
				.get()
				.checked_add(1)
				.expect("head batch depth exhausted"),
		);
	}

	#[cfg(wasm)]
	pub(crate) fn end_batch(&self, reconcile: bool) -> Result<(), DocumentHeadError> {
		let depth = self.batch_depth.get();
		assert!(depth > 0, "document-head batch is not active");
		self.batch_depth.set(depth - 1);
		if depth == 1 && reconcile {
			self.reconcile()?;
		}
		Ok(())
	}

	pub(crate) fn resolved_entries(&self) -> Vec<ResolvedHeadEntry> {
		self.registry.borrow().resolved_entries()
	}

	fn register(
		&self,
		kind: HeadSlotKind,
		head: Head,
	) -> Result<DocumentHeadRegistration, DocumentHeadError> {
		let id = self.registry.borrow_mut().register(kind, head);
		#[cfg(wasm)]
		if self.batch_depth.get() > 0 {
			return Ok(DocumentHeadRegistration {
				manager: self.clone(),
				id,
			});
		}
		if let Err(error) = self.reconcile() {
			self.registry.borrow_mut().remove(id);
			return Err(error);
		}
		Ok(DocumentHeadRegistration {
			manager: self.clone(),
			id,
		})
	}
}

impl DocumentHeadRegistration {
	pub(crate) fn replace(&self, head: Head) -> Result<(), DocumentHeadError> {
		let previous = self.manager.registry.borrow().head(self.id);
		if self.manager.registry.borrow_mut().replace(self.id, head) {
			#[cfg(wasm)]
			if self.manager.batch_depth.get() > 0 {
				return Ok(());
			}
			if let Err(error) = self.manager.reconcile() {
				if let Some(previous) = previous {
					self.manager
						.registry
						.borrow_mut()
						.replace(self.id, previous);
					let _ = self.manager.reconcile();
				}
				return Err(error);
			}
		}
		Ok(())
	}
}

impl Drop for DocumentHeadRegistration {
	fn drop(&mut self) {
		if self.manager.registry.borrow_mut().remove(self.id) {
			#[cfg(wasm)]
			if self.manager.batch_depth.get() > 0 {
				return;
			}
			if let Err(error) = self.manager.reconcile() {
				report_document_head_error(&error);
			}
		}
	}
}

impl Drop for DocumentHeadManagerGuard {
	fn drop(&mut self) {
		CURRENT_DOCUMENT_HEAD_MANAGER.with(|current| {
			current.replace(self.previous.take());
		});
	}
}

impl fmt::Display for DocumentHeadError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NoActiveManager => formatter.write_str("no document-head manager is active"),
			Self::DomOperation(message) => {
				write!(formatter, "document-head DOM operation failed: {message}")
			}
		}
	}
}

impl std::error::Error for DocumentHeadError {}

#[cfg(wasm)]
impl DocumentHeadError {
	pub(crate) fn into_mount_error(self) -> crate::component::MountError {
		match self {
			Self::NoActiveManager => crate::component::MountError::NoDocument,
			Self::DomOperation(_) => crate::component::MountError::AppendChildFailed,
		}
	}
}

pub(crate) fn with_document_head_manager<R>(
	manager: &DocumentHeadManager,
	f: impl FnOnce() -> R,
) -> R {
	let previous =
		CURRENT_DOCUMENT_HEAD_MANAGER.with(|current| current.replace(Some(manager.clone())));
	let _guard = DocumentHeadManagerGuard { previous };
	f()
}

pub(crate) fn current_document_head_manager() -> Result<DocumentHeadManager, DocumentHeadError> {
	if let Some(manager) = CURRENT_DOCUMENT_HEAD_MANAGER.with(|current| current.borrow().clone()) {
		return Ok(manager);
	}
	#[cfg(wasm)]
	if let Some(manager) = BROWSER_DOCUMENT_HEAD_MANAGER.with(|current| current.borrow().clone()) {
		return Ok(manager);
	}
	Err(DocumentHeadError::NoActiveManager)
}

pub(crate) fn report_document_head_error(_error: &DocumentHeadError) {
	crate::error_log!("{_error}");
}

#[cfg(wasm)]
pub(crate) fn ensure_browser_document_head_manager()
-> Result<DocumentHeadManager, DocumentHeadError> {
	BROWSER_DOCUMENT_HEAD_MANAGER.with(|current| {
		let mut current = current.borrow_mut();
		if let Some(manager) = current.as_ref() {
			return Ok(manager.clone());
		}
		let manager = DocumentHeadManager::new_browser(Head::new())?;
		current.replace(manager.clone());
		Ok(manager)
	})
}

#[cfg(test)]
mod tests {
	use super::{DocumentHeadManager, with_document_head_manager};
	use crate::component::{Head, cleanup_reactive_nodes};
	use crate::reactive::Signal;
	use crate::reactive::hooks::use_head;
	use crate::reactive::runtime::with_runtime;
	use serial_test::serial;

	#[test]
	#[serial(document_head)]
	fn retained_head_reuses_the_setup_manager_after_its_scope_exits() {
		cleanup_reactive_nodes();
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let manager = DocumentHeadManager::new(Head::new());
			let title = Signal::new("Initial title");

			with_document_head_manager(&manager, || {
				use_head(
					{
						let title = title.clone();
						move || Head::new().title(title.get())
					},
					crate::deps![title.clone()],
				);
			});

			assert_eq!(
				manager.registry.borrow().resolve().title.as_deref(),
				Some("Initial title")
			);

			title.set("Updated title");
			with_runtime(|runtime| runtime.flush_updates());

			assert_eq!(
				manager.registry.borrow().resolve().title.as_deref(),
				Some("Updated title")
			);
		});
		cleanup_reactive_nodes();
	}
}
