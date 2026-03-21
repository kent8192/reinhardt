//! Collection operations for filtering and transforming

use super::CollectionProxy;
use crate::proxy::ProxyResult;
use crate::proxy::ScalarValue;

/// Collection operations for filtering and transforming
#[derive(Debug, Clone)]
pub struct CollectionOperations {
	proxy: CollectionProxy,
}

impl CollectionOperations {
	/// Create new collection operations wrapper
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{CollectionProxy, collection::CollectionOperations};
	///
	/// let proxy = CollectionProxy::new("posts", "title");
	/// let ops = CollectionOperations::new(proxy);
	/// // Operations wrapper is ready to use
	/// ```
	pub fn new(proxy: CollectionProxy) -> Self {
		Self { proxy }
	}
	/// Filter collection by predicate
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_urls::proxy::{CollectionProxy, ScalarValue, collection::CollectionOperations};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("posts", "title");
	/// let ops = CollectionOperations::new(proxy);
	// let user = ...;
	// let filtered = ops.filter(&user, |v| v.is_null()).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn filter<T, F>(&self, source: &T, predicate: F) -> ProxyResult<Vec<ScalarValue>>
	where
		T: crate::proxy::reflection::Reflectable,
		F: Fn(&ScalarValue) -> bool,
	{
		let values = self.proxy.get_values(source).await?;
		Ok(values.into_iter().filter(|v| predicate(v)).collect())
	}
	/// Map collection values
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_urls::proxy::{CollectionProxy, ScalarValue, collection::CollectionOperations};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("posts", "title");
	/// let ops = CollectionOperations::new(proxy);
	// let user = ...;
	// let lengths = ops.map(&user, |v| match v {
	//     ScalarValue::String(s) => s.len(),
	//     _ => 0,
	// }).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn map<T, F, U>(&self, source: &T, mapper: F) -> ProxyResult<Vec<U>>
	where
		T: crate::proxy::reflection::Reflectable,
		F: Fn(&ScalarValue) -> U,
	{
		let values = self.proxy.get_values(source).await?;
		Ok(values.iter().map(mapper).collect())
	}
	/// Sort collection values
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_urls::proxy::{CollectionProxy, collection::CollectionOperations};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("posts", "created_at");
	/// let ops = CollectionOperations::new(proxy);
	// let user = ...;
	// let sorted = ops.sort(&user).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn sort<T>(&self, source: &T) -> ProxyResult<Vec<ScalarValue>>
	where
		T: crate::proxy::reflection::Reflectable,
	{
		let mut values = self.proxy.get_values(source).await?;
		values.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
		Ok(values)
	}
	/// Get distinct values
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_urls::proxy::{CollectionProxy, collection::CollectionOperations};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("posts", "category");
	/// let ops = CollectionOperations::new(proxy);
	// let user = ...;
	// let distinct = ops.distinct(&user).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn distinct<T>(&self, source: &T) -> ProxyResult<Vec<ScalarValue>>
	where
		T: crate::proxy::reflection::Reflectable,
	{
		let mut values = self.proxy.get_values(source).await?;
		values.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
		values.dedup_by(|a, b| format!("{:?}", a) == format!("{:?}", b));
		Ok(values)
	}
}
