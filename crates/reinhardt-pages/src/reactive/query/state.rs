/// Current phase of a query.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryPhase<T, E> {
	/// The query has no successful value or error yet.
	Pending,
	/// The query has loaded successfully.
	Success(T),
	/// The latest fetch failed.
	Error(E),
}

impl<T, E> QueryPhase<T, E> {
	/// Returns `true` if the query is pending.
	pub fn is_pending(&self) -> bool {
		matches!(self, Self::Pending)
	}

	/// Returns `true` if the query is successful.
	pub fn is_success(&self) -> bool {
		matches!(self, Self::Success(_))
	}

	/// Returns `true` if the query is in an error state.
	pub fn is_error(&self) -> bool {
		matches!(self, Self::Error(_))
	}

	/// Returns the success value if available.
	pub fn result(&self) -> Option<&T> {
		match self {
			Self::Success(value) => Some(value),
			_ => None,
		}
	}

	/// Returns the error value if available.
	pub fn error(&self) -> Option<&E> {
		match self {
			Self::Error(error) => Some(error),
			_ => None,
		}
	}
}
