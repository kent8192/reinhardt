//! Owned parameter formatting for named client routes.

/// Builds owned named-route parameters from heterogeneous displayable values.
///
/// Each key must be a string literal. Every value expression is evaluated once
/// and formatted into an owned [`String`].
#[macro_export]
macro_rules! route_params {
	() => {
		::std::vec::Vec::<(&'static str, ::std::string::String)>::new()
	};
	($($key:literal => $value:expr),+ $(,)?) => {
		::std::vec![
			$(($key, ::std::format!("{}", $value))),+
		]
	};
}

pub use crate::route_params;
