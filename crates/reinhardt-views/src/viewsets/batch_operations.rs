//! Batch operations support for ViewSets
//!
//! Provides functionality for performing multiple operations in a single request:
//! - Batch create (create multiple resources at once)
//! - Batch update (update multiple resources at once)
//! - Batch delete (delete multiple resources at once)
//! - Batch partial update (partial update of multiple resources)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Batch operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum BatchOperation<T> {
	/// Create operation
	#[serde(rename = "create")]
	Create {
		/// Data for the new resource.
		data: T,
	},
	/// Update operation (full update)
	#[serde(rename = "update")]
	Update {
		/// Identifier of the resource to update.
		id: String,
		/// Complete replacement data.
		data: T,
	},
	/// Partial update operation
	#[serde(rename = "partial_update")]
	PartialUpdate {
		/// Identifier of the resource to partially update.
		id: String,
		/// Partial data to merge into the resource.
		data: T,
	},
	/// Delete operation
	#[serde(rename = "delete")]
	Delete {
		/// Identifier of the resource to delete.
		id: String,
	},
}

/// Batch operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOperationResult<T> {
	/// Operation index in the request
	pub index: usize,
	/// Whether the operation succeeded
	pub success: bool,
	/// Result data (for create/update operations)
	pub data: Option<T>,
	/// Error message (if failed)
	pub error: Option<String>,
}

impl<T> BatchOperationResult<T> {
	/// Create a success result
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::BatchOperationResult;
	///
	/// let result = BatchOperationResult::success(0, Some("created".to_string()));
	/// assert!(result.success);
	/// assert_eq!(result.index, 0);
	/// ```
	pub fn success(index: usize, data: Option<T>) -> Self {
		Self {
			index,
			success: true,
			data,
			error: None,
		}
	}

	/// Create a failure result
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::BatchOperationResult;
	///
	/// let result: BatchOperationResult<String> = BatchOperationResult::failure(0, "Not found");
	/// assert!(!result.success);
	/// assert_eq!(result.error, Some("Not found".to_string()));
	/// ```
	pub fn failure(index: usize, error: impl Into<String>) -> Self {
		Self {
			index,
			success: false,
			data: None,
			error: Some(error.into()),
		}
	}
}

/// Batch request wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequest<T> {
	/// List of operations to perform
	pub operations: Vec<BatchOperation<T>>,
	/// Whether to stop on first error
	#[serde(default)]
	pub atomic: bool,
}

impl<T> BatchRequest<T> {
	/// Create a new batch request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::{BatchRequest, BatchOperation};
	///
	/// let request: BatchRequest<String> = BatchRequest::new(vec![
	///     BatchOperation::Create { data: "item1".to_string() },
	///     BatchOperation::Create { data: "item2".to_string() },
	/// ]);
	/// assert_eq!(request.operations.len(), 2);
	/// ```
	pub fn new(operations: Vec<BatchOperation<T>>) -> Self {
		Self {
			operations,
			atomic: false,
		}
	}

	/// Set atomic mode
	pub fn atomic(mut self) -> Self {
		self.atomic = true;
		self
	}

	/// Get the number of operations
	pub fn len(&self) -> usize {
		self.operations.len()
	}

	/// Check if batch is empty
	pub fn is_empty(&self) -> bool {
		self.operations.is_empty()
	}
}

/// Batch response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponse<T> {
	/// Results of operations
	pub results: Vec<BatchOperationResult<T>>,
	/// Total number of operations
	pub total: usize,
	/// Number of successful operations
	pub succeeded: usize,
	/// Number of failed operations
	pub failed: usize,
}

impl<T> BatchResponse<T> {
	/// Create a new batch response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::{BatchResponse, BatchOperationResult};
	///
	/// let results = vec![
	///     BatchOperationResult::success(0, Some("created".to_string())),
	///     BatchOperationResult::failure(1, "Error"),
	/// ];
	/// let response = BatchResponse::new(results);
	/// assert_eq!(response.total, 2);
	/// assert_eq!(response.succeeded, 1);
	/// assert_eq!(response.failed, 1);
	/// ```
	pub fn new(results: Vec<BatchOperationResult<T>>) -> Self {
		let total = results.len();
		let succeeded = results.iter().filter(|r| r.success).count();
		let failed = total - succeeded;

		Self {
			results,
			total,
			succeeded,
			failed,
		}
	}

	/// Check if all operations succeeded
	pub fn all_succeeded(&self) -> bool {
		self.failed == 0
	}

	/// Check if any operation failed
	pub fn any_failed(&self) -> bool {
		self.failed > 0
	}
}

/// Batch operation processor
pub struct BatchProcessor;

impl BatchProcessor {
	/// Process a batch request
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::{BatchProcessor, BatchRequest, BatchOperation};
	///
	/// let request: BatchRequest<String> = BatchRequest::new(vec![
	///     BatchOperation::Create { data: "item1".to_string() },
	/// ]);
	///
	/// let response = BatchProcessor::process(request, |op, index| {
	///     match op {
	///         BatchOperation::Create { data } => {
	///             Ok(format!("Created: {}", data))
	///         }
	///         _ => Err("Unsupported operation".to_string()),
	///     }
	/// });
	///
	/// assert!(response.all_succeeded());
	/// ```
	pub fn process<T, F>(request: BatchRequest<T>, mut handler: F) -> BatchResponse<T>
	where
		F: FnMut(&BatchOperation<T>, usize) -> std::result::Result<T, String>,
	{
		let mut results = Vec::new();

		for (index, operation) in request.operations.iter().enumerate() {
			match handler(operation, index) {
				Ok(data) => {
					results.push(BatchOperationResult::success(index, Some(data)));
				}
				Err(error) => {
					results.push(BatchOperationResult::failure(index, error));

					// Stop on first error in atomic mode
					if request.atomic {
						break;
					}
				}
			}
		}

		BatchResponse::new(results)
	}

	/// Validate batch request size
	pub fn validate_size<T>(
		request: &BatchRequest<T>,
		max_size: usize,
	) -> std::result::Result<(), String> {
		if request.operations.len() > max_size {
			return Err(format!(
				"Batch size {} exceeds maximum {}",
				request.operations.len(),
				max_size
			));
		}
		Ok(())
	}
}

/// Batch operation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStatistics {
	/// Number of operations by type
	pub by_type: HashMap<String, usize>,
	/// Total processing time (ms)
	pub processing_time_ms: u64,
}

impl BatchStatistics {
	/// Create a new batch statistics
	pub fn new() -> Self {
		Self {
			by_type: HashMap::new(),
			processing_time_ms: 0,
		}
	}

	/// Increment count for an operation type
	pub fn increment(&mut self, operation_type: impl Into<String>) {
		*self.by_type.entry(operation_type.into()).or_insert(0) += 1;
	}

	/// Set processing time
	pub fn set_processing_time(&mut self, ms: u64) {
		self.processing_time_ms = ms;
	}
}

impl Default for BatchStatistics {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_batch_operation_result_success() {
		let result = BatchOperationResult::success(0, Some("data".to_string()));
		assert!(result.success);
		assert_eq!(result.index, 0);
		assert_eq!(result.data, Some("data".to_string()));
		assert_eq!(result.error, None);
	}

	#[test]
	fn test_batch_operation_result_failure() {
		let result: BatchOperationResult<String> =
			BatchOperationResult::failure(1, "Error message");
		assert!(!result.success);
		assert_eq!(result.index, 1);
		assert_eq!(result.data, None);
		assert_eq!(result.error, Some("Error message".to_string()));
	}

	#[test]
	fn test_batch_request_new() {
		let operations = vec![
			BatchOperation::Create {
				data: "item1".to_string(),
			},
			BatchOperation::Create {
				data: "item2".to_string(),
			},
		];

		let request = BatchRequest::new(operations);
		assert_eq!(request.len(), 2);
		assert!(!request.atomic);
	}

	#[test]
	fn test_batch_request_atomic() {
		let request: BatchRequest<String> = BatchRequest::new(vec![]).atomic();
		assert!(request.atomic);
	}

	#[test]
	fn test_batch_response_statistics() {
		let results = vec![
			BatchOperationResult::success(0, Some("data1".to_string())),
			BatchOperationResult::success(1, Some("data2".to_string())),
			BatchOperationResult::failure(2, "Error"),
		];

		let response = BatchResponse::new(results);
		assert_eq!(response.total, 3);
		assert_eq!(response.succeeded, 2);
		assert_eq!(response.failed, 1);
		assert!(!response.all_succeeded());
		assert!(response.any_failed());
	}

	#[test]
	fn test_batch_processor_all_success() {
		let request = BatchRequest::new(vec![
			BatchOperation::Create {
				data: "item1".to_string(),
			},
			BatchOperation::Create {
				data: "item2".to_string(),
			},
		]);

		let response = BatchProcessor::process(request, |op, _index| match op {
			BatchOperation::Create { data } => Ok(format!("Created: {}", data)),
			_ => Err("Unsupported".to_string()),
		});

		assert_eq!(response.total, 2);
		assert_eq!(response.succeeded, 2);
		assert!(response.all_succeeded());
	}

	#[test]
	fn test_batch_processor_with_errors() {
		let request = BatchRequest::new(vec![
			BatchOperation::Create {
				data: "item1".to_string(),
			},
			BatchOperation::Create {
				data: "fail".to_string(),
			},
			BatchOperation::Create {
				data: "item3".to_string(),
			},
		]);

		let response = BatchProcessor::process(request, |op, _index| match op {
			BatchOperation::Create { data } => {
				if data == "fail" {
					Err("Failed".to_string())
				} else {
					Ok(format!("Created: {}", data))
				}
			}
			_ => Err("Unsupported".to_string()),
		});

		assert_eq!(response.total, 3);
		assert_eq!(response.succeeded, 2);
		assert_eq!(response.failed, 1);
	}

	#[test]
	fn test_batch_processor_atomic_mode() {
		let request = BatchRequest::new(vec![
			BatchOperation::Create {
				data: "item1".to_string(),
			},
			BatchOperation::Create {
				data: "fail".to_string(),
			},
			BatchOperation::Create {
				data: "item3".to_string(),
			},
		])
		.atomic();

		let response = BatchProcessor::process(request, |op, _index| match op {
			BatchOperation::Create { data } => {
				if data == "fail" {
					Err("Failed".to_string())
				} else {
					Ok(format!("Created: {}", data))
				}
			}
			_ => Err("Unsupported".to_string()),
		});

		// Only 2 operations should be processed (1 success + 1 failure)
		assert_eq!(response.results.len(), 2);
		assert_eq!(response.succeeded, 1);
		assert_eq!(response.failed, 1);
	}

	#[test]
	fn test_batch_processor_validate_size() {
		let request: BatchRequest<String> = BatchRequest::new(vec![
			BatchOperation::Create {
				data: "item1".to_string(),
			},
			BatchOperation::Create {
				data: "item2".to_string(),
			},
		]);

		assert!(BatchProcessor::validate_size(&request, 5).is_ok());
		assert!(BatchProcessor::validate_size(&request, 1).is_err());
	}

	#[test]
	fn test_batch_statistics() {
		let mut stats = BatchStatistics::new();
		stats.increment("create");
		stats.increment("create");
		stats.increment("update");
		stats.set_processing_time(1000);

		assert_eq!(stats.by_type.get("create"), Some(&2));
		assert_eq!(stats.by_type.get("update"), Some(&1));
		assert_eq!(stats.processing_time_ms, 1000);
	}

	#[test]
	fn test_batch_operation_serialization() {
		let op = BatchOperation::Create {
			data: "test".to_string(),
		};
		let json = serde_json::to_string(&op).unwrap();
		assert!(json.contains("\"operation\":\"create\""));
		assert!(json.contains("\"data\":\"test\""));
	}

	#[test]
	fn test_batch_request_is_empty() {
		let empty_request: BatchRequest<String> = BatchRequest::new(vec![]);
		assert!(empty_request.is_empty());

		let non_empty = BatchRequest::new(vec![BatchOperation::Create {
			data: "item".to_string(),
		}]);
		assert!(!non_empty.is_empty());
	}
}
