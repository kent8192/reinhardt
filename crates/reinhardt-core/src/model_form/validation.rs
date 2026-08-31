//! Validation contracts for generated model form payloads.

use crate::validators::ValidationErrors;

/// A normalized model form payload that can recover its original raw payload.
pub trait ModelFormCleanedPayload: Sized {
	/// The corresponding raw payload type.
	type Raw;

	/// Convert this normalized payload back into its raw representation.
	fn into_raw(self) -> Self::Raw;
}

/// A raw model form payload that can be normalized and validated.
pub trait ModelFormValidatingPayload: Sized {
	/// The normalized payload produced after successful validation.
	type Cleaned: ModelFormCleanedPayload<Raw = Self>;

	/// Normalize and validate this raw payload.
	fn clean_and_validate(self) -> Result<Self::Cleaned, ValidationErrors>;
}

#[cfg(test)]
mod tests {
	use super::*;

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

	#[test]
	fn cleaned_payload_returns_its_raw_payload() {
		let cleaned = Raw("name".to_string()).clean_and_validate().unwrap();

		assert_eq!(cleaned.into_raw(), Raw("name".to_string()));
	}
}
