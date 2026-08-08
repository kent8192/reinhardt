use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};
use std::time::Duration;

use super::identity::EntityTypeRegistry;
use super::{Entity, EntityIdentity};
use crate::reactive::{Signal, batch};

/// Owns the typed records for one normalized entity cache.
#[derive(Clone)]
pub struct EntityArena {
	inner: Rc<EntityArenaInner>,
}

struct EntityArenaInner {
	buckets: RefCell<HashMap<&'static str, Rc<dyn Any>>>,
	type_registry: Rc<RefCell<EntityTypeRegistry>>,
	next_ticket: Cell<u64>,
	active_query_tickets: RefCell<BTreeMap<u64, usize>>,
	gc_time: Duration,
}

impl EntityArena {
	/// Creates an empty entity arena with the supplied retention duration.
	pub fn new(gc_time: Duration) -> Self {
		Self {
			inner: Rc::new(EntityArenaInner {
				buckets: RefCell::new(HashMap::new()),
				type_registry: Rc::new(RefCell::new(EntityTypeRegistry::new())),
				next_ticket: Cell::new(1),
				active_query_tickets: RefCell::new(BTreeMap::new()),
				gc_time,
			}),
		}
	}

	/// Returns a reactive handle for an entity identity.
	pub fn entity<E>(&self, id: E::Id) -> EntityHandle<E>
	where
		E: Entity,
	{
		self.register_entity_type::<E>();
		let _identity = EntityIdentity::of::<E>(&id);
		let bucket = self.bucket::<E>();
		let signal = {
			let mut bucket = bucket.borrow_mut();
			let record = bucket
				.records
				.entry(id.clone())
				.or_insert_with(EntityRecord::vacant);
			record.handle_lease_count += 1;
			record.signal
		};

		EntityHandle {
			lease: Rc::new(EntityHandleLease { bucket, id, signal }),
		}
	}

	/// Stages and atomically publishes a group of entity replacements and removals.
	pub fn update_entities(&self, update: impl FnOnce(&mut EntityWriter<'_>)) {
		self.update_entities_with_precommit(update, |_| {});
	}

	pub(crate) fn stage(&self, update: impl FnOnce(&mut EntityWriter<'_>)) -> EntityStaging {
		let mut staging = EntityStaging::new(Rc::clone(&self.inner.type_registry));
		update(&mut EntityWriter {
			staging: &mut staging,
		});
		staging
	}

	pub(crate) fn commit_overlay(
		&self,
		overlay: EntityOverlay<'_>,
		ticket: EntityWriteTicket,
		publish: impl FnOnce(),
	) {
		let publications = overlay.commit(ticket);
		batch(|| {
			for publication in publications {
				publication.publish();
			}
			publish();
		});
	}

	fn update_entities_with_precommit(
		&self,
		update: impl FnOnce(&mut EntityWriter<'_>),
		precommit: impl FnOnce(&EntityOverlay<'_>),
	) {
		let ticket = self.issue_mutation_ticket();
		let staging = self.stage(update);

		let overlay = EntityOverlay::new(self, staging, ticket);
		precommit(&overlay);
		self.commit_overlay(overlay, ticket, || {});
	}

	pub(crate) fn issue_mutation_ticket(&self) -> EntityWriteTicket {
		self.issue_ticket()
	}

	pub(crate) fn acquire_query_ticket(&self) -> QueryTicketLease {
		let ticket = self.issue_ticket();
		let mut tickets = self.inner.active_query_tickets.borrow_mut();
		*tickets.entry(ticket.0).or_default() += 1;
		drop(tickets);

		QueryTicketLease {
			arena: Rc::downgrade(&self.inner),
			ticket,
		}
	}

	pub(crate) fn acquire_dependency<E>(&self, id: E::Id) -> Box<dyn Any>
	where
		E: Entity,
	{
		let bucket = self.bucket::<E>();
		{
			let mut bucket = bucket.borrow_mut();
			let record = bucket
				.records
				.entry(id.clone())
				.or_insert_with(EntityRecord::vacant);
			record.dependency_lease_count += 1;
		}

		Box::new(TypedEntityDependencyLease::<E> { bucket, id })
	}

	// Retained for the staged entity-GC scheduler integration in Task 8.
	#[allow(dead_code)]
	pub(crate) fn gc_time(&self) -> Duration {
		self.inner.gc_time
	}

	fn issue_ticket(&self) -> EntityWriteTicket {
		let ticket = self.inner.next_ticket.get();
		let next = ticket
			.checked_add(1)
			.expect("entity write ticket allocator exhausted");
		self.inner.next_ticket.set(next);
		EntityWriteTicket(ticket)
	}

	fn bucket<E>(&self) -> Rc<RefCell<EntityBucket<E>>>
	where
		E: Entity,
	{
		self.register_entity_type::<E>();
		let mut buckets = self.inner.buckets.borrow_mut();
		if let Some(existing) = buckets.get(E::TYPE) {
			return Rc::clone(existing)
				.downcast::<RefCell<EntityBucket<E>>>()
				.unwrap_or_else(|_| {
					panic!(
						"entity TYPE `{}` is registered with an incompatible bucket type",
						E::TYPE,
					)
				});
		}

		let bucket = Rc::new(RefCell::new(EntityBucket::default()));
		let erased: Rc<dyn Any> = bucket.clone();
		buckets.insert(E::TYPE, erased);
		bucket
	}

	fn current<E>(&self, id: &E::Id) -> Option<E>
	where
		E: Entity,
	{
		self.register_entity_type::<E>();
		let buckets = self.inner.buckets.borrow();
		let Some(bucket) = buckets.get(E::TYPE) else {
			return None;
		};
		let bucket = Rc::clone(bucket)
			.downcast::<RefCell<EntityBucket<E>>>()
			.unwrap_or_else(|_| {
				panic!(
					"entity TYPE `{}` is registered with an incompatible bucket type",
					E::TYPE,
				)
			});
		bucket
			.borrow()
			.records
			.get(id)
			.and_then(EntityRecord::value)
	}

	fn record_ticket<E>(&self, id: &E::Id) -> Option<EntityWriteTicket>
	where
		E: Entity,
	{
		self.register_entity_type::<E>();
		let buckets = self.inner.buckets.borrow();
		let bucket = buckets.get(E::TYPE)?;
		let bucket = Rc::clone(bucket)
			.downcast::<RefCell<EntityBucket<E>>>()
			.unwrap_or_else(|_| {
				panic!(
					"entity TYPE `{}` is registered with an incompatible bucket type",
					E::TYPE,
				)
			});
		bucket
			.borrow()
			.records
			.get(id)
			.and_then(|record| record.last_write_ticket)
	}

	fn register_entity_type<E>(&self)
	where
		E: Entity,
	{
		self.inner.type_registry.borrow_mut().register::<E>();
	}

	#[cfg(test)]
	pub(crate) fn handle_lease_count<E>(&self, id: &E::Id) -> usize
	where
		E: Entity,
	{
		let bucket = self.bucket::<E>();
		bucket
			.borrow()
			.records
			.get(id)
			.map_or(0, |record| record.handle_lease_count)
	}

	#[cfg(test)]
	pub(crate) fn dependency_lease_count<E>(&self, id: &E::Id) -> usize
	where
		E: Entity,
	{
		let bucket = self.bucket::<E>();
		bucket
			.borrow()
			.records
			.get(id)
			.map_or(0, |record| record.dependency_lease_count)
	}

	#[cfg(test)]
	pub(crate) fn record_write_ticket<E>(&self, id: &E::Id) -> Option<EntityWriteTicket>
	where
		E: Entity,
	{
		self.record_ticket::<E>(id)
	}

	pub(crate) fn record_is_removed<E>(&self, id: &E::Id) -> bool
	where
		E: Entity,
	{
		let bucket = self.bucket::<E>();
		matches!(
			bucket.borrow().records.get(id).map(|record| &record.state),
			Some(EntityRecordState::Removed),
		)
	}

	#[cfg(test)]
	pub(crate) fn active_query_ticket_count(&self, ticket: EntityWriteTicket) -> usize {
		self.inner
			.active_query_tickets
			.borrow()
			.get(&ticket.0)
			.copied()
			.unwrap_or_default()
	}

	#[cfg(test)]
	pub(crate) fn update_entities_with_test_precommit(
		&self,
		update: impl FnOnce(&mut EntityWriter<'_>),
		precommit: impl FnOnce(&EntityOverlay<'_>),
	) {
		self.update_entities_with_precommit(update, precommit);
	}
}

/// A reactive, leased view of one entity record.
pub struct EntityHandle<E>
where
	E: Entity,
{
	lease: Rc<EntityHandleLease<E>>,
}

impl<E> Clone for EntityHandle<E>
where
	E: Entity,
{
	fn clone(&self) -> Self {
		Self {
			lease: Rc::clone(&self.lease),
		}
	}
}

impl<E> EntityHandle<E>
where
	E: Entity,
{
	/// Returns the current entity value, or `None` for vacant and removed records.
	pub fn get(&self) -> Option<E> {
		self.lease.signal.get()
	}
}

struct EntityHandleLease<E>
where
	E: Entity,
{
	bucket: Rc<RefCell<EntityBucket<E>>>,
	id: E::Id,
	signal: Signal<Option<E>>,
}

impl<E> Drop for EntityHandleLease<E>
where
	E: Entity,
{
	fn drop(&mut self) {
		let mut bucket = self.bucket.borrow_mut();
		let record = bucket
			.records
			.get_mut(&self.id)
			.expect("entity handle lease record must outlive its handle");
		record.handle_lease_count = record.handle_lease_count.saturating_sub(1);
	}
}

struct TypedEntityDependencyLease<E>
where
	E: Entity,
{
	bucket: Rc<RefCell<EntityBucket<E>>>,
	id: E::Id,
}

impl<E> Drop for TypedEntityDependencyLease<E>
where
	E: Entity,
{
	fn drop(&mut self) {
		let mut bucket = self.bucket.borrow_mut();
		let record = bucket
			.records
			.get_mut(&self.id)
			.expect("entity dependency lease record must outlive its lease");
		record.dependency_lease_count = record.dependency_lease_count.saturating_sub(1);
	}
}

/// Receives ordered entity operations for one staged transaction.
pub struct EntityWriter<'a> {
	staging: &'a mut EntityStaging,
}

impl EntityWriter<'_> {
	/// Stages a complete replacement for an entity identity.
	pub fn upsert<E>(&mut self, entity: E)
	where
		E: Entity,
	{
		self.staging.type_registry.borrow_mut().register::<E>();
		let id = entity.entity_id();
		self.staging.operations.push(Box::new(TypedEntityOperation {
			identity: EntityIdentity::of::<E>(&id),
			id,
			state: StagedEntityState::Present(entity),
		}));
	}

	/// Stages a tombstone for an entity identity.
	pub fn remove<E>(&mut self, id: &E::Id)
	where
		E: Entity,
	{
		self.staging.type_registry.borrow_mut().register::<E>();
		self.staging
			.operations
			.push(Box::new(TypedEntityOperation::<E> {
				id: id.clone(),
				identity: EntityIdentity::of::<E>(id),
				state: StagedEntityState::Removed,
			}));
	}
}

pub(crate) struct EntityStaging {
	operations: Vec<Box<dyn ErasedEntityOperation>>,
	type_registry: Rc<RefCell<EntityTypeRegistry>>,
}

impl EntityStaging {
	fn new(type_registry: Rc<RefCell<EntityTypeRegistry>>) -> Self {
		Self {
			operations: Vec::new(),
			type_registry,
		}
	}
}

/// A read-only candidate view that overlays staged entity writes on live records.
pub(crate) struct EntityOverlay<'a> {
	arena: &'a EntityArena,
	operations: Vec<Box<dyn ErasedEntityOperation>>,
}

impl<'a> EntityOverlay<'a> {
	pub(crate) fn new(
		arena: &'a EntityArena,
		staging: EntityStaging,
		ticket: EntityWriteTicket,
	) -> Self {
		let mut positions = HashMap::new();
		let mut operations = Vec::new();
		for operation in staging.operations {
			let identity = operation.identity().clone();
			if let Some(previous) = positions.insert(identity, operations.len()) {
				operations[previous] = None;
			}
			operations.push(Some(operation));
		}

		Self {
			arena,
			operations: operations
				.into_iter()
				.flatten()
				.filter(|operation| operation.applies_to(arena, ticket))
				.collect(),
		}
	}

	pub(crate) fn get<E>(&self, id: &E::Id) -> Option<E>
	where
		E: Entity,
	{
		self.arena.register_entity_type::<E>();
		let identity = EntityIdentity::of::<E>(id);
		if let Some(operation) = self
			.operations
			.iter()
			.find(|operation| operation.identity() == &identity)
		{
			return operation
				.value()
				.and_then(|value| value.downcast_ref::<E>().cloned());
		}
		self.arena.current::<E>(id)
	}

	pub(crate) fn is_removed<E>(&self, id: &E::Id) -> bool
	where
		E: Entity,
	{
		self.arena.register_entity_type::<E>();
		let identity = EntityIdentity::of::<E>(id);
		if let Some(operation) = self
			.operations
			.iter()
			.find(|operation| operation.identity() == &identity)
		{
			return operation.is_removed();
		}
		self.arena.record_is_removed::<E>(id)
	}

	pub(crate) fn commit(self, ticket: EntityWriteTicket) -> Vec<Box<dyn EntityPublication>> {
		self.operations
			.iter()
			.map(|operation| operation.commit(self.arena, ticket))
			.collect()
	}
}

trait ErasedEntityOperation {
	fn identity(&self) -> &EntityIdentity;
	fn value(&self) -> Option<&dyn Any>;
	fn is_removed(&self) -> bool;
	fn applies_to(&self, arena: &EntityArena, ticket: EntityWriteTicket) -> bool;
	fn commit(&self, arena: &EntityArena, ticket: EntityWriteTicket) -> Box<dyn EntityPublication>;
}

struct TypedEntityOperation<E>
where
	E: Entity,
{
	id: E::Id,
	identity: EntityIdentity,
	state: StagedEntityState<E>,
}

enum StagedEntityState<E> {
	Present(E),
	Removed,
}

impl<E> ErasedEntityOperation for TypedEntityOperation<E>
where
	E: Entity,
{
	fn identity(&self) -> &EntityIdentity {
		&self.identity
	}

	fn value(&self) -> Option<&dyn Any> {
		match &self.state {
			StagedEntityState::Present(entity) => Some(entity),
			StagedEntityState::Removed => None,
		}
	}

	fn is_removed(&self) -> bool {
		matches!(self.state, StagedEntityState::Removed)
	}

	fn applies_to(&self, arena: &EntityArena, ticket: EntityWriteTicket) -> bool {
		arena
			.record_ticket::<E>(&self.id)
			.is_none_or(|last| ticket >= last)
	}

	fn commit(&self, arena: &EntityArena, ticket: EntityWriteTicket) -> Box<dyn EntityPublication> {
		let bucket = arena.bucket::<E>();
		let (signal, value) = {
			let mut bucket = bucket.borrow_mut();
			let record = bucket
				.records
				.entry(self.id.clone())
				.or_insert_with(EntityRecord::vacant);
			record.state = match &self.state {
				StagedEntityState::Present(entity) => EntityRecordState::Present(entity.clone()),
				StagedEntityState::Removed => EntityRecordState::Removed,
			};
			record.last_write_ticket = Some(ticket);
			(record.signal, record.value())
		};

		Box::new(TypedEntityPublication { signal, value })
	}
}

pub(crate) trait EntityPublication {
	fn publish(self: Box<Self>);
}

struct TypedEntityPublication<E>
where
	E: Entity,
{
	signal: Signal<Option<E>>,
	value: Option<E>,
}

impl<E> EntityPublication for TypedEntityPublication<E>
where
	E: Entity,
{
	fn publish(self: Box<Self>) {
		self.signal.set(self.value.clone());
	}
}

struct EntityBucket<E>
where
	E: Entity,
{
	records: HashMap<E::Id, EntityRecord<E>>,
}

impl<E> Default for EntityBucket<E>
where
	E: Entity,
{
	fn default() -> Self {
		Self {
			records: HashMap::new(),
		}
	}
}

struct EntityRecord<E>
where
	E: Entity,
{
	state: EntityRecordState<E>,
	signal: Signal<Option<E>>,
	handle_lease_count: usize,
	dependency_lease_count: usize,
	last_write_ticket: Option<EntityWriteTicket>,
}

impl<E> EntityRecord<E>
where
	E: Entity,
{
	fn vacant() -> Self {
		Self {
			state: EntityRecordState::Vacant,
			signal: Signal::new(None),
			handle_lease_count: 0,
			dependency_lease_count: 0,
			last_write_ticket: None,
		}
	}

	fn value(&self) -> Option<E> {
		match &self.state {
			EntityRecordState::Vacant | EntityRecordState::Removed => None,
			EntityRecordState::Present(entity) => Some(entity.clone()),
		}
	}
}

enum EntityRecordState<E> {
	Vacant,
	Present(E),
	Removed,
}

/// A client-local, start-ordered write ticket.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct EntityWriteTicket(u64);

/// Retains an active query ticket until the query request completes or is dropped.
pub(crate) struct QueryTicketLease {
	arena: Weak<EntityArenaInner>,
	ticket: EntityWriteTicket,
}

impl QueryTicketLease {
	// Query completion consumes this ticket when normalized requests land in Task 5.
	#[allow(dead_code)]
	pub(crate) fn ticket(&self) -> EntityWriteTicket {
		self.ticket
	}
}

impl Drop for QueryTicketLease {
	fn drop(&mut self) {
		let Some(arena) = self.arena.upgrade() else {
			return;
		};
		let mut tickets = arena.active_query_tickets.borrow_mut();
		let remove_ticket = {
			let count = tickets
				.get_mut(&self.ticket.0)
				.expect("active query ticket lease must be registered");
			*count -= 1;
			*count == 0
		};
		if remove_ticket {
			tickets.remove(&self.ticket.0);
		}
	}
}
