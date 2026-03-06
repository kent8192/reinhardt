//! Taggable trait definition
//!
//! Trait for models that can be tagged. Implement this trait directly
//! or use the `#[taggable]` attribute macro for auto-generation.

/// Trait for models that can be tagged
///
/// Models implementing this trait can be associated with tags via `TaggedItem`.
/// The trait provides the content type discriminator and object identifier
/// needed for the polymorphic many-to-many relationship.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_taggit::Taggable;
///
/// struct Food {
///     id: Option<i64>,
///     name: String,
/// }
///
/// impl Taggable for Food {
///     fn content_type_name() -> &'static str {
///         "Food"
///     }
///
///     fn object_id(&self) -> i64 {
///         self.id.unwrap_or(0)
///     }
/// }
/// ```
pub trait Taggable {
	/// Returns the content type name used as discriminator in `TaggedItem`
	///
	/// This should be a stable, unique identifier for the model type.
	/// Typically the struct name (e.g., "Food", "Article").
	fn content_type_name() -> &'static str;

	/// Returns the primary key of this instance
	///
	/// Used as `object_id` in `TaggedItem` to identify the specific
	/// instance being tagged.
	fn object_id(&self) -> i64;
}
