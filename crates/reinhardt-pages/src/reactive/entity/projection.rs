use std::any::{Any, TypeId, type_name};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::mem::size_of;

use serde::{Serialize, de::DeserializeOwned};

use super::{Entity, EntityArena, EntityIdentity, EntityOverlay, EntityWriter};

/// Describes how a query result is normalized into entities and reconstructed.
pub trait EntityProjection<T>: Clone + 'static {
	/// The serializable non-entity data needed to reconstruct the result.
	type Recipe: Clone + Serialize + DeserializeOwned + 'static;

	/// Stable versioned schema for this recipe.
	const SCHEMA: &'static str;

	/// Writes entities from a result and returns its non-entity reconstruction recipe.
	fn normalize(&self, value: T, entities: &mut EntityWriter<'_>) -> Self::Recipe;

	/// Declares every entity identity that materialization may access.
	fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies);

	/// Reconstructs the result from the candidate entity overlay.
	fn materialize(
		&self,
		recipe: &Self::Recipe,
		entities: &EntityReader<'_>,
	) -> ProjectionMaterialization<T>;

	/// Applies entity tombstones to a stored recipe.
	fn apply_removals(
		&self,
		recipe: &mut Self::Recipe,
		removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval;
}

/// The outcome of reconstructing a result from normalized entities.
#[derive(Debug, Eq, PartialEq)]
pub enum ProjectionMaterialization<T> {
	/// Every required entity was available.
	Ready(T),
	/// At least one required entity was absent or tombstoned.
	MissingRequired,
}

/// The outcome of applying entity tombstones to a projection recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionRemoval {
	/// No recipe data referenced a removed entity.
	Unchanged,
	/// The recipe was updated and remains materializable.
	Updated,
	/// A required entity was removed, invalidating the recipe.
	MissingRequired,
}

impl ProjectionRemoval {
	/// Converts whether a recipe changed into its removal outcome.
	pub const fn from_changed(changed: bool) -> Self {
		if changed {
			Self::Updated
		} else {
			Self::Unchanged
		}
	}
}

/// Exact entity identities declared by a projection recipe.
#[derive(Default)]
pub struct EntityDependencies {
	identities: HashSet<EntityIdentity>,
	loaders: HashMap<&'static str, Box<dyn ErasedEntityHydrationLoader>>,
}

impl EntityDependencies {
	/// Declares each ID as a required typed entity dependency.
	pub fn extend<E>(&mut self, ids: impl IntoIterator<Item = E::Id>)
	where
		E: Entity,
	{
		self.identities
			.extend(ids.into_iter().map(|id| EntityIdentity::of::<E>(&id)));
		match self.loaders.entry(E::TYPE) {
			std::collections::hash_map::Entry::Occupied(entry) => {
				let loader = entry.get();
				if loader.entity_type_id() != TypeId::of::<E>()
					|| loader.id_type_id() != TypeId::of::<E::Id>()
				{
					panic!(
						"entity dependency TYPE `{}` is already registered for entity type `{}` with ID type `{}`; cannot reuse it for entity type `{}` with ID type `{}`",
						E::TYPE,
						loader.entity_name(),
						loader.id_name(),
						type_name::<E>(),
						type_name::<E::Id>(),
					);
				}
			}
			std::collections::hash_map::Entry::Vacant(entry) => {
				entry.insert(Box::new(TypedEntityHydrationLoader::<E>(PhantomData)));
			}
		}
	}

	pub(crate) fn hydrate(&self, group: &EntityHydrationGroup, entities: &mut EntityWriter<'_>) {
		let loader = self.loaders.get(group.entity_type()).unwrap_or_else(|| {
			panic!(
				"entity hydration group for TYPE `{}` was not declared by this projection",
				group.entity_type(),
			)
		});
		loader.hydrate(group, &self.identities, entities);
	}

	pub(crate) fn acquire_leases(
		&self,
		arena: &EntityArena,
	) -> HashMap<EntityIdentity, Box<dyn Any>> {
		let mut leases = HashMap::with_capacity(self.identities.len());
		for loader in self.loaders.values() {
			loader.acquire_leases(arena, &self.identities, &mut leases);
		}
		leases
	}

	pub(crate) fn removed_identities(
		&self,
		overlay: &EntityOverlay<'_>,
	) -> HashSet<EntityIdentity> {
		let mut removed = HashSet::new();
		for loader in self.loaders.values() {
			loader.collect_removed(overlay, &self.identities, &mut removed);
		}
		removed
	}

	pub(crate) fn identities(&self) -> &HashSet<EntityIdentity> {
		&self.identities
	}

	#[allow(dead_code)] // Used by the staged erased projection materialization bridge below.
	fn contains(&self, identity: &EntityIdentity) -> bool {
		self.identities.contains(identity)
	}
}

pub(crate) struct EntityHydrationGroup {
	entity_type: String,
	records: Vec<EntityHydrationRecord>,
}

impl EntityHydrationGroup {
	pub(crate) fn new(entity_type: impl Into<String>, records: Vec<EntityHydrationRecord>) -> Self {
		Self {
			entity_type: entity_type.into(),
			records,
		}
	}

	fn entity_type(&self) -> &str {
		&self.entity_type
	}

	fn records(&self) -> &[EntityHydrationRecord] {
		&self.records
	}
}

pub(crate) struct EntityHydrationRecord {
	id: serde_json::Value,
	value: serde_json::Value,
}

impl EntityHydrationRecord {
	pub(crate) fn new(id: serde_json::Value, value: serde_json::Value) -> Self {
		Self { id, value }
	}
}

trait ErasedEntityHydrationLoader {
	fn entity_type_id(&self) -> TypeId;
	fn id_type_id(&self) -> TypeId;
	fn entity_name(&self) -> &'static str;
	fn id_name(&self) -> &'static str;
	fn hydrate(
		&self,
		group: &EntityHydrationGroup,
		declared: &HashSet<EntityIdentity>,
		entities: &mut EntityWriter<'_>,
	);
	fn acquire_leases(
		&self,
		arena: &EntityArena,
		declared: &HashSet<EntityIdentity>,
		leases: &mut HashMap<EntityIdentity, Box<dyn Any>>,
	);
	fn collect_removed(
		&self,
		overlay: &EntityOverlay<'_>,
		declared: &HashSet<EntityIdentity>,
		removed: &mut HashSet<EntityIdentity>,
	);
}

struct TypedEntityHydrationLoader<E>(PhantomData<fn() -> E>);

impl<E> ErasedEntityHydrationLoader for TypedEntityHydrationLoader<E>
where
	E: Entity,
{
	fn entity_type_id(&self) -> TypeId {
		TypeId::of::<E>()
	}

	fn id_type_id(&self) -> TypeId {
		TypeId::of::<E::Id>()
	}

	fn entity_name(&self) -> &'static str {
		type_name::<E>()
	}

	fn id_name(&self) -> &'static str {
		type_name::<E::Id>()
	}

	fn hydrate(
		&self,
		group: &EntityHydrationGroup,
		declared: &HashSet<EntityIdentity>,
		entities: &mut EntityWriter<'_>,
	) {
		if group.entity_type() != E::TYPE {
			panic!(
				"entity hydration loader for TYPE `{}` cannot consume group for TYPE `{}`",
				E::TYPE,
				group.entity_type(),
			);
		}

		for record in group.records() {
			let id = serde_json::from_value::<E::Id>(record.id.clone()).unwrap_or_else(|error| {
				panic!(
					"entity hydration loader for TYPE `{}` failed to deserialize ID type `{}`: {error}",
					E::TYPE,
					type_name::<E::Id>(),
				)
			});
			let entity =
				serde_json::from_value::<E>(record.value.clone()).unwrap_or_else(|error| {
					panic!(
						"entity hydration loader for TYPE `{}` failed to deserialize entity type `{}`: {error}",
						E::TYPE,
						type_name::<E>(),
					)
				});
			let entity_id = entity.entity_id();
			if entity_id != id {
				panic!(
					"entity hydration loader for TYPE `{}` received an entity whose ID differs from its hydration record",
					E::TYPE,
				);
			}
			let identity = EntityIdentity::of::<E>(&id);
			if !declared.contains(&identity) {
				panic!(
					"entity hydration loader for TYPE `{}` received undeclared canonical ID `{}`",
					E::TYPE,
					identity.canonical_id(),
				);
			}
			entities.upsert(entity);
		}
	}

	fn acquire_leases(
		&self,
		arena: &EntityArena,
		declared: &HashSet<EntityIdentity>,
		leases: &mut HashMap<EntityIdentity, Box<dyn Any>>,
	) {
		for identity in declared
			.iter()
			.filter(|identity| identity.entity_type() == E::TYPE)
		{
			let id =
				serde_json::from_str::<E::Id>(identity.canonical_id()).unwrap_or_else(|error| {
					panic!(
						"entity dependency TYPE `{}` failed to deserialize canonical ID as `{}`: {error}",
						E::TYPE,
						type_name::<E::Id>(),
					)
				});
			leases.insert(identity.clone(), arena.acquire_dependency::<E>(id));
		}
	}

	fn collect_removed(
		&self,
		overlay: &EntityOverlay<'_>,
		declared: &HashSet<EntityIdentity>,
		removed: &mut HashSet<EntityIdentity>,
	) {
		for identity in declared
			.iter()
			.filter(|identity| identity.entity_type() == E::TYPE)
		{
			let id =
				serde_json::from_str::<E::Id>(identity.canonical_id()).unwrap_or_else(|error| {
					panic!(
						"entity dependency TYPE `{}` failed to deserialize canonical ID as `{}`: {error}",
						E::TYPE,
						type_name::<E::Id>(),
					)
				});
			if overlay.is_removed::<E>(&id) {
				removed.insert(identity.clone());
			}
		}
	}
}

/// Typed entity access during projection materialization.
pub struct EntityReader<'a> {
	overlay: &'a EntityOverlay<'a>,
	accessed: RefCell<HashSet<EntityIdentity>>,
}

impl<'a> EntityReader<'a> {
	#[allow(dead_code)] // Called by the staged erased projection materialization bridge below.
	pub(crate) fn new(overlay: &'a EntityOverlay<'a>) -> Self {
		Self {
			overlay,
			accessed: RefCell::new(HashSet::new()),
		}
	}

	/// Reads one required entity.
	pub fn required<E>(&self, id: &E::Id) -> ProjectionMaterialization<E>
	where
		E: Entity,
	{
		self.record::<E>(id);
		self.overlay.get::<E>(id).map_or(
			ProjectionMaterialization::MissingRequired,
			ProjectionMaterialization::Ready,
		)
	}

	/// Reads one optional entity.
	pub fn optional<E>(&self, id: &E::Id) -> Option<E>
	where
		E: Entity,
	{
		self.record::<E>(id);
		self.overlay.get::<E>(id)
	}

	/// Reads a vector of required entities in recipe order.
	pub fn required_vec<E>(&self, ids: &[E::Id]) -> ProjectionMaterialization<Vec<E>>
	where
		E: Entity,
	{
		let mut values = Vec::with_capacity(ids.len());
		let mut missing = false;
		for id in ids {
			self.record::<E>(id);
			match self.overlay.get::<E>(id) {
				Some(entity) => values.push(entity),
				None => missing = true,
			}
		}

		if missing {
			ProjectionMaterialization::MissingRequired
		} else {
			ProjectionMaterialization::Ready(values)
		}
	}

	fn record<E>(&self, id: &E::Id)
	where
		E: Entity,
	{
		self.accessed
			.borrow_mut()
			.insert(EntityIdentity::of::<E>(id));
	}

	#[allow(dead_code)] // Called after staged erased projection materialization.
	fn validate(&self, dependencies: &EntityDependencies, diagnostics: &ProjectionDiagnostics) {
		let mut undeclared = self
			.accessed
			.borrow()
			.iter()
			.filter(|identity| !dependencies.contains(identity))
			.cloned()
			.collect::<Vec<_>>();
		undeclared.sort_by(|left, right| {
			left.entity_type()
				.cmp(right.entity_type())
				.then_with(|| left.canonical_id().cmp(right.canonical_id()))
		});

		if let Some(identity) = undeclared.first() {
			panic!(
				"entity projection adapter `{}` for query family `{}` with schema `{}` accessed undeclared entity `{}` with canonical ID `{}`",
				diagnostics.adapter_name,
				diagnostics.query_family_id,
				diagnostics.schema,
				identity.entity_type(),
				identity.canonical_id(),
			);
		}
	}
}

/// A set of entity identities removed from the normalized store.
pub struct RemovedEntities<'a> {
	identities: Cow<'a, HashSet<EntityIdentity>>,
}

impl<'a> RemovedEntities<'a> {
	/// Creates an owned removed-entity set from typed IDs.
	pub fn from_ids<E>(ids: impl IntoIterator<Item = E::Id>) -> Self
	where
		E: Entity,
	{
		Self {
			identities: Cow::Owned(
				ids.into_iter()
					.map(|id| EntityIdentity::of::<E>(&id))
					.collect(),
			),
		}
	}

	#[allow(dead_code)] // Entity-store tombstone collection is introduced by the following slice.
	pub(crate) fn borrowed(identities: &'a HashSet<EntityIdentity>) -> Self {
		Self {
			identities: Cow::Borrowed(identities),
		}
	}

	/// Returns whether the typed identity was removed.
	pub fn contains<E>(&self, id: &E::Id) -> bool
	where
		E: Entity,
	{
		self.identities.contains(&EntityIdentity::of::<E>(id))
	}
}

/// A projection adapter for one required entity value.
pub struct EntityValue<E>(PhantomData<fn() -> E>);

impl<E> EntityValue<E> {
	/// Creates a zero-sized required-entity projection adapter.
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

impl<E> Default for EntityValue<E> {
	fn default() -> Self {
		Self::new()
	}
}

impl<E> Clone for EntityValue<E> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<E> Copy for EntityValue<E> {}

impl<E> EntityProjection<E> for EntityValue<E>
where
	E: Entity,
{
	type Recipe = E::Id;

	const SCHEMA: &'static str = "entity-value-v1";

	fn normalize(&self, value: E, entities: &mut EntityWriter<'_>) -> Self::Recipe {
		let id = value.entity_id();
		entities.upsert(value);
		id
	}

	fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
		dependencies.extend::<E>([recipe.clone()]);
	}

	fn materialize(
		&self,
		recipe: &Self::Recipe,
		entities: &EntityReader<'_>,
	) -> ProjectionMaterialization<E> {
		entities.required::<E>(recipe)
	}

	fn apply_removals(
		&self,
		recipe: &mut Self::Recipe,
		removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval {
		if removed.contains::<E>(recipe) {
			ProjectionRemoval::MissingRequired
		} else {
			ProjectionRemoval::Unchanged
		}
	}
}

/// A projection adapter for an optional entity value.
pub struct OptionalEntity<E>(PhantomData<fn() -> E>);

impl<E> OptionalEntity<E> {
	/// Creates a zero-sized optional-entity projection adapter.
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

impl<E> Default for OptionalEntity<E> {
	fn default() -> Self {
		Self::new()
	}
}

impl<E> Clone for OptionalEntity<E> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<E> Copy for OptionalEntity<E> {}

impl<E> EntityProjection<Option<E>> for OptionalEntity<E>
where
	E: Entity,
{
	type Recipe = Option<E::Id>;

	const SCHEMA: &'static str = "optional-entity-v1";

	fn normalize(&self, value: Option<E>, entities: &mut EntityWriter<'_>) -> Self::Recipe {
		value.map(|entity| {
			let id = entity.entity_id();
			entities.upsert(entity);
			id
		})
	}

	fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
		dependencies.extend::<E>(recipe.iter().cloned());
	}

	fn materialize(
		&self,
		recipe: &Self::Recipe,
		entities: &EntityReader<'_>,
	) -> ProjectionMaterialization<Option<E>> {
		match recipe {
			Some(id) => match entities.required::<E>(id) {
				ProjectionMaterialization::Ready(entity) => {
					ProjectionMaterialization::Ready(Some(entity))
				}
				ProjectionMaterialization::MissingRequired => {
					ProjectionMaterialization::MissingRequired
				}
			},
			None => ProjectionMaterialization::Ready(None),
		}
	}

	fn apply_removals(
		&self,
		recipe: &mut Self::Recipe,
		removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval {
		let Some(id) = recipe else {
			return ProjectionRemoval::Unchanged;
		};
		if removed.contains::<E>(id) {
			*recipe = None;
			ProjectionRemoval::Updated
		} else {
			ProjectionRemoval::Unchanged
		}
	}
}

/// A projection adapter for an ordered vector of required entity values.
pub struct EntityVec<E>(PhantomData<fn() -> E>);

impl<E> EntityVec<E> {
	/// Creates a zero-sized entity-vector projection adapter.
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

impl<E> Default for EntityVec<E> {
	fn default() -> Self {
		Self::new()
	}
}

impl<E> Clone for EntityVec<E> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<E> Copy for EntityVec<E> {}

impl<E> EntityProjection<Vec<E>> for EntityVec<E>
where
	E: Entity,
{
	type Recipe = Vec<E::Id>;

	const SCHEMA: &'static str = "entity-vec-v1";

	fn normalize(&self, value: Vec<E>, entities: &mut EntityWriter<'_>) -> Self::Recipe {
		value
			.into_iter()
			.map(|entity| {
				let id = entity.entity_id();
				entities.upsert(entity);
				id
			})
			.collect()
	}

	fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
		dependencies.extend::<E>(recipe.iter().cloned());
	}

	fn materialize(
		&self,
		recipe: &Self::Recipe,
		entities: &EntityReader<'_>,
	) -> ProjectionMaterialization<Vec<E>> {
		entities.required_vec::<E>(recipe)
	}

	fn apply_removals(
		&self,
		recipe: &mut Self::Recipe,
		removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval {
		let previous_len = recipe.len();
		recipe.retain(|id| !removed.contains::<E>(id));
		ProjectionRemoval::from_changed(previous_len != recipe.len())
	}
}

// Query-cache storage wires this bridge into long-lived projection records in the following slice.
#[allow(dead_code)]
pub(crate) struct ErasedEntityProjection<T> {
	diagnostics: ProjectionDiagnostics,
	adapter: Box<dyn ErasedProjectionAdapter<T>>,
}

#[allow(dead_code)] // The store integration invokes these erased operations in the following slice.
impl<T: 'static> ErasedEntityProjection<T> {
	pub(crate) fn new<P>(query_family_id: &'static str, projection: P) -> Self
	where
		P: EntityProjection<T>,
	{
		let diagnostics = ProjectionDiagnostics {
			adapter_type: TypeId::of::<P>(),
			adapter_name: type_name::<P>(),
			query_family_id,
			schema: P::SCHEMA,
		};
		if diagnostics.schema.is_empty() {
			panic!(
				"entity projection adapter `{}` for query family `{}` with schema `{}` must define a non-empty schema",
				diagnostics.adapter_name, diagnostics.query_family_id, diagnostics.schema,
			);
		}
		if size_of::<P>() != 0 {
			panic!(
				"entity projection adapter `{}` for query family `{}` with schema `{}` must be zero-sized, but its size is {} bytes",
				diagnostics.adapter_name,
				diagnostics.query_family_id,
				diagnostics.schema,
				size_of::<P>(),
			);
		}

		Self {
			diagnostics,
			adapter: Box::new(TypedProjectionAdapter::<P, T> {
				projection,
				marker: PhantomData,
			}),
		}
	}

	pub(crate) fn adapter_type(&self) -> TypeId {
		self.diagnostics.adapter_type
	}

	pub(crate) fn adapter_name(&self) -> &'static str {
		self.diagnostics.adapter_name
	}

	pub(crate) fn schema(&self) -> &'static str {
		self.diagnostics.schema
	}

	pub(crate) fn normalize(&self, value: T, entities: &mut EntityWriter<'_>) -> Box<dyn Any> {
		self.adapter.normalize(value, entities)
	}

	pub(crate) fn clone_recipe(&self, recipe: &dyn Any) -> Box<dyn Any> {
		self.adapter.clone_recipe(recipe, &self.diagnostics)
	}

	pub(crate) fn dependencies(&self, recipe: &dyn Any) -> EntityDependencies {
		self.adapter.dependencies(recipe, &self.diagnostics)
	}

	pub(crate) fn materialize(
		&self,
		recipe: &dyn Any,
		overlay: &EntityOverlay<'_>,
	) -> ProjectionMaterialization<T> {
		let dependencies = self.dependencies(recipe);
		let reader = EntityReader::new(overlay);
		let materialization = self.adapter.materialize(recipe, &reader, &self.diagnostics);
		reader.validate(&dependencies, &self.diagnostics);
		materialization
	}

	pub(crate) fn apply_removals(
		&self,
		recipe: &mut dyn Any,
		removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval {
		self.adapter
			.apply_removals(recipe, removed, &self.diagnostics)
	}

	pub(crate) fn recipe_to_json(&self, recipe: &dyn Any) -> serde_json::Value {
		self.adapter.recipe_to_json(recipe, &self.diagnostics)
	}

	pub(crate) fn recipe_from_json(&self, recipe: &serde_json::Value) -> Box<dyn Any> {
		self.adapter.recipe_from_json(recipe, &self.diagnostics)
	}
}

pub(crate) fn erase_projection<T, P>(
	projection: P,
	query_family_id: &'static str,
) -> ErasedEntityProjection<T>
where
	T: 'static,
	P: EntityProjection<T>,
{
	ErasedEntityProjection::new(query_family_id, projection)
}

#[allow(dead_code)] // Cloned erased adapters are retained by query-cache records in the following slice.
impl<T: 'static> Clone for ErasedEntityProjection<T> {
	fn clone(&self) -> Self {
		Self {
			diagnostics: self.diagnostics,
			adapter: self.adapter.clone_adapter(),
		}
	}
}

#[allow(dead_code)] // Diagnostics are carried by the staged erased projection bridge.
#[derive(Clone, Copy)]
struct ProjectionDiagnostics {
	adapter_type: TypeId,
	adapter_name: &'static str,
	query_family_id: &'static str,
	schema: &'static str,
}

#[allow(dead_code)] // This trait is the private type-erasure boundary for query-cache records.
trait ErasedProjectionAdapter<T>: 'static {
	fn clone_adapter(&self) -> Box<dyn ErasedProjectionAdapter<T>>;
	fn normalize(&self, value: T, entities: &mut EntityWriter<'_>) -> Box<dyn Any>;
	fn clone_recipe(&self, recipe: &dyn Any, diagnostics: &ProjectionDiagnostics) -> Box<dyn Any>;
	fn dependencies(
		&self,
		recipe: &dyn Any,
		diagnostics: &ProjectionDiagnostics,
	) -> EntityDependencies;
	fn materialize(
		&self,
		recipe: &dyn Any,
		entities: &EntityReader<'_>,
		diagnostics: &ProjectionDiagnostics,
	) -> ProjectionMaterialization<T>;
	fn apply_removals(
		&self,
		recipe: &mut dyn Any,
		removed: &RemovedEntities<'_>,
		diagnostics: &ProjectionDiagnostics,
	) -> ProjectionRemoval;
	fn recipe_to_json(
		&self,
		recipe: &dyn Any,
		diagnostics: &ProjectionDiagnostics,
	) -> serde_json::Value;
	fn recipe_from_json(
		&self,
		recipe: &serde_json::Value,
		diagnostics: &ProjectionDiagnostics,
	) -> Box<dyn Any>;
}

#[allow(dead_code)] // Concrete adapters back the staged erased projection bridge.
struct TypedProjectionAdapter<P, T> {
	projection: P,
	marker: PhantomData<fn() -> T>,
}

#[allow(dead_code)] // Recipe downcasts are used by erased projection operations above.
impl<P, T> TypedProjectionAdapter<P, T>
where
	P: EntityProjection<T>,
{
	fn recipe<'a>(
		&self,
		recipe: &'a dyn Any,
		diagnostics: &ProjectionDiagnostics,
	) -> &'a P::Recipe {
		recipe.downcast_ref::<P::Recipe>().unwrap_or_else(|| {
			panic!(
				"entity projection adapter `{}` for query family `{}` with schema `{}` received an incompatible recipe type",
				diagnostics.adapter_name, diagnostics.query_family_id, diagnostics.schema,
			)
		})
	}

	fn recipe_mut<'a>(
		&self,
		recipe: &'a mut dyn Any,
		diagnostics: &ProjectionDiagnostics,
	) -> &'a mut P::Recipe {
		recipe.downcast_mut::<P::Recipe>().unwrap_or_else(|| {
			panic!(
				"entity projection adapter `{}` for query family `{}` with schema `{}` received an incompatible recipe type",
				diagnostics.adapter_name, diagnostics.query_family_id, diagnostics.schema,
			)
		})
	}
}

#[allow(dead_code)] // This implementation is reached through the erased bridge above.
impl<P, T> ErasedProjectionAdapter<T> for TypedProjectionAdapter<P, T>
where
	P: EntityProjection<T>,
	T: 'static,
{
	fn clone_adapter(&self) -> Box<dyn ErasedProjectionAdapter<T>> {
		Box::new(Self {
			projection: self.projection.clone(),
			marker: PhantomData,
		})
	}

	fn normalize(&self, value: T, entities: &mut EntityWriter<'_>) -> Box<dyn Any> {
		Box::new(self.projection.normalize(value, entities))
	}

	fn clone_recipe(&self, recipe: &dyn Any, diagnostics: &ProjectionDiagnostics) -> Box<dyn Any> {
		Box::new(self.recipe(recipe, diagnostics).clone())
	}

	fn dependencies(
		&self,
		recipe: &dyn Any,
		diagnostics: &ProjectionDiagnostics,
	) -> EntityDependencies {
		let mut dependencies = EntityDependencies::default();
		self.projection
			.dependencies(self.recipe(recipe, diagnostics), &mut dependencies);
		dependencies
	}

	fn materialize(
		&self,
		recipe: &dyn Any,
		entities: &EntityReader<'_>,
		diagnostics: &ProjectionDiagnostics,
	) -> ProjectionMaterialization<T> {
		self.projection
			.materialize(self.recipe(recipe, diagnostics), entities)
	}

	fn apply_removals(
		&self,
		recipe: &mut dyn Any,
		removed: &RemovedEntities<'_>,
		diagnostics: &ProjectionDiagnostics,
	) -> ProjectionRemoval {
		self.projection
			.apply_removals(self.recipe_mut(recipe, diagnostics), removed)
	}

	fn recipe_to_json(
		&self,
		recipe: &dyn Any,
		diagnostics: &ProjectionDiagnostics,
	) -> serde_json::Value {
		serde_json::to_value(self.recipe(recipe, diagnostics)).unwrap_or_else(|error| {
			panic!(
				"entity projection adapter `{}` for query family `{}` with schema `{}` failed to serialize its recipe: {error}",
				diagnostics.adapter_name, diagnostics.query_family_id, diagnostics.schema,
			)
		})
	}

	fn recipe_from_json(
		&self,
		recipe: &serde_json::Value,
		diagnostics: &ProjectionDiagnostics,
	) -> Box<dyn Any> {
		Box::new(serde_json::from_value::<P::Recipe>(recipe.clone()).unwrap_or_else(|error| {
			panic!(
				"entity projection adapter `{}` for query family `{}` with schema `{}` failed to deserialize its recipe: {error}",
				diagnostics.adapter_name, diagnostics.query_family_id, diagnostics.schema,
			)
		}))
	}
}
