//! Stable identities for normalized reactive entities.
//!
//! [`Entity`] associates an application-wide entity type with a canonical,
//! serializable identifier. The entity cache uses this contract to distinguish
//! entities from different types even when their raw IDs are identical.
//!
//! The public entity API is target-neutral. Import the same contracts from
//! [`crate::reactive`] or [`crate::prelude`] on native SSR and WASM clients:
//! [`EntityArena`], [`EntityHandle`], [`EntityProjection`], [`EntityValue`],
//! [`OptionalEntity`], [`EntityVec`], [`EntityDependencies`], [`EntityReader`],
//! [`EntityWriter`], [`ProjectionMaterialization`], [`ProjectionRemoval`], and
//! [`RemovedEntities`]. Hydration table types and adapters stay crate-private
//! because they are transport implementation details.

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
