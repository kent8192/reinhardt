//! Many-to-Many relationship definition
//!
//! Provides Many-to-Many relationship types for defining bidirectional
//! many-to-many relationships through an intermediate junction table.

use std::marker::PhantomData;

use super::foreign_key::CascadeAction;
use super::reverse::{ReverseRelationship, generate_reverse_accessor};

/// Many-to-Many relationship field
///
/// Represents a many-to-many relationship between two models through
/// an intermediate junction table.
///
/// # Type Parameters
///
/// * `T` - The type of the related model
/// * `K` - The type of the primary key field
///
/// # Examples
///
/// ```
/// use reinhardt_db::associations::ManyToMany;
///
/// #[derive(Clone)]
/// struct Student {
///     id: i64,
///     name: String,
/// }
///
/// #[derive(Clone)]
/// struct Course {
///     id: i64,
///     name: String,
/// }
///
/// // Define many-to-many relationship on Student model
/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
///     .through("student_courses")
///     .source_field("student_id")
///     .target_field("course_id");
/// ```
#[derive(Debug, Clone)]
pub struct ManyToMany<T, K> {
	/// The name of the accessor on the source model
	pub accessor_name: String,
	/// The name of the related accessor on the target model
	pub related_name: Option<String>,
	/// The name of the junction/through table
	pub through: Option<String>,
	/// The name of the foreign key field in the junction table pointing to source
	pub source_field: String,
	/// The name of the foreign key field in the junction table pointing to target
	pub target_field: String,
	/// Action to take when source object is deleted
	pub on_delete: CascadeAction,
	/// Whether to use lazy loading by default
	pub lazy: bool,
	/// Additional fields on the junction table
	pub through_fields: Vec<String>,
	/// Database constraint name prefix
	pub db_constraint_prefix: Option<String>,
	/// Phantom data for type parameters
	_phantom_t: PhantomData<T>,
	_phantom_k: PhantomData<K>,
}

impl<T, K> ManyToMany<T, K> {
	/// Create a new many-to-many relationship
	///
	/// # Arguments
	///
	/// * `accessor_name` - The name of the accessor on the source model
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ManyToMany;
	///
	/// #[derive(Clone)]
	/// struct Course {
	///     id: i64,
	/// }
	///
	/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses");
	/// assert_eq!(rel.accessor_name(), "courses");
	/// ```
	pub fn new(accessor_name: impl Into<String>) -> Self {
		Self {
			accessor_name: accessor_name.into(),
			related_name: None,
			through: None,
			source_field: String::new(),
			target_field: String::new(),
			on_delete: CascadeAction::Cascade,
			lazy: true,
			through_fields: Vec::new(),
			db_constraint_prefix: None,
			_phantom_t: PhantomData,
			_phantom_k: PhantomData,
		}
	}

	/// Set the reverse relation accessor name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ManyToMany;
	///
	/// #[derive(Clone)]
	/// struct Course {
	///     id: i64,
	/// }
	///
	/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
	///     .related_name("students");
	/// assert_eq!(rel.get_related_name(), Some("students"));
	/// ```
	pub fn related_name(mut self, name: impl Into<String>) -> Self {
		self.related_name = Some(name.into());
		self
	}

	/// Set the junction/through table name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ManyToMany;
	///
	/// #[derive(Clone)]
	/// struct Course {
	///     id: i64,
	/// }
	///
	/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
	///     .through("student_courses");
	/// assert_eq!(rel.get_through(), Some("student_courses"));
	/// ```
	pub fn through(mut self, table_name: impl Into<String>) -> Self {
		self.through = Some(table_name.into());
		self
	}

	/// Set the source foreign key field name in the junction table
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ManyToMany;
	///
	/// #[derive(Clone)]
	/// struct Course {
	///     id: i64,
	/// }
	///
	/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
	///     .source_field("student_id");
	/// assert_eq!(rel.get_source_field(), "student_id");
	/// ```
	pub fn source_field(mut self, field_name: impl Into<String>) -> Self {
		self.source_field = field_name.into();
		self
	}

	/// Set the target foreign key field name in the junction table
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ManyToMany;
	///
	/// #[derive(Clone)]
	/// struct Course {
	///     id: i64,
	/// }
	///
	/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
	///     .target_field("course_id");
	/// assert_eq!(rel.get_target_field(), "course_id");
	/// ```
	pub fn target_field(mut self, field_name: impl Into<String>) -> Self {
		self.target_field = field_name.into();
		self
	}

	/// Set the on_delete cascade action for the junction table
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::{ManyToMany, CascadeAction};
	///
	/// #[derive(Clone)]
	/// struct Course {
	///     id: i64,
	/// }
	///
	/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
	///     .on_delete(CascadeAction::Restrict);
	/// assert_eq!(rel.get_on_delete(), CascadeAction::Restrict);
	/// ```
	pub fn on_delete(mut self, action: CascadeAction) -> Self {
		self.on_delete = action;
		self
	}

	/// Set whether to use lazy loading
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ManyToMany;
	///
	/// #[derive(Clone)]
	/// struct Course {
	///     id: i64,
	/// }
	///
	/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
	///     .lazy(false);
	/// assert!(!rel.is_lazy());
	/// ```
	pub fn lazy(mut self, lazy: bool) -> Self {
		self.lazy = lazy;
		self
	}

	/// Add additional fields on the junction table
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ManyToMany;
	///
	/// #[derive(Clone)]
	/// struct Course {
	///     id: i64,
	/// }
	///
	/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
	///     .add_through_field("enrolled_at")
	///     .add_through_field("grade");
	/// assert_eq!(rel.get_through_fields().len(), 2);
	/// ```
	pub fn add_through_field(mut self, field_name: impl Into<String>) -> Self {
		self.through_fields.push(field_name.into());
		self
	}

	/// Set the database constraint name prefix
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::ManyToMany;
	///
	/// #[derive(Clone)]
	/// struct Course {
	///     id: i64,
	/// }
	///
	/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
	///     .db_constraint_prefix("m2m_students_courses");
	/// assert_eq!(rel.get_db_constraint_prefix(), Some("m2m_students_courses"));
	/// ```
	pub fn db_constraint_prefix(mut self, prefix: impl Into<String>) -> Self {
		self.db_constraint_prefix = Some(prefix.into());
		self
	}

	/// Get the accessor name
	pub fn accessor_name(&self) -> &str {
		&self.accessor_name
	}

	/// Get the related_name
	pub fn get_related_name(&self) -> Option<&str> {
		self.related_name.as_deref()
	}

	/// Get the through table name
	pub fn get_through(&self) -> Option<&str> {
		self.through.as_deref()
	}

	/// Get the source field name
	pub fn get_source_field(&self) -> &str {
		&self.source_field
	}

	/// Get the target field name
	pub fn get_target_field(&self) -> &str {
		&self.target_field
	}

	/// Get the on_delete action
	pub fn get_on_delete(&self) -> CascadeAction {
		self.on_delete
	}

	/// Check if lazy loading is enabled
	pub fn is_lazy(&self) -> bool {
		self.lazy
	}

	/// Get additional through fields
	pub fn get_through_fields(&self) -> &[String] {
		&self.through_fields
	}

	/// Get the database constraint prefix
	pub fn get_db_constraint_prefix(&self) -> Option<&str> {
		self.db_constraint_prefix.as_deref()
	}
}

impl<T, K> Default for ManyToMany<T, K> {
	fn default() -> Self {
		Self::new("related_items")
	}
}

impl<T, K> ReverseRelationship for ManyToMany<T, K> {
	/// Get the reverse accessor name, generating one if not explicitly set
	///
	/// For many-to-many relationships, generates a plural accessor name.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::associations::{ManyToMany, ReverseRelationship};
	///
	/// #[derive(Clone)]
	/// struct Course {
	///     id: i64,
	/// }
	///
	/// let rel: ManyToMany<Course, i64> = ManyToMany::new("courses");
	/// assert_eq!(rel.get_or_generate_reverse_name("Student"), "student_set");
	///
	/// let rel_with_name: ManyToMany<Course, i64> = ManyToMany::new("courses")
	///     .related_name("students");
	/// assert_eq!(rel_with_name.get_or_generate_reverse_name("Student"), "students");
	/// ```
	fn get_or_generate_reverse_name(&self, model_name: &str) -> String {
		self.related_name
			.clone()
			.unwrap_or_else(|| generate_reverse_accessor(model_name))
	}

	fn explicit_reverse_name(&self) -> Option<&str> {
		self.related_name.as_deref()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Allow dead_code: test model struct used for trait implementation verification
	#[allow(dead_code)]
	#[derive(Clone)]
	struct Student {
		id: i64,
		name: String,
	}

	// Allow dead_code: test model struct used for trait implementation verification
	#[allow(dead_code)]
	#[derive(Clone)]
	struct Course {
		id: i64,
		title: String,
	}

	#[test]
	fn test_many_to_many_creation() {
		let rel: ManyToMany<Course, i64> = ManyToMany::new("courses");
		assert_eq!(rel.accessor_name(), "courses");
		assert_eq!(rel.get_related_name(), None);
		assert_eq!(rel.get_through(), None);
		assert_eq!(rel.get_source_field(), "");
		assert_eq!(rel.get_target_field(), "");
		assert_eq!(rel.get_on_delete(), CascadeAction::Cascade);
		assert!(rel.is_lazy());
		assert_eq!(rel.get_through_fields().len(), 0);
	}

	#[test]
	fn test_many_to_many_builder() {
		let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
			.related_name("students")
			.through("student_courses")
			.source_field("student_id")
			.target_field("course_id")
			.on_delete(CascadeAction::Restrict)
			.lazy(false)
			.db_constraint_prefix("m2m_sc");

		assert_eq!(rel.accessor_name(), "courses");
		assert_eq!(rel.get_related_name(), Some("students"));
		assert_eq!(rel.get_through(), Some("student_courses"));
		assert_eq!(rel.get_source_field(), "student_id");
		assert_eq!(rel.get_target_field(), "course_id");
		assert_eq!(rel.get_on_delete(), CascadeAction::Restrict);
		assert!(!rel.is_lazy());
		assert_eq!(rel.get_db_constraint_prefix(), Some("m2m_sc"));
	}

	#[test]
	fn test_through_fields() {
		let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
			.add_through_field("enrolled_at")
			.add_through_field("grade")
			.add_through_field("status");

		assert_eq!(rel.get_through_fields().len(), 3);
		assert_eq!(rel.get_through_fields()[0], "enrolled_at");
		assert_eq!(rel.get_through_fields()[1], "grade");
		assert_eq!(rel.get_through_fields()[2], "status");
	}

	#[test]
	fn test_cascade_actions() {
		let actions = vec![
			CascadeAction::NoAction,
			CascadeAction::Restrict,
			CascadeAction::SetNull,
			CascadeAction::SetDefault,
			CascadeAction::Cascade,
		];

		for action in actions {
			let rel: ManyToMany<Course, i64> = ManyToMany::new("courses").on_delete(action);
			assert_eq!(rel.get_on_delete(), action);
		}
	}

	#[test]
	fn test_lazy_loading_configuration() {
		let rel1: ManyToMany<Course, i64> = ManyToMany::new("courses").lazy(true);
		assert!(rel1.is_lazy());

		let rel2: ManyToMany<Course, i64> = ManyToMany::new("courses").lazy(false);
		assert!(!rel2.is_lazy());
	}

	#[test]
	fn test_bidirectional_relationship() {
		// Student -> Courses
		let student_courses: ManyToMany<Course, i64> = ManyToMany::new("courses")
			.related_name("students")
			.through("enrollments")
			.source_field("student_id")
			.target_field("course_id");

		assert_eq!(student_courses.accessor_name(), "courses");
		assert_eq!(student_courses.get_related_name(), Some("students"));

		// The reverse side would be configured on the Course model
		let course_students: ManyToMany<Student, i64> = ManyToMany::new("students")
			.related_name("courses")
			.through("enrollments")
			.source_field("course_id")
			.target_field("student_id");

		assert_eq!(course_students.accessor_name(), "students");
		assert_eq!(course_students.get_related_name(), Some("courses"));
	}

	#[test]
	fn test_self_referential_relationship() {
		// Users who follow other users
		let followers: ManyToMany<Student, i64> = ManyToMany::new("followers")
			.related_name("following")
			.through("user_follows")
			.source_field("following_id")
			.target_field("follower_id");

		assert_eq!(followers.accessor_name(), "followers");
		assert_eq!(followers.get_related_name(), Some("following"));
		assert_eq!(followers.get_through(), Some("user_follows"));
	}

	#[test]
	fn test_with_additional_data() {
		// Many-to-many with timestamp and other metadata
		let rel: ManyToMany<Course, i64> = ManyToMany::new("courses")
			.through("enrollments")
			.add_through_field("enrolled_at")
			.add_through_field("grade")
			.add_through_field("completed")
			.add_through_field("notes");

		assert_eq!(rel.get_through_fields().len(), 4);
		assert!(
			rel.get_through_fields()
				.contains(&"enrolled_at".to_string())
		);
		assert!(rel.get_through_fields().contains(&"grade".to_string()));
		assert!(rel.get_through_fields().contains(&"completed".to_string()));
		assert!(rel.get_through_fields().contains(&"notes".to_string()));
	}
}
