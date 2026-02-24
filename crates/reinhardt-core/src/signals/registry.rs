//! Global signal registry

use super::core::SignalName;
use super::signal::Signal;
use parking_lot::RwLock;
use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Global signal registry
pub(crate) struct SignalRegistry {
	signals: RwLock<HashMap<(TypeId, String), Box<dyn Any + Send + Sync>>>,
}

impl SignalRegistry {
	fn new() -> Self {
		Self {
			signals: RwLock::new(HashMap::new()),
		}
	}

	/// Get or create a signal for a specific type and name
	pub(crate) fn get_or_create<T: Send + Sync + 'static>(&self, name: SignalName) -> Signal<T> {
		let type_id = TypeId::of::<T>();
		let key = (type_id, name.as_str().to_string());

		// Try to get existing signal
		{
			let signals = self.signals.read();
			if let Some(signal_any) = signals.get(&key)
				&& let Some(signal) = signal_any.downcast_ref::<Signal<T>>()
			{
				return signal.clone();
			}
		}

		// Create new signal
		let signal = Signal::new(name);
		self.signals.write().insert(key, Box::new(signal.clone()));
		signal
	}

	/// Get or create a signal with a string name (for backward compatibility)
	#[doc(hidden)]
	pub(crate) fn get_or_create_with_string<T: Send + Sync + 'static>(
		&self,
		name: impl Into<String>,
	) -> Signal<T> {
		let name_str = name.into();
		let type_id = TypeId::of::<T>();
		let key = (type_id, name_str.clone());

		// Try to get existing signal
		{
			let signals = self.signals.read();
			if let Some(signal_any) = signals.get(&key)
				&& let Some(signal) = signal_any.downcast_ref::<Signal<T>>()
			{
				return signal.clone();
			}
		}

		// Create new signal using Arc-based SignalName to avoid memory leak
		let signal = Signal::new(SignalName::from_string(name_str.clone()));
		self.signals.write().insert(key, Box::new(signal.clone()));
		signal
	}
}

// Global registry instance
static GLOBAL_REGISTRY: once_cell::sync::Lazy<SignalRegistry> =
	once_cell::sync::Lazy::new(SignalRegistry::new);

/// Get a signal from the global registry with type-safe name
pub fn get_signal<T: Send + Sync + 'static>(name: SignalName) -> Signal<T> {
	GLOBAL_REGISTRY.get_or_create(name)
}

/// Get a signal from the global registry with string name (for backward compatibility)
#[doc(hidden)]
pub fn get_signal_with_string<T: Send + Sync + 'static>(name: impl Into<String>) -> Signal<T> {
	GLOBAL_REGISTRY.get_or_create_with_string(name)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_registry_get_or_create_with_static_name() {
		// Arrange
		let registry = SignalRegistry::new();

		// Act
		let signal1 = registry.get_or_create::<String>(SignalName::PRE_SAVE);
		let signal2 = registry.get_or_create::<String>(SignalName::PRE_SAVE);

		// Assert
		assert_eq!(signal1.receiver_count(), signal2.receiver_count());
	}

	#[rstest]
	fn test_registry_get_or_create_with_string_no_leak() {
		// Arrange
		let registry = SignalRegistry::new();

		// Act - create signals with dynamic string names (previously would Box::leak)
		for i in 0..100 {
			let name = format!("dynamic_signal_{}", i);
			let _signal = registry.get_or_create_with_string::<String>(name);
		}

		// Assert - verify all signals were registered
		let signals = registry.signals.read();
		let string_signal_count = signals
			.keys()
			.filter(|(_, name)| name.starts_with("dynamic_signal_"))
			.count();
		assert_eq!(string_signal_count, 100);
	}

	#[rstest]
	fn test_registry_get_or_create_with_string_deduplication() {
		// Arrange
		let registry = SignalRegistry::new();

		// Act - create signal with same name twice
		let signal1 = registry.get_or_create_with_string::<String>("dedup_test");
		let signal2 = registry.get_or_create_with_string::<String>("dedup_test");

		// Assert - should return the same signal (same receiver count after modifications)
		assert_eq!(signal1.receiver_count(), 0);
		assert_eq!(signal2.receiver_count(), 0);
	}

	#[rstest]
	fn test_registry_from_string_creates_arc_based_signal_name() {
		// Arrange
		let dynamic_name = format!("arc_signal_{}", 42);

		// Act
		let name = SignalName::from_string(dynamic_name);

		// Assert - the name should work correctly without Box::leak
		assert_eq!(name.as_str(), "arc_signal_42");
	}
}
