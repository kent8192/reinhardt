//! Association collection for accessing collections through associations
//!
//! Provides SQLAlchemy-style association collections for simplifying access
//! to attributes of related objects in collections.

/// Type alias for collection getter function
type CollectionGetterFn<S, C> = Box<dyn Fn(&S) -> &Vec<C>>;

/// Type alias for attribute getter function
type AttributeGetterFn<C, T> = Box<dyn Fn(&C) -> &T>;

/// Association collection proxy for accessing collection attributes
///
/// This allows accessing attributes of items in a collection through a proxy,
/// similar to SQLAlchemy's association_proxy for collections.
///
/// # Type Parameters
///
/// * `S` - Source type (the type containing the collection)
/// * `C` - Collection item type
/// * `T` - Target attribute type
pub struct AssociationCollection<S, C, T> {
	collection_getter: CollectionGetterFn<S, C>,
	attribute_getter: AttributeGetterFn<C, T>,
}

impl<S, C, T> AssociationCollection<S, C, T> {
	/// Create a new association collection proxy
	///
	/// # Arguments
	///
	/// * `collection_getter` - Function to get the collection from the source
	/// * `attribute_getter` - Function to get the target attribute from each collection item
	///
	/// # Examples
	///
	/// ```ignore
	/// let proxy = AssociationCollection::new(
	///     |user: &User| &user.orders,
	///     |order: &Order| &order.product_name
	/// );
	/// ```
	pub fn new<F1, F2>(collection_getter: F1, attribute_getter: F2) -> Self
	where
		F1: Fn(&S) -> &Vec<C> + 'static,
		F2: Fn(&C) -> &T + 'static,
	{
		Self {
			collection_getter: Box::new(collection_getter),
			attribute_getter: Box::new(attribute_getter),
		}
	}

	/// Get all target attributes from the collection
	///
	/// # Arguments
	///
	/// * `source` - The source object containing the collection
	///
	/// # Returns
	///
	/// A vector of references to the target attributes
	pub fn get_all<'a>(&self, source: &'a S) -> Vec<&'a T>
	where
		C: 'a,
	{
		let collection = (self.collection_getter)(source);
		collection
			.iter()
			.map(|item| (self.attribute_getter)(item))
			.collect()
	}

	/// Count the number of items in the collection
	///
	/// # Arguments
	///
	/// * `source` - The source object containing the collection
	///
	/// # Returns
	///
	/// The number of items in the collection
	pub fn count(&self, source: &S) -> usize {
		let collection = (self.collection_getter)(source);
		collection.len()
	}

	/// Check if the collection is empty
	///
	/// # Arguments
	///
	/// * `source` - The source object containing the collection
	///
	/// # Returns
	///
	/// `true` if the collection is empty, `false` otherwise
	pub fn is_empty(&self, source: &S) -> bool {
		let collection = (self.collection_getter)(source);
		collection.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Allow dead_code: test model struct used for trait implementation verification
	#[allow(dead_code)]
	#[derive(Clone)]
	struct Order {
		id: i64,
		product_name: String,
	}

	// Allow dead_code: test model struct used for trait implementation verification
	#[allow(dead_code)]
	#[derive(Clone)]
	struct User {
		id: i64,
		orders: Vec<Order>,
	}

	#[test]
	fn test_association_collection_basic() {
		let user = User {
			id: 1,
			orders: vec![
				Order {
					id: 1,
					product_name: "Book".to_string(),
				},
				Order {
					id: 2,
					product_name: "Pen".to_string(),
				},
			],
		};

		let proxy = AssociationCollection::new(|u: &User| &u.orders, |o: &Order| &o.product_name);

		let products = proxy.get_all(&user);
		assert_eq!(products.len(), 2);
		assert_eq!(products[0], "Book");
		assert_eq!(products[1], "Pen");
	}

	#[test]
	fn test_association_collection_count() {
		let user = User {
			id: 1,
			orders: vec![
				Order {
					id: 1,
					product_name: "Item1".to_string(),
				},
				Order {
					id: 2,
					product_name: "Item2".to_string(),
				},
			],
		};

		let proxy = AssociationCollection::new(|u: &User| &u.orders, |o: &Order| &o.product_name);

		assert_eq!(proxy.count(&user), 2);
		assert!(!proxy.is_empty(&user));
	}

	#[test]
	fn test_association_collection_empty() {
		let user = User {
			id: 1,
			orders: vec![],
		};

		let proxy = AssociationCollection::new(|u: &User| &u.orders, |o: &Order| &o.product_name);

		assert_eq!(proxy.count(&user), 0);
		assert!(proxy.is_empty(&user));
	}
}
