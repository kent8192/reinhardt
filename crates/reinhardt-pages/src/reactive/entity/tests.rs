use std::any::Any;

use serde::{Deserialize, Serialize};

use super::Entity;
use super::EntityIdentity;

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

fn panic_message(panic: Box<dyn Any + Send>) -> String {
	if let Some(message) = panic.downcast_ref::<String>() {
		message.clone()
	} else if let Some(message) = panic.downcast_ref::<&str>() {
		(*message).to_string()
	} else {
		"non-string panic payload".to_string()
	}
}
