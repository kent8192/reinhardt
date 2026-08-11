use std::any::Any;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::identity::EntityTypeRegistry;
use super::projection::{EntityHydrationGroup, EntityHydrationRecord, ErasedEntityProjection};
use super::{
	Entity, EntityArena, EntityDependencies, EntityIdentity, EntityProjection, EntityValue,
	EntityVec, OptionalEntity, ProjectionMaterialization, ProjectionRemoval, RemovedEntities,
};
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
struct ProjectPage {
	title: String,
	projects: Vec<Project>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct ProjectPageRecipe {
	title: String,
	project_ids: Vec<u64>,
}

#[derive(Clone, Copy)]
struct ProjectPageProjection;

impl EntityProjection<ProjectPage> for ProjectPageProjection {
	type Recipe = ProjectPageRecipe;

	const SCHEMA: &'static str = "reactive.entity.tests.project-page-v1";

	fn normalize(
		&self,
		value: ProjectPage,
		entities: &mut super::EntityWriter<'_>,
	) -> Self::Recipe {
		let ProjectPage { title, projects } = value;
		let project_ids = projects.iter().map(Entity::entity_id).collect();
		for project in projects {
			entities.upsert(project);
		}
		ProjectPageRecipe { title, project_ids }
	}

	fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
		dependencies.extend::<Project>(recipe.project_ids.iter().copied());
	}

	fn materialize(
		&self,
		recipe: &Self::Recipe,
		entities: &super::EntityReader<'_>,
	) -> ProjectionMaterialization<ProjectPage> {
		match entities.required_vec::<Project>(&recipe.project_ids) {
			ProjectionMaterialization::Ready(projects) => {
				ProjectionMaterialization::Ready(ProjectPage {
					title: recipe.title.clone(),
					projects,
				})
			}
			ProjectionMaterialization::MissingRequired => {
				ProjectionMaterialization::MissingRequired
			}
		}
	}

	fn apply_removals(
		&self,
		recipe: &mut Self::Recipe,
		removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval {
		let previous_len = recipe.project_ids.len();
		recipe
			.project_ids
			.retain(|id| !removed.contains::<Project>(id));
		ProjectionRemoval::from_changed(previous_len != recipe.project_ids.len())
	}
}

#[derive(Clone, Copy)]
struct UndeclaredProjectProjection;

impl EntityProjection<Project> for UndeclaredProjectProjection {
	type Recipe = u64;

	const SCHEMA: &'static str = "reactive.entity.tests.undeclared-project-v1";

	fn normalize(&self, value: Project, _entities: &mut super::EntityWriter<'_>) -> Self::Recipe {
		value.id
	}

	fn dependencies(&self, _recipe: &Self::Recipe, _dependencies: &mut EntityDependencies) {}

	fn materialize(
		&self,
		recipe: &Self::Recipe,
		entities: &super::EntityReader<'_>,
	) -> ProjectionMaterialization<Project> {
		entities.required::<Project>(recipe)
	}

	fn apply_removals(
		&self,
		_recipe: &mut Self::Recipe,
		_removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval {
		ProjectionRemoval::Unchanged
	}
}

#[derive(Clone, Copy)]
struct EmptySchemaProjection;

impl EntityProjection<Project> for EmptySchemaProjection {
	type Recipe = u64;

	const SCHEMA: &'static str = "";

	fn normalize(&self, value: Project, entities: &mut super::EntityWriter<'_>) -> Self::Recipe {
		let id = value.id;
		entities.upsert(value);
		id
	}

	fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
		dependencies.extend::<Project>([*recipe]);
	}

	fn materialize(
		&self,
		recipe: &Self::Recipe,
		entities: &super::EntityReader<'_>,
	) -> ProjectionMaterialization<Project> {
		entities.required::<Project>(recipe)
	}

	fn apply_removals(
		&self,
		_recipe: &mut Self::Recipe,
		_removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval {
		ProjectionRemoval::Unchanged
	}
}

#[derive(Clone)]
struct StatefulProjection(u8);

impl EntityProjection<Project> for StatefulProjection {
	type Recipe = u64;

	const SCHEMA: &'static str = "reactive.entity.tests.stateful-v1";

	fn normalize(&self, value: Project, entities: &mut super::EntityWriter<'_>) -> Self::Recipe {
		let _state = self.0;
		let id = value.id;
		entities.upsert(value);
		id
	}

	fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
		dependencies.extend::<Project>([*recipe]);
	}

	fn materialize(
		&self,
		recipe: &Self::Recipe,
		entities: &super::EntityReader<'_>,
	) -> ProjectionMaterialization<Project> {
		entities.required::<Project>(recipe)
	}

	fn apply_removals(
		&self,
		_recipe: &mut Self::Recipe,
		_removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval {
		ProjectionRemoval::Unchanged
	}
}

#[test]
fn projection_round_trips_standard_and_composite_recipes() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		let direct = ErasedEntityProjection::new(
			"reactive.entity.tests.direct-project",
			EntityValue::<Project>::new(),
		);
		let optional = ErasedEntityProjection::new(
			"reactive.entity.tests.optional-project",
			OptionalEntity::<Project>::new(),
		);
		let vector = ErasedEntityProjection::new(
			"reactive.entity.tests.project-list",
			EntityVec::<Project>::new(),
		);
		let composite = ErasedEntityProjection::new(
			"reactive.entity.tests.project-page",
			ProjectPageProjection,
		);
		let direct_recipe = RefCell::new(None);
		let optional_recipe = RefCell::new(None);
		let vector_recipe = RefCell::new(None);
		let composite_recipe = RefCell::new(None);

		arena.update_entities_with_test_precommit(
			|writer| {
				direct_recipe.replace(Some(direct.normalize(
					Project {
						id: 1,
						name: "direct".to_string(),
					},
					writer,
				)));
				optional_recipe.replace(Some(optional.normalize(
					Some(Project {
						id: 2,
						name: "optional".to_string(),
					}),
					writer,
				)));
				vector_recipe.replace(Some(vector.normalize(
					vec![
						Project {
							id: 3,
							name: "first".to_string(),
						},
						Project {
							id: 4,
							name: "second".to_string(),
						},
						Project {
							id: 3,
							name: "first".to_string(),
						},
						Project {
							id: 9,
							name: "third".to_string(),
						},
					],
					writer,
				)));
				composite_recipe.replace(Some(composite.normalize(
					ProjectPage {
						title: "projects".to_string(),
						projects: vec![
							Project {
								id: 5,
								name: "alpha".to_string(),
							},
							Project {
								id: 6,
								name: "beta".to_string(),
							},
						],
					},
					writer,
				)));
			},
			|overlay| {
				let cloned_recipe = vector.clone_recipe(vector_recipe.borrow().as_deref().unwrap());
				assert_eq!(
					direct.materialize(direct_recipe.borrow().as_deref().unwrap(), overlay),
					ProjectionMaterialization::Ready(Project {
						id: 1,
						name: "direct".to_string(),
					}),
				);
				assert_eq!(
					optional.materialize(optional_recipe.borrow().as_deref().unwrap(), overlay),
					ProjectionMaterialization::Ready(Some(Project {
						id: 2,
						name: "optional".to_string(),
					})),
				);
				assert_eq!(
					vector.materialize(vector_recipe.borrow().as_deref().unwrap(), overlay),
					ProjectionMaterialization::Ready(vec![
						Project {
							id: 3,
							name: "first".to_string(),
						},
						Project {
							id: 4,
							name: "second".to_string(),
						},
						Project {
							id: 3,
							name: "first".to_string(),
						},
						Project {
							id: 9,
							name: "third".to_string(),
						},
					]),
				);
				assert_eq!(
					vector.materialize(cloned_recipe.as_ref(), overlay),
					ProjectionMaterialization::Ready(vec![
						Project {
							id: 3,
							name: "first".to_string(),
						},
						Project {
							id: 4,
							name: "second".to_string(),
						},
						Project {
							id: 3,
							name: "first".to_string(),
						},
						Project {
							id: 9,
							name: "third".to_string(),
						},
					]),
				);
				let recipe_json =
					composite.recipe_to_json(composite_recipe.borrow().as_deref().unwrap());
				let restored_recipe = composite.recipe_from_json(&recipe_json);
				assert_eq!(
					composite.materialize(restored_recipe.as_ref(), overlay),
					ProjectionMaterialization::Ready(ProjectPage {
						title: "projects".to_string(),
						projects: vec![
							Project {
								id: 5,
								name: "alpha".to_string(),
							},
							Project {
								id: 6,
								name: "beta".to_string(),
							},
						],
					}),
				);
			},
		);

		let removed = RemovedEntities::from_ids::<Project>([1, 2, 3, 5, 9]);
		assert_eq!(
			direct.apply_removals(direct_recipe.borrow_mut().as_deref_mut().unwrap(), &removed),
			ProjectionRemoval::MissingRequired,
		);
		assert_eq!(
			optional.apply_removals(
				optional_recipe.borrow_mut().as_deref_mut().unwrap(),
				&removed
			),
			ProjectionRemoval::Updated,
		);
		assert_eq!(
			vector.apply_removals(vector_recipe.borrow_mut().as_deref_mut().unwrap(), &removed),
			ProjectionRemoval::Updated,
		);
		assert_eq!(
			composite.apply_removals(
				composite_recipe.borrow_mut().as_deref_mut().unwrap(),
				&removed
			),
			ProjectionRemoval::Updated,
		);
		arena.update_entities_with_test_precommit(
			|_| {},
			|overlay| {
				assert_eq!(
					optional.materialize(optional_recipe.borrow().as_deref().unwrap(), overlay),
					ProjectionMaterialization::Ready(None),
				);
				assert_eq!(
					vector.materialize(vector_recipe.borrow().as_deref().unwrap(), overlay),
					ProjectionMaterialization::Ready(vec![Project {
						id: 4,
						name: "second".to_string(),
					}]),
				);
			},
		);
	});
}

#[test]
fn dependencies_hydrate_declared_typed_entities() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		let mut dependencies = EntityDependencies::default();
		dependencies.extend::<Project>([8]);
		let group = EntityHydrationGroup::new(
			Project::TYPE,
			vec![EntityHydrationRecord::new(
				serde_json::json!(8),
				serde_json::json!({ "id": 8, "name": "hydrated" }),
			)],
		);

		arena.update_entities(|writer| dependencies.hydrate(&group, writer));

		assert_eq!(
			arena.entity::<Project>(8).get(),
			Some(Project {
				id: 8,
				name: "hydrated".to_string(),
			}),
		);
	});
}

#[test]
fn projection_rejects_empty_schemas_and_stateful_adapters() {
	let empty_schema_panic = catch_unwind(|| {
		let _ = ErasedEntityProjection::<Project>::new(
			"reactive.entity.tests.empty-schema",
			EmptySchemaProjection,
		);
	})
	.expect_err("an empty projection schema must panic");
	assert_eq!(
		panic_message(empty_schema_panic),
		format!(
			"entity projection adapter `{}` for query family `reactive.entity.tests.empty-schema` with schema `` must define a non-empty schema",
			std::any::type_name::<EmptySchemaProjection>(),
		),
	);

	let stateful_panic = catch_unwind(|| {
		let _ = ErasedEntityProjection::<Project>::new(
			"reactive.entity.tests.stateful",
			StatefulProjection(1),
		);
	})
	.expect_err("a stateful projection adapter must panic");
	assert_eq!(
		panic_message(stateful_panic),
		format!(
			"entity projection adapter `{}` for query family `reactive.entity.tests.stateful` with schema `reactive.entity.tests.stateful-v1` must be zero-sized, but its size is 1 bytes",
			std::any::type_name::<StatefulProjection>(),
		),
	);
}

#[test]
fn projection_panics_when_materialization_reads_an_undeclared_entity() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 7,
				name: "undeclared".to_string(),
			});
		});
		let projection = ErasedEntityProjection::new(
			"reactive.entity.tests.undeclared-project",
			UndeclaredProjectProjection,
		);
		let recipe = RefCell::new(None);

		let panic = catch_unwind(AssertUnwindSafe(|| {
			arena.update_entities_with_test_precommit(
				|writer| {
					recipe.replace(Some(projection.normalize(
						Project {
							id: 7,
							name: "undeclared".to_string(),
						},
						writer,
					)));
				},
				|overlay| {
					let _ = projection.materialize(recipe.borrow().as_deref().unwrap(), overlay);
				},
			);
		}))
		.expect_err("reading an undeclared entity must panic");

		assert_eq!(
			panic_message(panic),
			format!(
				"entity projection adapter `{}` for query family `reactive.entity.tests.undeclared-project` with schema `reactive.entity.tests.undeclared-project-v1` accessed undeclared entity `reactive.entity.tests.project` with canonical ID `7`",
				std::any::type_name::<UndeclaredProjectProjection>(),
			),
		);
	});
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
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
	let mut registry = EntityTypeRegistry::new();
	let panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
		registry.register::<EmptyTypeEntity>();
	}))
	.expect_err("an empty entity TYPE must panic");

	assert!(panic_message(panic).contains("entity TYPE must not be empty"));
}

#[test]
fn identity_rejects_incompatible_type_reuse() {
	let mut registry = EntityTypeRegistry::new();
	registry.register::<ConflictingProject>();
	let panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
		registry.register::<ConflictingTask>();
	}))
	.expect_err("an incompatible entity TYPE reuse must panic");
	let message = panic_message(panic);

	assert!(message.contains(std::any::type_name::<ConflictingProject>()));
	assert!(message.contains(std::any::type_name::<ConflictingTask>()));
	assert!(message.contains(std::any::type_name::<u64>()));
	assert!(message.contains(std::any::type_name::<String>()));
}

#[test]
fn entity_type_registration_is_isolated_per_arena() {
	ReactiveScope::run(|| {
		let first_arena = EntityArena::new(Duration::from_secs(300));
		let second_arena = EntityArena::new(Duration::from_secs(300));

		let _first = first_arena.entity::<ConflictingProject>(7);
		let _second = second_arena.entity::<ConflictingTask>("7".to_string());

		assert_eq!(first_arena.handle_lease_count::<ConflictingProject>(&7), 1);
		assert_eq!(
			second_arena.handle_lease_count::<ConflictingTask>(&"7".to_string()),
			1
		);
	});
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
fn store_rejects_an_overlay_when_one_operation_is_stale() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		let first = arena.entity::<Project>(1);
		let second = arena.entity::<Project>(2);

		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 2,
				name: "newer".to_string(),
			});
		});
		arena.update_entities_with_test_precommit(
			|writer| {
				writer.upsert(Project {
					id: 1,
					name: "stale transaction".to_string(),
				});
				writer.upsert(Project {
					id: 2,
					name: "stale".to_string(),
				});
			},
			|_| {
				arena.update_entities(|writer| {
					writer.upsert(Project {
						id: 2,
						name: "reentrant".to_string(),
					});
				});
			},
		);

		assert_eq!(first.get(), None);
		assert_eq!(
			second.get(),
			Some(Project {
				id: 2,
				name: "reentrant".to_string(),
			}),
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

#[test]
fn gc_observes_handle_and_dependency_lease_retention() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::ZERO);
		let handle = arena.entity::<Project>(1);
		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 1,
				name: "retained".to_string(),
			});
		});
		assert_eq!(arena.handle_lease_count::<Project>(&1), 1);
		assert!(arena.entity_record_exists_for_test::<Project>(&1));
		drop(handle);
		assert_eq!(arena.handle_lease_count::<Project>(&1), 0);
	});
}

#[test]
fn standalone_arena_collects_zero_grace_records_after_the_final_handle() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::ZERO);
		let handle = arena.entity::<Project>(1);
		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 1,
				name: "temporary".to_string(),
			});
		});

		drop(handle);

		assert!(!arena.entity_record_exists_for_test::<Project>(&1));
		assert!(arena.entity::<Project>(1).get().is_none());
	});
}

#[test]
fn entity_handle_keeps_arena_alive_after_owner_drop() {
	let handle = ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(300));
		arena.update_entities(|writer| {
			writer.upsert(Project {
				id: 1,
				name: "retained".to_string(),
			});
		});
		arena.entity::<Project>(1)
	});

	assert_eq!(
		handle.get(),
		Some(Project {
			id: 1,
			name: "retained".to_string(),
		})
	);
}

#[test]
fn gc_generation_changes_when_a_handle_is_reacquired() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(1));
		let first = arena.entity::<Project>(1);
		let generation = arena.entity_gc_generation_for_test::<Project>(&1);
		drop(first);
		let scheduled = arena.entity_gc_generation_for_test::<Project>(&1);
		assert!(scheduled > generation);
		let second = arena.entity::<Project>(1);
		assert!(arena.entity_gc_generation_for_test::<Project>(&1) > scheduled);
		assert!(second.get().is_none());
	});
}

#[test]
fn gc_keeps_tombstones_until_their_grace_deadline() {
	ReactiveScope::run(|| {
		let arena = EntityArena::new(Duration::from_secs(1));
		arena.update_entities(|writer| writer.remove::<Project>(&1));
		assert!(arena.record_is_removed::<Project>(&1));
		assert!(arena.entity_record_exists_for_test::<Project>(&1));
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
