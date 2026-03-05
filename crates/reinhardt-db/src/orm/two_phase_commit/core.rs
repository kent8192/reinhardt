//! Two-Phase Commit (2PC) implementation for distributed transactions
//!
//! This module provides support for distributed transactions across multiple databases
//! using the two-phase commit protocol. The protocol consists of a prepare phase
//! where all participants vote on the transaction, followed by a commit or rollback phase.
//!
//! ## Architecture
//!
//! The implementation follows the X/Open XA specification:
//!
//! - **Coordinator**: Manages the 2PC protocol lifecycle
//! - **Participants**: Database connections that participate in the transaction
//! - **Transaction Log**: Persistent storage for recovery
//!
//! ## Usage
//!
//! ```rust,no_run
//! use reinhardt_db::orm::two_phase_commit::TwoPhaseCoordinator;
//! #[cfg(feature = "postgres")]
//! use reinhardt_db::orm::PostgresParticipantAdapter;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create coordinator
//! let mut coordinator = TwoPhaseCoordinator::new("global_txn_001");
//!
//! // Add participants (requires postgres feature)
//! #[cfg(feature = "postgres")]
//! {
//!     let primary_pool = sqlx::PgPool::connect("postgresql://localhost/primary_db").await?;
//!     let secondary_pool = sqlx::PgPool::connect("postgresql://localhost/secondary_db").await?;
//!     
//!     coordinator.add_participant(Box::new(PostgresParticipantAdapter::new("primary_db", primary_pool))).await?;
//!     coordinator.add_participant(Box::new(PostgresParticipantAdapter::new("secondary_db", secondary_pool))).await?;
//! }
//!
//! // Execute transaction
//! coordinator.begin().await?;
//! // ... perform operations on each participant ...
//! coordinator.commit().await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex as TokioMutex;

#[cfg(feature = "postgres")]
use crate::backends::PostgresTwoPhaseParticipant;

#[cfg(feature = "mysql")]
use crate::backends::{MySqlTwoPhaseParticipant, XaSessionPrepared, XaSessionStarted};

/// Errors that can occur during two-phase commit operations
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TwoPhaseError {
	/// A participant failed during the prepare phase
	PrepareFailed(String, String),
	/// A participant failed during the commit phase
	CommitFailed(String, String),
	/// A participant failed during the rollback phase
	RollbackFailed(String, String),
	/// Invalid transaction state for the requested operation
	InvalidState(String),
	/// No participants registered for the transaction
	NoParticipants,
	/// Transaction ID already exists
	DuplicateTransactionId(String),
	/// Participant already exists
	DuplicateParticipant(String),
	/// Network or connection error
	ConnectionError(String),
	/// Timeout during operation
	Timeout(String),
	/// Recovery failed
	RecoveryFailed(String),
	/// Transaction log error
	LogError(String),
	/// Database error
	DatabaseError(String),
}

impl std::fmt::Display for TwoPhaseError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			TwoPhaseError::PrepareFailed(participant, reason) => {
				write!(f, "Prepare failed for '{}': {}", participant, reason)
			}
			TwoPhaseError::CommitFailed(participant, reason) => {
				write!(f, "Commit failed for '{}': {}", participant, reason)
			}
			TwoPhaseError::RollbackFailed(participant, reason) => {
				write!(f, "Rollback failed for '{}': {}", participant, reason)
			}
			TwoPhaseError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
			TwoPhaseError::NoParticipants => write!(f, "No participants registered"),
			TwoPhaseError::DuplicateTransactionId(id) => {
				write!(f, "Transaction ID '{}' already exists", id)
			}
			TwoPhaseError::DuplicateParticipant(participant) => {
				write!(f, "Participant '{}' already registered", participant)
			}
			TwoPhaseError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
			TwoPhaseError::Timeout(msg) => write!(f, "Timeout: {}", msg),
			TwoPhaseError::RecoveryFailed(msg) => write!(f, "Recovery failed: {}", msg),
			TwoPhaseError::LogError(msg) => write!(f, "Transaction log error: {}", msg),
			TwoPhaseError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
		}
	}
}

impl std::error::Error for TwoPhaseError {}

/// Transaction state in the two-phase commit protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TransactionState {
	/// Transaction has not started
	NotStarted,
	/// Transaction is in progress
	Active,
	/// Preparing participants (First Phase in progress)
	Preparing,
	/// All participants have been prepared
	Prepared,
	/// Committing transaction (Second Phase in progress)
	Committing,
	/// Transaction has been committed
	Committed,
	/// Aborting transaction
	Aborting,
	/// Transaction has been aborted/rolled back
	Aborted,
}

/// Status of an individual participant
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParticipantStatus {
	/// Participant is active in transaction
	Active,
	/// Participant has successfully prepared
	Prepared,
	/// Participant has committed
	Committed,
	/// Participant has aborted
	Aborted,
}

/// Trait for two-phase commit participants
///
/// Participants must implement this trait to participate in distributed transactions.
#[async_trait(?Send)]
pub trait TwoPhaseParticipant: Send + Sync {
	/// Get the participant's identifier
	fn id(&self) -> &str;

	/// Begin a local transaction
	async fn begin(&self) -> Result<(), TwoPhaseError>;

	/// Prepare the transaction (First Phase)
	///
	/// This is the voting phase where the participant indicates whether it can commit.
	/// Returns Ok(()) if ready to commit, or an error if unable to commit.
	async fn prepare(&self, xid: String) -> Result<(), TwoPhaseError>;

	/// Commit the prepared transaction (Second Phase)
	///
	/// This commits the transaction that was previously prepared.
	async fn commit(&self, xid: String) -> Result<(), TwoPhaseError>;

	/// Rollback the transaction (Second Phase)
	///
	/// This rolls back the transaction, either before or after prepare.
	async fn rollback(&self, xid: String) -> Result<(), TwoPhaseError>;

	/// Recover prepared transactions
	///
	/// Returns a list of transaction IDs that are currently in the prepared state.
	async fn recover(&self) -> Result<Vec<String>, TwoPhaseError>;

	/// Get the current status
	fn status(&self) -> ParticipantStatus;

	/// Set the status
	fn set_status(&mut self, status: ParticipantStatus);
}

/// A participant in the distributed transaction
#[derive(Debug, Clone)]
pub struct Participant {
	pub db_alias: String,
	pub status: ParticipantStatus,
}

impl Participant {
	/// Create a new participant
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::Participant;
	///
	/// let participant = Participant::new("primary_db");
	/// assert_eq!(participant.db_alias, "primary_db");
	/// ```
	pub fn new(db_alias: impl Into<String>) -> Self {
		Self {
			db_alias: db_alias.into(),
			status: ParticipantStatus::Active,
		}
	}

	/// Check if participant is prepared
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::Participant;
	///
	/// let mut participant = Participant::new("db1");
	/// assert!(!participant.is_prepared());
	///
	/// participant.status = reinhardt_db::orm::two_phase_commit::ParticipantStatus::Prepared;
	/// assert!(participant.is_prepared());
	/// ```
	pub fn is_prepared(&self) -> bool {
		matches!(self.status, ParticipantStatus::Prepared)
	}
}

/// Two-Phase Commit coordinator for distributed transactions
///
/// Manages the two-phase commit protocol across multiple database participants.
/// The protocol ensures that either all participants commit or all abort.
#[derive(Debug)]
pub struct TwoPhaseCommit {
	transaction_id: String,
	state: Arc<StdMutex<TransactionState>>,
	participants: Arc<StdMutex<HashMap<String, Participant>>>,
}

impl TwoPhaseCommit {
	/// Create a new two-phase commit transaction
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::TwoPhaseCommit;
	///
	/// let tpc = TwoPhaseCommit::new("txn_001");
	/// assert_eq!(tpc.transaction_id(), "txn_001");
	/// assert_eq!(tpc.state().unwrap(), reinhardt_db::orm::two_phase_commit::TransactionState::NotStarted);
	/// ```
	pub fn new(transaction_id: impl Into<String>) -> Self {
		Self {
			transaction_id: transaction_id.into(),
			state: Arc::new(StdMutex::new(TransactionState::NotStarted)),
			participants: Arc::new(StdMutex::new(HashMap::new())),
		}
	}

	/// Get the transaction ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::TwoPhaseCommit;
	///
	/// let tpc = TwoPhaseCommit::new("my_transaction");
	/// assert_eq!(tpc.transaction_id(), "my_transaction");
	/// ```
	pub fn transaction_id(&self) -> &str {
		&self.transaction_id
	}

	/// Get the current transaction state
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::{TwoPhaseCommit, TransactionState};
	///
	/// let mut tpc = TwoPhaseCommit::new("txn_002");
	/// assert_eq!(tpc.state().unwrap(), TransactionState::NotStarted);
	///
	/// tpc.begin().unwrap();
	/// assert_eq!(tpc.state().unwrap(), TransactionState::Active);
	/// ```
	pub fn state(&self) -> Result<TransactionState, TwoPhaseError> {
		self.state
			.lock()
			.map(|s| *s)
			.map_err(|_| TwoPhaseError::InvalidState("Failed to acquire state lock".to_string()))
	}

	/// Begin the distributed transaction
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::{TwoPhaseCommit, TransactionState};
	///
	/// let mut tpc = TwoPhaseCommit::new("txn_003");
	/// tpc.begin().unwrap();
	/// assert_eq!(tpc.state().unwrap(), TransactionState::Active);
	/// ```
	pub fn begin(&mut self) -> Result<(), TwoPhaseError> {
		let mut state = self
			.state
			.lock()
			.map_err(|_| TwoPhaseError::InvalidState("Failed to acquire state lock".to_string()))?;

		if *state != TransactionState::NotStarted {
			return Err(TwoPhaseError::InvalidState(
				"Transaction already started".to_string(),
			));
		}

		*state = TransactionState::Active;
		Ok(())
	}

	/// Add a participant to the distributed transaction
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::TwoPhaseCommit;
	///
	/// let mut tpc = TwoPhaseCommit::new("txn_004");
	/// tpc.begin().unwrap();
	/// tpc.add_participant("database_1").unwrap();
	/// tpc.add_participant("database_2").unwrap();
	/// assert_eq!(tpc.participant_count(), 2);
	/// ```
	pub fn add_participant(&mut self, db_alias: impl Into<String>) -> Result<(), TwoPhaseError> {
		let state = self.state().unwrap();
		if state != TransactionState::Active {
			return Err(TwoPhaseError::InvalidState(
				"Can only add participants to active transaction".to_string(),
			));
		}

		let db_alias = db_alias.into();
		let mut participants = self.participants.lock().map_err(|_| {
			TwoPhaseError::InvalidState("Failed to acquire participants lock".to_string())
		})?;

		if participants.contains_key(&db_alias) {
			return Err(TwoPhaseError::DuplicateParticipant(db_alias));
		}

		participants.insert(db_alias.clone(), Participant::new(db_alias));
		Ok(())
	}

	/// Get the number of participants
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::TwoPhaseCommit;
	///
	/// let mut tpc = TwoPhaseCommit::new("txn_005");
	/// tpc.begin().unwrap();
	/// assert_eq!(tpc.participant_count(), 0);
	///
	/// tpc.add_participant("db1").unwrap();
	/// assert_eq!(tpc.participant_count(), 1);
	/// ```
	pub fn participant_count(&self) -> usize {
		self.participants.lock().map(|p| p.len()).unwrap_or(0)
	}

	/// Prepare phase: ask all participants to prepare for commit
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::{TwoPhaseCommit, TransactionState};
	///
	/// let mut tpc = TwoPhaseCommit::new("txn_006");
	/// tpc.begin().unwrap();
	/// tpc.add_participant("db1").unwrap();
	/// tpc.add_participant("db2").unwrap();
	///
	/// let result = tpc.prepare();
	/// assert!(result.is_ok());
	/// assert_eq!(tpc.state().unwrap(), TransactionState::Prepared);
	/// ```
	pub fn prepare(&mut self) -> Result<Vec<String>, TwoPhaseError> {
		let state = self.state().unwrap();
		if state != TransactionState::Active {
			return Err(TwoPhaseError::InvalidState(
				"Can only prepare active transaction".to_string(),
			));
		}

		let mut participants = self.participants.lock().map_err(|_| {
			TwoPhaseError::InvalidState("Failed to acquire participants lock".to_string())
		})?;

		if participants.is_empty() {
			return Err(TwoPhaseError::NoParticipants);
		}

		let mut prepared_sqls = Vec::new();

		for (db_alias, participant) in participants.iter_mut() {
			let sql = format!("PREPARE TRANSACTION '{}'", self.transaction_id);
			prepared_sqls.push(format!("{}: {}", db_alias, sql));
			participant.status = ParticipantStatus::Prepared;
		}

		let mut state = self
			.state
			.lock()
			.map_err(|_| TwoPhaseError::InvalidState("Failed to acquire state lock".to_string()))?;
		*state = TransactionState::Prepared;

		Ok(prepared_sqls)
	}

	/// Commit phase: commit the transaction on all prepared participants
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::{TwoPhaseCommit, TransactionState};
	///
	/// let mut tpc = TwoPhaseCommit::new("txn_007");
	/// tpc.begin().unwrap();
	/// tpc.add_participant("db1").unwrap();
	/// tpc.prepare().unwrap();
	///
	/// let result = tpc.commit();
	/// assert!(result.is_ok());
	/// assert_eq!(tpc.state().unwrap(), TransactionState::Committed);
	/// ```
	pub fn commit(&mut self) -> Result<Vec<String>, TwoPhaseError> {
		let state = self.state().unwrap();
		if state != TransactionState::Prepared {
			return Err(TwoPhaseError::InvalidState(
				"Can only commit prepared transaction".to_string(),
			));
		}

		let mut participants = self.participants.lock().map_err(|_| {
			TwoPhaseError::InvalidState("Failed to acquire participants lock".to_string())
		})?;

		let mut commit_sqls = Vec::new();

		for (db_alias, participant) in participants.iter_mut() {
			if !participant.is_prepared() {
				return Err(TwoPhaseError::CommitFailed(
					db_alias.clone(),
					"Participant not prepared".to_string(),
				));
			}

			let sql = format!("COMMIT PREPARED '{}'", self.transaction_id);
			commit_sqls.push(format!("{}: {}", db_alias, sql));
			participant.status = ParticipantStatus::Committed;
		}

		let mut state = self
			.state
			.lock()
			.map_err(|_| TwoPhaseError::InvalidState("Failed to acquire state lock".to_string()))?;
		*state = TransactionState::Committed;

		Ok(commit_sqls)
	}

	/// Rollback/abort the transaction on all participants
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::{TwoPhaseCommit, TransactionState};
	///
	/// let mut tpc = TwoPhaseCommit::new("txn_008");
	/// tpc.begin().unwrap();
	/// tpc.add_participant("db1").unwrap();
	///
	/// let result = tpc.rollback();
	/// assert!(result.is_ok());
	/// assert_eq!(tpc.state().unwrap(), TransactionState::Aborted);
	/// ```
	pub fn rollback(&mut self) -> Result<Vec<String>, TwoPhaseError> {
		let state = self.state().unwrap();
		if state != TransactionState::Active
			&& state != TransactionState::Prepared
			&& state != TransactionState::Preparing
		{
			return Err(TwoPhaseError::InvalidState(
				"Can only rollback active, prepared, or preparing transaction".to_string(),
			));
		}

		let mut participants = self.participants.lock().map_err(|_| {
			TwoPhaseError::InvalidState("Failed to acquire participants lock".to_string())
		})?;

		let mut rollback_sqls = Vec::new();

		for (db_alias, participant) in participants.iter_mut() {
			let sql = if participant.is_prepared() {
				format!("ROLLBACK PREPARED '{}'", self.transaction_id)
			} else {
				"ROLLBACK".to_string()
			};
			rollback_sqls.push(format!("{}: {}", db_alias, sql));
			participant.status = ParticipantStatus::Aborted;
		}

		let mut state = self
			.state
			.lock()
			.map_err(|_| TwoPhaseError::InvalidState("Failed to acquire state lock".to_string()))?;
		*state = TransactionState::Aborted;

		Ok(rollback_sqls)
	}

	/// Get all participants in the transaction
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::TwoPhaseCommit;
	///
	/// let mut tpc = TwoPhaseCommit::new("txn_009");
	/// tpc.begin().unwrap();
	/// tpc.add_participant("db1").unwrap();
	/// tpc.add_participant("db2").unwrap();
	///
	/// let participants = tpc.participants();
	/// assert_eq!(participants.len(), 2);
	/// assert!(participants.contains_key("db1"));
	/// assert!(participants.contains_key("db2"));
	/// ```
	pub fn participants(&self) -> HashMap<String, Participant> {
		self.participants
			.lock()
			.map(|p| p.clone())
			.unwrap_or_default()
	}

	/// Check if all participants are prepared
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::two_phase_commit::TwoPhaseCommit;
	///
	/// let mut tpc = TwoPhaseCommit::new("txn_010");
	/// tpc.begin().unwrap();
	/// tpc.add_participant("db1").unwrap();
	/// assert!(!tpc.all_prepared());
	///
	/// tpc.prepare().unwrap();
	/// assert!(tpc.all_prepared());
	/// ```
	pub fn all_prepared(&self) -> bool {
		self.participants
			.lock()
			.map(|p| p.values().all(|participant| participant.is_prepared()))
			.unwrap_or(false)
	}
}

impl Default for TwoPhaseCommit {
	fn default() -> Self {
		Self::new(uuid::Uuid::new_v4().to_string())
	}
}

/// Two-Phase Commit Coordinator
///
/// Coordinates distributed transactions across multiple database participants using
/// the two-phase commit protocol. This is the main interface for executing XA transactions.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_db::orm::two_phase_commit::TwoPhaseCoordinator;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut coordinator = TwoPhaseCoordinator::new("txn_001");
///
/// // Add participants
/// // coordinator.add_participant(participant1).await?;
/// // coordinator.add_participant(participant2).await?;
///
/// // Execute distributed transaction
/// coordinator.begin().await?;
/// // ... perform operations ...
/// coordinator.commit().await?;
/// # Ok(())
/// # }
/// ```
pub struct TwoPhaseCoordinator {
	transaction_id: String,
	state: Arc<TokioMutex<TransactionState>>,
	participants: Arc<TokioMutex<Vec<Box<dyn TwoPhaseParticipant>>>>,
	transaction_log: Option<Arc<dyn super::transaction_log::TransactionLog>>,
}

impl TwoPhaseCoordinator {
	/// Create a new two-phase commit coordinator
	pub fn new(transaction_id: impl Into<String>) -> Self {
		Self {
			transaction_id: transaction_id.into(),
			state: Arc::new(TokioMutex::new(TransactionState::NotStarted)),
			participants: Arc::new(TokioMutex::new(Vec::new())),
			transaction_log: None,
		}
	}

	/// Create a new coordinator with transaction logging
	pub fn with_log(
		transaction_id: impl Into<String>,
		log: Arc<dyn super::transaction_log::TransactionLog>,
	) -> Self {
		Self {
			transaction_id: transaction_id.into(),
			state: Arc::new(TokioMutex::new(TransactionState::NotStarted)),
			participants: Arc::new(TokioMutex::new(Vec::new())),
			transaction_log: Some(log),
		}
	}

	/// Record transaction state to log
	async fn log_state(&self, state: TransactionState) -> Result<(), TwoPhaseError> {
		if let Some(log) = &self.transaction_log {
			let participants = self.participants.lock().await;
			let participant_ids: Vec<String> =
				participants.iter().map(|p| p.id().to_string()).collect();

			let entry = super::transaction_log::TransactionLogEntry::new(
				&self.transaction_id,
				state,
				participant_ids,
			);
			log.write(&entry)?;
		}
		Ok(())
	}

	/// Get the transaction ID
	pub fn transaction_id(&self) -> &str {
		&self.transaction_id
	}

	/// Get the current transaction state
	pub async fn state(&self) -> Result<TransactionState, TwoPhaseError> {
		let state = self.state.lock().await;
		Ok(*state)
	}

	/// Add a participant to the coordinator
	pub async fn add_participant(
		&mut self,
		participant: Box<dyn TwoPhaseParticipant>,
	) -> Result<(), TwoPhaseError> {
		let mut participants = self.participants.lock().await;

		let id = participant.id().to_string();
		if participants.iter().any(|p| p.id() == id) {
			return Err(TwoPhaseError::DuplicateParticipant(id));
		}

		participants.push(participant);
		Ok(())
	}

	/// Begin the distributed transaction
	pub async fn begin(&mut self) -> Result<(), TwoPhaseError> {
		{
			let mut state = self.state.lock().await;

			if *state != TransactionState::NotStarted {
				return Err(TwoPhaseError::InvalidState(
					"Transaction already started".to_string(),
				));
			}

			*state = TransactionState::Active;
		}

		// Log state change
		self.log_state(TransactionState::Active).await?;

		// Begin transaction on all participants
		{
			let mut participants = self.participants.lock().await;

			for participant in participants.iter_mut() {
				participant.begin().await.map_err(|e| {
					TwoPhaseError::PrepareFailed(participant.id().to_string(), e.to_string())
				})?;
			}
		}

		Ok(())
	}

	/// Prepare phase: ask all participants to prepare for commit
	pub async fn prepare(&mut self) -> Result<(), TwoPhaseError> {
		{
			let mut state = self.state.lock().await;

			if *state != TransactionState::Active {
				return Err(TwoPhaseError::InvalidState(
					"Can only prepare active transaction".to_string(),
				));
			}

			*state = TransactionState::Preparing;
		}

		// Prepare all participants
		{
			let mut participants = self.participants.lock().await;

			if participants.is_empty() {
				return Err(TwoPhaseError::NoParticipants);
			}

			for participant in participants.iter_mut() {
				match participant.prepare(self.transaction_id.clone()).await {
					Ok(_) => {
						participant.set_status(ParticipantStatus::Prepared);
					}
					Err(e) => {
						let participant_id = participant.id().to_string();
						let error_msg = e.to_string();
						// Drop lock before calling rollback
						drop(participants);
						self.rollback().await?;
						return Err(TwoPhaseError::PrepareFailed(participant_id, error_msg));
					}
				}
			}
			// Lock is automatically released here
		}

		{
			let mut state = self.state.lock().await;
			*state = TransactionState::Prepared;
		}

		// Log state change
		self.log_state(TransactionState::Prepared).await?;

		Ok(())
	}

	/// Commit phase: commit the transaction on all prepared participants
	pub async fn commit(&mut self) -> Result<(), TwoPhaseError> {
		{
			let mut state = self.state.lock().await;

			if *state != TransactionState::Prepared {
				return Err(TwoPhaseError::InvalidState(
					"Can only commit prepared transaction".to_string(),
				));
			}

			*state = TransactionState::Committing;
		}

		let mut failed_participants = Vec::new();
		{
			let mut participants = self.participants.lock().await;

			// Commit all participants
			for participant in participants.iter_mut() {
				match participant.commit(self.transaction_id.clone()).await {
					Ok(_) => {
						participant.set_status(ParticipantStatus::Committed);
					}
					Err(e) => {
						// Log failure but continue committing others
						failed_participants.push((participant.id().to_string(), e.to_string()));
					}
				}
			}
		}

		if !failed_participants.is_empty() {
			let mut state = self.state.lock().await;
			*state = TransactionState::Prepared; // Return to prepared state for recovery
			let error_msg = failed_participants
				.iter()
				.map(|(id, err)| format!("{}: {}", id, err))
				.collect::<Vec<_>>()
				.join(", ");
			return Err(TwoPhaseError::CommitFailed(
				"Multiple participants".to_string(),
				error_msg,
			));
		}

		{
			let mut state = self.state.lock().await;
			*state = TransactionState::Committed;
		}

		// Log state change
		self.log_state(TransactionState::Committed).await?;

		// Remove completed transaction from log
		if let Some(log) = &self.transaction_log {
			let _ = log.delete(&self.transaction_id);
		}

		Ok(())
	}

	/// Rollback/abort the transaction on all participants
	pub async fn rollback(&mut self) -> Result<(), TwoPhaseError> {
		{
			let mut state = self.state.lock().await;

			if *state != TransactionState::Active
				&& *state != TransactionState::Prepared
				&& *state != TransactionState::Preparing
			{
				return Err(TwoPhaseError::InvalidState(
					"Can only rollback active, prepared, or preparing transaction".to_string(),
				));
			}

			*state = TransactionState::Aborting;
		}

		let mut failed_participants = Vec::new();
		{
			let mut participants = self.participants.lock().await;

			for participant in participants.iter_mut() {
				match participant.rollback(self.transaction_id.clone()).await {
					Ok(_) => {
						participant.set_status(ParticipantStatus::Aborted);
					}
					Err(e) => {
						failed_participants.push((participant.id().to_string(), e.to_string()));
					}
				}
			}
		}

		if !failed_participants.is_empty() {
			let error_msg = failed_participants
				.iter()
				.map(|(id, err)| format!("{}: {}", id, err))
				.collect::<Vec<_>>()
				.join(", ");
			return Err(TwoPhaseError::RollbackFailed(
				"Multiple participants".to_string(),
				error_msg,
			));
		}

		{
			let mut state = self.state.lock().await;
			*state = TransactionState::Aborted;
		}

		// Log state change
		self.log_state(TransactionState::Aborted).await?;

		// Remove aborted transaction from log
		if let Some(log) = &self.transaction_log {
			let _ = log.delete(&self.transaction_id);
		}

		Ok(())
	}

	/// Recover prepared transactions from all participants
	pub async fn recover_prepared_transactions(&mut self) -> Result<Vec<String>, TwoPhaseError> {
		let mut all_xids = Vec::new();
		{
			let mut participants = self.participants.lock().await;

			for participant in participants.iter_mut() {
				let xids = participant
					.recover()
					.await
					.map_err(|e| TwoPhaseError::RecoveryFailed(e.to_string()))?;
				all_xids.extend(xids);
			}
		}

		Ok(all_xids)
	}

	/// Get the number of participants
	pub async fn participant_count(&self) -> usize {
		self.participants.lock().await.len()
	}
}

/// PostgreSQL participant adapter
///
/// Adapts backend's `PostgresTwoPhaseParticipant` to ORM layer's `TwoPhaseParticipant` trait.
#[cfg(feature = "postgres")]
pub struct PostgresParticipantAdapter {
	id: String,
	backend: PostgresTwoPhaseParticipant,
	status: ParticipantStatus,
}

#[cfg(feature = "postgres")]
impl PostgresParticipantAdapter {
	/// Create a new PostgreSQL participant adapter
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::two_phase_commit::PostgresParticipantAdapter;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// let adapter = PostgresParticipantAdapter::new("db1", pool);
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(id: impl Into<String>, pool: sqlx::PgPool) -> Self {
		Self {
			id: id.into(),
			backend: PostgresTwoPhaseParticipant::new(pool),
			status: ParticipantStatus::Active,
		}
	}

	/// Create a new adapter from `Arc<PgPool>`
	pub fn from_pool_arc(id: impl Into<String>, pool: std::sync::Arc<sqlx::PgPool>) -> Self {
		Self {
			id: id.into(),
			backend: PostgresTwoPhaseParticipant::from_pool_arc(pool),
			status: ParticipantStatus::Active,
		}
	}
}

#[cfg(feature = "postgres")]
#[async_trait(?Send)]
impl TwoPhaseParticipant for PostgresParticipantAdapter {
	fn id(&self) -> &str {
		&self.id
	}

	async fn begin(&self) -> Result<(), TwoPhaseError> {
		self.backend
			.begin_by_xid(&self.id)
			.await
			.map_err(|e| TwoPhaseError::DatabaseError(e.to_string()))
	}

	async fn prepare(&self, xid: String) -> Result<(), TwoPhaseError> {
		self.backend
			.prepare_by_xid(&xid)
			.await
			.map_err(|e| TwoPhaseError::PrepareFailed(self.id.clone(), e.to_string()))
	}

	async fn commit(&self, xid: String) -> Result<(), TwoPhaseError> {
		self.backend
			.commit_managed(&xid)
			.await
			.map_err(|e| TwoPhaseError::CommitFailed(self.id.clone(), e.to_string()))
	}

	async fn rollback(&self, xid: String) -> Result<(), TwoPhaseError> {
		self.backend
			.rollback_managed(&xid)
			.await
			.map_err(|e| TwoPhaseError::RollbackFailed(self.id.clone(), e.to_string()))
	}

	async fn recover(&self) -> Result<Vec<String>, TwoPhaseError> {
		let txns = self
			.backend
			.list_prepared_transactions()
			.await
			.map_err(|e| TwoPhaseError::RecoveryFailed(e.to_string()))?;
		Ok(txns.into_iter().map(|t| t.gid).collect())
	}

	fn status(&self) -> ParticipantStatus {
		self.status.clone()
	}

	fn set_status(&mut self, status: ParticipantStatus) {
		self.status = status;
	}
}

/// XA session state for MySQL participant adapter
///
/// Represents the possible states of an XA transaction session:
/// - `Started`: Transaction has been started with XA START
/// - `Prepared`: Transaction has been prepared with XA END + XA PREPARE
///
/// Note: The `Ended` state is transient and not stored, as it immediately
/// transitions to `Prepared` within the `prepare()` method.
#[cfg(feature = "mysql")]
enum XaSessionState {
	/// Session in Started state (after XA START)
	Started(XaSessionStarted),
	/// Session in Prepared state (after XA END + XA PREPARE)
	Prepared(XaSessionPrepared),
}

/// MySQL participant adapter
///
/// Adapts backend's `MySqlTwoPhaseParticipant` to ORM layer's `TwoPhaseParticipant` trait.
/// Session management uses type-safe state transitions via XaSession types.
///
/// # Type-Safe Session Management
///
/// The adapter maintains an XA session with compile-time enforced state transitions:
/// - `XaSessionStarted` → `XaSessionEnded` → `XaSessionPrepared` → Committed/Rolled back
///
/// # XA Transaction Flow
///
/// MySQL XA transactions require a specific sequence:
/// 1. XA START - Begin transaction (returns XaSessionStarted)
/// 2. ... perform operations ...
/// 3. XA END - End transaction (consumes XaSessionStarted, returns XaSessionEnded)
/// 4. XA PREPARE - Prepare for commit (consumes XaSessionEnded, returns XaSessionPrepared)
/// 5. XA COMMIT or XA ROLLBACK - Finalize (consumes XaSessionPrepared)
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::orm::two_phase_commit::MySqlParticipantAdapter;
/// use sqlx::MySqlPool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
/// let adapter = MySqlParticipantAdapter::new("db1", pool);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "mysql")]
pub struct MySqlParticipantAdapter {
	id: String,
	backend: MySqlTwoPhaseParticipant,
	status: ParticipantStatus,
	session: Arc<StdMutex<Option<XaSessionState>>>,
}

#[cfg(feature = "mysql")]
impl MySqlParticipantAdapter {
	/// Create a new MySQL participant adapter
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::two_phase_commit::MySqlParticipantAdapter;
	/// use sqlx::MySqlPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = MySqlPool::connect("mysql://localhost/mydb").await?;
	/// let adapter = MySqlParticipantAdapter::new("db1", pool);
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(id: impl Into<String>, pool: sqlx::MySqlPool) -> Self {
		Self {
			id: id.into(),
			backend: MySqlTwoPhaseParticipant::new(pool),
			status: ParticipantStatus::Active,
			session: Arc::new(StdMutex::new(None)),
		}
	}

	/// Create a new adapter from `Arc<MySqlPool>`
	pub fn from_pool_arc(id: impl Into<String>, pool: std::sync::Arc<sqlx::MySqlPool>) -> Self {
		Self {
			id: id.into(),
			backend: MySqlTwoPhaseParticipant::from_pool_arc(pool),
			status: ParticipantStatus::Active,
			session: Arc::new(StdMutex::new(None)),
		}
	}
}

#[cfg(feature = "mysql")]
#[async_trait(?Send)]
impl TwoPhaseParticipant for MySqlParticipantAdapter {
	fn id(&self) -> &str {
		&self.id
	}

	async fn begin(&self) -> Result<(), TwoPhaseError> {
		// Create new XA session with participant's ID as XID
		let session = self
			.backend
			.begin(self.id.clone())
			.await
			.map_err(|e| TwoPhaseError::DatabaseError(e.to_string()))?;

		// Store session in Started state
		let mut session_guard = self.session.lock().map_err(|e| {
			TwoPhaseError::DatabaseError(format!("Failed to acquire session lock: {}", e))
		})?;
		*session_guard = Some(XaSessionState::Started(session));
		Ok(())
	}

	async fn prepare(&self, _xid: String) -> Result<(), TwoPhaseError> {
		// Take session from Started state
		let session = {
			let mut session_guard = self.session.lock().map_err(|e| {
				TwoPhaseError::PrepareFailed(
					self.id.clone(),
					format!("Failed to acquire session lock: {}", e),
				)
			})?;

			match session_guard.take() {
				Some(XaSessionState::Started(s)) => s,
				Some(XaSessionState::Prepared(_)) => {
					return Err(TwoPhaseError::InvalidState(format!(
						"Session already prepared for participant '{}'",
						self.id
					)));
				}
				None => {
					return Err(TwoPhaseError::InvalidState(format!(
						"No active session for participant '{}'",
						self.id
					)));
				}
			}
		};

		// MySQL requires XA END before XA PREPARE
		let ended_session = self
			.backend
			.end(session)
			.await
			.map_err(|e| TwoPhaseError::PrepareFailed(self.id.clone(), e.to_string()))?;

		let prepared_session = self
			.backend
			.prepare(ended_session)
			.await
			.map_err(|e| TwoPhaseError::PrepareFailed(self.id.clone(), e.to_string()))?;

		// Store session in Prepared state
		let mut session_guard = self.session.lock().map_err(|e| {
			TwoPhaseError::PrepareFailed(
				self.id.clone(),
				format!("Failed to acquire session lock: {}", e),
			)
		})?;
		*session_guard = Some(XaSessionState::Prepared(prepared_session));
		Ok(())
	}

	async fn commit(&self, _xid: String) -> Result<(), TwoPhaseError> {
		// Take session from Prepared state
		let session = {
			let mut session_guard = self.session.lock().map_err(|e| {
				TwoPhaseError::CommitFailed(
					self.id.clone(),
					format!("Failed to acquire session lock: {}", e),
				)
			})?;

			match session_guard.take() {
				Some(XaSessionState::Prepared(s)) => s,
				Some(XaSessionState::Started(_)) => {
					return Err(TwoPhaseError::InvalidState(format!(
						"Session not prepared for participant '{}'",
						self.id
					)));
				}
				None => {
					return Err(TwoPhaseError::InvalidState(format!(
						"No active session for participant '{}'",
						self.id
					)));
				}
			}
		};

		// Commit the prepared transaction
		self.backend
			.commit(session)
			.await
			.map_err(|e| TwoPhaseError::CommitFailed(self.id.clone(), e.to_string()))
	}

	async fn rollback(&self, _xid: String) -> Result<(), TwoPhaseError> {
		// Take session from either Started or Prepared state
		let session_state = {
			let mut session_guard = self.session.lock().map_err(|e| {
				TwoPhaseError::RollbackFailed(
					self.id.clone(),
					format!("Failed to acquire session lock: {}", e),
				)
			})?;

			session_guard.take().ok_or_else(|| {
				TwoPhaseError::InvalidState(format!(
					"No active session for participant '{}'",
					self.id
				))
			})?
		};

		// Rollback based on current state
		match session_state {
			XaSessionState::Started(s) => self
				.backend
				.rollback_started(s)
				.await
				.map_err(|e| TwoPhaseError::RollbackFailed(self.id.clone(), e.to_string())),
			XaSessionState::Prepared(s) => self
				.backend
				.rollback_prepared(s)
				.await
				.map_err(|e| TwoPhaseError::RollbackFailed(self.id.clone(), e.to_string())),
		}
	}

	async fn recover(&self) -> Result<Vec<String>, TwoPhaseError> {
		let txns = self
			.backend
			.list_prepared_transactions()
			.await
			.map_err(|e| TwoPhaseError::RecoveryFailed(e.to_string()))?;
		Ok(txns.into_iter().map(|t| t.xid).collect())
	}

	fn status(&self) -> ParticipantStatus {
		self.status.clone()
	}

	fn set_status(&mut self, status: ParticipantStatus) {
		self.status = status;
	}
}

/// Mock participant for testing
#[cfg(test)]
pub struct MockParticipant {
	id: String,
	status: std::sync::Mutex<ParticipantStatus>,
	should_fail_prepare: bool,
	should_fail_commit: bool,
	should_fail_rollback: bool,
}

#[cfg(test)]
impl MockParticipant {
	pub fn new(id: impl Into<String>) -> Self {
		Self {
			id: id.into(),
			status: std::sync::Mutex::new(ParticipantStatus::Active),
			should_fail_prepare: false,
			should_fail_commit: false,
			should_fail_rollback: false,
		}
	}

	pub fn with_prepare_failure(mut self) -> Self {
		self.should_fail_prepare = true;
		self
	}

	pub fn with_commit_failure(mut self) -> Self {
		self.should_fail_commit = true;
		self
	}

	pub fn with_rollback_failure(mut self) -> Self {
		self.should_fail_rollback = true;
		self
	}
}

#[cfg(test)]
#[async_trait(?Send)]
impl TwoPhaseParticipant for MockParticipant {
	fn id(&self) -> &str {
		&self.id
	}

	async fn begin(&self) -> Result<(), TwoPhaseError> {
		Ok(())
	}

	async fn prepare(&self, _xid: String) -> Result<(), TwoPhaseError> {
		if self.should_fail_prepare {
			Err(TwoPhaseError::PrepareFailed(
				self.id.clone(),
				"Simulated prepare failure".to_string(),
			))
		} else {
			*self.status.lock().unwrap() = ParticipantStatus::Prepared;
			Ok(())
		}
	}

	async fn commit(&self, _xid: String) -> Result<(), TwoPhaseError> {
		if self.should_fail_commit {
			Err(TwoPhaseError::CommitFailed(
				self.id.clone(),
				"Simulated commit failure".to_string(),
			))
		} else {
			*self.status.lock().unwrap() = ParticipantStatus::Committed;
			Ok(())
		}
	}

	async fn rollback(&self, _xid: String) -> Result<(), TwoPhaseError> {
		if self.should_fail_rollback {
			Err(TwoPhaseError::RollbackFailed(
				self.id.clone(),
				"Simulated rollback failure".to_string(),
			))
		} else {
			*self.status.lock().unwrap() = ParticipantStatus::Aborted;
			Ok(())
		}
	}

	async fn recover(&self) -> Result<Vec<String>, TwoPhaseError> {
		Ok(Vec::new())
	}

	fn status(&self) -> ParticipantStatus {
		self.status.lock().unwrap().clone()
	}

	fn set_status(&mut self, status: ParticipantStatus) {
		*self.status.lock().unwrap() = status;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_new_transaction() {
		let tpc = TwoPhaseCommit::new("txn_test_001");
		assert_eq!(tpc.transaction_id(), "txn_test_001");
		assert_eq!(tpc.state().unwrap(), TransactionState::NotStarted);
		assert_eq!(tpc.participant_count(), 0);
	}

	#[test]
	fn test_begin_transaction() {
		let mut tpc = TwoPhaseCommit::new("txn_test_002");
		let result = tpc.begin();
		assert!(result.is_ok());
		assert_eq!(tpc.state().unwrap(), TransactionState::Active);
	}

	#[test]
	fn test_cannot_begin_twice() {
		let mut tpc = TwoPhaseCommit::new("txn_test_003");
		tpc.begin().unwrap();
		let result = tpc.begin();
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			TwoPhaseError::InvalidState(_)
		));
	}

	#[test]
	fn test_add_participant() {
		let mut tpc = TwoPhaseCommit::new("txn_test_004");
		tpc.begin().unwrap();
		let result = tpc.add_participant("db1");
		assert!(result.is_ok());
		assert_eq!(tpc.participant_count(), 1);
	}

	#[test]
	fn test_cannot_add_duplicate_participant() {
		let mut tpc = TwoPhaseCommit::new("txn_test_005");
		tpc.begin().unwrap();
		tpc.add_participant("db1").unwrap();
		let result = tpc.add_participant("db1");
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			TwoPhaseError::DuplicateParticipant(_)
		));
	}

	#[test]
	fn test_prepare_phase() {
		let mut tpc = TwoPhaseCommit::new("txn_test_006");
		tpc.begin().unwrap();
		tpc.add_participant("db1").unwrap();
		tpc.add_participant("db2").unwrap();

		let result = tpc.prepare();
		assert!(result.is_ok());
		assert_eq!(tpc.state().unwrap(), TransactionState::Prepared);
		assert!(tpc.all_prepared());

		let sqls = result.unwrap();
		assert_eq!(sqls.len(), 2);
	}

	#[test]
	fn test_prepare_without_participants() {
		let mut tpc = TwoPhaseCommit::new("txn_test_007");
		tpc.begin().unwrap();
		let result = tpc.prepare();
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), TwoPhaseError::NoParticipants));
	}

	#[test]
	fn test_commit_phase() {
		let mut tpc = TwoPhaseCommit::new("txn_test_008");
		tpc.begin().unwrap();
		tpc.add_participant("db1").unwrap();
		tpc.prepare().unwrap();

		let result = tpc.commit();
		assert!(result.is_ok());
		assert_eq!(tpc.state().unwrap(), TransactionState::Committed);

		let sqls = result.unwrap();
		assert_eq!(sqls.len(), 1);
		assert!(sqls[0].contains("COMMIT PREPARED"));
	}

	#[test]
	fn test_cannot_commit_without_prepare() {
		let mut tpc = TwoPhaseCommit::new("txn_test_009");
		tpc.begin().unwrap();
		tpc.add_participant("db1").unwrap();

		let result = tpc.commit();
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			TwoPhaseError::InvalidState(_)
		));
	}

	#[test]
	fn test_rollback_active_transaction() {
		let mut tpc = TwoPhaseCommit::new("txn_test_010");
		tpc.begin().unwrap();
		tpc.add_participant("db1").unwrap();

		let result = tpc.rollback();
		assert!(result.is_ok());
		assert_eq!(tpc.state().unwrap(), TransactionState::Aborted);

		let sqls = result.unwrap();
		assert_eq!(sqls.len(), 1);
		assert!(sqls[0].contains("ROLLBACK"));
	}

	#[test]
	fn test_rollback_prepared_transaction() {
		let mut tpc = TwoPhaseCommit::new("txn_test_011");
		tpc.begin().unwrap();
		tpc.add_participant("db1").unwrap();
		tpc.prepare().unwrap();

		let result = tpc.rollback();
		assert!(result.is_ok());
		assert_eq!(tpc.state().unwrap(), TransactionState::Aborted);

		let sqls = result.unwrap();
		assert_eq!(sqls.len(), 1);
		assert!(sqls[0].contains("ROLLBACK PREPARED"));
	}

	#[test]
	fn test_multiple_participants_prepare_commit() {
		let mut tpc = TwoPhaseCommit::new("txn_test_012");
		tpc.begin().unwrap();
		tpc.add_participant("primary_db").unwrap();
		tpc.add_participant("secondary_db").unwrap();
		tpc.add_participant("cache_db").unwrap();

		assert_eq!(tpc.participant_count(), 3);

		tpc.prepare().unwrap();
		assert!(tpc.all_prepared());

		tpc.commit().unwrap();
		assert_eq!(tpc.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	fn test_participant_new() {
		let participant = Participant::new("test_db");
		assert_eq!(participant.db_alias, "test_db");
		assert_eq!(participant.status, ParticipantStatus::Active);
		assert!(!participant.is_prepared());
	}

	#[test]
	fn test_participant_is_prepared() {
		let mut participant = Participant::new("test_db");
		assert!(!participant.is_prepared());

		participant.status = ParticipantStatus::Prepared;
		assert!(participant.is_prepared());
	}

	#[test]
	fn test_get_participants() {
		let mut tpc = TwoPhaseCommit::new("txn_test_013");
		tpc.begin().unwrap();
		tpc.add_participant("db1").unwrap();
		tpc.add_participant("db2").unwrap();

		let participants = tpc.participants();
		assert_eq!(participants.len(), 2);
		assert!(participants.contains_key("db1"));
		assert!(participants.contains_key("db2"));
	}

	#[test]
	fn test_default_transaction_id() {
		let tpc = TwoPhaseCommit::default();
		assert!(!tpc.transaction_id().is_empty());
		assert_eq!(tpc.state().unwrap(), TransactionState::NotStarted);
	}

	#[test]
	fn test_transaction_state_transitions() {
		let mut tpc = TwoPhaseCommit::new("txn_test_014");
		assert_eq!(tpc.state().unwrap(), TransactionState::NotStarted);

		tpc.begin().unwrap();
		assert_eq!(tpc.state().unwrap(), TransactionState::Active);

		tpc.add_participant("db1").unwrap();
		tpc.prepare().unwrap();
		assert_eq!(tpc.state().unwrap(), TransactionState::Prepared);

		tpc.commit().unwrap();
		assert_eq!(tpc.state().unwrap(), TransactionState::Committed);
	}

	#[test]
	fn test_error_display() {
		let err = TwoPhaseError::PrepareFailed("db1".to_string(), "Connection lost".to_string());
		assert_eq!(err.to_string(), "Prepare failed for 'db1': Connection lost");

		let err = TwoPhaseError::NoParticipants;
		assert_eq!(err.to_string(), "No participants registered");

		let err = TwoPhaseError::DuplicateParticipant("db1".to_string());
		assert_eq!(err.to_string(), "Participant 'db1' already registered");
	}

	#[test]
	fn test_cannot_add_participant_before_begin() {
		let mut tpc = TwoPhaseCommit::new("txn_test_015");
		let result = tpc.add_participant("db1");
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			TwoPhaseError::InvalidState(_)
		));
	}

	// TwoPhaseCoordinator tests
	#[tokio::test]
	async fn test_coordinator_basic_flow() {
		let mut coordinator = TwoPhaseCoordinator::new("coord_txn_001");

		coordinator
			.add_participant(Box::new(MockParticipant::new("db1")))
			.await
			.unwrap();
		coordinator
			.add_participant(Box::new(MockParticipant::new("db2")))
			.await
			.unwrap();

		assert_eq!(coordinator.participant_count().await, 2);

		coordinator.begin().await.unwrap();
		assert_eq!(coordinator.state().await.unwrap(), TransactionState::Active);

		coordinator.prepare().await.unwrap();
		assert_eq!(
			coordinator.state().await.unwrap(),
			TransactionState::Prepared
		);

		coordinator.commit().await.unwrap();
		assert_eq!(
			coordinator.state().await.unwrap(),
			TransactionState::Committed
		);
	}

	#[tokio::test]
	async fn test_coordinator_prepare_failure_triggers_rollback() {
		let mut coordinator = TwoPhaseCoordinator::new("coord_txn_002");

		coordinator
			.add_participant(Box::new(MockParticipant::new("db1")))
			.await
			.unwrap();
		coordinator
			.add_participant(Box::new(MockParticipant::new("db2").with_prepare_failure()))
			.await
			.unwrap();

		coordinator.begin().await.unwrap();

		let result = coordinator.prepare().await;
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			TwoPhaseError::PrepareFailed(_, _)
		));

		// After prepare failure, transaction should be aborted
		assert_eq!(
			coordinator.state().await.unwrap(),
			TransactionState::Aborted
		);
	}

	#[tokio::test]
	async fn test_coordinator_rollback() {
		let mut coordinator = TwoPhaseCoordinator::new("coord_txn_003");

		coordinator
			.add_participant(Box::new(MockParticipant::new("db1")))
			.await
			.unwrap();
		coordinator
			.add_participant(Box::new(MockParticipant::new("db2")))
			.await
			.unwrap();

		coordinator.begin().await.unwrap();
		coordinator.prepare().await.unwrap();

		coordinator.rollback().await.unwrap();
		assert_eq!(
			coordinator.state().await.unwrap(),
			TransactionState::Aborted
		);
	}

	#[tokio::test]
	async fn test_coordinator_commit_failure() {
		let mut coordinator = TwoPhaseCoordinator::new("coord_txn_004");

		coordinator
			.add_participant(Box::new(MockParticipant::new("db1")))
			.await
			.unwrap();
		coordinator
			.add_participant(Box::new(MockParticipant::new("db2").with_commit_failure()))
			.await
			.unwrap();

		coordinator.begin().await.unwrap();
		coordinator.prepare().await.unwrap();

		let result = coordinator.commit().await;
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			TwoPhaseError::CommitFailed(_, _)
		));

		// After commit failure, state should return to Prepared for recovery
		assert_eq!(
			coordinator.state().await.unwrap(),
			TransactionState::Prepared
		);
	}

	#[tokio::test]
	async fn test_coordinator_no_participants() {
		let mut coordinator = TwoPhaseCoordinator::new("coord_txn_005");

		coordinator.begin().await.unwrap();

		let result = coordinator.prepare().await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), TwoPhaseError::NoParticipants));
	}

	#[tokio::test]
	async fn test_coordinator_duplicate_participant() {
		let mut coordinator = TwoPhaseCoordinator::new("coord_txn_006");

		coordinator
			.add_participant(Box::new(MockParticipant::new("db1")))
			.await
			.unwrap();
		let result = coordinator
			.add_participant(Box::new(MockParticipant::new("db1")))
			.await;

		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			TwoPhaseError::DuplicateParticipant(_)
		));
	}

	#[tokio::test]
	async fn test_coordinator_recover() {
		let mut coordinator = TwoPhaseCoordinator::new("coord_txn_007");

		coordinator
			.add_participant(Box::new(MockParticipant::new("db1")))
			.await
			.unwrap();

		let xids = coordinator.recover_prepared_transactions().await.unwrap();
		assert_eq!(xids.len(), 0); // Mock returns empty list
	}

	#[tokio::test]
	async fn test_coordinator_multiple_participants() {
		let mut coordinator = TwoPhaseCoordinator::new("coord_txn_008");

		for i in 1..=5 {
			coordinator
				.add_participant(Box::new(MockParticipant::new(format!("db{}", i))))
				.await
				.unwrap();
		}

		assert_eq!(coordinator.participant_count().await, 5);

		coordinator.begin().await.unwrap();
		coordinator.prepare().await.unwrap();
		coordinator.commit().await.unwrap();

		assert_eq!(
			coordinator.state().await.unwrap(),
			TransactionState::Committed
		);
	}
}
