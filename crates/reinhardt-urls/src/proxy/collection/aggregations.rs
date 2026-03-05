//! Aggregation operations on collections

use super::CollectionProxy;
use crate::proxy::ProxyResult;
use crate::proxy::ScalarValue;

/// Aggregation operations on collections
#[derive(Debug, Clone)]
pub struct CollectionAggregations {
	proxy: CollectionProxy,
}

impl CollectionAggregations {
	/// Create new collection aggregations wrapper
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{CollectionProxy, collection::CollectionAggregations};
	///
	/// let proxy = CollectionProxy::new("sales", "amount");
	/// let agg = CollectionAggregations::new(proxy);
	/// // Aggregations wrapper is ready to use
	/// ```
	pub fn new(proxy: CollectionProxy) -> Self {
		Self { proxy }
	}
	/// Sum numeric values in collection
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_urls::proxy::{CollectionProxy, collection::CollectionAggregations};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("sales", "amount");
	/// let agg = CollectionAggregations::new(proxy);
	// let product = ...;
	// let total = agg.sum(&product).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn sum<T>(&self, source: &T) -> ProxyResult<f64>
	where
		T: crate::proxy::reflection::Reflectable,
	{
		let values = self.proxy.get_values(source).await?;
		let mut sum = 0.0;
		for value in values {
			match value {
				ScalarValue::Integer(i) => sum += i as f64,
				ScalarValue::Float(f) => sum += f,
				_ => {}
			}
		}
		Ok(sum)
	}
	/// Average of numeric values in collection
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_urls::proxy::{CollectionProxy, collection::CollectionAggregations};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("reviews", "rating");
	/// let agg = CollectionAggregations::new(proxy);
	// let product = ...;
	// let average = agg.avg(&product).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn avg<T>(&self, source: &T) -> ProxyResult<f64>
	where
		T: crate::proxy::reflection::Reflectable,
	{
		let values = self.proxy.get_values(source).await?;
		let mut sum = 0.0;
		let mut count = 0;
		for value in &values {
			match value {
				ScalarValue::Integer(i) => {
					sum += *i as f64;
					count += 1;
				}
				ScalarValue::Float(f) => {
					sum += f;
					count += 1;
				}
				_ => {}
			}
		}
		if count == 0 {
			Ok(0.0)
		} else {
			Ok(sum / count as f64)
		}
	}
	/// Minimum value in collection
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_urls::proxy::{CollectionProxy, collection::CollectionAggregations};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("prices", "amount");
	/// let agg = CollectionAggregations::new(proxy);
	// let product = ...;
	// let min_price = agg.min(&product).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn min<T>(&self, source: &T) -> ProxyResult<Option<ScalarValue>>
	where
		T: crate::proxy::reflection::Reflectable,
	{
		let values = self.proxy.get_values(source).await?;
		Ok(values.into_iter().min_by(|a, b| {
			use std::cmp::Ordering;
			match (a, b) {
				(ScalarValue::Integer(x), ScalarValue::Integer(y)) => x.cmp(y),
				(ScalarValue::Float(x), ScalarValue::Float(y)) => {
					x.partial_cmp(y).unwrap_or(Ordering::Equal)
				}
				(ScalarValue::Integer(x), ScalarValue::Float(y)) => {
					(*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal)
				}
				(ScalarValue::Float(x), ScalarValue::Integer(y)) => {
					x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal)
				}
				_ => format!("{:?}", a).cmp(&format!("{:?}", b)),
			}
		}))
	}
	/// Maximum value in collection
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_urls::proxy::{CollectionProxy, collection::CollectionAggregations};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let proxy = CollectionProxy::new("bids", "amount");
	/// let agg = CollectionAggregations::new(proxy);
	// let auction = ...;
	// let highest_bid = agg.max(&auction).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn max<T>(&self, source: &T) -> ProxyResult<Option<ScalarValue>>
	where
		T: crate::proxy::reflection::Reflectable,
	{
		let values = self.proxy.get_values(source).await?;
		Ok(values.into_iter().max_by(|a, b| {
			use std::cmp::Ordering;
			match (a, b) {
				(ScalarValue::Integer(x), ScalarValue::Integer(y)) => x.cmp(y),
				(ScalarValue::Float(x), ScalarValue::Float(y)) => {
					x.partial_cmp(y).unwrap_or(Ordering::Equal)
				}
				(ScalarValue::Integer(x), ScalarValue::Float(y)) => {
					(*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal)
				}
				(ScalarValue::Float(x), ScalarValue::Integer(y)) => {
					x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal)
				}
				_ => format!("{:?}", a).cmp(&format!("{:?}", b)),
			}
		}))
	}
}
