//! Task-local circular dependency detection mechanism
//!
//! This module provides a deterministic mechanism for detecting circular references during DI
//! dependency resolution, safe for use in async work-stealing runtimes.
//!
//! ## Features
//!
//! - **O(1) Circular Detection**: Fast lookup using `HashSet<TypeId>`
//! - **Task-local**: State follows async tasks across thread migrations (safe with Tokio's
//!   work-stealing scheduler)
//! - **Depth Limiting**: `MAX_RESOLUTION_DEPTH` prevents pathological cases
//! - **Deterministic**: Always checks for cycles at every depth (no sampling)
//! - **RAII**: Automatic cleanup via `ResolutionGuard`
//!
//! ## Performance Goals
//!
//! - Cache hit: < 5% overhead (completely skip circular detection)
//! - Cache miss: 10-20% overhead (optimized detection)

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::future::Future;

/// Maximum resolution depth (prevents pathological cases)
const MAX_RESOLUTION_DEPTH: usize = 100;

/// Internal state for cycle detection, stored per-task
struct CycleDetectionState {
	/// Set of types currently being resolved (O(1) circular detection)
	resolution_set: HashSet<TypeId>,
	/// Resolution depth counter
	resolution_depth: usize,
	/// Type name mapping (for error messages)
	type_names: HashMap<TypeId, &'static str>,
	/// Resolution path (for displaying circular paths)
	resolution_path: Vec<(TypeId, &'static str)>,
}

impl CycleDetectionState {
	fn new() -> Self {
		Self {
			resolution_set: HashSet::new(),
			resolution_depth: 0,
			type_names: HashMap::new(),
			resolution_path: Vec::new(),
		}
	}
}

tokio::task_local! {
	/// Task-local cycle detection state. Follows the task across thread boundaries
	/// in work-stealing async runtimes.
	static CYCLE_STATE: RefCell<CycleDetectionState>;
}

/// Execute a future within a cycle detection scope.
///
/// This must be called at the outermost resolution entry point to initialize
/// the task-local cycle detection state. If a scope is already active (nested
/// resolution), the provided future runs directly within the existing scope.
pub async fn with_cycle_detection_scope<F, T>(f: F) -> T
where
	F: Future<Output = T>,
{
	// Check if we are already inside a scope
	let already_scoped = CYCLE_STATE.try_with(|_| ()).is_ok();
	if already_scoped {
		// Already within a scope; run directly
		f.await
	} else {
		// Initialize a new scope
		CYCLE_STATE
			.scope(RefCell::new(CycleDetectionState::new()), f)
			.await
	}
}

/// Access the task-local state, returning an error if no scope is active.
fn with_state<R>(f: impl FnOnce(&RefCell<CycleDetectionState>) -> R) -> Result<R, CycleError> {
	CYCLE_STATE.try_with(f).map_err(|_| CycleError::NoScope)
}

/// Circular reference check (O(1))
///
/// Checks if the specified type is currently in the resolution stack.
fn check_circular_dependency(type_id: TypeId) -> Result<(), CycleError> {
	with_state(|state| {
		let state_ref = state.borrow();
		if state_ref.resolution_set.contains(&type_id) {
			// Circular detection: Get type name and construct error
			let type_name = state_ref
				.type_names
				.get(&type_id)
				.copied()
				.unwrap_or("<unknown>");
			let cycle_path = build_cycle_path_inner(&state_ref, type_id);
			return Err(CycleError::CircularDependency {
				type_name: type_name.to_string(),
				path: cycle_path,
			});
		}
		Ok(())
	})?
}

/// Record the start of resolution
///
/// Called at the start of type resolution to prepare for circular detection.
/// The returned `ResolutionGuard` automates cleanup using the RAII pattern.
///
/// A cycle detection scope must be active (via [`with_cycle_detection_scope`]).
pub fn begin_resolution(
	type_id: TypeId,
	type_name: &'static str,
) -> Result<ResolutionGuard, CycleError> {
	// Depth check
	let depth = with_state(|state| {
		let mut s = state.borrow_mut();
		s.resolution_depth += 1;
		s.resolution_depth
	})?;

	if depth > MAX_RESOLUTION_DEPTH {
		// Depth exceeded: Cleanup and error
		let _ = with_state(|state| {
			let mut s = state.borrow_mut();
			s.resolution_depth -= 1;
		});
		return Err(CycleError::MaxDepthExceeded(depth));
	}

	// Always perform deterministic circular check (no sampling).
	// Decrement depth on error since ResolutionGuard is not yet created.
	if let Err(e) = check_circular_dependency(type_id) {
		let _ = with_state(|state| {
			let mut s = state.borrow_mut();
			s.resolution_depth -= 1;
		});
		return Err(e);
	}

	// Add to set and path
	with_state(|state| {
		let mut s = state.borrow_mut();
		s.resolution_set.insert(type_id);
		s.resolution_path.push((type_id, type_name));
	})?;

	Ok(ResolutionGuard::Tracked(type_id))
}

/// RAII guard: Automatic cleanup on Drop
///
/// When resolution is complete, the type is removed from the stack.
#[derive(Debug)]
pub enum ResolutionGuard {
	/// Tracking circular detection
	Tracked(TypeId),
}

impl Drop for ResolutionGuard {
	fn drop(&mut self) {
		let ResolutionGuard::Tracked(type_id) = self;
		let _ = CYCLE_STATE.try_with(|state| {
			let mut s = state.borrow_mut();
			s.resolution_set.remove(type_id);
			if let Some(pos) = s.resolution_path.iter().rposition(|(id, _)| id == type_id) {
				s.resolution_path.remove(pos);
			}
			s.resolution_depth = s.resolution_depth.saturating_sub(1);
		});
	}
}

/// Register type name
///
/// Registers the type name in the mapping for display in error messages.
/// A cycle detection scope must be active.
pub fn register_type_name<T: 'static>(name: &'static str) {
	let _ = CYCLE_STATE.try_with(|state| {
		state
			.borrow_mut()
			.type_names
			.insert(TypeId::of::<T>(), name);
	});
}

/// Build circular path (internal, borrows state)
fn build_cycle_path_inner(state: &CycleDetectionState, current_type_id: TypeId) -> String {
	let type_name = state
		.type_names
		.get(&current_type_id)
		.copied()
		.unwrap_or("<unknown>");

	if let Some(cycle_start) = state
		.resolution_path
		.iter()
		.position(|(id, _)| *id == current_type_id)
	{
		let cycle: Vec<&str> = state.resolution_path[cycle_start..]
			.iter()
			.map(|(_, name)| *name)
			.collect();
		format!("{} -> {}", cycle.join(" -> "), type_name)
	} else {
		format!("Unknown cycle involving {}", type_name)
	}
}

/// Circular dependency error
#[derive(Debug, thiserror::Error)]
pub enum CycleError {
	/// Circular dependency detected
	#[error(
		"Circular dependency detected: {type_name}\n  Path: {path}\nThis forms a cycle that cannot be resolved."
	)]
	CircularDependency {
		/// Name of the type involved in the cycle
		type_name: String,
		/// Circular path (format: A -> B -> C -> A)
		path: String,
	},

	/// Maximum resolution depth exceeded
	#[error(
		"Maximum resolution depth exceeded: {0}\nThis likely indicates an extremely deep or circular dependency chain."
	)]
	MaxDepthExceeded(usize),

	/// No cycle detection scope is active
	#[error(
		"Cycle detection called outside of a task-local scope. Use `with_cycle_detection_scope` to initialize."
	)]
	NoScope,
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// Dummy type for testing
	struct TypeA;
	struct TypeB;
	struct TypeC;

	#[rstest]
	#[tokio::test]
	async fn test_simple_cycle_detection() {
		// Arrange
		with_cycle_detection_scope(async {
			let type_a = TypeId::of::<TypeA>();
			register_type_name::<TypeA>("TypeA");

			// Act
			let guard_a = begin_resolution(type_a, "TypeA").unwrap();

			// Assert: Attempting to resolve TypeA again should cause circular error
			let result = begin_resolution(type_a, "TypeA");
			assert!(matches!(result, Err(CycleError::CircularDependency { .. })));

			// Act: Drop guard to cleanup
			drop(guard_a);

			// Assert: After cleanup, resolution should succeed again
			let result = begin_resolution(type_a, "TypeA");
			assert!(result.is_ok());
		})
		.await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_depth_limit() {
		// Arrange
		with_cycle_detection_scope(async {
			use std::marker::PhantomData;

			let type1 = TypeId::of::<PhantomData<[u8; 0]>>();
			let type2 = TypeId::of::<PhantomData<[u8; 1]>>();
			let type3 = TypeId::of::<PhantomData<[u8; 2]>>();

			// Act & Assert: Depth tracking works correctly
			let guard1 = begin_resolution(type1, "Type1").unwrap();
			let depth1 = CYCLE_STATE.with(|state| state.borrow().resolution_depth);
			assert_eq!(depth1, 1, "Depth should be 1 after first resolution");

			let guard2 = begin_resolution(type2, "Type2").unwrap();
			let depth2 = CYCLE_STATE.with(|state| state.borrow().resolution_depth);
			assert_eq!(depth2, 2, "Depth should be 2 after second resolution");

			let guard3 = begin_resolution(type3, "Type3").unwrap();
			let depth3 = CYCLE_STATE.with(|state| state.borrow().resolution_depth);
			assert_eq!(depth3, 3, "Depth should be 3 after third resolution");

			// Act: Drop guards in reverse order
			drop(guard3);
			let depth_after_drop3 = CYCLE_STATE.with(|state| state.borrow().resolution_depth);
			assert_eq!(
				depth_after_drop3, 2,
				"Depth should be 2 after dropping guard3"
			);

			drop(guard2);
			let depth_after_drop2 = CYCLE_STATE.with(|state| state.borrow().resolution_depth);
			assert_eq!(
				depth_after_drop2, 1,
				"Depth should be 1 after dropping guard2"
			);

			drop(guard1);
			let depth_after_drop1 = CYCLE_STATE.with(|state| state.borrow().resolution_depth);
			assert_eq!(
				depth_after_drop1, 0,
				"Depth should be 0 after dropping all guards"
			);
		})
		.await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_no_scope_returns_error() {
		// Arrange: Do NOT wrap in with_cycle_detection_scope

		// Act
		let type_a = TypeId::of::<TypeA>();
		let result = begin_resolution(type_a, "TypeA");

		// Assert
		assert!(matches!(result, Err(CycleError::NoScope)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_nested_scope_reuses_existing() {
		// Arrange
		with_cycle_detection_scope(async {
			let type_a = TypeId::of::<TypeA>();
			register_type_name::<TypeA>("TypeA");
			let _guard_a = begin_resolution(type_a, "TypeA").unwrap();

			// Act: Nested scope call should reuse existing state
			with_cycle_detection_scope(async {
				// Assert: TypeA should still be detected as circular
				let result = begin_resolution(type_a, "TypeA");
				assert!(
					matches!(result, Err(CycleError::CircularDependency { .. })),
					"Nested scope should share state with outer scope"
				);
			})
			.await;
		})
		.await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_deterministic_detection_at_deep_depth() {
		// Arrange: Verify that cycles are always detected, even at depth > 50
		// (Previously, sampling would skip checks at deep depths)
		with_cycle_detection_scope(async {
			let mut guards = Vec::new();

			// Create 60 unique types to push depth past 50
			macro_rules! push_guard {
				($($idx:literal),*) => {
					$(
						{
							use std::marker::PhantomData;
							let type_id = TypeId::of::<PhantomData<[u8; $idx]>>();
							let name: &'static str = concat!("Type", stringify!($idx));
							guards.push(begin_resolution(type_id, name).unwrap());
						}
					)*
				};
			}

			push_guard!(
				100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115,
				116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131,
				132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147,
				148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159
			);

			// Act: Verify depth is > 50
			let current_depth = CYCLE_STATE.with(|state| state.borrow().resolution_depth);
			assert_eq!(current_depth, 60, "Depth should be 60");

			// Assert: Cycle detection at depth > 50 should be deterministic
			// Re-resolving an already-tracked type must always be detected
			use std::marker::PhantomData;
			let type_at_depth_55 = TypeId::of::<PhantomData<[u8; 155]>>();
			let result = begin_resolution(type_at_depth_55, "Type155");
			assert!(
				matches!(result, Err(CycleError::CircularDependency { .. })),
				"Cycle must be detected deterministically at depth > 50"
			);
		})
		.await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_cycle_path_display() {
		// Arrange
		with_cycle_detection_scope(async {
			let type_a = TypeId::of::<TypeA>();
			let type_b = TypeId::of::<TypeB>();
			let type_c = TypeId::of::<TypeC>();
			register_type_name::<TypeA>("TypeA");
			register_type_name::<TypeB>("TypeB");
			register_type_name::<TypeC>("TypeC");

			// Act: Build a chain A -> B -> C -> A
			let _guard_a = begin_resolution(type_a, "TypeA").unwrap();
			let _guard_b = begin_resolution(type_b, "TypeB").unwrap();
			let _guard_c = begin_resolution(type_c, "TypeC").unwrap();
			let result = begin_resolution(type_a, "TypeA");

			// Assert
			match result {
				Err(CycleError::CircularDependency { path, .. }) => {
					assert_eq!(path, "TypeA -> TypeB -> TypeC -> TypeA");
				}
				other => panic!("Expected CircularDependency, got {:?}", other),
			}
		})
		.await;
	}
}
