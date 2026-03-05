//! Association proxy for accessing single object attributes through associations
//!
//! Provides SQLAlchemy-style association proxies for simplifying access
//! to attributes of related objects through foreign key relationships.

/// Association proxy for accessing single object attributes
///
/// This allows accessing attributes of related objects through a proxy,
/// similar to SQLAlchemy's association_proxy for foreign keys and one-to-one relationships.
///
/// # Type Parameters
///
/// * `S` - Source type (the type containing the association)
/// * `A` - Associated type (the related object type)
/// * `T` - Target attribute type
pub struct AssociationProxy<S, A, T> {
	association_getter: Box<dyn Fn(&S) -> &A>,
	attribute_getter: Box<dyn Fn(&A) -> &T>,
}

impl<S, A, T> AssociationProxy<S, A, T> {
	/// Create a new association proxy
	///
	/// # Arguments
	///
	/// * `association_getter` - Function to get the associated object from the source
	/// * `attribute_getter` - Function to get the target attribute from the associated object
	///
	/// # Examples
	///
	/// ```ignore
	/// let proxy = AssociationProxy::new(
	///     |user: &User| &user.address,
	///     |address: &Address| &address.city
	/// );
	/// ```
	pub fn new<F1, F2>(association_getter: F1, attribute_getter: F2) -> Self
	where
		F1: Fn(&S) -> &A + 'static,
		F2: Fn(&A) -> &T + 'static,
	{
		Self {
			association_getter: Box::new(association_getter),
			attribute_getter: Box::new(attribute_getter),
		}
	}

	/// Get the target attribute through the association
	///
	/// # Arguments
	///
	/// * `source` - The source object containing the association
	///
	/// # Returns
	///
	/// A reference to the target attribute
	pub fn get<'a>(&self, source: &'a S) -> &'a T
	where
		A: 'a,
	{
		let associated = (self.association_getter)(source);
		(self.attribute_getter)(associated)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Allow dead_code: test model struct used for trait implementation verification
	#[allow(dead_code)]
	#[derive(Clone)]
	struct Address {
		city: String,
		country: String,
	}

	// Allow dead_code: test model struct used for trait implementation verification
	#[allow(dead_code)]
	#[derive(Clone)]
	struct User {
		id: i64,
		address: Address,
	}

	#[test]
	fn test_association_proxy_basic() {
		let address = Address {
			city: "Tokyo".to_string(),
			country: "Japan".to_string(),
		};

		let user = User { id: 1, address };

		let city_proxy = AssociationProxy::new(|u: &User| &u.address, |a: &Address| &a.city);

		assert_eq!(city_proxy.get(&user), "Tokyo");
	}

	#[test]
	fn test_association_proxy_multiple_attributes() {
		let address = Address {
			city: "Paris".to_string(),
			country: "France".to_string(),
		};

		let user = User { id: 1, address };

		let city_proxy = AssociationProxy::new(|u: &User| &u.address, |a: &Address| &a.city);

		let country_proxy = AssociationProxy::new(|u: &User| &u.address, |a: &Address| &a.country);

		assert_eq!(city_proxy.get(&user), "Paris");
		assert_eq!(country_proxy.get(&user), "France");
	}

	#[test]
	fn test_association_proxy_full_object() {
		let address = Address {
			city: "Berlin".to_string(),
			country: "Germany".to_string(),
		};

		let user = User { id: 1, address };

		let address_proxy = AssociationProxy::new(|u: &User| &u.address, |a: &Address| a);

		let addr = address_proxy.get(&user);
		assert_eq!(addr.city, "Berlin");
		assert_eq!(addr.country, "Germany");
	}
}
