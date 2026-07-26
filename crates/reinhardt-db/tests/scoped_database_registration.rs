use reinhardt_core::exception::DatabaseErrorKind;
use reinhardt_db::orm::{
	get_connection_registration, install_scoped_database, reinitialize_database,
};
use serial_test::serial;

#[tokio::test]
#[serial(scoped_database_registration)]
async fn scoped_registration_is_available_through_global_accessor() {
	reinitialize_database("sqlite::memory:")
		.await
		.expect("baseline database should initialize");

	let scoped = install_scoped_database("sqlite::memory:")
		.await
		.expect("scoped database should initialize");
	let (current_lease, current) = get_connection_registration()
		.await
		.expect("scoped registration should exist");

	assert_eq!(current, scoped.connection());
	assert_eq!(current_lease.handle(), scoped.connection());
}

#[tokio::test]
#[serial(scoped_database_registration)]
async fn scoped_registration_restores_previous_database_on_drop() {
	reinitialize_database("sqlite::memory:")
		.await
		.expect("baseline database should initialize");
	let (_, baseline) = get_connection_registration()
		.await
		.expect("baseline registration should exist");

	{
		let scoped = install_scoped_database("sqlite::memory:")
			.await
			.expect("scoped database should initialize");
		let (_, current) = get_connection_registration()
			.await
			.expect("scoped registration should exist");
		assert_eq!(current, scoped.connection());
		assert_ne!(current, baseline);
	}

	let (_, restored) = get_connection_registration()
		.await
		.expect("baseline registration should be restored");
	assert_eq!(restored, baseline);
}

#[tokio::test]
#[serial(scoped_database_registration)]
async fn scoped_registration_restores_previous_database_during_unwind() {
	reinitialize_database("sqlite::memory:")
		.await
		.expect("baseline database should initialize");
	let (_, baseline) = get_connection_registration()
		.await
		.expect("baseline registration should exist");
	let scoped = install_scoped_database("sqlite::memory:")
		.await
		.expect("scoped database should initialize");

	let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
		let _scoped = scoped;
		panic!("trigger scoped database guard cleanup");
	}));

	assert!(unwind.is_err());
	let (_, restored) = get_connection_registration()
		.await
		.expect("baseline registration should be restored");
	assert_eq!(restored, baseline);
}

#[tokio::test]
#[serial(scoped_database_registration)]
async fn scoped_registration_lease_keeps_connection_alive_until_last_owner_drops() {
	reinitialize_database("sqlite::memory:")
		.await
		.expect("baseline database should initialize");
	let scoped = install_scoped_database("sqlite::memory:")
		.await
		.expect("scoped database should initialize");
	let handle = scoped.connection();
	let retained_lease = scoped.lease();

	drop(scoped);
	handle
		.execute("SELECT 1", vec![])
		.await
		.expect("retained lease should keep scoped connection available");
	drop(retained_lease);

	let error = handle
		.execute("SELECT 1", vec![])
		.await
		.expect_err("scoped connection should expire after its last lease drops");
	assert_eq!(
		error
			.database_error()
			.expect("expired handle should return a database error")
			.kind(),
		DatabaseErrorKind::ConnectionHandleExpired
	);
}

#[tokio::test]
#[serial(scoped_database_registration)]
async fn nested_scoped_registrations_restore_in_last_in_first_out_order() {
	reinitialize_database("sqlite::memory:")
		.await
		.expect("baseline database should initialize");
	let (_, baseline) = get_connection_registration()
		.await
		.expect("baseline registration should exist");
	let outer = install_scoped_database("sqlite::memory:")
		.await
		.expect("outer scoped database should initialize");
	let outer_handle = outer.connection();
	let inner = install_scoped_database("sqlite::memory:")
		.await
		.expect("inner scoped database should initialize");
	let inner_handle = inner.connection();

	let (_, current) = get_connection_registration()
		.await
		.expect("inner scoped registration should exist");
	assert_eq!(current, inner_handle);
	assert_ne!(current, outer_handle);

	drop(inner);
	let (_, restored_outer) = get_connection_registration()
		.await
		.expect("outer scoped registration should be restored");
	assert_eq!(restored_outer, outer_handle);

	drop(outer);
	let (_, restored_baseline) = get_connection_registration()
		.await
		.expect("baseline registration should be restored");
	assert_eq!(restored_baseline, baseline);
}

#[tokio::test]
#[serial(scoped_database_registration)]
async fn nested_scoped_registrations_skip_an_outer_guard_dropped_first() {
	reinitialize_database("sqlite::memory:")
		.await
		.expect("baseline database should initialize");
	let (_, baseline) = get_connection_registration()
		.await
		.expect("baseline registration should exist");
	let outer = install_scoped_database("sqlite::memory:")
		.await
		.expect("outer scoped database should initialize");
	let outer_handle = outer.connection();
	let inner = install_scoped_database("sqlite::memory:")
		.await
		.expect("inner scoped database should initialize");
	let inner_handle = inner.connection();

	drop(outer);
	let (_, current) = get_connection_registration()
		.await
		.expect("inner scoped registration should remain installed");
	assert_eq!(current, inner_handle);
	assert_ne!(current, outer_handle);

	drop(inner);
	let (_, restored) = get_connection_registration()
		.await
		.expect("baseline registration should be restored");
	assert_eq!(restored, baseline);
}

#[tokio::test]
#[serial(scoped_database_registration)]
async fn three_nested_scopes_restore_the_nearest_active_predecessor_out_of_order() {
	reinitialize_database("sqlite::memory:")
		.await
		.expect("baseline database should initialize");
	let (_, baseline) = get_connection_registration()
		.await
		.expect("baseline registration should exist");
	let outer = install_scoped_database("sqlite::memory:")
		.await
		.expect("outer scoped database should initialize");
	let outer_handle = outer.connection();
	let middle = install_scoped_database("sqlite::memory:")
		.await
		.expect("middle scoped database should initialize");
	let middle_handle = middle.connection();
	let inner = install_scoped_database("sqlite::memory:")
		.await
		.expect("inner scoped database should initialize");
	let inner_handle = inner.connection();

	drop(middle);
	drop(inner);
	let (_, restored_outer) = get_connection_registration()
		.await
		.expect("outer scoped registration should be restored");
	assert_eq!(restored_outer, outer_handle);
	assert_ne!(restored_outer, middle_handle);
	assert_ne!(restored_outer, inner_handle);

	drop(outer);
	let (_, restored_baseline) = get_connection_registration()
		.await
		.expect("baseline registration should be restored");
	assert_eq!(restored_baseline, baseline);
}

#[tokio::test]
#[serial(scoped_database_registration)]
async fn scoped_registration_does_not_overwrite_an_external_replacement_on_drop() {
	reinitialize_database("sqlite::memory:")
		.await
		.expect("baseline database should initialize");
	let scoped = install_scoped_database("sqlite::memory:")
		.await
		.expect("scoped database should initialize");
	reinitialize_database("sqlite::memory:")
		.await
		.expect("external replacement should initialize");
	let (_, replacement) = get_connection_registration()
		.await
		.expect("external replacement should exist");
	assert_ne!(replacement, scoped.connection());

	drop(scoped);

	let (_, current) = get_connection_registration()
		.await
		.expect("external replacement should remain registered");
	assert_eq!(current, replacement);
}
