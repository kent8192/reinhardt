//! Task Dependency Graph (DAG)
//!
//! Provides a Directed Acyclic Graph implementation for managing complex task dependencies.
//! Unlike `TaskChain` which only supports linear execution, `TaskDAG` enables:
//! - Complex dependency relationships
//! - Detection of tasks ready for parallel execution
//! - Cycle detection
//! - Topological sorting for execution order
//!
//! # Examples
//!
//! ```rust
//! use reinhardt_tasks::{TaskDAG, TaskId};
//!
//! let mut dag = TaskDAG::new();
//!
//! // Add tasks
//! let task_a = TaskId::new();
//! let task_b = TaskId::new();
//! let task_c = TaskId::new();
//!
//! dag.add_task(task_a).unwrap();
//! dag.add_task(task_b).unwrap();
//! dag.add_task(task_c).unwrap();
//!
//! // Define dependencies: B depends on A, C depends on B
//! dag.add_dependency(task_b, task_a).unwrap();
//! dag.add_dependency(task_c, task_b).unwrap();
//!
//! // Get execution order
//! let order = dag.topological_sort().unwrap();
//! assert_eq!(order.len(), 3);
//! ```

use crate::{TaskError, TaskId, TaskResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Task node status within the DAG
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::TaskNodeStatus;
///
/// let status = TaskNodeStatus::Pending;
/// assert_eq!(status, TaskNodeStatus::Pending);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskNodeStatus {
	/// Task is waiting for dependencies
	Pending,
	/// Task's dependencies are satisfied and ready to execute
	Ready,
	/// Task is currently executing
	Running,
	/// Task completed successfully
	Completed,
	/// Task failed during execution
	Failed,
}

/// A node in the task dependency graph
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::{TaskNode, TaskId, TaskNodeStatus};
///
/// let task_id = TaskId::new();
/// let node = TaskNode::new(task_id);
/// assert_eq!(node.id(), task_id);
/// assert_eq!(node.status(), TaskNodeStatus::Pending);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
	/// Task identifier
	id: TaskId,
	/// IDs of tasks this node depends on
	dependencies: Vec<TaskId>,
	/// Current status of this task
	status: TaskNodeStatus,
}

impl TaskNode {
	/// Create a new task node
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskNode, TaskId};
	///
	/// let task_id = TaskId::new();
	/// let node = TaskNode::new(task_id);
	/// ```
	pub fn new(id: TaskId) -> Self {
		Self {
			id,
			dependencies: Vec::new(),
			status: TaskNodeStatus::Pending,
		}
	}

	/// Get the task ID
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskNode, TaskId};
	///
	/// let task_id = TaskId::new();
	/// let node = TaskNode::new(task_id);
	/// assert_eq!(node.id(), task_id);
	/// ```
	pub fn id(&self) -> TaskId {
		self.id
	}

	/// Get the task dependencies
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskNode, TaskId};
	///
	/// let node = TaskNode::new(TaskId::new());
	/// assert_eq!(node.dependencies().len(), 0);
	/// ```
	pub fn dependencies(&self) -> &[TaskId] {
		&self.dependencies
	}

	/// Get the task status
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskNode, TaskId, TaskNodeStatus};
	///
	/// let node = TaskNode::new(TaskId::new());
	/// assert_eq!(node.status(), TaskNodeStatus::Pending);
	/// ```
	pub fn status(&self) -> TaskNodeStatus {
		self.status
	}

	/// Add a dependency
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskNode, TaskId};
	///
	/// let mut node = TaskNode::new(TaskId::new());
	/// let dep_id = TaskId::new();
	/// node.add_dependency(dep_id);
	/// assert_eq!(node.dependencies().len(), 1);
	/// ```
	pub fn add_dependency(&mut self, task_id: TaskId) {
		if !self.dependencies.contains(&task_id) {
			self.dependencies.push(task_id);
		}
	}

	/// Set the task status
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskNode, TaskId, TaskNodeStatus};
	///
	/// let mut node = TaskNode::new(TaskId::new());
	/// node.set_status(TaskNodeStatus::Running);
	/// assert_eq!(node.status(), TaskNodeStatus::Running);
	/// ```
	pub fn set_status(&mut self, status: TaskNodeStatus) {
		self.status = status;
	}
}

/// Directed Acyclic Graph for task dependencies
///
/// Manages complex task dependencies and provides topological sorting for execution order.
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::{TaskDAG, TaskId};
///
/// let mut dag = TaskDAG::new();
/// let task_a = TaskId::new();
/// let task_b = TaskId::new();
///
/// dag.add_task(task_a).unwrap();
/// dag.add_task(task_b).unwrap();
/// dag.add_dependency(task_b, task_a).unwrap();
///
/// let order = dag.topological_sort().unwrap();
/// assert_eq!(order.len(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDAG {
	/// Map of task IDs to task nodes
	nodes: HashMap<TaskId, TaskNode>,
	/// Adjacency list: task -> tasks that depend on it
	dependents: HashMap<TaskId, Vec<TaskId>>,
}

impl TaskDAG {
	/// Create a new empty task DAG
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::TaskDAG;
	///
	/// let dag = TaskDAG::new();
	/// assert_eq!(dag.task_count(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			nodes: HashMap::new(),
			dependents: HashMap::new(),
		}
	}

	/// Add a task to the DAG
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskDAG, TaskId};
	///
	/// let mut dag = TaskDAG::new();
	/// let task_id = TaskId::new();
	/// dag.add_task(task_id).unwrap();
	/// assert_eq!(dag.task_count(), 1);
	/// ```
	///
	/// # Errors
	///
	/// Returns an error if the task already exists in the DAG.
	pub fn add_task(&mut self, task_id: TaskId) -> TaskResult<()> {
		if self.nodes.contains_key(&task_id) {
			return Err(TaskError::ExecutionFailed(format!(
				"Task {} already exists in DAG",
				task_id
			)));
		}

		self.nodes.insert(task_id, TaskNode::new(task_id));
		self.dependents.insert(task_id, Vec::new());
		Ok(())
	}

	/// Add a dependency between tasks
	///
	/// `task_id` depends on `depends_on`, meaning `depends_on` must complete before `task_id`.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskDAG, TaskId};
	///
	/// let mut dag = TaskDAG::new();
	/// let task_a = TaskId::new();
	/// let task_b = TaskId::new();
	///
	/// dag.add_task(task_a).unwrap();
	/// dag.add_task(task_b).unwrap();
	/// dag.add_dependency(task_b, task_a).unwrap();
	/// ```
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - Either task doesn't exist in the DAG
	/// - The dependency would create a cycle
	pub fn add_dependency(&mut self, task_id: TaskId, depends_on: TaskId) -> TaskResult<()> {
		// Validate both tasks exist
		if !self.nodes.contains_key(&task_id) {
			return Err(TaskError::TaskNotFound(task_id.to_string()));
		}
		if !self.nodes.contains_key(&depends_on) {
			return Err(TaskError::TaskNotFound(depends_on.to_string()));
		}

		// Add dependency to the task node
		if let Some(node) = self.nodes.get_mut(&task_id) {
			node.add_dependency(depends_on);
		}

		// Add to dependents adjacency list
		if let Some(deps) = self.dependents.get_mut(&depends_on)
			&& !deps.contains(&task_id)
		{
			deps.push(task_id);
		}

		// Verify no cycles were created
		self.detect_cycle()?;

		Ok(())
	}

	/// Get the number of tasks in the DAG
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskDAG, TaskId};
	///
	/// let mut dag = TaskDAG::new();
	/// dag.add_task(TaskId::new()).unwrap();
	/// dag.add_task(TaskId::new()).unwrap();
	/// assert_eq!(dag.task_count(), 2);
	/// ```
	pub fn task_count(&self) -> usize {
		self.nodes.len()
	}

	/// Get a task node by ID
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskDAG, TaskId};
	///
	/// let mut dag = TaskDAG::new();
	/// let task_id = TaskId::new();
	/// dag.add_task(task_id).unwrap();
	///
	/// let node = dag.get_task(task_id);
	/// assert!(node.is_some());
	/// ```
	pub fn get_task(&self, task_id: TaskId) -> Option<&TaskNode> {
		self.nodes.get(&task_id)
	}

	/// Get tasks that are ready to execute (all dependencies satisfied)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskDAG, TaskId, TaskNodeStatus};
	///
	/// let mut dag = TaskDAG::new();
	/// let task_a = TaskId::new();
	/// let task_b = TaskId::new();
	///
	/// dag.add_task(task_a).unwrap();
	/// dag.add_task(task_b).unwrap();
	/// dag.add_dependency(task_b, task_a).unwrap();
	///
	/// let ready = dag.get_ready_tasks();
	/// assert_eq!(ready.len(), 1); // Only task_a has no dependencies
	/// ```
	pub fn get_ready_tasks(&self) -> Vec<TaskId> {
		self.nodes
			.values()
			.filter(|node| {
				// Task is ready if it's pending and all dependencies are completed
				node.status() == TaskNodeStatus::Pending
					&& node
						.dependencies()
						.iter()
						.all(|dep_id| match self.nodes.get(dep_id) {
							Some(dep_node) => dep_node.status() == TaskNodeStatus::Completed,
							None => false,
						})
			})
			.map(|node| node.id())
			.collect()
	}

	/// Mark a task as completed
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskDAG, TaskId, TaskNodeStatus};
	///
	/// let mut dag = TaskDAG::new();
	/// let task_id = TaskId::new();
	/// dag.add_task(task_id).unwrap();
	///
	/// dag.mark_completed(task_id).unwrap();
	/// assert_eq!(dag.get_task(task_id).unwrap().status(), TaskNodeStatus::Completed);
	/// ```
	///
	/// # Errors
	///
	/// Returns an error if the task doesn't exist in the DAG.
	pub fn mark_completed(&mut self, task_id: TaskId) -> TaskResult<()> {
		let node = self
			.nodes
			.get_mut(&task_id)
			.ok_or_else(|| TaskError::TaskNotFound(task_id.to_string()))?;

		node.set_status(TaskNodeStatus::Completed);
		Ok(())
	}

	/// Mark a task as failed
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskDAG, TaskId, TaskNodeStatus};
	///
	/// let mut dag = TaskDAG::new();
	/// let task_id = TaskId::new();
	/// dag.add_task(task_id).unwrap();
	///
	/// dag.mark_failed(task_id).unwrap();
	/// assert_eq!(dag.get_task(task_id).unwrap().status(), TaskNodeStatus::Failed);
	/// ```
	///
	/// # Errors
	///
	/// Returns an error if the task doesn't exist in the DAG.
	pub fn mark_failed(&mut self, task_id: TaskId) -> TaskResult<()> {
		let node = self
			.nodes
			.get_mut(&task_id)
			.ok_or_else(|| TaskError::TaskNotFound(task_id.to_string()))?;

		node.set_status(TaskNodeStatus::Failed);
		Ok(())
	}

	/// Mark a task as running
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskDAG, TaskId, TaskNodeStatus};
	///
	/// let mut dag = TaskDAG::new();
	/// let task_id = TaskId::new();
	/// dag.add_task(task_id).unwrap();
	///
	/// dag.mark_running(task_id).unwrap();
	/// assert_eq!(dag.get_task(task_id).unwrap().status(), TaskNodeStatus::Running);
	/// ```
	///
	/// # Errors
	///
	/// Returns an error if the task doesn't exist in the DAG.
	pub fn mark_running(&mut self, task_id: TaskId) -> TaskResult<()> {
		let node = self
			.nodes
			.get_mut(&task_id)
			.ok_or_else(|| TaskError::TaskNotFound(task_id.to_string()))?;

		node.set_status(TaskNodeStatus::Running);
		Ok(())
	}

	/// Perform topological sort using Kahn's algorithm
	///
	/// Returns an execution order that respects all dependencies.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskDAG, TaskId};
	///
	/// let mut dag = TaskDAG::new();
	/// let task_a = TaskId::new();
	/// let task_b = TaskId::new();
	/// let task_c = TaskId::new();
	///
	/// dag.add_task(task_a).unwrap();
	/// dag.add_task(task_b).unwrap();
	/// dag.add_task(task_c).unwrap();
	/// dag.add_dependency(task_b, task_a).unwrap();
	/// dag.add_dependency(task_c, task_b).unwrap();
	///
	/// let order = dag.topological_sort().unwrap();
	/// assert_eq!(order.len(), 3);
	/// // task_a must come before task_b, task_b must come before task_c
	/// let a_pos = order.iter().position(|&id| id == task_a).unwrap();
	/// let b_pos = order.iter().position(|&id| id == task_b).unwrap();
	/// let c_pos = order.iter().position(|&id| id == task_c).unwrap();
	/// assert!(a_pos < b_pos);
	/// assert!(b_pos < c_pos);
	/// ```
	///
	/// # Errors
	///
	/// Returns an error if the graph contains a cycle.
	pub fn topological_sort(&self) -> TaskResult<Vec<TaskId>> {
		// Calculate in-degree for each node
		let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
		for (task_id, node) in &self.nodes {
			in_degree.insert(*task_id, node.dependencies().len());
		}

		// Queue of nodes with no dependencies
		let mut queue: VecDeque<TaskId> = in_degree
			.iter()
			.filter(|(_, degree)| **degree == 0)
			.map(|(task_id, _)| *task_id)
			.collect();

		let mut sorted = Vec::new();

		while let Some(task_id) = queue.pop_front() {
			sorted.push(task_id);

			// Reduce in-degree for all dependents
			if let Some(deps) = self.dependents.get(&task_id) {
				for &dependent in deps {
					if let Some(degree) = in_degree.get_mut(&dependent) {
						*degree -= 1;
						if *degree == 0 {
							queue.push_back(dependent);
						}
					}
				}
			}
		}

		// If sorted doesn't include all nodes, there's a cycle
		if sorted.len() != self.nodes.len() {
			return Err(TaskError::ExecutionFailed(
				"Cycle detected in task dependencies".to_string(),
			));
		}

		Ok(sorted)
	}

	/// Detect if there's a cycle in the graph using iterative DFS
	///
	/// Uses an explicit stack instead of recursion to avoid stack overflow
	/// on deeply nested dependency graphs.
	///
	/// # Errors
	///
	/// Returns an error if a cycle is detected.
	fn detect_cycle(&self) -> TaskResult<()> {
		let mut visited = HashSet::new();
		let mut rec_stack = HashSet::new();

		for &start_id in self.nodes.keys() {
			if visited.contains(&start_id) {
				continue;
			}

			// Explicit stack: (task_id, dependency_index, is_entering)
			// is_entering=true means we are visiting this node for the first time
			let mut stack: Vec<(TaskId, usize, bool)> = vec![(start_id, 0, true)];

			while let Some((task_id, dep_idx, is_entering)) = stack.last_mut() {
				if *is_entering {
					visited.insert(*task_id);
					rec_stack.insert(*task_id);
					*is_entering = false;
				}

				let deps = self
					.nodes
					.get(task_id)
					.map(|n| n.dependencies())
					.unwrap_or(&[]);

				if *dep_idx < deps.len() {
					let dep_id = deps[*dep_idx];
					*dep_idx += 1;

					if rec_stack.contains(&dep_id) {
						return Err(TaskError::ExecutionFailed(format!(
							"Cycle detected: {} -> {}",
							task_id, dep_id
						)));
					}

					if !visited.contains(&dep_id) {
						stack.push((dep_id, 0, true));
					}
				} else {
					// All dependencies processed, backtrack
					rec_stack.remove(task_id);
					stack.pop();
				}
			}
		}

		Ok(())
	}
}

impl Default for TaskDAG {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_dag_creation() {
		// Arrange & Act
		let dag = TaskDAG::new();

		// Assert
		assert_eq!(dag.task_count(), 0);
	}

	#[rstest]
	fn test_add_task() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_id = TaskId::new();

		// Act
		dag.add_task(task_id).unwrap();

		// Assert
		assert_eq!(dag.task_count(), 1);
		assert!(dag.get_task(task_id).is_some());
	}

	#[rstest]
	fn test_add_duplicate_task() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_id = TaskId::new();
		dag.add_task(task_id).unwrap();

		// Act
		let result = dag.add_task(task_id);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_add_dependency() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_a = TaskId::new();
		let task_b = TaskId::new();
		dag.add_task(task_a).unwrap();
		dag.add_task(task_b).unwrap();

		// Act
		dag.add_dependency(task_b, task_a).unwrap();

		// Assert
		let node_b = dag.get_task(task_b).unwrap();
		assert_eq!(node_b.dependencies().len(), 1);
		assert_eq!(node_b.dependencies()[0], task_a);
	}

	#[rstest]
	fn test_add_dependency_nonexistent_task() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_a = TaskId::new();
		let task_b = TaskId::new();
		dag.add_task(task_a).unwrap();

		// Act
		let result = dag.add_dependency(task_a, task_b);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_cycle_detection() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_a = TaskId::new();
		let task_b = TaskId::new();
		let task_c = TaskId::new();

		dag.add_task(task_a).unwrap();
		dag.add_task(task_b).unwrap();
		dag.add_task(task_c).unwrap();

		dag.add_dependency(task_b, task_a).unwrap();
		dag.add_dependency(task_c, task_b).unwrap();

		// Act - creating a cycle: a -> b -> c -> a
		let result = dag.add_dependency(task_a, task_c);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_topological_sort_simple() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_a = TaskId::new();
		let task_b = TaskId::new();
		let task_c = TaskId::new();

		dag.add_task(task_a).unwrap();
		dag.add_task(task_b).unwrap();
		dag.add_task(task_c).unwrap();

		// a -> b -> c
		dag.add_dependency(task_b, task_a).unwrap();
		dag.add_dependency(task_c, task_b).unwrap();

		// Act
		let order = dag.topological_sort().unwrap();

		// Assert
		assert_eq!(order.len(), 3);
		let a_pos = order.iter().position(|&id| id == task_a).unwrap();
		let b_pos = order.iter().position(|&id| id == task_b).unwrap();
		let c_pos = order.iter().position(|&id| id == task_c).unwrap();
		assert!(a_pos < b_pos);
		assert!(b_pos < c_pos);
	}

	#[rstest]
	fn test_topological_sort_diamond() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_a = TaskId::new();
		let task_b = TaskId::new();
		let task_c = TaskId::new();
		let task_d = TaskId::new();

		dag.add_task(task_a).unwrap();
		dag.add_task(task_b).unwrap();
		dag.add_task(task_c).unwrap();
		dag.add_task(task_d).unwrap();

		// Diamond: a -> b, a -> c, b -> d, c -> d
		dag.add_dependency(task_b, task_a).unwrap();
		dag.add_dependency(task_c, task_a).unwrap();
		dag.add_dependency(task_d, task_b).unwrap();
		dag.add_dependency(task_d, task_c).unwrap();

		// Act
		let order = dag.topological_sort().unwrap();

		// Assert
		assert_eq!(order.len(), 4);
		let a_pos = order.iter().position(|&id| id == task_a).unwrap();
		let b_pos = order.iter().position(|&id| id == task_b).unwrap();
		let c_pos = order.iter().position(|&id| id == task_c).unwrap();
		let d_pos = order.iter().position(|&id| id == task_d).unwrap();

		// a must come before b and c
		assert!(a_pos < b_pos);
		assert!(a_pos < c_pos);
		// b and c must come before d
		assert!(b_pos < d_pos);
		assert!(c_pos < d_pos);
	}

	#[rstest]
	fn test_get_ready_tasks() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_a = TaskId::new();
		let task_b = TaskId::new();
		let task_c = TaskId::new();

		dag.add_task(task_a).unwrap();
		dag.add_task(task_b).unwrap();
		dag.add_task(task_c).unwrap();

		// a -> b -> c
		dag.add_dependency(task_b, task_a).unwrap();
		dag.add_dependency(task_c, task_b).unwrap();

		// Assert - initially, only task_a should be ready
		let ready = dag.get_ready_tasks();
		assert_eq!(ready.len(), 1);
		assert!(ready.contains(&task_a));

		// Act - after marking a as completed, b should be ready
		dag.mark_completed(task_a).unwrap();
		let ready = dag.get_ready_tasks();

		// Assert
		assert_eq!(ready.len(), 1);
		assert!(ready.contains(&task_b));

		// Act - after marking b as completed, c should be ready
		dag.mark_completed(task_b).unwrap();
		let ready = dag.get_ready_tasks();

		// Assert
		assert_eq!(ready.len(), 1);
		assert!(ready.contains(&task_c));
	}

	#[rstest]
	fn test_mark_status() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_id = TaskId::new();
		dag.add_task(task_id).unwrap();

		// Assert - initial status
		assert_eq!(
			dag.get_task(task_id).unwrap().status(),
			TaskNodeStatus::Pending
		);

		// Act & Assert - running
		dag.mark_running(task_id).unwrap();
		assert_eq!(
			dag.get_task(task_id).unwrap().status(),
			TaskNodeStatus::Running
		);

		// Act & Assert - completed
		dag.mark_completed(task_id).unwrap();
		assert_eq!(
			dag.get_task(task_id).unwrap().status(),
			TaskNodeStatus::Completed
		);
	}

	#[rstest]
	fn test_mark_failed() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_id = TaskId::new();
		dag.add_task(task_id).unwrap();

		// Act
		dag.mark_failed(task_id).unwrap();

		// Assert
		assert_eq!(
			dag.get_task(task_id).unwrap().status(),
			TaskNodeStatus::Failed
		);
	}

	#[rstest]
	fn test_parallel_execution_detection() {
		// Arrange
		let mut dag = TaskDAG::new();
		let task_a = TaskId::new();
		let task_b = TaskId::new();
		let task_c = TaskId::new();
		let task_d = TaskId::new();

		dag.add_task(task_a).unwrap();
		dag.add_task(task_b).unwrap();
		dag.add_task(task_c).unwrap();
		dag.add_task(task_d).unwrap();

		// a -> b, a -> c, (b,c) -> d
		dag.add_dependency(task_b, task_a).unwrap();
		dag.add_dependency(task_c, task_a).unwrap();
		dag.add_dependency(task_d, task_b).unwrap();
		dag.add_dependency(task_d, task_c).unwrap();

		// Act - after completing a, both b and c should be ready
		dag.mark_completed(task_a).unwrap();
		let ready = dag.get_ready_tasks();

		// Assert
		assert_eq!(ready.len(), 2);
		assert!(ready.contains(&task_b));
		assert!(ready.contains(&task_c));
	}

	#[rstest]
	fn test_deep_dependency_chain_does_not_stack_overflow() {
		// Arrange - build a deep linear chain: t0 -> t1 -> t2 -> ... -> t999
		// Iterative DFS handles this without stack overflow, while recursive
		// DFS would fail for sufficiently deep chains.
		let mut dag = TaskDAG::new();
		let depth = 1000;
		let mut task_ids = Vec::with_capacity(depth);

		for _ in 0..depth {
			let id = TaskId::new();
			dag.add_task(id).unwrap();
			task_ids.push(id);
		}

		for i in 1..depth {
			dag.add_dependency(task_ids[i], task_ids[i - 1]).unwrap();
		}

		// Act - topological sort and cycle detection should succeed
		let order = dag.topological_sort().unwrap();

		// Assert
		assert_eq!(order.len(), depth);
		// Verify ordering: each task appears after its dependency
		for i in 1..depth {
			let dep_pos = order.iter().position(|&id| id == task_ids[i - 1]).unwrap();
			let task_pos = order.iter().position(|&id| id == task_ids[i]).unwrap();
			assert!(dep_pos < task_pos);
		}
	}

	#[rstest]
	fn test_cycle_detection_on_deep_chain_with_back_edge() {
		// Arrange - build a deep chain and then add a back-edge to form a cycle
		let mut dag = TaskDAG::new();
		let depth = 500;
		let mut task_ids = Vec::with_capacity(depth);

		for _ in 0..depth {
			let id = TaskId::new();
			dag.add_task(id).unwrap();
			task_ids.push(id);
		}

		for i in 1..depth {
			dag.add_dependency(task_ids[i], task_ids[i - 1]).unwrap();
		}

		// Act - add a back-edge from the first to the last, creating a cycle
		let result = dag.add_dependency(task_ids[0], task_ids[depth - 1]);

		// Assert
		assert!(result.is_err());
	}

	// Regression test for #756: iterative DFS cycle detection must handle chains
	// of 10,000 nodes without a stack overflow. A naive recursive DFS would exhaust
	// the default thread stack (typically 8 MiB) for chains this deep.
	#[rstest]
	fn test_deep_chain_10k_nodes_does_not_stack_overflow() {
		// Arrange - linear chain: t0 -> t1 -> ... -> t9999
		let mut dag = TaskDAG::new();
		let depth = 10_000;
		let mut task_ids = Vec::with_capacity(depth);

		for _ in 0..depth {
			let id = TaskId::new();
			dag.add_task(id).unwrap();
			task_ids.push(id);
		}

		for i in 1..depth {
			dag.add_dependency(task_ids[i], task_ids[i - 1]).unwrap();
		}

		// Act - topological sort on the 10k-node chain
		let order = dag.topological_sort().unwrap();

		// Assert - all nodes present and ordering is preserved
		assert_eq!(order.len(), depth);
		for i in 1..depth {
			let prev_pos = order.iter().position(|&id| id == task_ids[i - 1]).unwrap();
			let curr_pos = order.iter().position(|&id| id == task_ids[i]).unwrap();
			assert!(
				prev_pos < curr_pos,
				"task_ids[{}] must precede task_ids[{}]",
				i - 1,
				i
			);
		}
	}

	// Regression test for #756: cycle detection on a 10k-node chain with a back-edge
	// must correctly detect the cycle using the iterative algorithm.
	#[rstest]
	fn test_deep_chain_10k_nodes_back_edge_detected() {
		// Arrange - linear chain of 10,000 nodes
		let mut dag = TaskDAG::new();
		let depth = 10_000;
		let mut task_ids = Vec::with_capacity(depth);

		for _ in 0..depth {
			let id = TaskId::new();
			dag.add_task(id).unwrap();
			task_ids.push(id);
		}

		for i in 1..depth {
			dag.add_dependency(task_ids[i], task_ids[i - 1]).unwrap();
		}

		// Act - add a back-edge from the first node to the last node
		let result = dag.add_dependency(task_ids[0], task_ids[depth - 1]);

		// Assert - cycle must be detected
		assert!(
			result.is_err(),
			"back-edge cycle must be detected in 10k chain"
		);
	}
}
