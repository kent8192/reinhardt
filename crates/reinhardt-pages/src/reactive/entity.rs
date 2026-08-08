//! Stable identities for normalized reactive entities.
//!
//! [`Entity`] associates an application-wide entity type with a canonical,
//! serializable identifier. The entity cache uses this contract to distinguish
//! entities from different types even when their raw IDs are identical.

mod identity;
mod projection;
mod store;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

pub use identity::Entity;
pub(crate) use identity::EntityIdentity;
pub use projection::{
	EntityDependencies, EntityProjection, EntityReader, EntityValue, EntityVec, OptionalEntity,
	ProjectionMaterialization, ProjectionRemoval, RemovedEntities,
};
pub(crate) use store::EntityOverlay;
pub use store::{EntityArena, EntityHandle, EntityWriter};
