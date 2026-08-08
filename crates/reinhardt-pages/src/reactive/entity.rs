//! Stable identities and projections for normalized reactive entities.
//!
//! [`Entity`] associates an application-wide entity type with a canonical,
//! serializable identifier. The entity cache uses this contract to distinguish
//! entities from different types even when their raw IDs are identical. Keep
//! [`Entity::TYPE`] non-empty and stable for the lifetime of the cache contract;
//! changing it intentionally creates a new namespace. Reusing a `TYPE` for an
//! incompatible entity or ID Rust type is rejected by the arena.
//!
//! Normalization is opt-in at the query descriptor. Plain queries keep their
//! existing storage and hydration path. A normalized descriptor stores entity
//! records plus a serializable projection recipe, while observers continue to
//! receive the original `T` value through [`crate::reactive::QueryHandle`].
//! The public entity API is target-neutral. Import the same contracts from
//! [`crate::reactive`] or [`crate::prelude`] on native SSR and WASM clients:
//! [`EntityArena`], [`EntityHandle`], [`EntityProjection`], [`EntityValue`],
//! [`OptionalEntity`], [`EntityVec`], [`EntityDependencies`], [`EntityReader`],
//! [`EntityWriter`], [`ProjectionMaterialization`], [`ProjectionRemoval`], and
//! [`RemovedEntities`]. Hydration table types remain private transport details;
//! projection adapters are public, application-owned cache contracts.
//!
//! # Standard projection adapters
//!
//! [`EntityValue`] is the required single-entity adapter, [`OptionalEntity`] is
//! the optional single-entity adapter, and [`EntityVec`] stores an ordered
//! vector of required entities. Add one with [`crate::reactive::QueryDescriptor::with_entities`]:
//!
//! ```rust,no_run
//! use reinhardt_pages::{Entity, EntityValue, QueryFamily};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct Project {
//!     id: u64,
//!     name: String,
//! }
//!
//! impl Entity for Project {
//!     type Id = u64;
//!     const TYPE: &'static str = "example.project";
//!
//!     fn entity_id(&self) -> Self::Id {
//!         self.id
//!     }
//! }
//!
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct LoadError;
//!
//! let family = QueryFamily::<u64, Project, LoadError>::new("projects.detail.v1");
//! let descriptor = family
//!     .query(7, || async {
//!         Ok::<_, LoadError>(Project {
//!             id: 7,
//!             name: String::from("Reinhardt"),
//!         })
//!     })
//!     .with_entities(EntityValue::<Project>::new());
//! let _ = descriptor;
//! ```
//!
//! The adapters have different removal semantics. Removing the required value
//! makes internal materialization report `MissingRequired`, marks the query
//! stale (`normalization_missing` internally), and always retains its last
//! successful `T` with `QueryStatus::Success`, even for inactive or disabled
//! handles. Only an active enabled [`crate::reactive::QueryHandle`] observer
//! automatically schedules at most one recovery refetch; inactive or disabled
//! handles wait for an enabled mount or an explicit refetch. Removing an
//! optional value changes it to `None`; removing an ID from an [`EntityVec`]
//! removes that ID while preserving the remaining order. A direct
//! [`EntityHandle::get`] likewise returns `None` for a vacant or tombstoned
//! record.
//!
//! # Custom projections
//!
//! Implement [`EntityProjection`] when a result contains several entities or a
//! non-entity recipe. The adapter must be a zero-sized type with no runtime
//! state, give its recipe a versioned non-empty [`EntityProjection::SCHEMA`],
//! declare every identity it may read, and use [`EntityReader`] for all
//! materialization reads. Upserts are complete replacements: a projection does
//! not infer collection membership, relationships, cascades, patches, or
//! optimistic rollback.
//!
//! ```rust,no_run
//! use reinhardt_pages::{
//!     Entity, EntityDependencies, EntityProjection, EntityReader, EntityWriter,
//!     ProjectionMaterialization, ProjectionRemoval, RemovedEntities,
//! };
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct Project {
//!     id: u64,
//!     name: String,
//! }
//! impl Entity for Project {
//!     type Id = u64;
//!     const TYPE: &'static str = "example.project";
//!     fn entity_id(&self) -> Self::Id { self.id }
//! }
//!
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct User {
//!     id: u64,
//!     name: String,
//! }
//! impl Entity for User {
//!     type Id = u64;
//!     const TYPE: &'static str = "example.user";
//!     fn entity_id(&self) -> Self::Id { self.id }
//! }
//!
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct ProjectView {
//!     project: Project,
//!     owner: Option<User>,
//! }
//!
//! #[derive(Clone, Copy)]
//! struct ProjectViewProjection;
//!
//! impl EntityProjection<ProjectView> for ProjectViewProjection {
//!     type Recipe = (u64, Option<u64>);
//!     const SCHEMA: &'static str = "project-view.v1";
//!
//!     fn normalize(
//!         &self,
//!         value: ProjectView,
//!         entities: &mut EntityWriter<'_>,
//!     ) -> Self::Recipe {
//!         let ProjectView { project, owner } = value;
//!         let project_id = project.entity_id();
//!         entities.upsert(project);
//!         let owner_id = owner.map(|owner| {
//!             let id = owner.entity_id();
//!             entities.upsert(owner);
//!             id
//!         });
//!         (project_id, owner_id)
//!     }
//!
//!     fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
//!         dependencies.extend::<Project>([recipe.0]);
//!         if let Some(owner_id) = recipe.1 {
//!             dependencies.extend::<User>([owner_id]);
//!         }
//!     }
//!
//!     fn materialize(
//!         &self,
//!         recipe: &Self::Recipe,
//!         entities: &EntityReader<'_>,
//!     ) -> ProjectionMaterialization<ProjectView> {
//!         let project = match entities.required::<Project>(&recipe.0) {
//!             ProjectionMaterialization::Ready(project) => project,
//!             ProjectionMaterialization::MissingRequired => {
//!                 return ProjectionMaterialization::MissingRequired;
//!             }
//!         };
//!         let owner = recipe.1.as_ref().and_then(|id| entities.optional::<User>(id));
//!         ProjectionMaterialization::Ready(ProjectView { project, owner })
//!     }
//!
//!     fn apply_removals(
//!         &self,
//!         recipe: &mut Self::Recipe,
//!         removed: &RemovedEntities<'_>,
//!     ) -> ProjectionRemoval {
//!         if removed.contains::<Project>(&recipe.0) {
//!             return ProjectionRemoval::MissingRequired;
//!         }
//!         if let Some(owner_id) = recipe.1
//!             && removed.contains::<User>(&owner_id)
//!         {
//!             recipe.1 = None;
//!             return ProjectionRemoval::Updated;
//!         }
//!         ProjectionRemoval::Unchanged
//!     }
//! }
//! ```
//!
//! # Atomic entity mutations and leases
//!
//! [`crate::reactive::QueryClient::upsert_entity`] and
//! [`crate::reactive::QueryClient::remove_entity`] are convenience wrappers
//! around [`crate::reactive::QueryClient::update_entities`]. The transaction
//! stages all replacements and tombstones, validates them, then publishes every
//! affected query snapshot and entity signal in one reactive batch. Call these
//! methods only after a mutation succeeds. `upsert` replaces the whole record;
//! it does not infer membership or cascade to related entities.
//!
//! ```rust,no_run
//! use reinhardt_pages::{Entity, QueryClient, QueryDefaults};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct Project { id: u64, name: String }
//! impl Entity for Project {
//!     type Id = u64;
//!     const TYPE: &'static str = "example.project";
//!     fn entity_id(&self) -> Self::Id { self.id }
//! }
//!
//! let client = QueryClient::new(QueryDefaults::new());
//! client.upsert_entity(Project { id: 7, name: String::from("First") });
//! client.update_entities(|entities| {
//!     entities.upsert(Project { id: 7, name: String::from("Replacement") });
//! });
//! client.remove_entity::<Project>(&7);
//! assert!(client.entity::<Project>(7).get().is_none());
//! ```
//!
//! Entity leases keep records alive while a query dependency or an
//! [`EntityHandle`] exists. When the lease count reaches zero, the arena
//! schedules a GC deadline for the present record or tombstone using the
//! client's default `gc_time`. Collection is blocked when an active query
//! ticket is older than the record's last applied write ticket, then rechecked
//! when that ticket is dropped. Reacquiring a handle invalidates an older
//! deadline.
//!
//! # SSR and hydration
//!
//! A request-owned [`crate::reactive::QueryClient::new_ssr`] tracks the entity
//! identities read by normalized queries and handles. The renderer serializes
//! each reachable `(TYPE, canonical ID)` once, even when several recipes share
//! it. Browser hydration installs that table into the existing application
//! client before the first observer materializes its recipe, so a normalized
//! query can render without a duplicate fetch. Invalid versions, duplicate
//! identities, type mismatches, or missing required entities reject the
//! normalized snapshot; plain query snapshots continue to use their existing
//! wire format.
//!
//! # Query Client V2 compatibility
//!
//! Normalization is a descriptor-level opt-in and does not change the Query
//! Client V2 family, key, options, or status APIs. Existing plain descriptors
//! require no migration. [`crate::reactive::QueryHandle<T, E>`] still exposes
//! `T` through `snapshot()` and `data()`, not an internal recipe or type-erased
//! entity record. Add `.with_entities(...)` only to families whose result
//! should participate in the normalized cache.

pub(crate) mod hydration;
mod identity;
mod projection;
mod store;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

pub(crate) use hydration::{
	ENTITY_TABLE_HYDRATION_ID, ENTITY_TABLE_VERSION, EntityHydrationEnvelope, EntityHydrationRow,
	NormalizedHydrationKind, NormalizedQueryHydrationSnapshot, NormalizedQueryHydrationState,
};
pub use identity::Entity;
pub(crate) use identity::EntityIdentity;
pub use projection::{
	EntityDependencies, EntityProjection, EntityReader, EntityValue, EntityVec, OptionalEntity,
	ProjectionMaterialization, ProjectionRemoval, RemovedEntities,
};
pub(crate) use projection::{ErasedEntityProjection, erase_projection};
pub use store::{EntityArena, EntityHandle, EntityWriter};
// Query-client completion consumes these staged store boundaries in the next slice.
#[allow(unused_imports)]
pub(crate) use store::{EntityOverlay, EntityStaging, EntityWriteTicket, QueryTicketLease};
