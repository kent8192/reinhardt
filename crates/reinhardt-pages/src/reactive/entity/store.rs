use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::time::Duration;
#[cfg(not(wasm))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(any(wasm, test))]
use super::EntityDependencies;
#[cfg(native)]
use super::EntityHydrationRow;
use super::identity::EntityTypeRegistry;
use super::projection::EntityHydrationGroup;
#[cfg(any(wasm, test))]
use super::projection::EntityHydrationRecord;
use super::{ENTITY_TABLE_VERSION, Entity, EntityHydrationEnvelope, EntityIdentity};
use crate::reactive::{Signal, batch};
use reinhardt_core::reactive::ReactiveScope;
use serde_json::Value;

type EntityGcScheduler = Rc<dyn Fn(EntityIdentity, u64, u64)>;
type EntityGcCollected = Rc<dyn Fn(EntityIdentity)>;

#[cfg(not(wasm))]
fn standalone_now_ms() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
		.unwrap_or_default()
}

#[cfg(wasm)]
fn standalone_now_ms() -> u64 {
	js_sys::Date::now() as u64
}

/// Owns the typed records for one normalized entity cache.
#[derive(Clone)]
pub struct EntityArena {
	inner: Rc<EntityArenaInner>,
}

struct EntityArenaInner {
	buckets: RefCell<HashMap<&'static str, Rc<dyn Any>>>,
	gc_buckets: RefCell<HashMap<&'static str, Rc<dyn ErasedEntityBucket>>>,
	type_registry: Rc<RefCell<EntityTypeRegistry>>,
	next_ticket: Cell<u64>,
	active_query_tickets: RefCell<BTreeMap<u64, usize>>,
	ticket_blocked_gc: RefCell<HashSet<EntityIdentity>>,
	gc_time: Duration,
	gc_scheduler: RefCell<Option<EntityGcScheduler>>,
	gc_collected: RefCell<Option<EntityGcCollected>>,
	clock: RefCell<Rc<dyn Fn() -> u64>>,
	ssr_reachability: Cell<bool>,
	reachable_identities: RefCell<HashSet<EntityIdentity>>,
	hydration_groups: RefCell<BTreeMap<String, EntityHydrationGroup>>,
	hydration_ticket: Cell<Option<EntityWriteTicket>>,
	_scope: Rc<ReactiveScope>,
}

impl EntityArena {
	/// Creates an empty entity arena with the supplied retention duration.
	pub fn new(gc_time: Duration) -> Self {
		let scope = Rc::new(ReactiveScope::new());
		Self {
			inner: Rc::new(EntityArenaInner {
				buckets: RefCell::new(HashMap::new()),
				gc_buckets: RefCell::new(HashMap::new()),
				type_registry: Rc::new(RefCell::new(EntityTypeRegistry::new())),
				next_ticket: Cell::new(1),
				active_query_tickets: RefCell::new(BTreeMap::new()),
				ticket_blocked_gc: RefCell::new(HashSet::new()),
				gc_time,
				gc_scheduler: RefCell::new(None),
				gc_collected: RefCell::new(None),
				clock: RefCell::new(Rc::new(standalone_now_ms)),
				ssr_reachability: Cell::new(false),
				reachable_identities: RefCell::new(HashSet::new()),
				hydration_groups: RefCell::new(BTreeMap::new()),
				hydration_ticket: Cell::new(None),
				_scope: scope,
			}),
		}
	}

	/// Collects standalone entity records whose retention deadlines have elapsed.
	///
	/// Query clients drive this maintenance automatically. Applications that own
	/// an `EntityArena` directly should call this method from their maintenance loop.
	pub fn run_due_maintenance(&self) {
		let now_ms = (self.inner.clock.borrow())();
		let deadlines = self
			.inner
			.gc_buckets
			.borrow()
			.values()
			.flat_map(|bucket| bucket.gc_deadlines())
			.collect::<Vec<_>>();
		for (identity, generation, due_ms) in deadlines {
			if due_ms <= now_ms {
				self.collect_entity_gc(&identity, generation, now_ms);
			}
		}
	}

	/// Enables request-scoped entity reachability tracking for SSR serialization.
	pub(crate) fn enable_ssr_reachability(&self) {
		self.inner.ssr_reachability.set(true);
	}

	/// Records an identity read by an SSR query or entity handle.
	pub(crate) fn mark_reachable(&self, identity: EntityIdentity) {
		if self.inner.ssr_reachability.get() {
			self.inner
				.reachable_identities
				.borrow_mut()
				.insert(identity);
		}
	}

	/// Serializes the present identities reached during this SSR request.
	#[cfg(native)]
	pub(crate) fn reachable_hydration_envelope(&self) -> EntityHydrationEnvelope {
		let mut entities = BTreeMap::<String, Vec<EntityHydrationRow>>::new();
		let identities = self
			.inner
			.reachable_identities
			.borrow()
			.iter()
			.cloned()
			.collect::<Vec<_>>();
		for identity in identities {
			let Some(row) = self
				.inner
				.gc_buckets
				.borrow()
				.get(identity.entity_type())
				.and_then(|bucket| bucket.hydration_row(&identity))
			else {
				continue;
			};
			entities
				.entry(identity.entity_type().to_string())
				.or_default()
				.push(row);
		}
		for rows in entities.values_mut() {
			rows.sort_by(|left, right| {
				canonical_json_value(&left.id).cmp(&canonical_json_value(&right.id))
			});
		}
		EntityHydrationEnvelope {
			version: ENTITY_TABLE_VERSION,
			entities,
		}
	}

	/// Stages the browser-side entity table for typed registration and deferred recipes.
	#[cfg(any(wasm, test))]
	pub(crate) fn install_hydration_envelope(&self, envelope: EntityHydrationEnvelope) {
		if envelope.version != ENTITY_TABLE_VERSION {
			panic!(
				"normalized entity hydration table has unsupported version {}; expected {}",
				envelope.version, ENTITY_TABLE_VERSION
			);
		}
		let mut groups = self.inner.hydration_groups.borrow_mut();
		if !groups.is_empty() {
			return;
		}
		for (entity_type, rows) in envelope.entities {
			let mut seen = HashSet::new();
			let records = rows
				.into_iter()
				.map(|row| {
					let canonical_id = canonical_json_value(&row.id);
					if !seen.insert(canonical_id) {
						panic!(
							"normalized entity hydration table contains duplicate identity in TYPE `{entity_type}`"
						);
					}
					EntityHydrationRecord::new(row.id, row.value)
				})
				.collect();
			groups.insert(
				entity_type.clone(),
				EntityHydrationGroup::new(entity_type, records),
			);
		}
		drop(groups);
		let ticket = EntityWriteTicket(0);
		self.inner.hydration_ticket.set(Some(ticket));
		let registered = self
			.inner
			.hydration_groups
			.borrow()
			.iter()
			.filter_map(|(entity_type, group)| {
				self.inner
					.gc_buckets
					.borrow()
					.get(entity_type.as_str())
					.cloned()
					.map(|bucket| (bucket, group.clone()))
			})
			.collect::<Vec<_>>();
		for (bucket, group) in registered {
			bucket.hydrate_group(self, &group, ticket);
		}
	}

	/// Materializes all groups declared by one normalized recipe in one baseline transaction.
	#[cfg(any(wasm, test))]
	pub(crate) fn hydrate_dependencies(&self, dependencies: &EntityDependencies) {
		let mut selected = Vec::new();
		for entity_type in dependencies.entity_types() {
			if let Some(group) = self
				.inner
				.hydration_groups
				.borrow()
				.get(entity_type)
				.cloned()
			{
				selected.push(group);
			}
		}
		if selected.is_empty() {
			return;
		}
		let ticket = self
			.inner
			.hydration_ticket
			.get()
			.unwrap_or_else(|| self.issue_mutation_ticket());
		let staging = self.stage(|entities| {
			for group in &selected {
				dependencies.hydrate_all(group, entities);
			}
		});
		let overlay = EntityOverlay::new(self, staging, ticket);
		self.commit_overlay(overlay, ticket, || {}, || {});
	}

	/// Installs the client-local deadline scheduler used by entity leases.
	pub(crate) fn configure_gc_scheduler(
		&self,
		scheduler: EntityGcScheduler,
		clock: Rc<dyn Fn() -> u64>,
		gc_collected: EntityGcCollected,
	) {
		*self.inner.gc_scheduler.borrow_mut() = Some(scheduler);
		*self.inner.gc_collected.borrow_mut() = Some(gc_collected);
		*self.inner.clock.borrow_mut() = clock;
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
				.or_insert_with(|| self.inner._scope.enter(EntityRecord::vacant));
			if record.lease_count() == 0 {
				record.gc_generation = record.gc_generation.wrapping_add(1);
				record.gc_due_ms = None;
				self.inner
					.ticket_blocked_gc
					.borrow_mut()
					.remove(&EntityIdentity::of::<E>(&id));
			}
			record.handle_lease_count += 1;
			record.signal
		};

		EntityHandle {
			lease: Rc::new(EntityHandleLease {
				arena: Rc::downgrade(&self.inner),
				bucket,
				id,
				signal,
			}),
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
		commit_structure: impl FnOnce(),
		publish_signal: impl FnOnce(),
	) {
		let publications = overlay.commit(ticket);
		commit_structure();
		batch(|| {
			for publication in publications {
				publication.publish();
			}
			publish_signal();
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
		self.commit_overlay(overlay, ticket, || {}, || {});
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
				.or_insert_with(|| self.inner._scope.enter(EntityRecord::vacant));
			if record.lease_count() == 0 {
				record.gc_generation = record.gc_generation.wrapping_add(1);
				record.gc_due_ms = None;
				self.inner
					.ticket_blocked_gc
					.borrow_mut()
					.remove(&EntityIdentity::of::<E>(&id));
			}
			record.dependency_lease_count += 1;
		}

		Box::new(TypedEntityDependencyLease::<E> {
			arena: Rc::downgrade(&self.inner),
			bucket,
			id,
		})
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
		self.inner.gc_buckets.borrow_mut().insert(
			E::TYPE,
			Rc::new(ErasedEntityBucketImpl::<E>::new(Rc::clone(&bucket))),
		);
		drop(buckets);
		#[cfg(any(wasm, test))]
		self.hydrate_registered_type::<E>();
		bucket
	}

	#[cfg(any(wasm, test))]
	fn hydrate_registered_type<E>(&self)
	where
		E: Entity,
	{
		let Some(group) = self.inner.hydration_groups.borrow().get(E::TYPE).cloned() else {
			return;
		};
		let ticket = self
			.inner
			.hydration_ticket
			.get()
			.unwrap_or_else(|| self.issue_mutation_ticket());
		let bucket = self
			.inner
			.gc_buckets
			.borrow()
			.get(E::TYPE)
			.cloned()
			.expect("a registered entity type must have an erased bucket");
		bucket.hydrate_group(self, &group, ticket);
	}

	fn current<E>(&self, id: &E::Id) -> Option<E>
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
	pub(crate) fn entity_record_exists_for_test<E>(&self, id: &E::Id) -> bool
	where
		E: Entity,
	{
		let bucket = self.bucket::<E>();
		bucket.borrow().records.contains_key(id)
	}

	#[cfg(test)]
	pub(crate) fn entity_gc_generation_for_test<E>(&self, id: &E::Id) -> u64
	where
		E: Entity,
	{
		let bucket = self.bucket::<E>();
		bucket
			.borrow()
			.records
			.get(id)
			.map_or(0, |record| record.gc_generation)
	}

	#[cfg(test)]
	pub(crate) fn entity_gc_due_ms_for_test<E>(&self, id: &E::Id) -> Option<u64>
	where
		E: Entity,
	{
		let bucket = self.bucket::<E>();
		bucket
			.borrow()
			.records
			.get(id)
			.and_then(|record| record.gc_due_ms)
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

	pub(crate) fn entity_deadline_is_current(
		&self,
		identity: &EntityIdentity,
		generation: u64,
	) -> bool {
		self.inner
			.gc_buckets
			.borrow()
			.get(identity.entity_type())
			.is_some_and(|bucket| bucket.deadline_is_current(identity, generation))
	}

	pub(crate) fn collect_entity_gc(
		&self,
		identity: &EntityIdentity,
		generation: u64,
		now_ms: u64,
	) -> bool {
		let Some(bucket) = self
			.inner
			.gc_buckets
			.borrow()
			.get(identity.entity_type())
			.cloned()
		else {
			return false;
		};
		match bucket.collect_if_due(
			identity,
			generation,
			now_ms,
			&self.inner.active_query_tickets.borrow(),
		) {
			EntityGcResult::Collected => {
				self.inner.ticket_blocked_gc.borrow_mut().remove(identity);
				if let Some(callback) = self.inner.gc_collected.borrow().as_ref() {
					callback(identity.clone());
				}
				true
			}
			EntityGcResult::Stale => {
				self.inner.ticket_blocked_gc.borrow_mut().remove(identity);
				false
			}
			EntityGcResult::BlockedByTicket => {
				self.inner
					.ticket_blocked_gc
					.borrow_mut()
					.insert(identity.clone());
				false
			}
		}
	}

	fn schedule_unleased_entity_gc<E>(&self, id: &E::Id)
	where
		E: Entity,
	{
		let schedule = {
			let bucket = self.bucket::<E>();
			let mut bucket = bucket.borrow_mut();
			let Some(record) = bucket.records.get_mut(id) else {
				return;
			};
			if record.lease_count() != 0 {
				return;
			}
			let generation = record.gc_generation.wrapping_add(1);
			record.gc_generation = generation;
			let due_ms = (self.inner.clock.borrow())()
				.saturating_add(self.inner.gc_time.as_millis().min(u64::MAX as u128) as u64);
			record.gc_due_ms = Some(due_ms);
			let identity = EntityIdentity::of::<E>(id);
			self.inner.ticket_blocked_gc.borrow_mut().remove(&identity);
			Some((identity, generation, due_ms))
		};
		if let Some((identity, generation, due_ms)) = schedule {
			Self::notify_entity_gc(&self.inner, identity, generation, due_ms);
		}
	}

	fn notify_entity_gc(
		inner: &Rc<EntityArenaInner>,
		identity: EntityIdentity,
		generation: u64,
		due_ms: u64,
	) {
		if let Some(scheduler) = inner.gc_scheduler.borrow().as_ref() {
			scheduler(identity, generation, due_ms);
		} else if due_ms <= (inner.clock.borrow())() {
			EntityArena {
				inner: Rc::clone(inner),
			}
			.collect_entity_gc(&identity, generation, due_ms);
		}
	}

	fn release_entity_lease<E>(
		inner: &Rc<EntityArenaInner>,
		bucket: &Rc<RefCell<EntityBucket<E>>>,
		id: &E::Id,
	) where
		E: Entity,
	{
		let schedule = {
			let mut bucket = bucket.borrow_mut();
			let record = bucket
				.records
				.get_mut(id)
				.expect("entity lease record must outlive its lease");
			if record.lease_count() == 0 {
				let generation = record.gc_generation.wrapping_add(1);
				record.gc_generation = generation;
				let due_ms = (inner.clock.borrow())()
					.saturating_add(inner.gc_time.as_millis().min(u64::MAX as u128) as u64);
				record.gc_due_ms = Some(due_ms);
				let identity = EntityIdentity::of::<E>(id);
				inner.ticket_blocked_gc.borrow_mut().remove(&identity);
				Some((identity, generation, due_ms))
			} else {
				None
			}
		};
		if let Some((identity, generation, due_ms)) = schedule {
			Self::notify_entity_gc(inner, identity, generation, due_ms);
		}
	}

	fn recheck_ticket_blocked_gc(inner: &Rc<EntityArenaInner>) {
		let identities = inner
			.ticket_blocked_gc
			.borrow()
			.iter()
			.cloned()
			.collect::<Vec<_>>();
		let now_ms = (inner.clock.borrow())();
		let buckets = inner.gc_buckets.borrow();
		let mut reschedule = Vec::new();
		let mut collect = Vec::new();
		for identity in identities {
			let Some(bucket) = buckets.get(identity.entity_type()) else {
				inner.ticket_blocked_gc.borrow_mut().remove(&identity);
				continue;
			};
			let Some((generation, due_ms)) = bucket.gc_deadline(&identity) else {
				inner.ticket_blocked_gc.borrow_mut().remove(&identity);
				continue;
			};
			if due_ms <= now_ms {
				collect.push((identity, generation));
			} else {
				reschedule.push((identity, generation, due_ms));
			}
		}
		drop(buckets);
		for (identity, generation, due_ms) in reschedule {
			if let Some(scheduler) = inner.gc_scheduler.borrow().as_ref() {
				scheduler(identity, generation, due_ms);
			}
		}
		for (identity, generation) in collect {
			let arena = EntityArena {
				inner: Rc::clone(inner),
			};
			arena.collect_entity_gc(&identity, generation, now_ms);
		}
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
		if let Some(arena) = self.lease.arena.upgrade() {
			let arena = EntityArena { inner: arena };
			arena.mark_reachable(EntityIdentity::of::<E>(&self.lease.id));
		}
		self.lease.signal.get()
	}
}

struct EntityHandleLease<E>
where
	E: Entity,
{
	arena: Weak<EntityArenaInner>,
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
		let should_schedule = record.lease_count() == 0;
		drop(bucket);
		if should_schedule && let Some(arena) = self.arena.upgrade() {
			EntityArena::release_entity_lease(&arena, &self.bucket, &self.id);
		}
	}
}

struct TypedEntityDependencyLease<E>
where
	E: Entity,
{
	arena: Weak<EntityArenaInner>,
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
		let should_schedule = record.lease_count() == 0;
		drop(bucket);
		if should_schedule && let Some(arena) = self.arena.upgrade() {
			EntityArena::release_entity_lease(&arena, &self.bucket, &self.id);
		}
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

	pub(crate) fn affected_identities(&self) -> HashSet<EntityIdentity> {
		self.operations
			.iter()
			.map(|operation| operation.identity().clone())
			.collect()
	}

	pub(crate) fn removed_identities(&self) -> HashSet<EntityIdentity> {
		self.operations
			.iter()
			.filter(|operation| operation.is_removed())
			.map(|operation| operation.identity().clone())
			.collect()
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
		let (signal, value, should_schedule) = {
			let mut bucket = bucket.borrow_mut();
			let record = bucket
				.records
				.entry(self.id.clone())
				.or_insert_with(|| arena.inner._scope.enter(EntityRecord::vacant));
			record.state = match &self.state {
				StagedEntityState::Present(entity) => EntityRecordState::Present(entity.clone()),
				StagedEntityState::Removed => EntityRecordState::Removed,
			};
			record.last_write_ticket = Some(ticket);
			let should_schedule = record.lease_count() == 0;
			if !should_schedule {
				record.gc_generation = record.gc_generation.wrapping_add(1);
				record.gc_due_ms = None;
				arena
					.inner
					.ticket_blocked_gc
					.borrow_mut()
					.remove(&self.identity);
			}
			(record.signal, record.value(), should_schedule)
		};
		if should_schedule {
			arena.schedule_unleased_entity_gc::<E>(&self.id);
		}

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

trait ErasedEntityBucket {
	fn deadline_is_current(&self, identity: &EntityIdentity, generation: u64) -> bool;
	fn gc_deadline(&self, identity: &EntityIdentity) -> Option<(u64, u64)>;
	fn gc_deadlines(&self) -> Vec<(EntityIdentity, u64, u64)>;
	#[cfg(native)]
	fn hydration_row(&self, identity: &EntityIdentity) -> Option<EntityHydrationRow>;
	#[cfg(any(wasm, test))]
	fn hydrate_group(
		&self,
		arena: &EntityArena,
		group: &EntityHydrationGroup,
		ticket: EntityWriteTicket,
	);
	fn collect_if_due(
		&self,
		identity: &EntityIdentity,
		generation: u64,
		now_ms: u64,
		active_tickets: &BTreeMap<u64, usize>,
	) -> EntityGcResult;
}

struct ErasedEntityBucketImpl<E>
where
	E: Entity,
{
	bucket: Rc<RefCell<EntityBucket<E>>>,
}

impl<E> ErasedEntityBucketImpl<E>
where
	E: Entity,
{
	fn new(bucket: Rc<RefCell<EntityBucket<E>>>) -> Self {
		Self { bucket }
	}

	fn parse_id(identity: &EntityIdentity) -> Option<E::Id> {
		if identity.entity_type() != E::TYPE {
			return None;
		}
		serde_json::from_str(identity.canonical_id()).ok()
	}
}

impl<E> ErasedEntityBucket for ErasedEntityBucketImpl<E>
where
	E: Entity,
{
	#[cfg(any(wasm, test))]
	fn hydrate_group(
		&self,
		arena: &EntityArena,
		group: &EntityHydrationGroup,
		ticket: EntityWriteTicket,
	) {
		let ids = group
			.records()
			.iter()
			.map(|record| {
				serde_json::from_value::<E::Id>(record.id.clone()).unwrap_or_else(|error| {
					panic!(
						"entity hydration TYPE `{}` failed to deserialize ID type `{}`: {error}",
						E::TYPE,
						std::any::type_name::<E::Id>()
					)
				})
			})
			.collect::<Vec<_>>();
		let mut dependencies = EntityDependencies::default();
		dependencies.extend::<E>(ids);
		let staging = arena.stage(|entities| dependencies.hydrate(group, entities));
		let overlay = EntityOverlay::new(arena, staging, ticket);
		arena.commit_overlay(overlay, ticket, || {}, || {});
	}

	#[cfg(native)]
	fn hydration_row(&self, identity: &EntityIdentity) -> Option<EntityHydrationRow> {
		let id = Self::parse_id(identity)?;
		let bucket = self.bucket.borrow();
		let record = bucket.records.get(&id)?;
		let entity = record.value()?;
		let value = serde_json::to_value(&entity).unwrap_or_else(|error| {
			panic!(
				"entity TYPE `{}` failed to serialize Rust type `{}` for hydration: {error}",
				E::TYPE,
				std::any::type_name::<E>(),
			)
		});
		let row_id = serde_json::to_value(entity.entity_id()).unwrap_or_else(|error| {
			panic!(
				"entity TYPE `{}` failed to serialize ID type `{}` for hydration: {error}",
				E::TYPE,
				std::any::type_name::<E::Id>(),
			)
		});
		if canonical_json_value(&row_id) != identity.canonical_id() {
			panic!(
				"entity TYPE `{}` produced a hydration row whose value ID does not match its identity",
				E::TYPE
			);
		}
		Some(EntityHydrationRow { id: row_id, value })
	}

	fn deadline_is_current(&self, identity: &EntityIdentity, generation: u64) -> bool {
		let Some(id) = Self::parse_id(identity) else {
			return false;
		};
		self.bucket
			.borrow()
			.records
			.get(&id)
			.is_some_and(|record| record.lease_count() == 0 && record.gc_generation == generation)
	}

	fn gc_deadline(&self, identity: &EntityIdentity) -> Option<(u64, u64)> {
		let id = Self::parse_id(identity)?;
		self.bucket.borrow().records.get(&id).and_then(|record| {
			record
				.gc_due_ms
				.map(|due_ms| (record.gc_generation, due_ms))
		})
	}

	fn gc_deadlines(&self) -> Vec<(EntityIdentity, u64, u64)> {
		self.bucket
			.borrow()
			.records
			.iter()
			.filter_map(|(id, record)| {
				record
					.gc_due_ms
					.map(|due_ms| (EntityIdentity::of::<E>(id), record.gc_generation, due_ms))
			})
			.collect()
	}

	fn collect_if_due(
		&self,
		identity: &EntityIdentity,
		generation: u64,
		now_ms: u64,
		active_tickets: &BTreeMap<u64, usize>,
	) -> EntityGcResult {
		let Some(id) = Self::parse_id(identity) else {
			return EntityGcResult::Stale;
		};
		let mut bucket = self.bucket.borrow_mut();
		let Some(record) = bucket.records.get(&id) else {
			return EntityGcResult::Stale;
		};
		if record.lease_count() != 0
			|| record.gc_generation != generation
			|| record.gc_due_ms.is_none_or(|due_ms| due_ms > now_ms)
		{
			return EntityGcResult::Stale;
		}
		if record.last_write_ticket.is_some_and(|last| {
			active_tickets
				.keys()
				.next()
				.is_some_and(|active| *active < last.0)
		}) {
			return EntityGcResult::BlockedByTicket;
		}
		bucket.records.remove(&id);
		EntityGcResult::Collected
	}
}

fn canonical_json_value(value: &Value) -> String {
	crate::reactive::query::canonical_json::encode(value)
		.unwrap_or_else(|error| panic!("entity hydration identity is not valid JSON: {error}"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntityGcResult {
	Collected,
	BlockedByTicket,
	Stale,
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
	gc_generation: u64,
	gc_due_ms: Option<u64>,
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
			gc_generation: 0,
			gc_due_ms: None,
		}
	}

	fn lease_count(&self) -> usize {
		self.handle_lease_count + self.dependency_lease_count
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
		drop(tickets);
		if remove_ticket {
			EntityArena::recheck_ticket_blocked_gc(&arena);
		}
	}
}
