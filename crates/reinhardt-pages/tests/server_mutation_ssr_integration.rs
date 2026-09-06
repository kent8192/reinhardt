#![cfg(not(target_arch = "wasm32"))]

include!("support/server_mutation_shared.rs");

use std::cell::Cell;
use std::rc::Rc;

use reinhardt_core::reactive::ReactiveScope;
use reinhardt_pages::MutationDispatchOutcome;

#[test]
fn component_renders_shared_ready_marker_on_native() {
	ReactiveScope::run(|| {
		let html = ClusterMutationComponent.render().render_to_string();

		assert_eq!(
			html.matches(r#"<div id="cluster-mutations-ready">ready</div>"#)
				.count(),
			1,
			"{html}"
		);
	});
}

#[test]
fn native_server_mutation_handles_are_inert_before_form_or_callback_work() {
	ReactiveScope::run(|| {
		let create_form = cluster_create_form!();
		let create_form_events = Rc::new(Cell::new(0));
		let create_public_callbacks = Rc::new(Cell::new(0));
		let create_runtime = use_form(&create_form)
			.on_submit_success({
				let create_form_events = Rc::clone(&create_form_events);
				move |_| create_form_events.set(create_form_events.get() + 1)
			})
			.on_submit_error({
				let create_form_events = Rc::clone(&create_form_events);
				move |_| create_form_events.set(create_form_events.get() + 1)
			})
			.build();
		let create = create_form
			.server_mutation(&create_runtime)
			.on_success({
				let create_public_callbacks = Rc::clone(&create_public_callbacks);
				move |_| create_public_callbacks.set(create_public_callbacks.get() + 1)
			})
			.on_error({
				let create_public_callbacks = Rc::clone(&create_public_callbacks);
				move |_| create_public_callbacks.set(create_public_callbacks.get() + 1)
			})
			.build();

		let update_form =
			UpdateClusterRequestClientForm::new().with_defaults(default_update_request());
		let update_form_events = Rc::new(Cell::new(0));
		let update_public_callbacks = Rc::new(Cell::new(0));
		let update_runtime = use_form(&update_form)
			.on_submit_success({
				let update_form_events = Rc::clone(&update_form_events);
				move |_| update_form_events.set(update_form_events.get() + 1)
			})
			.on_submit_error({
				let update_form_events = Rc::clone(&update_form_events);
				move |_| update_form_events.set(update_form_events.get() + 1)
			})
			.build();
		let update = update_form
			.server_mutation(&update_runtime)
			.on_success({
				let update_public_callbacks = Rc::clone(&update_public_callbacks);
				move |_| update_public_callbacks.set(update_public_callbacks.get() + 1)
			})
			.on_error({
				let update_public_callbacks = Rc::clone(&update_public_callbacks);
				move |_| update_public_callbacks.set(update_public_callbacks.get() + 1)
			})
			.build();

		let delete_callbacks = Rc::new(Cell::new(0));
		let delete = use_server_mutation(delete_cluster::mutation())
			.on_success({
				let delete_callbacks = Rc::clone(&delete_callbacks);
				move |_| delete_callbacks.set(delete_callbacks.get() + 1)
			})
			.on_error({
				let delete_callbacks = Rc::clone(&delete_callbacks);
				move |_| delete_callbacks.set(delete_callbacks.get() + 1)
			})
			.build();

		assert_eq!(
			create.dispatch(),
			MutationDispatchOutcome::UnsupportedTarget
		);
		assert_eq!(
			update.dispatch(),
			MutationDispatchOutcome::UnsupportedTarget
		);
		assert_eq!(
			delete.dispatch("cluster-1".to_owned()),
			MutationDispatchOutcome::UnsupportedTarget
		);

		assert_eq!(create_form_events.get(), 0);
		assert_eq!(create_public_callbacks.get(), 0);
		assert_eq!(update_form_events.get(), 0);
		assert_eq!(update_public_callbacks.get(), 0);
		assert_eq!(delete_callbacks.get(), 0);
		assert!(!create_runtime.form_state().is_submitting.get());
		assert!(create_runtime.form_state().field_errors.get().is_empty());
		assert!(!update_runtime.form_state().is_submitting.get());
		assert!(update_runtime.form_state().field_errors.get().is_empty());
	});
}
