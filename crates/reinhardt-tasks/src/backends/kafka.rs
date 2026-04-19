use crate::{
	Task, TaskId, TaskStatus,
	backend::{TaskBackend, TaskExecutionError},
	registry::SerializedTask,
};
use async_trait::async_trait;
use reinhardt_streaming::StreamingError;
use reinhardt_streaming::kafka::{KafkaConfig, KafkaConsumer, KafkaProducer};
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

const TASK_TOPIC: &str = "reinhardt-tasks";

/// On-wire envelope published to Kafka. Carries the originating `TaskId`
/// alongside the task payload so that `dequeue` can reconstruct the same id
/// observed by the producer, enabling status/data lookups across producer and
/// consumer processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskEnvelope {
	id: TaskId,
	task: SerializedTask,
}

/// Kafka-backed task queue. Tasks are published as JSON to the `reinhardt-tasks` topic.
///
/// Status tracking is in-memory; for distributed deployments, extend with a
/// persistent status store (e.g., Redis or a database).
pub struct KafkaTaskBackend {
	producer: Arc<KafkaProducer>,
	consumer: Arc<KafkaConsumer>,
	statuses: Mutex<HashMap<TaskId, TaskStatus>>,
	task_data: Mutex<HashMap<TaskId, SerializedTask>>,
}

impl KafkaTaskBackend {
	/// Connect to the Kafka cluster described by `config` and build a backend.
	pub async fn connect(config: &KafkaConfig) -> Result<Self, StreamingError> {
		let producer = KafkaProducer::connect(config).await?;
		let consumer = KafkaConsumer::connect(config).await?;
		Ok(Self {
			producer: Arc::new(producer),
			consumer: Arc::new(consumer),
			statuses: Mutex::new(HashMap::new()),
			task_data: Mutex::new(HashMap::new()),
		})
	}
}

#[async_trait]
impl TaskBackend for KafkaTaskBackend {
	async fn enqueue(&self, task: Box<dyn Task>) -> Result<TaskId, TaskExecutionError> {
		let id = task.id();
		let serialized = SerializedTask::new(task.name().to_owned(), "{}".to_owned());
		let envelope = TaskEnvelope {
			id,
			task: serialized.clone(),
		};
		let payload = serde_json::to_vec(&envelope)
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		self.producer
			.send_raw(TASK_TOPIC, payload)
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		self.task_data.lock().unwrap().insert(id, serialized);
		self.statuses
			.lock()
			.unwrap()
			.insert(id, TaskStatus::Pending);
		Ok(id)
	}

	async fn dequeue(&self) -> Result<Option<TaskId>, TaskExecutionError> {
		match self
			.consumer
			.receive_raw(TASK_TOPIC)
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?
		{
			None => Ok(None),
			Some((bytes, _offset)) => {
				let envelope: TaskEnvelope = serde_json::from_slice(&bytes)
					.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;
				let TaskEnvelope { id, task } = envelope;
				self.task_data.lock().unwrap().insert(id, task);
				self.statuses
					.lock()
					.unwrap()
					.entry(id)
					.or_insert(TaskStatus::Pending);
				Ok(Some(id))
			}
		}
	}

	async fn get_status(&self, task_id: TaskId) -> Result<TaskStatus, TaskExecutionError> {
		self.statuses
			.lock()
			.unwrap()
			.get(&task_id)
			.copied()
			.ok_or(TaskExecutionError::NotFound(task_id))
	}

	async fn update_status(
		&self,
		task_id: TaskId,
		status: TaskStatus,
	) -> Result<(), TaskExecutionError> {
		self.statuses.lock().unwrap().insert(task_id, status);
		Ok(())
	}

	async fn get_task_data(
		&self,
		task_id: TaskId,
	) -> Result<Option<SerializedTask>, TaskExecutionError> {
		Ok(self.task_data.lock().unwrap().get(&task_id).cloned())
	}

	fn backend_name(&self) -> &str {
		"kafka"
	}
}
