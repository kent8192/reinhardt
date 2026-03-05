//! Hybrid property definitions

use std::marker::PhantomData;

/// Type alias for instance getter function
type InstanceGetterFn<T, R> = Box<dyn Fn(&T) -> R + Send + Sync>;

/// Type alias for expression getter function
type ExpressionGetterFn = Box<dyn Fn() -> String + Send + Sync>;

/// A hybrid property that works at both instance and class level
pub struct HybridProperty<T, R> {
	instance_getter: InstanceGetterFn<T, R>,
	expression_getter: Option<ExpressionGetterFn>,
	_phantom: PhantomData<T>,
}

impl<T, R> HybridProperty<T, R> {
	/// Creates a new hybrid property with an instance-level getter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::hybrid::property::HybridProperty;
	///
	/// struct User {
	///     name: String,
	/// }
	///
	/// let prop = HybridProperty::new(|user: &User| user.name.clone());
	/// let user = User { name: "Alice".to_string() };
	/// assert_eq!(prop.get(&user), "Alice");
	/// ```
	pub fn new<F>(instance_getter: F) -> Self
	where
		F: Fn(&T) -> R + Send + Sync + 'static,
	{
		Self {
			instance_getter: Box::new(instance_getter),
			expression_getter: None,
			_phantom: PhantomData,
		}
	}
	/// Adds a SQL expression getter to the hybrid property
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::hybrid::property::HybridProperty;
	///
	/// struct User {
	///     first_name: String,
	///     last_name: String,
	/// }
	///
	/// let prop = HybridProperty::new(|user: &User| {
	///     format!("{} {}", user.first_name, user.last_name)
	/// })
	/// .with_expression(|| "CONCAT(first_name, ' ', last_name)".to_string());
	///
	/// assert_eq!(prop.expression(), Some("CONCAT(first_name, ' ', last_name)".to_string()));
	/// ```
	pub fn with_expression<F>(mut self, expression_getter: F) -> Self
	where
		F: Fn() -> String + Send + Sync + 'static,
	{
		self.expression_getter = Some(Box::new(expression_getter));
		self
	}
	/// Get the value for an instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::hybrid::property::HybridProperty;
	///
	/// struct Product {
	///     price: f64,
	///     tax_rate: f64,
	/// }
	///
	/// let total_price = HybridProperty::new(|product: &Product| {
	///     product.price * (1.0 + product.tax_rate)
	/// });
	///
	/// let product = Product { price: 100.0, tax_rate: 0.08 };
	/// assert_eq!(total_price.get(&product), 108.0);
	/// ```
	pub fn get(&self, instance: &T) -> R {
		(self.instance_getter)(instance)
	}
	/// Get the SQL expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::hybrid::property::HybridProperty;
	///
	/// struct User {
	///     email: String,
	/// }
	///
	/// let lower_email = HybridProperty::new(|user: &User| user.email.to_lowercase())
	///     .with_expression(|| "LOWER(email)".to_string());
	///
	/// assert_eq!(lower_email.expression(), Some("LOWER(email)".to_string()));
	///
	/// // Without expression
	/// let simple_prop = HybridProperty::new(|user: &User| user.email.clone());
	/// assert_eq!(simple_prop.expression(), None);
	/// ```
	pub fn expression(&self) -> Option<String> {
		self.expression_getter.as_ref().map(|f| f())
	}
}

/// Type alias for instance method function
type InstanceMethodFn<T, A, R> = Box<dyn Fn(&T, A) -> R + Send + Sync>;

/// Type alias for expression method function
type ExpressionMethodFn<A> = Box<dyn Fn(A) -> String + Send + Sync>;

/// A hybrid method that works at both instance and class level
pub struct HybridMethod<T, A, R> {
	instance_method: InstanceMethodFn<T, A, R>,
	expression_method: Option<ExpressionMethodFn<A>>,
	_phantom: PhantomData<(T, A)>,
}

impl<T, A, R> HybridMethod<T, A, R> {
	/// Creates a new hybrid method with an instance-level method
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::hybrid::property::HybridMethod;
	///
	/// struct User {
	///     name: String,
	/// }
	///
	/// let greet = HybridMethod::new(|user: &User, greeting: &str| {
	///     format!("{}, {}!", greeting, user.name)
	/// });
	///
	/// let user = User { name: "Bob".to_string() };
	/// assert_eq!(greet.call(&user, "Hello"), "Hello, Bob!");
	/// ```
	pub fn new<F>(instance_method: F) -> Self
	where
		F: Fn(&T, A) -> R + Send + Sync + 'static,
	{
		Self {
			instance_method: Box::new(instance_method),
			expression_method: None,
			_phantom: PhantomData,
		}
	}
	/// Adds a SQL expression method to the hybrid method
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::hybrid::property::HybridMethod;
	///
	/// struct User {
	///     age: i32,
	/// }
	///
	/// let is_older_than = HybridMethod::new(|user: &User, min_age: i32| {
	///     user.age > min_age
	/// })
	/// .with_expression(|min_age: i32| {
	///     format!("age > {}", min_age)
	/// });
	///
	/// assert_eq!(is_older_than.expression(18), Some("age > 18".to_string()));
	/// ```
	pub fn with_expression<F>(mut self, expression_method: F) -> Self
	where
		F: Fn(A) -> String + Send + Sync + 'static,
	{
		self.expression_method = Some(Box::new(expression_method));
		self
	}
	/// Call the method for an instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::hybrid::property::HybridMethod;
	///
	/// struct Calculator {
	///     base: i32,
	/// }
	///
	/// let add = HybridMethod::new(|calc: &Calculator, value: i32| {
	///     calc.base + value
	/// });
	///
	/// let calc = Calculator { base: 10 };
	/// assert_eq!(add.call(&calc, 5), 15);
	/// ```
	pub fn call(&self, instance: &T, arg: A) -> R {
		(self.instance_method)(instance, arg)
	}
	/// Get the SQL expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::hybrid::property::HybridMethod;
	///
	/// struct User {
	///     score: i32,
	/// }
	///
	/// let passes_threshold = HybridMethod::new(|user: &User, threshold: i32| {
	///     user.score >= threshold
	/// })
	/// .with_expression(|threshold: i32| {
	///     format!("score >= {}", threshold)
	/// });
	///
	/// assert_eq!(passes_threshold.expression(50), Some("score >= 50".to_string()));
	///
	/// // Without expression
	/// let simple_method = HybridMethod::new(|user: &User, threshold: i32| {
	///     user.score >= threshold
	/// });
	/// assert_eq!(simple_method.expression(50), None);
	/// ```
	pub fn expression(&self, arg: A) -> Option<String> {
		self.expression_method.as_ref().map(|f| f(arg))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	struct User {
		first_name: String,
		last_name: String,
	}

	#[test]
	fn test_hybrid_property_unit() {
		let prop =
			HybridProperty::new(|user: &User| format!("{} {}", user.first_name, user.last_name))
				.with_expression(|| "CONCAT(first_name, ' ', last_name)".to_string());

		let user = User {
			first_name: "John".to_string(),
			last_name: "Doe".to_string(),
		};

		assert_eq!(prop.get(&user), "John Doe");
		assert!(prop.expression().is_some());
	}
}
