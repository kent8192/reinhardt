use crate::{StreamingError, kafka::{KafkaConfig, KafkaConsumer, KafkaProducer}};
use async_trait::async_trait;
use reinhardt_tasks::{
    Task, TaskId, TaskStatus,
    backend::{TaskBackend, TaskExecutionError},
    registry::SerializedTask,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const TASK_TOPIC: &str = "reinhardt-tasks";

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
        let payload = serialized
            .to_json()
            .map_err(|e| TaskExecutionError::BackendError(e.to_string()))?
            .into_bytes();

        self.producer
            .send_raw(TASK_TOPIC, payload)
            .await
            .map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

        self.task_data
            .lock()
            .unwrap()
            .insert(id, SerializedTask::new(task.name().to_owned(), "{}".to_owned()));
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
                let json = String::from_utf8(bytes)
                    .map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;
                let serialized = SerializedTask::from_json(&json)
                    .map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;
                let id = TaskId::new();
                self.task_data.lock().unwrap().insert(id, serialized);
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
