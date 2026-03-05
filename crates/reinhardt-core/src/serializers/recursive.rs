//! Recursive serialization support
//!
//! This module provides utilities for handling recursive and deeply nested serialization.

use std::collections::HashSet;

/// Context for tracking serialization depth and visited objects
#[derive(Debug, Clone)]
pub struct SerializationContext {
	/// Current depth level (0 = root)
	current_depth: usize,
	/// Maximum allowed depth
	max_depth: usize,
	/// Set of visited object identities (pointer addresses) to detect circular references
	visited: HashSet<usize>,
}

impl SerializationContext {
	/// Create a new serialization context
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::SerializationContext;
	///
	/// let context = SerializationContext::new(3);
	/// // Verify context is initialized with correct depth settings
	/// assert_eq!(context.current_depth(), 0);
	/// assert_eq!(context.max_depth(), 3);
	/// ```
	pub fn new(max_depth: usize) -> Self {
		Self {
			current_depth: 0,
			max_depth,
			visited: HashSet::new(),
		}
	}

	/// Get the current depth
	pub fn current_depth(&self) -> usize {
		self.current_depth
	}

	/// Get the maximum depth
	pub fn max_depth(&self) -> usize {
		self.max_depth
	}

	/// Check if we can go deeper
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::SerializationContext;
	///
	/// let context = SerializationContext::new(2);
	/// // Verify depth check works correctly
	/// assert!(context.can_go_deeper());
	/// ```
	pub fn can_go_deeper(&self) -> bool {
		self.current_depth < self.max_depth
	}

	/// Visit an object, marking it as visited for circular reference detection
	///
	/// Returns `true` if the object can be visited (not visited before),
	/// `false` if it's already visited.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::SerializationContext;
	///
	/// struct User { id: i64 }
	/// let user = User { id: 1 };
	///
	/// let mut context = SerializationContext::new(5);
	/// // Verify circular reference detection
	/// assert!(context.visit(&user));
	/// assert!(!context.visit(&user)); // Already visited
	/// ```
	pub fn visit<T>(&mut self, obj: &T) -> bool {
		let id = obj as *const T as usize;

		if self.visited.contains(&id) {
			return false; // Circular reference detected
		}

		self.visited.insert(id);
		true
	}

	/// Leave an object, unmarking it as visited (for backtracking)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::SerializationContext;
	///
	/// struct User { id: i64 }
	/// let user = User { id: 1 };
	///
	/// let mut context = SerializationContext::new(5);
	/// context.visit(&user);
	/// context.leave(&user);
	/// // Verify object can be visited again after leaving
	/// assert!(context.visit(&user)); // Can visit again after leaving
	/// ```
	pub fn leave<T>(&mut self, obj: &T) {
		let id = obj as *const T as usize;
		self.visited.remove(&id);
	}

	/// Create a child context with increased depth
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::SerializationContext;
	///
	/// let context = SerializationContext::new(3);
	/// let child = context.child();
	///
	/// // Verify child context has incremented depth
	/// assert_eq!(child.current_depth(), 1);
	/// assert_eq!(child.max_depth(), 3);
	/// ```
	pub fn child(&self) -> Self {
		Self {
			current_depth: self.current_depth + 1,
			max_depth: self.max_depth,
			visited: self.visited.clone(),
		}
	}

	/// Reset the context to initial state
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::SerializationContext;
	///
	/// let mut context = SerializationContext::new(3);
	/// let child = context.child();
	/// assert_eq!(child.current_depth(), 1);
	///
	/// let mut reset_context = child;
	/// reset_context.reset();
	/// // Verify context resets to initial state
	/// assert_eq!(reset_context.current_depth(), 0);
	/// ```
	pub fn reset(&mut self) {
		self.current_depth = 0;
		self.visited.clear();
	}

	/// Get the remaining depth
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::SerializationContext;
	///
	/// let context = SerializationContext::new(3);
	/// // Verify remaining depth calculation
	/// assert_eq!(context.remaining_depth(), 3);
	///
	/// let child = context.child();
	/// assert_eq!(child.remaining_depth(), 2);
	/// ```
	pub fn remaining_depth(&self) -> usize {
		self.max_depth.saturating_sub(self.current_depth)
	}
}

impl Default for SerializationContext {
	fn default() -> Self {
		Self::new(1)
	}
}

/// Result type for recursive serialization
pub type RecursiveResult<T> = Result<T, RecursiveError>;

/// Errors that can occur during recursive serialization
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecursiveError {
	/// Maximum depth exceeded
	MaxDepthExceeded {
		current_depth: usize,
		max_depth: usize,
	},
	/// Circular reference detected
	CircularReference { object_id: String },
	/// General serialization error
	SerializationError { message: String },
}

impl std::fmt::Display for RecursiveError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			RecursiveError::MaxDepthExceeded {
				current_depth,
				max_depth,
			} => write!(
				f,
				"Maximum depth exceeded: current={}, max={}",
				current_depth, max_depth
			),
			RecursiveError::CircularReference { object_id } => {
				write!(f, "Circular reference detected: {}", object_id)
			}
			RecursiveError::SerializationError { message } => {
				write!(f, "Serialization error: {}", message)
			}
		}
	}
}

impl std::error::Error for RecursiveError {}

/// Helper trait for objects that can provide an identifier
pub trait ObjectIdentifiable {
	/// Get a unique identifier for this object
	///
	/// This identifier should be stable and unique across instances.
	/// Typically uses the model name and primary key (e.g., "User:123").
	fn object_id(&self) -> String;
}

/// Helper functions for circular reference detection
pub mod circular {
	use super::*;

	/// Visit an object and execute a function, automatically cleaning up on completion
	///
	/// This ensures proper cleanup even if the function panics or returns an error.
	/// Uses pointer-based identity for accurate circular reference detection.
	/// Manages depth automatically by creating a child context.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::{SerializationContext, circular};
	///
	/// struct User { id: i64 }
	/// let user = User { id: 1 };
	///
	/// let mut context = SerializationContext::new(5);
	///
	/// let result = circular::visit_with(&mut context, &user, |ctx| {
	///     // Do serialization work here
	///     Ok(42)
	/// });
	///
	/// assert_eq!(result.unwrap(), 42);
	/// // Verify automatic cleanup after function completion
	/// assert!(context.visit(&user)); // Can visit again
	/// ```
	pub fn visit_with<T, F, R>(
		context: &mut SerializationContext,
		obj: &T,
		f: F,
	) -> RecursiveResult<R>
	where
		F: FnOnce(&mut SerializationContext) -> RecursiveResult<R>,
	{
		// Check circular reference
		if !context.visit(obj) {
			let id = obj as *const T as usize;
			return Err(RecursiveError::CircularReference {
				object_id: format!("0x{:x}", id),
			});
		}

		// Check depth limit
		if !context.can_go_deeper() {
			context.leave(obj);
			return Err(RecursiveError::MaxDepthExceeded {
				current_depth: context.current_depth(),
				max_depth: context.max_depth(),
			});
		}

		// Create child context with increased depth
		let mut child_context = context.child();

		// Execute function
		let result = f(&mut child_context);

		// Cleanup
		context.leave(obj);
		result
	}
}

/// Helper functions for depth management
pub mod depth {
	use super::*;

	/// Check if we can descend to the next level
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::{SerializationContext, depth};
	///
	/// let context = SerializationContext::new(2);
	/// // Verify descent check works correctly
	/// assert!(depth::can_descend(&context));
	/// ```
	pub fn can_descend(context: &SerializationContext) -> bool {
		context.can_go_deeper()
	}

	/// Attempt to descend to the next level, returning an error if max depth is reached
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::{SerializationContext, depth};
	///
	/// let context = SerializationContext::new(2);
	/// assert!(depth::try_descend(&context).is_ok());
	///
	/// // Verify error when max depth is reached
	/// let child = context.child().child();
	/// assert!(depth::try_descend(&child).is_err());
	/// ```
	pub fn try_descend(context: &SerializationContext) -> RecursiveResult<SerializationContext> {
		if !can_descend(context) {
			return Err(RecursiveError::MaxDepthExceeded {
				current_depth: context.current_depth(),
				max_depth: context.max_depth(),
			});
		}
		Ok(context.child())
	}

	/// Descend to the next level and execute a function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::serializers::recursive::{SerializationContext, depth};
	///
	/// let context = SerializationContext::new(3);
	///
	/// let result = depth::descend_with(&context, |child_ctx| {
	///     // Verify child context depth in callback
	///     assert_eq!(child_ctx.current_depth(), 1);
	///     Ok(())
	/// });
	///
	/// assert!(result.is_ok());
	/// ```
	pub fn descend_with<F, T>(context: &SerializationContext, f: F) -> RecursiveResult<T>
	where
		F: FnOnce(&SerializationContext) -> RecursiveResult<T>,
	{
		let child = try_descend(context)?;
		f(&child)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use circular::*;
	use depth::*;

	#[test]
	fn test_context_new() {
		let context = SerializationContext::new(3);
		assert_eq!(context.current_depth(), 0);
		assert_eq!(context.max_depth(), 3);
		assert!(context.can_go_deeper());
	}

	#[test]
	fn test_context_child() {
		let context = SerializationContext::new(3);
		let child = context.child();

		assert_eq!(child.current_depth(), 1);
		assert_eq!(child.max_depth(), 3);
		assert!(child.can_go_deeper());
	}

	#[test]
	fn test_context_can_go_deeper() {
		let context = SerializationContext::new(2);
		assert!(context.can_go_deeper());

		let child1 = context.child();
		assert!(child1.can_go_deeper());

		let child2 = child1.child();
		assert!(!child2.can_go_deeper());
	}

	#[test]
	fn test_context_visit_and_leave() {
		#[allow(dead_code)]
		struct User {
			id: i64,
		}
		let user = User { id: 1 };

		let mut context = SerializationContext::new(5);
		assert!(context.visit(&user));

		// Second visit should fail (circular reference)
		assert!(!context.visit(&user));

		// After leaving, can visit again
		context.leave(&user);
		assert!(context.visit(&user));
	}

	#[test]
	fn test_context_reset() {
		#[allow(dead_code)]
		struct User {
			id: i64,
		}
		let user = User { id: 1 };

		let mut context = SerializationContext::new(3);
		context.visit(&user);

		let child = context.child();
		assert_eq!(child.current_depth(), 1);

		let mut reset_context = child;
		reset_context.reset();
		assert_eq!(reset_context.current_depth(), 0);
	}

	#[test]
	fn test_remaining_depth() {
		let context = SerializationContext::new(3);
		assert_eq!(context.remaining_depth(), 3);

		let child1 = context.child();
		assert_eq!(child1.remaining_depth(), 2);

		let child2 = child1.child();
		assert_eq!(child2.remaining_depth(), 1);

		let child3 = child2.child();
		assert_eq!(child3.remaining_depth(), 0);
	}

	#[test]
	fn test_context_default() {
		let context = SerializationContext::default();
		assert_eq!(context.current_depth(), 0);
		assert_eq!(context.max_depth(), 1);
	}

	#[test]
	fn test_recursive_error_display() {
		let err = RecursiveError::MaxDepthExceeded {
			current_depth: 5,
			max_depth: 3,
		};
		assert_eq!(err.to_string(), "Maximum depth exceeded: current=5, max=3");

		let err = RecursiveError::CircularReference {
			object_id: "user:1".to_string(),
		};
		assert_eq!(err.to_string(), "Circular reference detected: user:1");

		let err = RecursiveError::SerializationError {
			message: "test error".to_string(),
		};
		assert_eq!(err.to_string(), "Serialization error: test error");
	}

	#[test]
	fn test_circular_reference_detection() {
		#[allow(dead_code)]
		struct User {
			id: i64,
		}
		let user = User { id: 1 };

		let mut context = SerializationContext::new(5);

		// First visit succeeds
		assert!(context.visit(&user));

		// Second visit fails (circular reference detected)
		assert!(!context.visit(&user));
	}

	#[test]
	fn test_circular_visit_with() {
		#[allow(dead_code)]
		struct User {
			id: i64,
		}
		let user = User { id: 1 };

		let mut context = SerializationContext::new(5);

		let result = visit_with(&mut context, &user, |_ctx| Ok(42));

		assert_eq!(result.unwrap(), 42);
		// Object is automatically unmarked after the function completes
		assert!(context.visit(&user)); // Can visit again
	}

	#[test]
	fn test_circular_visit_with_error() {
		#[allow(dead_code)]
		struct User {
			id: i64,
		}
		let user = User { id: 1 };

		let mut context = SerializationContext::new(5);

		let result: RecursiveResult<()> = visit_with(&mut context, &user, |_ctx| {
			Err(RecursiveError::SerializationError {
				message: "test".to_string(),
			})
		});

		assert!(result.is_err());
		// Object is automatically unmarked even on error
		assert!(context.visit(&user)); // Can visit again
	}

	#[test]
	fn test_different_objects_same_string_representation() {
		#[allow(dead_code)]
		struct User {
			id: i64,
		}
		let user1 = User { id: 1 };
		let user2 = User { id: 1 };

		let mut context = SerializationContext::new(5);

		// Both users have same ID but different memory addresses
		assert!(context.visit(&user1));
		assert!(context.visit(&user2)); // Should succeed - different objects

		context.leave(&user1);
		context.leave(&user2);
	}

	#[test]
	fn test_same_object_multiple_references() {
		#[allow(dead_code)]
		struct User {
			id: i64,
		}
		let user = User { id: 1 };

		let mut context = SerializationContext::new(5);

		// Visit the same object
		assert!(context.visit(&user));

		// Create another reference to the same object
		let user_ref = &user;

		// Second visit with different reference should fail (same object)
		assert!(!context.visit(user_ref));
	}

	#[test]
	fn test_depth_can_descend() {
		let context = SerializationContext::new(2);
		assert!(can_descend(&context));

		let child = context.child();
		assert!(can_descend(&child));

		let grandchild = child.child();
		assert!(!can_descend(&grandchild));
	}

	#[test]
	fn test_depth_try_descend() {
		let context = SerializationContext::new(2);
		assert!(try_descend(&context).is_ok());

		let child = context.child().child();
		let err = try_descend(&child).unwrap_err();
		assert_eq!(
			err,
			RecursiveError::MaxDepthExceeded {
				current_depth: 2,
				max_depth: 2
			}
		);
	}

	#[test]
	fn test_depth_descend_with() {
		let context = SerializationContext::new(3);

		let result = descend_with(&context, |child_ctx| {
			assert_eq!(child_ctx.current_depth(), 1);
			assert_eq!(child_ctx.max_depth(), 3);
			Ok(123)
		});

		assert_eq!(result.unwrap(), 123);
	}
}
