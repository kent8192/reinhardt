//! Lock poisoning recovery utilities
//!
//! Provides helper functions to recover from poisoned `Mutex` and `RwLock` guards.
//! When a thread panics while holding a lock, the lock becomes "poisoned" in Rust's
//! standard library. These utilities allow recovering the guard with a warning log,
//! rather than propagating the panic to all subsequent lock users.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Errors from lock recovery operations
#[derive(Debug, thiserror::Error)]
pub enum LockRecoveryError {
	/// Lock was poisoned and recovery yielded the guard
	#[error("Lock was poisoned: {0}")]
	Poisoned(String),

	/// Lock acquisition timed out
	#[error("Lock acquisition timed out")]
	Timeout,
}

/// Recover a read guard from a potentially poisoned `RwLock`.
///
/// If the lock is poisoned (a previous holder panicked), this function
/// recovers the guard and logs a warning. The data may be in an
/// inconsistent state.
///
/// # Examples
///
/// ```
/// use std::sync::RwLock;
/// use reinhardt_utils::utils_core::lock_recovery::recover_rwlock_read;
///
/// let lock = RwLock::new(42);
/// let guard = recover_rwlock_read(&lock).unwrap();
/// assert_eq!(*guard, 42);
/// ```
pub fn recover_rwlock_read<T>(
	lock: &RwLock<T>,
) -> Result<RwLockReadGuard<'_, T>, LockRecoveryError> {
	match lock.read() {
		Ok(guard) => Ok(guard),
		Err(poison_err) => {
			tracing::warn!(
				"RwLock read guard recovered from poisoned state. \
				 Data may be inconsistent."
			);
			Ok(poison_err.into_inner())
		}
	}
}

/// Recover a write guard from a potentially poisoned `RwLock`.
///
/// If the lock is poisoned, this function recovers the guard and logs
/// a warning. The caller should verify data consistency.
///
/// # Examples
///
/// ```
/// use std::sync::RwLock;
/// use reinhardt_utils::utils_core::lock_recovery::recover_rwlock_write;
///
/// let lock = RwLock::new(42);
/// let mut guard = recover_rwlock_write(&lock).unwrap();
/// *guard = 100;
/// assert_eq!(*guard, 100);
/// ```
pub fn recover_rwlock_write<T>(
	lock: &RwLock<T>,
) -> Result<RwLockWriteGuard<'_, T>, LockRecoveryError> {
	match lock.write() {
		Ok(guard) => Ok(guard),
		Err(poison_err) => {
			tracing::warn!(
				"RwLock write guard recovered from poisoned state. \
				 Data may be inconsistent."
			);
			Ok(poison_err.into_inner())
		}
	}
}

/// Recover a guard from a potentially poisoned `Mutex`.
///
/// If the mutex is poisoned, this function recovers the guard and
/// logs a warning. The caller should verify data consistency.
///
/// # Examples
///
/// ```
/// use std::sync::Mutex;
/// use reinhardt_utils::utils_core::lock_recovery::recover_mutex;
///
/// let lock = Mutex::new(42);
/// let guard = recover_mutex(&lock).unwrap();
/// assert_eq!(*guard, 42);
/// ```
pub fn recover_mutex<T>(lock: &Mutex<T>) -> Result<MutexGuard<'_, T>, LockRecoveryError> {
	match lock.lock() {
		Ok(guard) => Ok(guard),
		Err(poison_err) => {
			tracing::warn!(
				"Mutex guard recovered from poisoned state. \
				 Data may be inconsistent."
			);
			Ok(poison_err.into_inner())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::sync::Arc;
	use std::thread;

	#[rstest]
	fn test_recover_rwlock_read_normal() {
		// Arrange
		let lock = RwLock::new(42);

		// Act
		let guard = recover_rwlock_read(&lock);

		// Assert
		assert!(guard.is_ok());
		assert_eq!(*guard.unwrap(), 42);
	}

	#[rstest]
	fn test_recover_rwlock_write_normal() {
		// Arrange
		let lock = RwLock::new(42);

		// Act
		let guard = recover_rwlock_write(&lock);

		// Assert
		assert!(guard.is_ok());
		assert_eq!(*guard.unwrap(), 42);
	}

	#[rstest]
	fn test_recover_mutex_normal() {
		// Arrange
		let lock = Mutex::new(42);

		// Act
		let guard = recover_mutex(&lock);

		// Assert
		assert!(guard.is_ok());
		assert_eq!(*guard.unwrap(), 42);
	}

	#[rstest]
	fn test_recover_mutex_from_poisoned() {
		// Arrange
		let lock = Arc::new(Mutex::new(42));
		let lock_clone = Arc::clone(&lock);

		// Poison the mutex by panicking while holding it
		let _ = thread::spawn(move || {
			let _guard = lock_clone.lock().unwrap();
			panic!("intentional panic to poison the lock");
		})
		.join();

		// Act - the lock is now poisoned
		assert!(lock.lock().is_err(), "Lock should be poisoned");
		let result = recover_mutex(&lock);

		// Assert
		assert!(result.is_ok(), "Should recover from poisoned mutex");
		assert_eq!(*result.unwrap(), 42);
	}

	#[rstest]
	fn test_recover_rwlock_read_from_poisoned() {
		// Arrange
		let lock = Arc::new(RwLock::new(42));
		let lock_clone = Arc::clone(&lock);

		// Poison the rwlock by panicking while holding write guard
		let _ = thread::spawn(move || {
			let _guard = lock_clone.write().unwrap();
			panic!("intentional panic to poison the lock");
		})
		.join();

		// Act
		assert!(lock.read().is_err(), "Lock should be poisoned");
		let result = recover_rwlock_read(&lock);

		// Assert
		assert!(result.is_ok(), "Should recover from poisoned RwLock");
		assert_eq!(*result.unwrap(), 42);
	}

	#[rstest]
	fn test_recover_rwlock_write_from_poisoned() {
		// Arrange
		let lock = Arc::new(RwLock::new(42));
		let lock_clone = Arc::clone(&lock);

		// Poison the rwlock by panicking while holding write guard
		let _ = thread::spawn(move || {
			let _guard = lock_clone.write().unwrap();
			panic!("intentional panic to poison the lock");
		})
		.join();

		// Act
		assert!(lock.write().is_err(), "Lock should be poisoned");
		let result = recover_rwlock_write(&lock);

		// Assert
		assert!(result.is_ok(), "Should recover from poisoned RwLock");
		assert_eq!(*result.unwrap(), 42);
	}
}
