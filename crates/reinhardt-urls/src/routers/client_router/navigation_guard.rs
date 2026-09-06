//! Stable identifiers for asynchronous navigation guards.

/// Identifies a navigation guard independently of the route that uses it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NavigationGuardId(&'static str);

impl NavigationGuardId {
	/// Creates a navigation guard identifier from a stable string.
	pub const fn new(value: &'static str) -> Self {
		Self(value)
	}

	/// Returns the stable string representation.
	pub const fn as_str(self) -> &'static str {
		self.0
	}
}
