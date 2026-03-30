//! Collection association proxies for one-to-many and many-to-many relationships

pub mod aggregations;
pub mod operations;

pub use aggregations::CollectionAggregations;
pub use operations::CollectionOperations;

use crate::proxy::{ProxyError, ProxyResult, ScalarValue};
use serde::{Deserialize, Serialize};

/// Collection proxy for accessing multiple related objects' attributes
///
/// Used for one-to-many and many-to-many relationships where the proxy
/// returns a collection of scalar values.
///
/// ## Example
///
/// ```rust,no_run
/// # use reinhardt_urls::proxy::CollectionProxy;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # struct User;
/// # let user = User;
/// // User has many posts, access all post titles directly
/// let titles_proxy: CollectionProxy = CollectionProxy::new("posts", "title");
/// // let titles: Vec<String> = titles_proxy.get_values(&user).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CollectionProxy {
	/// Name of the relationship
	pub relationship: String,

	/// Name of the attribute on the related objects
	pub attribute: String,

	/// Whether to remove duplicates
	pub unique: bool,

	/// Whether to deduplicate values
	pub deduplicate: bool,

	/// Loading strategy for the relationship
	pub loading_strategy: Option<crate::proxy::LoadingStrategy>,

	/// Factory for creating new instances from scalar values
	pub factory: Option<std::sync::Arc<dyn super::reflection::ReflectableFactory>>,

	/// Whether caching is enabled
	caching: bool,

	/// Cache time-to-live in seconds
	cache_ttl: Option<u64>,

	/// Whether streaming is enabled
	streaming: bool,

	/// Whether triggers are enabled
	triggers: bool,

	/// Trigger events to monitor
	trigger_events: Vec<String>,

	/// Stored procedure name
	stored_procedure: Option<String>,

	/// Stored procedure parameters
	procedure_params: Vec<(String, String)>,

	/// Target database name
	database: Option<String>,

	/// Fallback database name
	fallback_database: Option<String>,

	/// Whether async loading is enabled
	async_loading: bool,

	/// Whether concurrent access is supported
	concurrent_access: bool,

	/// Whether this proxy targets a database view
	is_view: bool,

	/// Batch size for bulk operations
	batch_size: Option<usize>,

	/// Chunk size for processing batches
	chunk_size: Option<usize>,

	/// Whether cascade delete/update is enabled
	cascade: bool,

	/// Whether version tracking is enabled for items
	version_tracking: bool,

	/// Memory limit in bytes for collection operations
	memory_limit: Option<usize>,
}

impl CollectionProxy {
	/// Create a new collection proxy
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title");
	/// assert_eq!(proxy.relationship, "posts");
	/// assert_eq!(proxy.attribute, "title");
	/// assert!(!proxy.unique);
	/// ```
	pub fn new(relationship: &str, attribute: &str) -> Self {
		Self {
			relationship: relationship.to_string(),
			attribute: attribute.to_string(),
			unique: false,
			deduplicate: false,
			loading_strategy: None,
			factory: None,
			caching: false,
			cache_ttl: None,
			streaming: false,
			triggers: false,
			trigger_events: Vec::new(),
			stored_procedure: None,
			procedure_params: Vec::new(),
			database: None,
			fallback_database: None,
			async_loading: false,
			concurrent_access: false,
			is_view: false,
			batch_size: None,
			chunk_size: None,
			cascade: false,
			version_tracking: false,
			memory_limit: None,
		}
	}
	/// Create a collection proxy that removes duplicates
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::unique("tags", "name");
	/// assert_eq!(proxy.relationship, "tags");
	/// assert_eq!(proxy.attribute, "name");
	/// assert!(proxy.unique);
	/// ```
	pub fn unique(relationship: &str, attribute: &str) -> Self {
		Self {
			relationship: relationship.to_string(),
			attribute: attribute.to_string(),
			unique: true,
			deduplicate: false,
			loading_strategy: None,
			factory: None,
			caching: false,
			cache_ttl: None,
			streaming: false,
			triggers: false,
			trigger_events: Vec::new(),
			stored_procedure: None,
			procedure_params: Vec::new(),
			database: None,
			fallback_database: None,
			async_loading: false,
			concurrent_access: false,
			is_view: false,
			batch_size: None,
			chunk_size: None,
			cascade: false,
			version_tracking: false,
			memory_limit: None,
		}
	}

	/// Create a collection proxy with a factory for creating instances
	pub fn with_factory(
		relationship: &str,
		attribute: &str,
		factory: std::sync::Arc<dyn super::reflection::ReflectableFactory>,
	) -> Self {
		Self {
			relationship: relationship.to_string(),
			attribute: attribute.to_string(),
			unique: false,
			deduplicate: false,
			loading_strategy: None,
			factory: Some(factory),
			caching: false,
			cache_ttl: None,
			streaming: false,
			triggers: false,
			trigger_events: Vec::new(),
			stored_procedure: None,
			procedure_params: Vec::new(),
			database: None,
			fallback_database: None,
			async_loading: false,
			concurrent_access: false,
			is_view: false,
			batch_size: None,
			chunk_size: None,
			cascade: false,
			version_tracking: false,
			memory_limit: None,
		}
	}

	/// Set the factory for this proxy
	pub fn set_factory(
		&mut self,
		factory: std::sync::Arc<dyn super::reflection::ReflectableFactory>,
	) {
		self.factory = Some(factory);
	}
	/// Get collection of values from related objects
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("posts", "title");
	// Assuming \`user\` implements Reflectable
	// let titles = proxy.get_values(&user).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn get_values<T>(&self, source: &T) -> ProxyResult<Vec<ScalarValue>>
	where
		T: super::reflection::Reflectable,
	{
		// 1. Access the relationship on source
		let relationship = source
			.get_relationship(&self.relationship)
			.ok_or_else(|| ProxyError::RelationshipNotFound(self.relationship.clone()))?;

		// 2. Downcast to Vec<Box<dyn Reflectable>>
		let collection = super::reflection::downcast_relationship::<
			Vec<Box<dyn super::reflection::Reflectable>>,
		>(relationship)?;

		// 3. Extract the attribute from each item
		let mut values = Vec::new();
		for item in collection.iter() {
			let value = item
				.get_attribute(&self.attribute)
				.ok_or_else(|| ProxyError::AttributeNotFound(self.attribute.clone()))?;
			values.push(value);
		}

		// 4. Optionally remove duplicates
		if self.unique {
			values.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
			values.dedup_by(|a, b| format!("{:?}", a) == format!("{:?}", b));
		}

		Ok(values)
	}
	/// Set collection of values by creating/updating related objects
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::{CollectionProxy, ScalarValue};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("tags", "name");
	/// let values = vec![ScalarValue::String("rust".to_string())];
	// let mut user = ...;
	// proxy.set_values(&mut user, values).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn set_values<T>(&self, source: &mut T, values: Vec<ScalarValue>) -> ProxyResult<()>
	where
		T: super::reflection::Reflectable,
	{
		// 1. Check if factory is configured
		let factory = self
			.factory
			.as_ref()
			.ok_or(ProxyError::FactoryNotConfigured)?;

		// 2. Access the relationship
		let relationship = source
			.get_relationship_mut(&self.relationship)
			.ok_or_else(|| ProxyError::RelationshipNotFound(self.relationship.clone()))?;

		// 3. Downcast to Vec<Box<dyn Reflectable>>
		let collection = relationship
			.downcast_mut::<Vec<Box<dyn super::reflection::Reflectable>>>()
			.ok_or_else(|| ProxyError::TypeMismatch {
				expected: "Vec<Box<dyn Reflectable>>".to_string(),
				actual: "unknown".to_string(),
			})?;

		// 4. Clear existing collection
		collection.clear();

		// 5. Create new objects from scalar values and add to collection
		for value in values {
			let new_object = factory.create_from_scalar(&self.attribute, value)?;
			collection.push(new_object);
		}

		Ok(())
	}
	/// Append a value to the collection
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::{CollectionProxy, ScalarValue};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("tags", "name");
	// let mut user = ...;
	// proxy.append(&mut user, ScalarValue::String("new_tag".to_string())).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn append<T>(&self, source: &mut T, value: ScalarValue) -> ProxyResult<()>
	where
		T: super::reflection::Reflectable,
	{
		// 1. Check if factory is configured
		let factory = self
			.factory
			.as_ref()
			.ok_or(ProxyError::FactoryNotConfigured)?;

		// 2. Access the relationship
		let relationship = source
			.get_relationship_mut(&self.relationship)
			.ok_or_else(|| ProxyError::RelationshipNotFound(self.relationship.clone()))?;

		// 3. Downcast to Vec<Box<dyn Reflectable>>
		let collection = relationship
			.downcast_mut::<Vec<Box<dyn super::reflection::Reflectable>>>()
			.ok_or_else(|| ProxyError::TypeMismatch {
				expected: "Vec<Box<dyn Reflectable>>".to_string(),
				actual: "unknown".to_string(),
			})?;

		// 4. Create new object from scalar value
		let new_object = factory.create_from_scalar(&self.attribute, value)?;

		// 5. Append to collection
		collection.push(new_object);

		Ok(())
	}
	/// Remove a value from the collection
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::{CollectionProxy, ScalarValue};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("tags", "name");
	// let mut user = ...;
	// proxy.remove(&mut user, ScalarValue::String("old_tag".to_string())).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn remove<T>(&self, source: &mut T, value: ScalarValue) -> ProxyResult<()>
	where
		T: super::reflection::Reflectable,
	{
		// 1. Access the relationship
		let relationship = source
			.get_relationship_mut(&self.relationship)
			.ok_or_else(|| ProxyError::RelationshipNotFound(self.relationship.clone()))?;

		// 2. Downcast to Vec<Box<dyn Reflectable>>
		let collection = relationship
			.downcast_mut::<Vec<Box<dyn super::reflection::Reflectable>>>()
			.ok_or_else(|| ProxyError::TypeMismatch {
				expected: "Vec<Box<dyn Reflectable>>".to_string(),
				actual: "unknown".to_string(),
			})?;

		// 3. Find and remove items with matching attribute value
		collection.retain(|item| {
			item.get_attribute(&self.attribute)
				.map(|v| v != value)
				.unwrap_or(true)
		});

		Ok(())
	}
	/// Check if the collection contains a value
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::{CollectionProxy, ScalarValue};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("tags", "name");
	// let user = ...;
	// let has_tag = proxy.contains(&user, ScalarValue::String("rust".to_string())).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn contains<T>(&self, source: &T, value: ScalarValue) -> ProxyResult<bool>
	where
		T: super::reflection::Reflectable,
	{
		let values = self.get_values(source).await?;
		Ok(values.contains(&value))
	}
	/// Get the count of items in the collection
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("posts", "title");
	// let user = ...;
	// let count = proxy.count(&user).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn count<T>(&self, source: &T) -> ProxyResult<usize>
	where
		T: super::reflection::Reflectable,
	{
		// 1. Access the relationship
		let relationship = source
			.get_relationship(&self.relationship)
			.ok_or_else(|| ProxyError::RelationshipNotFound(self.relationship.clone()))?;

		// 2. Downcast to Vec<Box<dyn Reflectable>>
		let collection = super::reflection::downcast_relationship::<
			Vec<Box<dyn super::reflection::Reflectable>>,
		>(relationship)?;

		// 3. Return the count
		Ok(collection.len())
	}
	/// Filter collection by a condition on the proxy attribute
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_urls::proxy::{CollectionProxy, query::{FilterCondition, FilterOp}};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("posts", "status");
	/// let condition = FilterCondition::new("status", FilterOp::eq("published"));
	// let user = ...;
	// let filtered = proxy.filter(&user, condition).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn filter<T>(
		&self,
		source: &T,
		condition: crate::proxy::query::FilterCondition,
	) -> ProxyResult<Vec<ScalarValue>>
	where
		T: super::reflection::Reflectable,
	{
		// Get all values first
		let values = self.get_values(source).await?;

		// Filter using the condition
		let filtered: Vec<ScalarValue> = values
			.into_iter()
			.filter(|v| condition.matches(v))
			.collect();

		Ok(filtered)
	}
	/// Filter collection using a custom predicate
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::{CollectionProxy, ScalarValue};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("posts", "views");
	// let user = ...;
	// let popular = proxy.filter_by(&user, |v| {
	//     matches!(v, ScalarValue::Integer(n) if *n > 1000)
	// }).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn filter_by<T, F>(&self, source: &T, predicate: F) -> ProxyResult<Vec<ScalarValue>>
	where
		T: super::reflection::Reflectable,
		F: Fn(&ScalarValue) -> bool,
	{
		// Get all values first
		let values = self.get_values(source).await?;

		// Filter using the predicate
		let filtered: Vec<ScalarValue> = values.into_iter().filter(|v| predicate(v)).collect();

		Ok(filtered)
	}

	// ========== Accessor methods ==========

	/// Get the relationship name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title");
	/// assert_eq!(proxy.relationship(), "posts");
	/// ```
	pub fn relationship(&self) -> &str {
		&self.relationship
	}

	/// Get the attribute name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title");
	/// assert_eq!(proxy.attribute(), "title");
	/// ```
	pub fn attribute(&self) -> &str {
		&self.attribute
	}

	/// Check if this proxy removes duplicates
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::unique("tags", "name");
	/// assert!(proxy.is_unique());
	///
	/// let proxy2 = CollectionProxy::new("posts", "title");
	/// assert!(!proxy2.is_unique());
	/// ```
	pub fn is_unique(&self) -> bool {
		self.unique
	}

	/// Check if this proxy deduplicates values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("tags", "name").with_deduplication(true);
	/// assert!(proxy.deduplicates());
	/// ```
	pub fn deduplicates(&self) -> bool {
		self.deduplicate
	}

	// ========== Builder pattern methods ==========

	/// Set whether to remove duplicates (builder pattern)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("tags", "name").with_unique(true);
	/// assert!(proxy.is_unique());
	/// ```
	pub fn with_unique(mut self, unique: bool) -> Self {
		self.unique = unique;
		self
	}

	/// Set whether to deduplicate values (builder pattern)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("tags", "name").with_deduplication(true);
	/// assert!(proxy.deduplicates());
	/// ```
	pub fn with_deduplication(mut self, deduplicate: bool) -> Self {
		self.deduplicate = deduplicate;
		self
	}

	/// Set the loading strategy (builder pattern)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{CollectionProxy, LoadingStrategy};
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_loading_strategy(LoadingStrategy::Joined);
	/// ```
	pub fn with_loading_strategy(mut self, strategy: crate::proxy::LoadingStrategy) -> Self {
		self.loading_strategy = Some(strategy);
		self
	}

	/// Filter collection where any item matches the condition
	///
	/// Returns a new proxy with filtered values where at least one item
	/// in the collection matches the specified field and value.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// // Example: Filter posts where any tag matches "rust"
	/// // let proxy = CollectionProxy::new("posts", "tags");
	/// // let filtered = proxy.filter_with_any("name", "rust").await?;
	/// ```
	pub async fn filter_with_any<T>(
		&self,
		source: &T,
		field: &str,
		value: &str,
	) -> ProxyResult<Vec<ScalarValue>>
	where
		T: super::reflection::Reflectable,
	{
		// Get the collection
		let rel = source
			.get_relationship(&self.relationship)
			.ok_or_else(|| ProxyError::RelationshipNotFound(self.relationship.clone()))?;

		let collection = super::reflection::downcast_relationship::<
			Vec<Box<dyn super::reflection::Reflectable>>,
		>(rel)?;

		// Filter values where any item in the collection matches the field and value
		let mut filtered_values = Vec::new();
		for item in collection.iter() {
			// Check if any attribute of the item matches
			if let Some(attr_value) = item.get_attribute(field) {
				let matches = match &attr_value {
					ScalarValue::String(s) => s.contains(value),
					_ => format!("{:?}", attr_value).contains(value),
				};

				if matches {
					// Add the proxy's target attribute to filtered values
					if let Some(value) = item.get_attribute(&self.attribute) {
						filtered_values.push(value);
					}
				}
			}
		}

		Ok(filtered_values)
	}

	/// Filter collection where item has specific relationship
	///
	/// Filters the collection to include only items that have a specific
	/// relationship with the given value.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// // Example: Filter posts that have comments from "Alice"
	/// // let proxy = CollectionProxy::new("posts", "title");
	/// // let source = ...;
	/// // let filtered = proxy.filter_with_has(&source, "comments", "Alice").await?;
	/// ```
	pub async fn filter_with_has<T>(
		&self,
		source: &T,
		relationship: &str,
		value: &str,
	) -> ProxyResult<Vec<ScalarValue>>
	where
		T: super::reflection::Reflectable,
	{
		// Get the collection
		let rel = source
			.get_relationship(&self.relationship)
			.ok_or_else(|| ProxyError::RelationshipNotFound(self.relationship.clone()))?;

		let collection = super::reflection::downcast_relationship::<
			Vec<Box<dyn super::reflection::Reflectable>>,
		>(rel)?;

		// Filter items that have the specified relationship
		let mut filtered_values = Vec::new();
		for item in collection.iter() {
			// Check if item has the specified relationship
			if let Some(item_rel) = item.get_relationship(relationship) {
				// Check if the relationship contains the value
				if let Ok(rel_collection) = super::reflection::downcast_relationship::<
					Vec<Box<dyn super::reflection::Reflectable>>,
				>(item_rel)
				{
					let has_value = rel_collection.iter().any(|rel_item| {
						if let Some(ScalarValue::String(s)) =
							rel_item.get_attribute(&self.attribute).as_ref()
						{
							s == value
						} else {
							false
						}
					});

					if has_value {
						// Add the item's attribute to filtered values
						if let Some(value) = item.get_attribute(&self.attribute) {
							filtered_values.push(value);
						}
					}
				}
			}
		}

		Ok(filtered_values)
	}

	/// Configure cascade behavior for related operations
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_cascade(true);
	/// assert!(proxy.is_cascade());
	/// ```
	pub fn with_cascade(mut self, cascade: bool) -> Self {
		self.cascade = cascade;
		self
	}

	/// Merge with another collection proxy
	///
	/// Merges two collection proxies by combining their values.
	/// Note: This operation requires both proxies to work on the same source object.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// // Merge values from two different relationships
	/// // let proxy1 = CollectionProxy::new("posts", "title");
	/// // let proxy2 = CollectionProxy::new("drafts", "title");
	/// // let source = ...;
	/// // let merged = proxy1.merge(&source, proxy2, &source).await?;
	/// ```
	pub async fn merge<T>(
		&self,
		source1: &T,
		other: &Self,
		source2: &T,
	) -> ProxyResult<Vec<ScalarValue>>
	where
		T: super::reflection::Reflectable,
	{
		// Get values from both proxies
		let mut values1 = self.get_values(source1).await?;
		let values2 = other.get_values(source2).await?;

		// Merge the values
		values1.extend(values2);

		// Optionally deduplicate if unique is set
		if self.unique || other.unique {
			values1.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
			values1.dedup_by(|a, b| format!("{:?}", a) == format!("{:?}", b));
		}

		Ok(values1)
	}

	/// Set batch size for collection operations
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_batch_size(100);
	/// assert_eq!(proxy.batch_size(), Some(100));
	/// ```
	pub fn with_batch_size(mut self, batch_size: usize) -> Self {
		self.batch_size = Some(batch_size);
		self
	}

	/// Enable async loading for collection
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_async_loading(true);
	/// ```
	pub fn with_async_loading(mut self, async_loading: bool) -> Self {
		self.async_loading = async_loading;
		self
	}

	/// Enable streaming for large collections
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("logs", "message")
	///     .with_streaming(true);
	/// ```
	pub fn with_streaming(mut self, streaming: bool) -> Self {
		self.streaming = streaming;
		self
	}

	/// Enable caching for collection queries
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_caching(true);
	/// ```
	pub fn with_caching(mut self, caching: bool) -> Self {
		self.caching = caching;
		self
	}

	/// Configure database for collection operations
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_database("analytics");
	/// ```
	pub fn with_database(mut self, database: &str) -> Self {
		self.database = Some(database.to_string());
		self
	}

	/// Set memory limit for collection operations
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_memory_limit(1024 * 1024);
	/// assert_eq!(proxy.memory_limit(), Some(1024 * 1024));
	/// ```
	pub fn with_memory_limit(mut self, memory_limit: usize) -> Self {
		self.memory_limit = Some(memory_limit);
		self
	}

	/// Configure stored procedure for collection operations
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_stored_procedure("get_posts");
	/// ```
	pub fn with_stored_procedure(mut self, procedure: &str) -> Self {
		self.stored_procedure = Some(procedure.to_string());
		self
	}

	/// Configure triggers for collection operations
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_triggers(true);
	/// ```
	pub fn with_triggers(mut self, triggers: bool) -> Self {
		self.triggers = triggers;
		self
	}

	/// Enable version tracking for collection items
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_version_tracking(true);
	/// assert!(proxy.is_version_tracking());
	/// ```
	pub fn with_version_tracking(mut self, version_tracking: bool) -> Self {
		self.version_tracking = version_tracking;
		self
	}

	/// Bulk insert multiple items into the collection
	///
	/// Inserts multiple scalar values into the collection by creating
	/// Reflectable objects using the configured factory.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::{CollectionProxy, ScalarValue};
	///
	/// // Requires factory to be configured
	/// // let proxy = CollectionProxy::with_factory("tags", "name", factory)
	/// //     .with_batch_size(100);
	/// // let mut source = ...;
	/// // let values = vec![
	/// //     ScalarValue::String("rust".to_string()),
	/// //     ScalarValue::String("python".to_string()),
	/// // ];
	/// // proxy.bulk_insert(&mut source, values).await?;
	/// ```
	pub async fn bulk_insert<T>(&self, source: &mut T, items: Vec<ScalarValue>) -> ProxyResult<()>
	where
		T: super::reflection::Reflectable,
	{
		// Check if factory is configured
		let factory = self
			.factory
			.as_ref()
			.ok_or(ProxyError::FactoryNotConfigured)?;

		// Access the relationship
		let relationship = source
			.get_relationship_mut(&self.relationship)
			.ok_or_else(|| ProxyError::RelationshipNotFound(self.relationship.clone()))?;

		// Downcast to Vec<Box<dyn Reflectable>>
		let collection = relationship
			.downcast_mut::<Vec<Box<dyn super::reflection::Reflectable>>>()
			.ok_or_else(|| ProxyError::TypeMismatch {
				expected: "Vec<Box<dyn Reflectable>>".to_string(),
				actual: "unknown".to_string(),
			})?;

		// Determine batch size (default to all items if not set)
		let batch_size = self.batch_size.unwrap_or(items.len());

		// Process items in batches
		for chunk in items.chunks(batch_size) {
			for value in chunk {
				let new_object = factory.create_from_scalar(&self.attribute, value.clone())?;
				collection.push(new_object);
			}
		}

		Ok(())
	}

	/// Clear all items from the collection
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// // Requires a mutable source object with Reflectable trait
	/// // let mut source = ...;
	/// // let proxy = CollectionProxy::new("tags", "name");
	/// // proxy.clear_on(&mut source).await?;
	/// ```
	pub async fn clear_on<T>(&self, source: &mut T) -> ProxyResult<()>
	where
		T: super::reflection::Reflectable,
	{
		// Access the relationship
		let relationship = source
			.get_relationship_mut(&self.relationship)
			.ok_or_else(|| ProxyError::RelationshipNotFound(self.relationship.clone()))?;

		// Downcast to Vec<Box<dyn Reflectable>>
		let collection = relationship
			.downcast_mut::<Vec<Box<dyn super::reflection::Reflectable>>>()
			.ok_or_else(|| ProxyError::TypeMismatch {
				expected: "Vec<Box<dyn Reflectable>>".to_string(),
				actual: "unknown".to_string(),
			})?;

		// Clear the collection
		collection.clear();

		Ok(())
	}

	/// Check if this proxy targets a database view
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("user_view", "name")
	///     .with_view(true);
	/// assert!(proxy.is_view());
	/// ```
	pub fn is_view(&self) -> bool {
		self.is_view
	}

	/// Configure this proxy to target a database view
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("user_summary", "name")
	///     .with_view(true);
	/// assert!(proxy.is_view());
	/// ```
	pub fn with_view(mut self, is_view: bool) -> Self {
		self.is_view = is_view;
		self
	}

	/// Update items with version tracking
	///
	/// Updates collection items with optimistic locking based on version numbers.
	/// This method ensures that updates only succeed if the version matches,
	/// preventing concurrent modification conflicts.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_urls::proxy::{CollectionProxy, ScalarValue};
	///
	/// // Requires version tracking to be enabled
	/// // let proxy = CollectionProxy::new("documents", "content")
	/// //     .with_version_tracking(true);
	/// // let mut source = ...;
	/// // let new_value = ScalarValue::String("updated content".to_string());
	/// // proxy.update_with_version(&mut source, new_value, 1).await?;
	/// ```
	pub async fn update_with_version<T>(
		&self,
		source: &mut T,
		value: ScalarValue,
		expected_version: i64,
	) -> ProxyResult<()>
	where
		T: super::reflection::Reflectable,
	{
		// Check if version tracking is enabled
		if !self.version_tracking {
			return Err(ProxyError::VersionTrackingNotEnabled);
		}

		// Access the relationship
		let relationship = source
			.get_relationship_mut(&self.relationship)
			.ok_or_else(|| ProxyError::RelationshipNotFound(self.relationship.clone()))?;

		// Downcast to Vec<Box<dyn Reflectable>>
		let collection = relationship
			.downcast_mut::<Vec<Box<dyn super::reflection::Reflectable>>>()
			.ok_or_else(|| ProxyError::TypeMismatch {
				expected: "Vec<Box<dyn Reflectable>>".to_string(),
				actual: "unknown".to_string(),
			})?;

		// Update items with version check
		for item in collection.iter_mut() {
			// Check if item has version field
			if let Some(current_version) = item.get_attribute("version")
				&& let ScalarValue::Integer(ver) = current_version
			{
				// Check version match
				if ver == expected_version {
					// Update the value using set_attribute
					item.set_attribute(&self.attribute, value.clone())?;
					// Increment version
					item.set_attribute("version", ScalarValue::Integer(ver + 1))?;
				} else {
					return Err(ProxyError::VersionMismatch {
						expected: expected_version,
						actual: ver,
					});
				}
			}
		}

		Ok(())
	}

	/// Configure cache time-to-live in seconds
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("products", "price")
	///     .with_caching(true)
	///     .with_cache_ttl(3600); // 1 hour
	/// ```
	pub fn with_cache_ttl(mut self, ttl: u64) -> Self {
		self.cache_ttl = Some(ttl);
		self
	}

	/// Configure chunk size for batch operations
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("orders", "total")
	///     .with_batch_size(100)
	///     .with_chunk_size(10); // Process 10 items at a time
	/// assert_eq!(proxy.chunk_size(), Some(10));
	/// ```
	pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
		self.chunk_size = Some(chunk_size);
		self
	}

	/// Configure concurrent access settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("sessions", "data")
	///     .with_concurrent_access(true);
	/// ```
	pub fn with_concurrent_access(mut self, concurrent: bool) -> Self {
		self.concurrent_access = concurrent;
		self
	}

	/// Configure fallback database for read operations
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("users", "name")
	///     .with_database("primary")
	///     .with_fallback_database("replica");
	/// ```
	pub fn with_fallback_database(mut self, database: &str) -> Self {
		self.fallback_database = Some(database.to_string());
		self
	}

	/// Configure parameters for stored procedures
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("reports", "data")
	///     .with_stored_procedure("generate_report")
	///     .with_procedure_params(&[("start_date", "2024-01-01"), ("end_date", "2024-12-31")]);
	/// ```
	pub fn with_procedure_params(mut self, params: &[(&str, &str)]) -> Self {
		self.procedure_params = params
			.iter()
			.map(|(k, v)| (k.to_string(), v.to_string()))
			.collect();
		self
	}

	/// Configure trigger events to monitor
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("audit_logs", "action")
	///     .with_triggers(true)
	///     .with_trigger_events(&["INSERT", "UPDATE", "DELETE"]);
	/// ```
	pub fn with_trigger_events(mut self, events: &[&str]) -> Self {
		self.trigger_events = events.iter().map(|e| e.to_string()).collect();
		self
	}

	// Getter methods

	/// Check if caching is enabled
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_caching(true);
	/// assert!(proxy.is_cached());
	/// ```
	pub fn is_cached(&self) -> bool {
		self.caching
	}

	/// Get cache time-to-live
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_cache_ttl(300);
	/// assert_eq!(proxy.cache_ttl(), Some(300));
	/// ```
	pub fn cache_ttl(&self) -> Option<u64> {
		self.cache_ttl
	}

	/// Check if triggers are enabled
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("users", "name")
	///     .with_triggers(true);
	/// assert!(proxy.has_triggers());
	/// ```
	pub fn has_triggers(&self) -> bool {
		self.triggers
	}

	/// Get trigger events
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("logs", "action")
	///     .with_trigger_events(&["INSERT", "UPDATE"]);
	/// assert_eq!(proxy.trigger_events().len(), 2);
	/// ```
	pub fn trigger_events(&self) -> &[String] {
		&self.trigger_events
	}

	/// Get stored procedure name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("reports", "data")
	///     .with_stored_procedure("generate_report");
	/// assert_eq!(proxy.stored_procedure(), Some("generate_report"));
	/// ```
	pub fn stored_procedure(&self) -> Option<&str> {
		self.stored_procedure.as_deref()
	}

	/// Get stored procedure parameters
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("reports", "data")
	///     .with_procedure_params(&[("year", "2024")]);
	/// assert_eq!(proxy.procedure_params().len(), 1);
	/// ```
	pub fn procedure_params(&self) -> &[(String, String)] {
		&self.procedure_params
	}

	/// Get target database name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("users", "name")
	///     .with_database("primary");
	/// assert_eq!(proxy.database(), Some("primary"));
	/// ```
	pub fn database(&self) -> Option<&str> {
		self.database.as_deref()
	}

	/// Get fallback database name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("users", "name")
	///     .with_fallback_database("replica");
	/// assert_eq!(proxy.fallback_database(), Some("replica"));
	/// ```
	pub fn fallback_database(&self) -> Option<&str> {
		self.fallback_database.as_deref()
	}

	/// Check if async loading is enabled
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_async_loading(true);
	/// assert!(proxy.is_async_loading());
	/// ```
	pub fn is_async_loading(&self) -> bool {
		self.async_loading
	}

	/// Check if concurrent access is supported
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("sessions", "data")
	///     .with_concurrent_access(true);
	/// assert!(proxy.supports_concurrent_access());
	/// ```
	pub fn supports_concurrent_access(&self) -> bool {
		self.concurrent_access
	}

	/// Get batch size
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("orders", "total")
	///     .with_batch_size(100);
	/// assert_eq!(proxy.batch_size(), Some(100));
	/// ```
	pub fn batch_size(&self) -> Option<usize> {
		self.batch_size
	}

	/// Get chunk size
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("orders", "total")
	///     .with_chunk_size(10);
	/// assert_eq!(proxy.chunk_size(), Some(10));
	/// ```
	pub fn chunk_size(&self) -> Option<usize> {
		self.chunk_size
	}

	/// Check if cascade is enabled
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("posts", "title")
	///     .with_cascade(true);
	/// assert!(proxy.is_cascade());
	/// ```
	pub fn is_cascade(&self) -> bool {
		self.cascade
	}

	/// Check if version tracking is enabled
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("documents", "content")
	///     .with_version_tracking(true);
	/// assert!(proxy.is_version_tracking());
	/// ```
	pub fn is_version_tracking(&self) -> bool {
		self.version_tracking
	}

	/// Get memory limit
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::CollectionProxy;
	///
	/// let proxy = CollectionProxy::new("large_data", "content")
	///     .with_memory_limit(1024 * 1024);
	/// assert_eq!(proxy.memory_limit(), Some(1024 * 1024));
	/// ```
	pub fn memory_limit(&self) -> Option<usize> {
		self.memory_limit
	}
}

// Manual Debug implementation for CollectionProxy
impl std::fmt::Debug for CollectionProxy {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CollectionProxy")
			.field("relationship", &self.relationship)
			.field("attribute", &self.attribute)
			.field("unique", &self.unique)
			.field("deduplicate", &self.deduplicate)
			.field("loading_strategy", &self.loading_strategy)
			.field("factory", &self.factory.as_ref().map(|_| "Some(<factory>)"))
			.finish()
	}
}

// Manual Serialize implementation (factory and loading_strategy are not serializable)
impl Serialize for CollectionProxy {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		use serde::ser::SerializeStruct;
		let mut state = serializer.serialize_struct("CollectionProxy", 8)?;
		state.serialize_field("relationship", &self.relationship)?;
		state.serialize_field("attribute", &self.attribute)?;
		state.serialize_field("unique", &self.unique)?;
		state.serialize_field("batch_size", &self.batch_size)?;
		state.serialize_field("chunk_size", &self.chunk_size)?;
		state.serialize_field("cascade", &self.cascade)?;
		state.serialize_field("version_tracking", &self.version_tracking)?;
		state.serialize_field("memory_limit", &self.memory_limit)?;
		// loading_strategy and factory are not serialized (not Serialize)
		state.end()
	}
}

// Manual Deserialize implementation (factory and loading_strategy are restored as None)
impl<'de> Deserialize<'de> for CollectionProxy {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		struct CollectionProxyVisitor;

		impl<'de> serde::de::Visitor<'de> for CollectionProxyVisitor {
			type Value = CollectionProxy;

			fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
				formatter.write_str("struct CollectionProxy")
			}

			fn visit_map<V>(self, mut map: V) -> Result<CollectionProxy, V::Error>
			where
				V: serde::de::MapAccess<'de>,
			{
				let mut relationship = None;
				let mut attribute = None;
				let mut unique = None;
				let mut caching = None;
				let mut cache_ttl = None;
				let mut batch_size = None;
				let mut chunk_size = None;
				let mut cascade = None;
				let mut version_tracking = None;
				let mut memory_limit = None;

				while let Some(key) = map.next_key::<String>()? {
					match key.as_str() {
						"relationship" => {
							if relationship.is_some() {
								return Err(serde::de::Error::duplicate_field("relationship"));
							}
							relationship = Some(map.next_value()?);
						}
						"attribute" => {
							if attribute.is_some() {
								return Err(serde::de::Error::duplicate_field("attribute"));
							}
							attribute = Some(map.next_value()?);
						}
						"unique" => {
							if unique.is_some() {
								return Err(serde::de::Error::duplicate_field("unique"));
							}
							unique = Some(map.next_value()?);
						}
						"caching" => {
							if caching.is_some() {
								return Err(serde::de::Error::duplicate_field("caching"));
							}
							caching = Some(map.next_value()?);
						}
						"cache_ttl" => {
							if cache_ttl.is_some() {
								return Err(serde::de::Error::duplicate_field("cache_ttl"));
							}
							cache_ttl = Some(map.next_value()?);
						}
						"batch_size" => {
							if batch_size.is_some() {
								return Err(serde::de::Error::duplicate_field("batch_size"));
							}
							batch_size = Some(map.next_value()?);
						}
						"chunk_size" => {
							if chunk_size.is_some() {
								return Err(serde::de::Error::duplicate_field("chunk_size"));
							}
							chunk_size = Some(map.next_value()?);
						}
						"cascade" => {
							if cascade.is_some() {
								return Err(serde::de::Error::duplicate_field("cascade"));
							}
							cascade = Some(map.next_value()?);
						}
						"version_tracking" => {
							if version_tracking.is_some() {
								return Err(serde::de::Error::duplicate_field("version_tracking"));
							}
							version_tracking = Some(map.next_value()?);
						}
						"memory_limit" => {
							if memory_limit.is_some() {
								return Err(serde::de::Error::duplicate_field("memory_limit"));
							}
							memory_limit = Some(map.next_value()?);
						}
						_ => {
							// Ignore unknown fields
							let _ = map.next_value::<serde::de::IgnoredAny>()?;
						}
					}
				}

				let relationship =
					relationship.ok_or_else(|| serde::de::Error::missing_field("relationship"))?;
				let attribute =
					attribute.ok_or_else(|| serde::de::Error::missing_field("attribute"))?;
				let unique = unique.ok_or_else(|| serde::de::Error::missing_field("unique"))?;

				Ok(CollectionProxy {
					relationship,
					attribute,
					unique,
					deduplicate: false,     // Default value
					loading_strategy: None, // Cannot be deserialized
					factory: None,          // Cannot be deserialized
					caching: caching.unwrap_or(false),
					cache_ttl,
					streaming: false,
					triggers: false,
					trigger_events: Vec::new(),
					stored_procedure: None,
					procedure_params: Vec::new(),
					database: None,
					fallback_database: None,
					async_loading: false,
					concurrent_access: false,
					is_view: false,
					batch_size,
					chunk_size,
					cascade: cascade.unwrap_or(false),
					version_tracking: version_tracking.unwrap_or(false),
					memory_limit,
				})
			}
		}

		const FIELDS: &[&str] = &[
			"relationship",
			"attribute",
			"unique",
			"batch_size",
			"chunk_size",
			"cascade",
			"version_tracking",
			"memory_limit",
		];
		deserializer.deserialize_struct("CollectionProxy", FIELDS, CollectionProxyVisitor)
	}
}
