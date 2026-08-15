//! Query and request context types for admin changelists.

use hyper::{HeaderMap, Method, Uri};
use reinhardt_db::orm::{Filter, FilterCondition};
use reinhardt_http::Request;
use std::net::SocketAddr;
use std::sync::Arc;

/// Append-only query customization for an admin changelist.
#[derive(Clone, Debug)]
pub struct AdminQuery {
	table_name: Arc<str>,
	conditions: Vec<FilterCondition>,
}

impl AdminQuery {
	pub(crate) fn new(table_name: impl Into<Arc<str>>) -> Self {
		Self {
			table_name: table_name.into(),
			conditions: Vec::new(),
		}
	}

	pub(crate) fn table_name(&self) -> &str {
		&self.table_name
	}

	pub(crate) fn conditions(&self) -> &[FilterCondition] {
		&self.conditions
	}

	/// Append a filter condition to this query.
	pub fn filter(mut self, filter: Filter) -> Self {
		self.conditions.push(filter.into());
		self
	}

	/// Append a filter condition to this query.
	pub fn filter_condition(mut self, condition: FilterCondition) -> Self {
		self.conditions.push(condition);
		self
	}
}

/// Read-only request data supplied to admin query customizations.
#[derive(Clone)]
pub struct AdminRequestContext {
	request: Arc<Request>,
}

impl AdminRequestContext {
	pub(crate) fn new(request: Arc<Request>) -> Self {
		Self { request }
	}

	/// Return the request method.
	pub fn method(&self) -> &Method {
		&self.request.method
	}

	/// Return the request URI.
	pub fn uri(&self) -> &Uri {
		&self.request.uri
	}

	/// Return the request headers.
	pub fn headers(&self) -> &HeaderMap {
		&self.request.headers
	}

	/// Return whether the request is secure.
	pub fn is_secure(&self) -> bool {
		self.request.is_secure()
	}

	/// Return the remote client address, when available.
	pub fn remote_addr(&self) -> Option<SocketAddr> {
		self.request.remote_addr
	}
}
