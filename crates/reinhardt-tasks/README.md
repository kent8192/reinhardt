# reinhardt-tasks

Background task processing

## Overview

Background task queue for executing long-running or scheduled tasks asynchronously.

Supports task scheduling, retries, task priorities, and multiple worker processes.

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.21", features = ["tasks"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-rc.21", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-rc.21", features = ["full"] }      # All features
```

Then import task features:

```rust
use reinhardt::tasks::{Task, TaskQueue, TaskExecutor};
use reinhardt::tasks::backend::{TaskBackend, RedisTaskBackend};
```

**Note:** Task features are included in the `standard` and `full` feature presets.

## Features

### Implemented ✓

#### Core Task System

- **Task Trait**: Basic task interface
  - Task ID (`TaskId`): UUID-based unique identifier
  - Task name and priority management
  - Priority range: 0-9 (default: 5)
- **TaskExecutor Trait**: Asynchronous task execution interface
- **TaskStatus**: Task lifecycle management
  - `Pending`: Waiting
  - `Running`: Executing
  - `Success`: Completed successfully
  - `Failure`: Failed
  - `Retry`: Retrying

#### Task Backends

- **TaskBackend Trait**: Task backend abstraction interface
  - Task enqueuing (`enqueue`)
  - Task dequeuing (`dequeue`)
  - Status retrieval (`get_status`)
  - Status update (`update_status`)
- **DummyBackend**: Dummy backend for testing
  - Simple implementation that always succeeds
- **ImmediateBackend**: Backend for immediate execution
  - For synchronous task execution
- **RedisTaskBackend** (feature: `redis-backend`): Redis-based distributed task queue
  - Task metadata storage using Redis
  - Queue-based task distribution
  - Customizable key prefix
- **SqliteBackend** (feature: `database-backend`): SQLite-based task persistence
  - Task storage in SQLite database
  - Automatic table creation
  - FIFO-based task retrieval
- **RabbitMQBackend** (feature: `rabbitmq-backend`): RabbitMQ-based message queue
  - AMQP protocol for reliable messaging
  - Persistent task storage with durable queues
  - Prefetch count for worker concurrency control
  - Delivery mode configuration (persistent/transient)
  - Metadata store abstraction for task tracking

#### Task Queue

- **TaskQueue**: Task queue management
  - Configurable queue name
  - Retry count configuration (default: 3)
  - Task enqueuing via backend
- **QueueConfig**: Queue configuration
  - Customizable queue name
  - Maximum retry count setting

#### Task Scheduling

- **Scheduler**: Task scheduler
  - Task and schedule registration
  - Foundation for schedule-based task execution
- **Schedule Trait**: Schedule interface
  - Next execution time calculation
- **CronSchedule**: Cron expression-based scheduling
  - Cron expression storage and management

#### Worker System

- **Worker**: Task worker
  - Concurrent execution count configuration (default: 4)
  - Task retrieval and execution from backend
  - Graceful shutdown
  - Task processing loop (polling-based)
  - Error handling and status updates
  - Shutdown signaling via broadcast channel
- **WorkerConfig**: Worker configuration
  - Worker name setting
  - Concurrent execution count customization
  - Polling interval configuration (default: 1 second)

#### Task Chains

- **TaskChain**: Task chain management
  - Sequential execution of multiple tasks
  - Chain status management (Pending, Running, Completed, Failed)
  - Task addition and chain progression control
- **TaskChainBuilder**: Builder pattern for chain construction
  - Fluent interface for adding tasks
  - Bulk task addition
- **ChainStatus**: Chain lifecycle management

#### Result Handling

- **TaskOutput**: Task execution result
  - Task ID and string representation of result
- **TaskResult**: Task result type
  - Error handling via Result type
- **TaskResultMetadata**: Result metadata with status
  - Management of status, result, error, and timestamp
- **ResultBackend Trait**: Result persistence interface
  - Result storage (`store_result`)
  - Result retrieval (`get_result`)
  - Result deletion (`delete_result`)
- **MemoryResultBackend**: In-memory result backend
  - Result storage for testing
  - Concurrent access control via RwLock

#### Retry & Backoff

- **RetryStrategy**: Retry strategy configuration
  - Exponential backoff (`exponential_backoff`)
  - Fixed delay (`fixed_delay`)
  - No retry (`no_retry`)
  - Configuration for max retries, initial delay, max delay, multiplier
  - Jitter support (Thundering Herd Problem mitigation)
- **RetryState**: Retry state tracking
  - Retry attempt count recording
  - Next retry delay calculation
  - Retry eligibility determination
  - State reset

#### Error Handling

- **TaskError**: Task-related errors
  - Execution failure (`ExecutionFailed`)
  - Task not found (`TaskNotFound`)
  - Queue error (`QueueError`)
  - Serialization failure (`SerializationError`)
  - Timeout (`Timeout`)
  - Max retries exceeded (`MaxRetriesExceeded`)
- **TaskExecutionError**: Backend execution errors
  - Execution failure, task not found, backend error

### RabbitMQ Backend

The RabbitMQ backend provides production-ready message queue integration:

```rust
use reinhardt::tasks::backends::rabbitmq::{RabbitMQBackend, RabbitMQConfig, DeliveryMode};

let config = RabbitMQConfig {
    uri: "amqp://localhost:5672".to_string(),
    queue_name: "my_tasks".to_string(),
    prefetch_count: 10,
    delivery_mode: DeliveryMode::Persistent,
};

let backend = RabbitMQBackend::new(config).await?;
```

#### Configuration Options

- `uri`: RabbitMQ connection URI (e.g., `amqp://user:pass@host:5672/vhost`)
- `queue_name`: Name of the queue to use for tasks
- `prefetch_count`: Number of tasks to prefetch per worker (default: 1)
- `delivery_mode`: Persistent or Transient message delivery

#### Delivery Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `Persistent` | Messages survive broker restart | Production tasks |
| `Transient` | Messages lost on broker restart | Low-priority tasks |

### Metadata Store

The metadata store provides task metadata persistence separate from the task queue:

```rust
use reinhardt::tasks::backends::metadata_store::{MetadataStore, TaskMetadata, TaskStatus};

// Store task metadata
let metadata = TaskMetadata {
    id: "task-123".to_string(),
    name: "process_order".to_string(),
    status: TaskStatus::Pending,
    created_at: Utc::now(),
    updated_at: Utc::now(),
    task_data: serde_json::json!({"order_id": 456}),
};

store.store(metadata)?;

// Update task status
store.update_status("task-123", TaskStatus::Running)?;

// Retrieve metadata
let metadata = store.get("task-123")?;
```

#### Available Implementations

- **InMemoryMetadataStore**: In-memory storage for testing and development

### Backend Comparison

| Backend | Persistence | Scalability | Use Case |
|---------|-------------|-------------|----------|
| **Dummy** | No | - | Unit testing |
| **Immediate** | No | Single process | Development, synchronous tasks |
| **Redis** | Yes | High | Production, caching |
| **RabbitMQ** | Yes | Very High | Production, messaging, complex routing |
| **SQLite** | Yes | Low | Small-scale production, embedded |

#### Choosing a Backend

- **Development**: Use `ImmediateBackend` for simplicity
- **Testing**: Use `DummyBackend` or `InMemoryMetadataStore`
- **Small-scale production**: Use `SqliteBackend`
- **Large-scale production**: Use `RabbitMQBackend` or `RedisTaskBackend`
- **Complex routing needs**: Use `RabbitMQBackend` for exchange-based routing

## Testing

Redis backend tests are executed using TestContainers:

```bash
cargo test --package reinhardt-tasks --features all-backends
```

Tests run serially with `#[serial(redis)]` attribute to prevent Redis container conflicts.
