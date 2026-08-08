use std::any::Any;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{Entity, EntityArena, EntityIdentity};
use crate::reactive::{Effect, ReactiveScope};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Project {
	id: u64,
	name: String,
}

impl Entity for Project {
	type Id = u64;

	const TYPE: &'static str = "reactive.entity.tests.project";

	fn entity_id(&self) -> Self::Id {
		self.id
	}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Task {
	id: u64,
	name: String,
}

impl Entity for Task {
	type Id = u64;

	const TYPE: &'static str = "reactive.entity.tests.task";

	fn entity_id(&self) -> Self::Id {
		self.id
	}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct StructuredId {
	z: u64,
	a: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct StructuredProject {
	id: StructuredId,
}

impl Entity for StructuredProject {
	type Id = StructuredId;

	const TYPE: &'static str = "reactive.entity.tests.structured-project";

	fn entity_id(&self) -> Self::Id {
		self.id.clone()
	}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct EmptyTypeEntity;

impl Entity for EmptyTypeEntity {
	type Id = u64;

	const TYPE: &'static str = "";

	fn entity_id(&self) -> Self::Id {
		7
	}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct ConflictingProject;

impl Entity for ConflictingProject {
	type Id = u64;

	const TYPE: &'static str = "reactive.entity.tests.conflicting";

	fn entity_id(&self) -> Self::Id {
		7
	}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct ConflictingTask;

impl Entity for ConflictingTask {
	type Id = String;

	const TYPE: &'static str = "reactive.entity.tests.conflicting";

	fn entity_id(&self) -> Self::Id {
		"7".to_string()
	}
}

#[test]
fn erased_identity_includes_stable_entity_type() {
	let project = EntityIdentity::of::<Project>(&7);
	let task = EntityIdentity::of::<Task>(&7);

	assert_eq!(project.entity_type(), "reactive.entity.tests.project");
	assert_eq!(project.canonical_id(), "7");
	assert_ne!(project, task);
}

#[test]
fn identity_canonicalizes_structured_ids() {
	let first = EntityIdentity::of::<StructuredProject>(&StructuredId { z: 7, a: 3 });
	let second = EntityIdentity::of::<StructuredProject>(&StructuredId { a: 3, z: 7 });

	assert_eq!(first.canonical_id(), r#"{"a":3,"z":7}"#);
	assert_eq!(first, second);
}

#[test]
fn identity_rejects_empty_entity_type() {
	let panic = std::panic::catch_unwind(|| EntityIdentity::of::<EmptyTypeEntity>(&7))
		.expect_err("an empty entity TYPE must panic");

	assert!(panic_message(panic).contains("entity TYPE must not be empty"));
}

#[test]
fn identity_rejects_incompatible_type_reuse() {
	let _ = EntityIdentity::of::<ConflictingProject>(&7);
	let panic =
		std::panic::catch_unwind(|| EntityIdentity::of::<ConflictingTask>(&"7".to_string()))
			.expect_err("an incompatible entity TYPE reuse must panic");
	let message = panic_message(panic);

	assert!(message.contains(std::any::type_name::<ConflictingProject>()));
	assert!(message.contains(std::any::type_name::<ConflictingTask>()));
	assert!(message.contains(std::any::type_name::<u64>()));
	assert!(message.contains(std::any::type_name::<String>()));
}

#[test]
fn store_vacant_handle_returns_none_without_write_precedence() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));

		let handle = arena.entity::<Project>(1);

		assert_eq!(handle.get(), None);
		assert_eq!(arena.handle_lease_count::<Project>(&1), 1);
		assert_eq!(arena.record_write_ticket::<Project>(&1), None);
	});
}

#[test]
fn store_upsert_replaces_the_complete_entity_value() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		let handle = arena.entity::<Project>(1);

		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 1,
				name: "first".to_string(),
			});
		});
		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 1,
				name: "replacement".to_string(),
			});
		});

		assert_eq!(
			handle.get(),
			Some(Project {
				id: 1,
				name: "replacement".to_string(),
			}),
		);
	});
}

#[test]
fn store_remove_publishes_a_retained_tombstone() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		let handle = arena.entity::<Project>(1);

		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 1,
				name: "present".to_string(),
			});
		});
		let present_ticket = arena
			.record_write_ticket::<Project>(&1)
			.expect("an upsert must record a write ticket");
		arena.update_entities(|writer| writer.remove::<Project>(&1));

		assert_eq!(handle.get(), None);
		assert!(arena.record_is_removed::<Project>(&1));
		assert!(
			arena
				.record_write_ticket::<Project>(&1)
				.expect("a tombstone must retain its write ticket")
				> present_ticket
		);
	});
}

#[test]
fn store_transaction_publishes_only_the_final_value() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		let handle = arena.entity::<Project>(1);
		let observed = Rc::new(RefCell::new(Vec::new()));
		let observed_for_effect = Rc::clone(&observed);
		let handle_for_effect = handle.clone();
		let _effect = Effect::new(move || {
			observed_for_effect
				.borrow_mut()
				.push(handle_for_effect.get());
		});

		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 1,
				name: "first".to_string(),
			});
			writer.remove::<Project>(&1);
			writer.upsert(Project {
				id: 1,
				name: "final".to_string(),
			});
		});

		assert_eq!(
			handle.get(),
			Some(Project {
				id: 1,
				name: "final".to_string(),
			}),
		);
		assert_eq!(
			observed.borrow().as_slice(),
			&[
				None,
				Some(Project {
					id: 1,
					name: "final".to_string(),
				}),
			],
		);
	});
}

#[test]
fn store_callback_panic_rolls_back_staged_writes() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		let handle = arena.entity::<Project>(1);
		let observed = Rc::new(RefCell::new(Vec::new()));
		let observed_for_effect = Rc::clone(&observed);
		let handle_for_effect = handle.clone();
		let _effect = Effect::new(move || {
			observed_for_effect
				.borrow_mut()
				.push(handle_for_effect.get());
		});
		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 1,
				name: "stable".to_string(),
			});
		});
		let stable_ticket = arena
			.record_write_ticket::<Project>(&1)
			.expect("the stable write must record a ticket");
		observed.borrow_mut().clear();

		let panic = catch_unwind(AssertUnwindSafe(|| {
			arena.update_entities(|writer| {
				writer.upsert(Project {
					id: 1,
					name: "discarded".to_string(),
				});
				panic!("callback failure");
			});
		}));

		assert!(panic.is_err());
		assert_eq!(
			handle.get(),
			Some(Project {
				id: 1,
				name: "stable".to_string(),
			}),
		);
		assert_eq!(
			arena.record_write_ticket::<Project>(&1),
			Some(stable_ticket)
		);
		assert!(!arena.record_is_removed::<Project>(&1));
		assert_eq!(observed.borrow().as_slice(), &[]);
	});
}

#[test]
fn store_precommit_panic_rolls_back_staged_writes() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		let handle = arena.entity::<Project>(1);
		let observed = Rc::new(RefCell::new(Vec::new()));
		let observed_for_effect = Rc::clone(&observed);
		let handle_for_effect = handle.clone();
		let _effect = Effect::new(move || {
			observed_for_effect
				.borrow_mut()
				.push(handle_for_effect.get());
		});
		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 1,
				name: "stable".to_string(),
			});
		});
		let stable_ticket = arena
			.record_write_ticket::<Project>(&1)
			.expect("the stable write must record a ticket");
		observed.borrow_mut().clear();

		let panic = catch_unwind(AssertUnwindSafe(|| {
			arena.update_entities_with_test_precommit(
				|writer| {
					writer.upsert(Project {
						id: 1,
						name: "discarded".to_string(),
					});
				},
				|_| panic!("precommit validation failure"),
			);
		}));

		assert!(panic.is_err());
		assert_eq!(
			handle.get(),
			Some(Project {
				id: 1,
				name: "stable".to_string(),
			}),
		);
		assert_eq!(
			arena.record_write_ticket::<Project>(&1),
			Some(stable_ticket)
		);
		assert!(!arena.record_is_removed::<Project>(&1));
		assert_eq!(observed.borrow().as_slice(), &[]);
	});
}

#[test]
fn store_handle_clones_share_one_lease() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		let handle = arena.entity::<Project>(1);
		let clone = handle.clone();

		assert_eq!(arena.handle_lease_count::<Project>(&1), 1);
		drop(clone);
		assert_eq!(arena.handle_lease_count::<Project>(&1), 1);
		drop(handle);
		assert_eq!(arena.handle_lease_count::<Project>(&1), 0);
	});
}

#[test]
fn ticket_query_leases_are_counted_and_ordered_with_mutations() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		let first_query = arena.acquire_query_ticket();
		let second_query = arena.acquire_query_ticket();
		let mutation = arena.issue_mutation_ticket();

		assert!(first_query.ticket() < second_query.ticket());
		assert!(second_query.ticket() < mutation);
		assert_eq!(arena.active_query_ticket_count(first_query.ticket()), 1);
		assert_eq!(arena.active_query_ticket_count(second_query.ticket()), 1);
		drop(first_query);
		assert_eq!(arena.active_query_ticket_count(mutation), 0);
		let second_ticket = second_query.ticket();
		drop(second_query);
		assert_eq!(arena.active_query_ticket_count(second_ticket), 0);
	});
}

fn panic_message(panic: Box<dyn Any + Send>) -> String {
	if let Some(message) = panic.downcast_ref::<String>() {
		message.clone()
	} else if let Some(message) = panic.downcast_ref::<&str>() {
		(*message).to_string()
	} else {
		"non-string panic payload".to_string()
	}
}
