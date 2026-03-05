//! Integration tests for the receiver macro
//!
//! Tests that verify:
//! - Basic receiver registration via inventory
//! - Auto-connection of receivers
//! - Signal dispatch to receivers
//!
//! Note: These tests were moved from reinhardt-macros crate to avoid circular
//! dependency issues during crates.io publishing.

use reinhardt_core::signals::{SignalError, auto_connect_receivers, get_signal_with_string};
use reinhardt_macros::receiver;
use serial_test::serial;
use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// Global counters for testing receiver invocation
static SAVE_HANDLER_CALLS: AtomicUsize = AtomicUsize::new(0);
static DELETE_HANDLER_CALLS: AtomicUsize = AtomicUsize::new(0);
static CUSTOM_HANDLER_CALLS: AtomicUsize = AtomicUsize::new(0);

// Test basic receiver registration
// Note: Receivers must accept Arc<dyn Any + Send + Sync> for type erasure
#[receiver(signal = "post_save")]
async fn on_save_handler(_instance: Arc<dyn Any + Send + Sync>) -> Result<(), SignalError> {
	SAVE_HANDLER_CALLS.fetch_add(1, Ordering::SeqCst);
	Ok(())
}

#[receiver(signal = "pre_delete", priority = 10)]
async fn on_delete_handler(_instance: Arc<dyn Any + Send + Sync>) -> Result<(), SignalError> {
	DELETE_HANDLER_CALLS.fetch_add(1, Ordering::SeqCst);
	Ok(())
}

#[receiver(signal = "custom_signal", dispatch_uid = "custom_receiver")]
async fn custom_handler(_instance: Arc<dyn Any + Send + Sync>) -> Result<(), SignalError> {
	CUSTOM_HANDLER_CALLS.fetch_add(1, Ordering::SeqCst);
	Ok(())
}

#[test]
fn test_receiver_macro_compiles() {
	// This test verifies that the macro compiles successfully
	// The actual registration happens via inventory
}

#[tokio::test]
#[serial(receiver_registry)]
async fn test_auto_connect_receivers() {
	// Call auto-connect to register all receivers
	auto_connect_receivers();

	// If this doesn't panic, the basic registration worked
	// Note: Full integration testing would require actual signal dispatch
}

#[tokio::test]
#[serial(receiver_registry)]
async fn test_receiver_invocation_via_signal() {
	// Reset counters
	SAVE_HANDLER_CALLS.store(0, Ordering::SeqCst);
	DELETE_HANDLER_CALLS.store(0, Ordering::SeqCst);
	CUSTOM_HANDLER_CALLS.store(0, Ordering::SeqCst);

	// Auto-connect all receivers
	auto_connect_receivers();

	// Get signals and send data
	type AnyData = Arc<dyn Any + Send + Sync>;

	// Test post_save signal
	let post_save_signal = get_signal_with_string::<AnyData>("post_save");
	let test_data: Arc<dyn Any + Send + Sync> = Arc::new(String::from("test_data"));
	post_save_signal.send(test_data).await.unwrap();

	// Verify save handler was called
	assert_eq!(SAVE_HANDLER_CALLS.load(Ordering::SeqCst), 1);

	// Test pre_delete signal
	let pre_delete_signal = get_signal_with_string::<AnyData>("pre_delete");
	let test_data2: Arc<dyn Any + Send + Sync> = Arc::new(String::from("test_data2"));
	pre_delete_signal.send(test_data2).await.unwrap();

	// Verify delete handler was called
	assert_eq!(DELETE_HANDLER_CALLS.load(Ordering::SeqCst), 1);

	// Test custom signal
	let custom_signal = get_signal_with_string::<AnyData>("custom_signal");
	let test_data3: Arc<dyn Any + Send + Sync> = Arc::new(String::from("test_data3"));
	custom_signal.send(test_data3).await.unwrap();

	// Verify custom handler was called
	assert_eq!(CUSTOM_HANDLER_CALLS.load(Ordering::SeqCst), 1);
}

#[tokio::test]
#[serial(receiver_registry)]
async fn test_receiver_priority_ordering() {
	use std::sync::Mutex;

	// Track call order
	let call_order: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

	// Auto-connect receivers
	auto_connect_receivers();

	// Get pre_delete signal (has priority 10 receiver)
	type AnyData = Arc<dyn Any + Send + Sync>;
	let signal = get_signal_with_string::<AnyData>("pre_delete");

	// Add lower priority receiver
	let call_order_clone: Arc<Mutex<Vec<&'static str>>> = Arc::clone(&call_order);
	signal.connect_with_options(
		move |_data: Arc<AnyData>| {
			let call_order = Arc::clone(&call_order_clone);
			async move {
				call_order.lock().unwrap().push("low_priority");
				Ok(())
			}
		},
		None,
		None,
		0, // Lower priority
	);

	// Send signal
	let test_data: Arc<dyn Any + Send + Sync> = Arc::new(String::from("test_data"));
	signal.send(test_data).await.unwrap();

	// Note: Actual priority ordering verification would require access to receiver execution order
	// This test verifies that the priority parameter is accepted without errors
}
