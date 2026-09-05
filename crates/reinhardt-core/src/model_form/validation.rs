//! Validation contracts for generated model form payloads.

use crate::validators::ValidationErrors;

/// A normalized model form payload that can recover its original raw payload.
///
/// This target-neutral P2 contract is available with equivalent payload
/// semantics on native and WASM targets.
pub trait ModelFormCleanedPayload: Sized {
	/// The corresponding raw payload type.
	///
	/// **Parity: P2.** Native and WASM targets expose the same generated raw
	/// payload representation.
	type Raw;

	/// Convert this normalized payload back into its raw representation.
	///
	/// **Parity: P2.** Available with equivalent payload semantics on native and WASM targets.
	fn into_raw(self) -> Self::Raw;
}

/// A raw model form payload that can be normalized and validated for creation.
///
/// This target-neutral P2 contract performs the same generated field and
/// synchronous application validation on native and WASM targets.
pub trait ModelFormValidatingPayload: Sized {
	/// The normalized payload produced after successful validation.
	///
	/// **Parity: P2.** Native and WASM targets expose the same generated cleaned
	/// payload representation.
	type Cleaned: ModelFormCleanedPayload<Raw = Self>;

	/// Normalize and validate this raw payload for model creation.
	///
	/// **Parity: P2.** Runs equivalent generated validation on native and WASM targets.
	fn clean_and_validate(self) -> Result<Self::Cleaned, ValidationErrors>;

	/// Normalize and validate while deferring required file fields to the
	/// multipart boundary.
	///
	/// **Parity: P2.** Generated native and WASM payloads expose the same
	/// snapshot-validation hook; other implementations retain strict validation.
	/// Generated implementations reject names outside required file or image descriptors.
	#[doc(hidden)]
	fn clean_and_validate_with_deferred_required_fields(
		self,
		_deferred_fields: &[&str],
	) -> Result<Self::Cleaned, ValidationErrors> {
		self.clean_and_validate()
	}

	/// Normalizes and validates while deferring one required relationship identifier.
	///
	/// **Parity: P0.** Native inline formsets override this hidden compatibility
	/// hook; other implementations retain strict create validation.
	/// Generated native payloads include required server-owned relationships
	/// excluded from the public schema, using the model's trusted relationship metadata.
	#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
	#[doc(hidden)]
	fn clean_and_validate_with_deferred_required_field(
		self,
		_deferred_field: &str,
	) -> Result<Self::Cleaned, ValidationErrors> {
		self.clean_and_validate()
	}
}

/// A raw model form payload that can validate a partial model update.
///
/// This target-neutral P2 contract merges omitted values from the existing
/// model for synchronous validation on both native and WASM targets. The
/// returned cleaned payload remains partial so applying it preserves omissions.
pub trait ModelFormUpdatingPayload: ModelFormValidatingPayload {
	/// The model type whose existing values complete an update candidate.
	///
	/// **Parity: P2.** Native and WASM targets use the same generated model shape
	/// when merging values for update validation.
	type Model;

	/// Normalize a partial update and validate its post-merge model values.
	///
	/// **Parity: P2.** Merges and validates equivalent candidate values on native and WASM targets.
	fn clean_and_validate_for_update(
		self,
		existing: &Self::Model,
	) -> Result<Self::Cleaned, ValidationErrors>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[derive(Debug, PartialEq)]
	struct Raw(String);

	#[derive(Debug, PartialEq)]
	struct Cleaned(Raw);

	impl ModelFormCleanedPayload for Cleaned {
		type Raw = Raw;

		fn into_raw(self) -> Raw {
			self.0
		}
	}

	impl ModelFormValidatingPayload for Raw {
		type Cleaned = Cleaned;

		fn clean_and_validate(self) -> Result<Self::Cleaned, ValidationErrors> {
			Ok(Cleaned(self))
		}
	}

	#[rstest]
	fn cleaned_payload_returns_its_raw_payload() {
		let cleaned = Raw("name".to_string()).clean_and_validate().unwrap();

		assert_eq!(cleaned.into_raw(), Raw("name".to_string()));
	}
}
