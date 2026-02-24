//! Zone configuration types for CockroachDB
//!
//! This module provides types for zone configuration, which controls replica placement
//! and constraints in CockroachDB.
//!
//! # Overview
//!
//! Zone configurations in CockroachDB determine how data is distributed across your
//! cluster. They control:
//!
//! - **Replication Factor**: How many copies of data to maintain
//! - **Placement Constraints**: Where replicas should be located
//! - **Lease Preferences**: Which replicas should serve reads
//!
//! # Use Cases
//!
//! ## High Availability
//!
//! Increase the number of replicas to survive more simultaneous failures:
//!
//! ```rust
//! use reinhardt_query::types::ZoneConfig;
//!
//! let zone = ZoneConfig::new()
//!     .num_replicas(5); // Survives 2 simultaneous node failures
//! ```
//!
//! ## Data Locality
//!
//! Place data in specific regions for compliance or performance:
//!
//! ```rust
//! use reinhardt_query::types::ZoneConfig;
//!
//! let zone = ZoneConfig::new()
//!     .add_constraint("+region=us-east-1")
//!     .add_constraint("+region=us-west-1");
//! ```
//!
//! ## Read Performance
//!
//! Direct reads to specific zones for lower latency:
//!
//! ```rust
//! use reinhardt_query::types::ZoneConfig;
//!
//! let zone = ZoneConfig::new()
//!     .add_lease_preference("+region=us-east-1"); // Prioritize us-east-1 for reads
//! ```
//!
//! ## Complete Multi-Region Setup
//!
//! ```rust
//! use reinhardt_query::types::ZoneConfig;
//!
//! let zone = ZoneConfig::new()
//!     .num_replicas(7) // 3 regions × 2 replicas + 1 for fault tolerance
//!     .add_constraint("+region=us-east-1")
//!     .add_constraint("+region=us-west-1")
//!     .add_constraint("+region=eu-west-1")
//!     .add_lease_preference("+region=us-east-1"); // Primary region for reads
//! ```

/// Zone configuration for CockroachDB
///
/// This struct represents zone configuration options for databases, tables, or indexes.
/// Zone configurations control how CockroachDB distributes and replicates data across
/// your cluster.
///
/// # Fields
///
/// - `num_replicas`: Number of copies of data to maintain (default: 3)
/// - `constraints`: Placement rules for replicas (required/prohibited locations)
/// - `lease_preferences`: Preferences for which replicas serve reads
///
/// # Constraint Format
///
/// Constraints use a `+` (required) or `-` (prohibited) prefix:
///
/// - `+region=us-east-1`: Replicas MUST be in us-east-1
/// - `-region=us-west-1`: Replicas MUST NOT be in us-west-1
/// - `+zone=a`: Replicas MUST be in zone 'a'
///
/// # Lease Preferences Format
///
/// Lease preferences determine which replica serves reads. They use the same
/// format as constraints but represent priorities rather than requirements.
///
/// # Examples
///
/// ## Basic Configuration
///
/// ```rust
/// use reinhardt_query::types::ZoneConfig;
///
/// let zone = ZoneConfig::new()
///     .num_replicas(3)
///     .add_constraint("+region=us-east-1")
///     .add_lease_preference("+region=us-east-1");
/// ```
///
/// ## Multi-Region High Availability
///
/// ```rust
/// use reinhardt_query::types::ZoneConfig;
///
/// // 5 replicas across 3 regions
/// let zone = ZoneConfig::new()
///     .num_replicas(5)
///     .add_constraint("+region=us-east-1")
///     .add_constraint("+region=us-west-1")
///     .add_constraint("+region=eu-west-1")
///     .add_lease_preference("+region=us-east-1"); // Prefer us-east-1 for reads
/// ```
///
/// ## Exclude Specific Zones
///
/// ```rust
/// use reinhardt_query::types::ZoneConfig;
///
/// // Avoid certain zones for compliance
/// let zone = ZoneConfig::new()
///     .num_replicas(3)
///     .add_constraint("+region=us-east-1")
///     .add_constraint("-zone=deprecated"); // Exclude deprecated zones
/// ```
///
/// ## Tiered Read Preferences
///
/// ```rust
/// use reinhardt_query::types::ZoneConfig;
///
/// // Primary: us-east-1, Secondary: us-west-1
/// let zone = ZoneConfig::new()
///     .num_replicas(3)
///     .add_lease_preference("+region=us-east-1")
///     .add_lease_preference("+region=us-west-1"); // Fallback if us-east-1 unavailable
/// ```
#[derive(Debug, Clone, Default)]
pub struct ZoneConfig {
	pub(crate) num_replicas: Option<i32>,
	pub(crate) constraints: Vec<String>,
	pub(crate) lease_preferences: Vec<String>,
}

impl ZoneConfig {
	/// Create a new zone configuration
	pub fn new() -> Self {
		Self {
			num_replicas: None,
			constraints: Vec::new(),
			lease_preferences: Vec::new(),
		}
	}

	/// Set the number of replicas
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::ZoneConfig;
	///
	/// let zone = ZoneConfig::new().num_replicas(3);
	/// ```
	pub fn num_replicas(mut self, replicas: i32) -> Self {
		self.num_replicas = Some(replicas);
		self
	}

	/// Add a constraint
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::ZoneConfig;
	///
	/// let zone = ZoneConfig::new()
	///     .add_constraint("+region=us-east-1")
	///     .add_constraint("+zone=a");
	/// ```
	pub fn add_constraint<S: Into<String>>(mut self, constraint: S) -> Self {
		self.constraints.push(constraint.into());
		self
	}

	/// Add a lease preference
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::ZoneConfig;
	///
	/// let zone = ZoneConfig::new()
	///     .add_lease_preference("+region=us-east-1");
	/// ```
	pub fn add_lease_preference<S: Into<String>>(mut self, preference: S) -> Self {
		self.lease_preferences.push(preference.into());
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_zone_config_new() {
		let zone = ZoneConfig::new();
		assert!(zone.num_replicas.is_none());
		assert!(zone.constraints.is_empty());
		assert!(zone.lease_preferences.is_empty());
	}

	#[rstest]
	fn test_zone_config_num_replicas() {
		let zone = ZoneConfig::new().num_replicas(3);
		assert_eq!(zone.num_replicas, Some(3));
	}

	#[rstest]
	fn test_zone_config_add_constraint() {
		let zone = ZoneConfig::new()
			.add_constraint("+region=us-east-1")
			.add_constraint("+zone=a");
		assert_eq!(zone.constraints.len(), 2);
		assert_eq!(zone.constraints[0], "+region=us-east-1");
		assert_eq!(zone.constraints[1], "+zone=a");
	}

	#[rstest]
	fn test_zone_config_add_lease_preference() {
		let zone = ZoneConfig::new()
			.add_lease_preference("+region=us-east-1")
			.add_lease_preference("+zone=a");
		assert_eq!(zone.lease_preferences.len(), 2);
		assert_eq!(zone.lease_preferences[0], "+region=us-east-1");
		assert_eq!(zone.lease_preferences[1], "+zone=a");
	}

	#[rstest]
	fn test_zone_config_all_options() {
		let zone = ZoneConfig::new()
			.num_replicas(5)
			.add_constraint("+region=us-east-1")
			.add_constraint("-region=us-west-1")
			.add_lease_preference("+region=us-east-1");
		assert_eq!(zone.num_replicas, Some(5));
		assert_eq!(zone.constraints.len(), 2);
		assert_eq!(zone.lease_preferences.len(), 1);
	}
}
