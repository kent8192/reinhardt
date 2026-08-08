use std::any::{TypeId, type_name};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, OnceLock};

use serde::{Serialize, de::DeserializeOwned};

use crate::reactive::query::canonical_json;

/// Describes a value that can be stored in the normalized entity cache.
///
/// `TYPE` is a stable, application-wide cache contract. Changing it creates a
/// distinct entity namespace; reusing it for incompatible Rust types panics.
pub trait Entity: Clone + Serialize + DeserializeOwned + 'static {
	type Id: Clone + Eq + Hash + Serialize + DeserializeOwned + 'static;

	const TYPE: &'static str;

	fn entity_id(&self) -> Self::Id;
}

/// Type-erased cache identity for one entity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct EntityIdentity {
	entity_type: &'static str,
	canonical_id: String,
}

impl EntityIdentity {
	pub(crate) fn of<E>(id: &E::Id) -> Self
	where
		E: Entity,
	{
		EntityTypeRegistry::register::<E>();
		let canonical_id = canonical_json::encode(id).unwrap_or_else(|error| {
			panic!(
				"failed to encode entity TYPE `{}` ID type `{}` as canonical JSON: {error}",
				E::TYPE,
				type_name::<E::Id>(),
			)
		});

		Self {
			entity_type: E::TYPE,
			canonical_id,
		}
	}

	pub(crate) fn entity_type(&self) -> &'static str {
		self.entity_type
	}

	pub(crate) fn canonical_id(&self) -> &str {
		&self.canonical_id
	}
}

#[derive(Clone, Copy)]
pub(crate) struct EntityTypeRegistration {
	entity: TypeId,
	id: TypeId,
	entity_name: &'static str,
	id_name: &'static str,
}

impl EntityTypeRegistration {
	fn of<E>() -> Self
	where
		E: Entity,
	{
		Self {
			entity: TypeId::of::<E>(),
			id: TypeId::of::<E::Id>(),
			entity_name: type_name::<E>(),
			id_name: type_name::<E::Id>(),
		}
	}

	fn is_compatible_with(&self, other: Self) -> bool {
		self.entity == other.entity && self.id == other.id
	}
}

pub(crate) struct EntityTypeRegistry {
	registrations: HashMap<&'static str, EntityTypeRegistration>,
}

impl EntityTypeRegistry {
	fn global() -> &'static Mutex<Self> {
		static REGISTRY: OnceLock<Mutex<EntityTypeRegistry>> = OnceLock::new();
		REGISTRY.get_or_init(|| {
			Mutex::new(Self {
				registrations: HashMap::new(),
			})
		})
	}

	fn register<E>()
	where
		E: Entity,
	{
		if E::TYPE.is_empty() {
			panic!(
				"entity TYPE must not be empty for entity type `{}` with ID type `{}`",
				type_name::<E>(),
				type_name::<E::Id>(),
			);
		}

		let registration = EntityTypeRegistration::of::<E>();
		let conflict = {
			let mut registry = Self::global()
				.lock()
				.unwrap_or_else(|poisoned| poisoned.into_inner());
			match registry.registrations.get(E::TYPE).copied() {
				Some(existing) if existing.is_compatible_with(registration) => None,
				Some(existing) => Some(existing),
				None => {
					registry.registrations.insert(E::TYPE, registration);
					None
				}
			}
		};

		if let Some(existing) = conflict {
			panic!(
				"entity TYPE `{}` is already registered for entity type `{}` with ID type `{}`; \
				 cannot reuse it for entity type `{}` with ID type `{}`",
				E::TYPE,
				existing.entity_name,
				existing.id_name,
				registration.entity_name,
				registration.id_name,
			);
		}
	}
}
