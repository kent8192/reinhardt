//! Loading strategies for relationships
//!
//! Provides lazy loading and eager loading strategies for optimizing
//! relationship queries and avoiding N+1 problems.

use std::marker::PhantomData;

/// Loading strategy for relationships
///
/// Determines when and how related objects are loaded from the database.
///
/// # Examples
///
/// ```
/// use reinhardt_db::associations::LoadingStrategy;
///
/// let lazy = LoadingStrategy::Lazy;
/// let eager = LoadingStrategy::Eager;
/// let select_in = LoadingStrategy::SelectIn;
/// let joined = LoadingStrategy::Joined;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadingStrategy {
	/// Load related objects only when accessed (default)
	///
	/// Generates a separate query when the relationship is accessed.
	/// Can lead to N+1 query problem if not careful.
	#[default]
	Lazy,

	/// Load related objects immediately with the parent
	///
	/// Uses JOIN or separate queries to load all related objects upfront.
	Eager,

	/// Load related objects using SELECT IN strategy
	///
	/// Collects all parent IDs and fetches related objects in a single query.
	/// More efficient than lazy loading for multiple objects.
	SelectIn,

	/// Load related objects using JOIN
	///
	/// Uses SQL JOIN to fetch parent and related objects in a single query.
	/// Most efficient for single object queries.
	Joined,

	/// Load related objects using subquery
	///
	/// Uses a subquery to fetch related objects.
	/// Useful for complex filtering scenarios.
	Subquery,
}

/// Lazy loader for relationships
///
/// Loads related objects only when they are accessed.
///
/// # Type Parameters
///
/// * `T` - The type of the related model
///
/// # Examples
///
/// ```
/// use reinhardt_db::associations::LazyLoader;
///
/// #[derive(Clone)]
/// struct Post {
///     id: i64,
///     title: String,
/// }
///
/// let loader: LazyLoader<Post> = LazyLoader::new();
/// ```
#[derive(Debug, Clone)]
pub struct LazyLoader<T> {
	_phantom: PhantomData<T>,
}

impl<T> LazyLoader<T> {
	/// Create a new lazy loader
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::LazyLoader;
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let loader: LazyLoader<Post> = LazyLoader::new();
	/// ```
	pub fn new() -> Self {
		Self {
			_phantom: PhantomData,
		}
	}
}

impl<T> Default for LazyLoader<T> {
	fn default() -> Self {
		Self::new()
	}
}

/// Eager loader for relationships
///
/// Loads related objects immediately when loading the parent object.
///
/// # Type Parameters
///
/// * `T` - The type of the related model
///
/// # Examples
///
/// ```
/// use reinhardt_db::associations::EagerLoader;
///
/// #[derive(Clone)]
/// struct Post {
///     id: i64,
///     title: String,
/// }
///
/// let loader: EagerLoader<Post> = EagerLoader::new();
/// ```
#[derive(Debug, Clone)]
pub struct EagerLoader<T> {
	/// The loading strategy to use
	pub strategy: LoadingStrategy,
	_phantom: PhantomData<T>,
}

impl<T> EagerLoader<T> {
	/// Create a new eager loader with default strategy
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::EagerLoader;
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let loader: EagerLoader<Post> = EagerLoader::new();
	/// ```
	pub fn new() -> Self {
		Self {
			strategy: LoadingStrategy::SelectIn,
			_phantom: PhantomData,
		}
	}

	/// Create a new eager loader with specified strategy
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::{EagerLoader, LoadingStrategy};
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let loader: EagerLoader<Post> = EagerLoader::with_strategy(LoadingStrategy::Joined);
	/// ```
	pub fn with_strategy(strategy: LoadingStrategy) -> Self {
		Self {
			strategy,
			_phantom: PhantomData,
		}
	}

	/// Get the loading strategy
	pub fn strategy(&self) -> LoadingStrategy {
		self.strategy
	}
}

impl<T> Default for EagerLoader<T> {
	fn default() -> Self {
		Self::new()
	}
}

/// SelectIn loader for relationships
///
/// Loads related objects by collecting parent IDs and using WHERE IN clause.
/// More efficient than lazy loading for multiple objects.
///
/// # Type Parameters
///
/// * `T` - The type of the related model
///
/// # Examples
///
/// ```
/// use reinhardt_db::associations::SelectInLoader;
///
/// #[derive(Clone)]
/// struct Post {
///     id: i64,
///     title: String,
/// }
///
/// let loader: SelectInLoader<Post> = SelectInLoader::new();
/// ```
#[derive(Debug, Clone)]
pub struct SelectInLoader<T> {
	/// Maximum number of IDs to include in a single IN clause
	pub batch_size: Option<usize>,
	_phantom: PhantomData<T>,
}

impl<T> SelectInLoader<T> {
	/// Create a new select-in loader
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::SelectInLoader;
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let loader: SelectInLoader<Post> = SelectInLoader::new();
	/// ```
	pub fn new() -> Self {
		Self {
			batch_size: None,
			_phantom: PhantomData,
		}
	}

	/// Set the batch size for IN clauses
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::SelectInLoader;
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let loader: SelectInLoader<Post> = SelectInLoader::new()
	///     .batch_size(100);
	/// ```
	pub fn batch_size(mut self, size: usize) -> Self {
		self.batch_size = Some(size);
		self
	}

	/// Get the batch size
	pub fn get_batch_size(&self) -> Option<usize> {
		self.batch_size
	}
}

impl<T> Default for SelectInLoader<T> {
	fn default() -> Self {
		Self::new()
	}
}

/// Joined loader for relationships
///
/// Loads related objects using SQL JOIN in a single query.
/// Most efficient for single object queries.
///
/// # Type Parameters
///
/// * `T` - The type of the related model
///
/// # Examples
///
/// ```
/// use reinhardt_db::associations::JoinedLoader;
///
/// #[derive(Clone)]
/// struct Post {
///     id: i64,
///     title: String,
/// }
///
/// let loader: JoinedLoader<Post> = JoinedLoader::new();
/// ```
#[derive(Debug, Clone)]
pub struct JoinedLoader<T> {
	/// Whether to use LEFT JOIN (true) or INNER JOIN (false)
	pub outer_join: bool,
	_phantom: PhantomData<T>,
}

impl<T> JoinedLoader<T> {
	/// Create a new joined loader with INNER JOIN
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::JoinedLoader;
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let loader: JoinedLoader<Post> = JoinedLoader::new();
	/// ```
	pub fn new() -> Self {
		Self {
			outer_join: false,
			_phantom: PhantomData,
		}
	}

	/// Create a new joined loader with LEFT JOIN
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::JoinedLoader;
	///
	/// #[derive(Clone)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// let loader: JoinedLoader<Post> = JoinedLoader::outer();
	/// ```
	pub fn outer() -> Self {
		Self {
			outer_join: true,
			_phantom: PhantomData,
		}
	}

	/// Check if using outer join
	pub fn is_outer_join(&self) -> bool {
		self.outer_join
	}
}

impl<T> Default for JoinedLoader<T> {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Allow dead_code: test model struct used for trait implementation verification
	#[allow(dead_code)]
	#[derive(Clone)]
	struct Post {
		id: i64,
		author_id: i64,
		title: String,
	}

	#[test]
	fn test_loading_strategy_default() {
		assert_eq!(LoadingStrategy::default(), LoadingStrategy::Lazy);
	}

	#[test]
	fn test_loading_strategies() {
		let strategies = vec![
			LoadingStrategy::Lazy,
			LoadingStrategy::Eager,
			LoadingStrategy::SelectIn,
			LoadingStrategy::Joined,
			LoadingStrategy::Subquery,
		];

		for strategy in strategies {
			// Just ensure they can be created and compared
			assert_eq!(strategy, strategy);
		}
	}

	#[test]
	fn test_lazy_loader_creation() {
		let _loader: LazyLoader<Post> = LazyLoader::new();
		let _loader2: LazyLoader<Post> = LazyLoader::default();
		// LazyLoader is just a marker type
	}

	#[test]
	fn test_eager_loader_creation() {
		let loader: EagerLoader<Post> = EagerLoader::new();
		assert_eq!(loader.strategy(), LoadingStrategy::SelectIn);
	}

	#[test]
	fn test_eager_loader_with_strategy() {
		let loader: EagerLoader<Post> = EagerLoader::with_strategy(LoadingStrategy::Joined);
		assert_eq!(loader.strategy(), LoadingStrategy::Joined);

		let loader2: EagerLoader<Post> = EagerLoader::with_strategy(LoadingStrategy::Subquery);
		assert_eq!(loader2.strategy(), LoadingStrategy::Subquery);
	}

	#[test]
	fn test_select_in_loader_creation() {
		let loader: SelectInLoader<Post> = SelectInLoader::new();
		assert_eq!(loader.get_batch_size(), None);
	}

	#[test]
	fn test_select_in_loader_batch_size() {
		let loader: SelectInLoader<Post> = SelectInLoader::new().batch_size(100);
		assert_eq!(loader.get_batch_size(), Some(100));

		let loader2: SelectInLoader<Post> = SelectInLoader::new().batch_size(500);
		assert_eq!(loader2.get_batch_size(), Some(500));
	}

	#[test]
	fn test_joined_loader_creation() {
		let loader: JoinedLoader<Post> = JoinedLoader::new();
		assert!(!loader.is_outer_join());
	}

	#[test]
	fn test_joined_loader_outer() {
		let loader: JoinedLoader<Post> = JoinedLoader::outer();
		assert!(loader.is_outer_join());
	}

	#[test]
	fn test_joined_loader_inner() {
		let loader: JoinedLoader<Post> = JoinedLoader::new();
		assert!(!loader.is_outer_join());
	}
}
