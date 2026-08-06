//! Model and Field Inspection
//!
//! This module provides introspection capabilities for examining model metadata,
//! field definitions, relationships, indexes, and constraints at runtime.

use super::constraints::{CheckConstraint, Constraint, ForeignKeyConstraint, UniqueConstraint};
use super::field_codec::{DatabaseStorageKind, FieldDomain};
use super::fields::{Field, FieldKwarg};
use super::indexes::Index;
use super::relationship::RelationshipType;
use crate::orm::Model;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Field information extracted from inspection
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInfo {
	/// Field name
	pub name: String,
	/// Field type path (e.g., "reinhardt.orm.models.CharField")
	pub field_type: String,
	/// Physical database storage used by this field, when known.
	pub storage_kind: Option<DatabaseStorageKind>,
	/// Structured value constraints associated with this field.
	pub domain: Option<FieldDomain>,
	/// Is this field nullable?
	pub nullable: bool,
	/// Is this field the primary key?
	pub primary_key: bool,
	/// Is this field unique?
	pub unique: bool,
	/// Is this field blank allowed?
	pub blank: bool,
	/// Is this field editable?
	pub editable: bool,
	/// Default value if any
	pub default: Option<FieldKwarg>,
	/// Database default value if any
	pub db_default: Option<FieldKwarg>,
	/// Database column name (if different from field name)
	pub db_column: Option<String>,
	/// Field choices if any
	pub choices: Option<Vec<(String, String)>>,
	/// Additional field-specific attributes
	pub attributes: HashMap<String, FieldKwarg>,
}

impl FieldInfo {
	/// Create a new FieldInfo from a Field trait object
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::FieldInfo;
	///
	/// let mut field = CharField::new(100);
	/// field.set_attributes_from_name("username");
	/// let field_info = FieldInfo::from_field(&field);
	///
	/// assert_eq!(field_info.name, "username");
	/// assert_eq!(field_info.field_type, "reinhardt.orm.models.CharField");
	/// assert!(!field_info.nullable);
	/// ```
	pub fn from_field<F: Field>(field: &F) -> Self {
		let deconstruction = field.deconstruct();
		let mut attributes = HashMap::new();

		for (key, value) in deconstruction.kwargs.iter() {
			if !matches!(
				key.as_str(),
				"null"
					| "blank" | "primary_key"
					| "unique" | "editable"
					| "default" | "db_default"
					| "db_column" | "choices"
			) {
				attributes.insert(key.clone(), value.clone());
			}
		}

		Self {
			name: deconstruction.name.unwrap_or_else(|| "unknown".to_string()),
			field_type: deconstruction.path,
			storage_kind: None,
			domain: None,
			nullable: deconstruction
				.kwargs
				.get("null")
				.and_then(|v| match v {
					FieldKwarg::Bool(b) => Some(*b),
					_ => None,
				})
				.unwrap_or(false),
			primary_key: field.is_primary_key(),
			unique: deconstruction
				.kwargs
				.get("unique")
				.and_then(|v| match v {
					FieldKwarg::Bool(b) => Some(*b),
					_ => None,
				})
				.unwrap_or(false),
			blank: deconstruction
				.kwargs
				.get("blank")
				.and_then(|v| match v {
					FieldKwarg::Bool(b) => Some(*b),
					_ => None,
				})
				.unwrap_or(false),
			editable: deconstruction
				.kwargs
				.get("editable")
				.and_then(|v| match v {
					FieldKwarg::Bool(b) => Some(*b),
					_ => None,
				})
				.unwrap_or(true),
			default: deconstruction.kwargs.get("default").cloned(),
			db_default: deconstruction.kwargs.get("db_default").cloned(),
			db_column: deconstruction
				.kwargs
				.get("db_column")
				.and_then(|v| match v {
					FieldKwarg::String(s) => Some(s.clone()),
					_ => None,
				}),
			choices: deconstruction.kwargs.get("choices").and_then(|v| match v {
				FieldKwarg::Choices(c) => Some(c.clone()),
				_ => None,
			}),
			attributes,
		}
	}

	/// Get the database column name (db_column or field name)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::FieldInfo;
	///
	/// let mut field = CharField::new(100);
	/// field.set_attributes_from_name("username");
	/// let field_info = FieldInfo::from_field(&field);
	///
	/// assert_eq!(field_info.db_column_name(), "username");
	/// ```
	pub fn db_column_name(&self) -> &str {
		self.db_column.as_deref().unwrap_or(&self.name)
	}

	/// Check if this field has choices defined
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::FieldInfo;
	///
	/// let mut field = CharField::with_choices(
	///     10,
	///     vec![("A".to_string(), "Active".to_string())],
	/// );
	/// field.set_attributes_from_name("status");
	/// let field_info = FieldInfo::from_field(&field);
	///
	/// assert!(field_info.has_choices());
	/// ```
	pub fn has_choices(&self) -> bool {
		self.choices.is_some()
	}
}

/// Returns the inspection field path for a typed database storage kind.
#[doc(hidden)]
pub fn database_field_type_path(storage_kind: DatabaseStorageKind) -> &'static str {
	match storage_kind {
		DatabaseStorageKind::Bool => "reinhardt.orm.models.BooleanField",
		DatabaseStorageKind::I32 => "reinhardt.orm.models.IntegerField",
		DatabaseStorageKind::I64 => "reinhardt.orm.models.BigIntegerField",
		DatabaseStorageKind::F32 | DatabaseStorageKind::F64 => "reinhardt.orm.models.FloatField",
		DatabaseStorageKind::Decimal => "reinhardt.orm.models.DecimalField",
		DatabaseStorageKind::String => "reinhardt.orm.models.CharField",
		DatabaseStorageKind::Bytes => "reinhardt.orm.models.BinaryField",
		DatabaseStorageKind::Json => "reinhardt.orm.models.JsonField",
		#[cfg(feature = "pgvector")]
		DatabaseStorageKind::Vector(_) => "reinhardt.orm.models.VectorField",
		DatabaseStorageKind::Uuid => "reinhardt.orm.models.UuidField",
		DatabaseStorageKind::Date => "reinhardt.orm.models.DateField",
		DatabaseStorageKind::Time => "reinhardt.orm.models.TimeField",
		DatabaseStorageKind::DateTime => "reinhardt.orm.models.DateTimeField",
		DatabaseStorageKind::NaiveDateTime => "reinhardt.orm.models.DateTimeField",
	}
}

/// Maps typed database storage metadata into migration field metadata.
#[cfg(feature = "migrations")]
#[doc(hidden)]
pub fn database_storage_field_type(
	storage_kind: DatabaseStorageKind,
	max_length: Option<u32>,
) -> crate::migrations::FieldType {
	use crate::migrations::FieldType;

	match storage_kind {
		DatabaseStorageKind::Bool => FieldType::Boolean,
		DatabaseStorageKind::I32 => FieldType::Integer,
		DatabaseStorageKind::I64 => FieldType::BigInteger,
		DatabaseStorageKind::F32 => FieldType::Float,
		DatabaseStorageKind::F64 => FieldType::Double,
		DatabaseStorageKind::Decimal => FieldType::Decimal {
			precision: 38,
			scale: 10,
		},
		DatabaseStorageKind::String => FieldType::VarChar(
			max_length.expect("string database fields require max_length attribute"),
		),
		DatabaseStorageKind::Bytes => FieldType::Binary,
		DatabaseStorageKind::Json => FieldType::Jsonb,
		#[cfg(feature = "pgvector")]
		DatabaseStorageKind::Vector(dimensions) => FieldType::Vector { dimensions },
		DatabaseStorageKind::Uuid => FieldType::Uuid,
		DatabaseStorageKind::Date => FieldType::Date,
		DatabaseStorageKind::Time => FieldType::Time,
		DatabaseStorageKind::DateTime => FieldType::TimestampTz,
		DatabaseStorageKind::NaiveDateTime => FieldType::DateTime,
	}
}

/// Relationship information extracted from inspection
#[derive(Debug, Clone, PartialEq)]
pub struct RelationInfo {
	/// Relationship name
	pub name: String,
	/// Type of relationship
	pub relationship_type: RelationshipType,
	/// Foreign key field name
	pub foreign_key: Option<String>,
	/// Related model name
	pub related_model: String,
	/// Back reference name
	pub back_populates: Option<String>,
	/// Through table name for ManyToMany relationships
	pub through_table: Option<String>,
	/// Source field name in through table (for ManyToMany)
	pub source_field: Option<String>,
	/// Target field name in through table (for ManyToMany)
	pub target_field: Option<String>,
}

impl RelationInfo {
	/// Create a new RelationInfo
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::inspection::RelationInfo;
	/// use reinhardt_db::orm::relationship::RelationshipType;
	///
	/// let info = RelationInfo::new(
	///     "posts",
	///     RelationshipType::OneToMany,
	///     "Post",
	/// );
	///
	/// assert_eq!(info.name, "posts");
	/// assert_eq!(info.relationship_type, RelationshipType::OneToMany);
	/// ```
	pub fn new(
		name: impl Into<String>,
		relationship_type: RelationshipType,
		related_model: impl Into<String>,
	) -> Self {
		Self {
			name: name.into(),
			relationship_type,
			foreign_key: None,
			related_model: related_model.into(),
			back_populates: None,
			through_table: None,
			source_field: None,
			target_field: None,
		}
	}

	/// Set the foreign key field name
	pub fn with_foreign_key(mut self, foreign_key: impl Into<String>) -> Self {
		self.foreign_key = Some(foreign_key.into());
		self
	}

	/// Set the back reference name
	pub fn with_back_populates(mut self, back_populates: impl Into<String>) -> Self {
		self.back_populates = Some(back_populates.into());
		self
	}

	/// Set the through table name for ManyToMany relationships
	pub fn with_through_table(mut self, through_table: impl Into<String>) -> Self {
		self.through_table = Some(through_table.into());
		self
	}

	/// Set the source field name in the through table
	pub fn with_source_field(mut self, source_field: impl Into<String>) -> Self {
		self.source_field = Some(source_field.into());
		self
	}

	/// Set the target field name in the through table
	pub fn with_target_field(mut self, target_field: impl Into<String>) -> Self {
		self.target_field = Some(target_field.into());
		self
	}
}

/// Index information extracted from inspection
#[cfg(feature = "pgvector")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexMetadataType {
	/// HNSW approximate vector index.
	Hnsw {
		/// Maximum number of connections per layer.
		m: Option<u16>,
		/// Candidate list size used while constructing the index.
		ef_construction: Option<u16>,
	},
	/// IVFFlat approximate vector index.
	Ivfflat {
		/// Number of inverted lists.
		lists: Option<u32>,
	},
}

/// Index information extracted from inspection
#[derive(Debug, Clone, PartialEq)]
pub struct IndexInfo {
	/// Index name
	pub name: String,
	/// Fields included in the index
	pub fields: Vec<String>,
	/// Is this a unique index?
	pub unique: bool,
	/// Partial index condition
	pub condition: Option<String>,
	/// Typed index method and options.
	#[cfg(feature = "pgvector")]
	pub index_type: Option<IndexMetadataType>,
	/// PostgreSQL operator class.
	#[cfg(feature = "pgvector")]
	pub operator_class: Option<String>,
	/// Index expressions.
	#[cfg(feature = "pgvector")]
	pub expressions: Option<Vec<String>>,
}

impl IndexInfo {
	/// Creates index metadata from its feature-independent fields.
	pub fn new(
		name: impl Into<String>,
		fields: Vec<String>,
		unique: bool,
		condition: Option<String>,
	) -> Self {
		Self {
			name: name.into(),
			fields,
			unique,
			condition,
			#[cfg(feature = "pgvector")]
			index_type: None,
			#[cfg(feature = "pgvector")]
			operator_class: None,
			#[cfg(feature = "pgvector")]
			expressions: None,
		}
	}

	/// Create IndexInfo from an Index
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::indexes::Index;
	/// use reinhardt_db::orm::inspection::IndexInfo;
	///
	/// let index = Index::new("email_idx", vec!["email".to_string()]);
	/// let info = IndexInfo::from_index(&index);
	///
	/// assert_eq!(info.name, "email_idx");
	/// assert_eq!(info.fields.len(), 1);
	/// assert!(!info.unique);
	/// ```
	pub fn from_index(index: &Index) -> Self {
		Self::new(
			index.name.clone(),
			index.fields.clone(),
			index.unique,
			index.condition.clone(),
		)
	}
}

/// Constraint information extracted from inspection
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintInfo {
	/// Constraint name
	pub name: String,
	/// Type of constraint
	pub constraint_type: ConstraintType,
	/// SQL definition
	pub definition: String,
	/// Logical model fields that participate in the constraint
	pub fields: Vec<String>,
	/// Optional predicate for a partial constraint
	pub condition: Option<String>,
	/// Whether constraint enforcement can be deferred
	pub deferrable: bool,
	/// Whether multiple NULL values are considered distinct
	pub nulls_distinct: Option<bool>,
}

/// Type of database constraint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintType {
	/// CHECK constraint
	Check,
	/// UNIQUE constraint
	Unique,
	/// FOREIGN KEY constraint
	ForeignKey,
}

impl ConstraintInfo {
	/// Create ConstraintInfo from a CheckConstraint
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::constraints::CheckConstraint;
	/// use reinhardt_db::orm::inspection::{ConstraintInfo, ConstraintType};
	///
	/// let constraint = CheckConstraint::new("age_check", "age >= 18");
	/// let info = ConstraintInfo::from_check(&constraint);
	///
	/// assert_eq!(info.name, "age_check");
	/// assert_eq!(info.constraint_type, ConstraintType::Check);
	/// ```
	pub fn from_check(constraint: &CheckConstraint) -> Self {
		Self {
			name: constraint.name.clone(),
			constraint_type: ConstraintType::Check,
			definition: constraint.to_sql(),
			fields: Vec::new(),
			condition: None,
			deferrable: false,
			nulls_distinct: None,
		}
	}

	/// Create ConstraintInfo from a UniqueConstraint
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::constraints::UniqueConstraint;
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::{ConstraintInfo, ConstraintType, FieldInfo};
	///
	/// let mut email = CharField::new(255);
	/// email.base.db_column = Some("email_addr".to_string());
	/// email.set_attributes_from_name("email");
	/// let model_fields = vec![FieldInfo::from_field(&email)];
	/// let constraint = UniqueConstraint::new("email_unique", vec!["email_addr".to_string()]);
	/// let info = ConstraintInfo::from_unique(&constraint, &model_fields);
	///
	/// assert_eq!(info.name, "email_unique");
	/// assert_eq!(info.constraint_type, ConstraintType::Unique);
	/// assert_eq!(info.fields, vec!["email"]);
	/// ```
	pub fn from_unique(constraint: &UniqueConstraint, model_fields: &[FieldInfo]) -> Self {
		let fields = constraint
			.fields
			.iter()
			.map(|column| {
				model_fields
					.iter()
					.find(|field| field.db_column_name() == column)
					.map_or_else(|| column.clone(), |field| field.name.clone())
			})
			.collect();
		Self {
			name: constraint.name.clone(),
			constraint_type: ConstraintType::Unique,
			definition: constraint.to_sql(),
			fields,
			condition: constraint.condition.clone(),
			deferrable: false,
			nulls_distinct: None,
		}
	}

	/// Create ConstraintInfo from a ForeignKeyConstraint
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::constraints::{ForeignKeyConstraint, OnDelete, OnUpdate};
	/// use reinhardt_db::orm::inspection::{ConstraintInfo, ConstraintType};
	///
	/// let constraint = ForeignKeyConstraint {
	///     name: "fk_user".to_string(),
	///     field: "user_id".to_string(),
	///     references_table: "users".to_string(),
	///     references_field: "id".to_string(),
	///     on_delete: OnDelete::Cascade,
	///     on_update: OnUpdate::NoAction,
	/// };
	/// let info = ConstraintInfo::from_foreign_key(&constraint);
	///
	/// assert_eq!(info.name, "fk_user");
	/// assert_eq!(info.constraint_type, ConstraintType::ForeignKey);
	/// ```
	pub fn from_foreign_key(constraint: &ForeignKeyConstraint) -> Self {
		Self {
			name: constraint.name.clone(),
			constraint_type: ConstraintType::ForeignKey,
			definition: constraint.to_sql(),
			fields: Vec::new(),
			condition: None,
			deferrable: false,
			nulls_distinct: None,
		}
	}
}

/// Inspector for model metadata
///
/// Provides methods to inspect field definitions, relationships,
/// indexes, and constraints of a model at runtime.
pub struct ModelInspector<M: Model> {
	_phantom: PhantomData<M>,
}

impl<M: Model> ModelInspector<M> {
	/// Create a new ModelInspector
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::inspection::ModelInspector;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Deserialize, Serialize};
	///
	/// #[derive(Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: i32,
	///     username: String,
	/// }
	///
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// impl Model for User {
	///     type PrimaryKey = i32;
	///     type Fields = UserFields;
	///     type Objects = reinhardt_db::orm::Manager<Self>;
	///
	///     fn table_name() -> &'static str {
	///         "users"
	///     }
	///
	///     fn new_fields() -> Self::Fields {
	///         UserFields
	///     }
	///
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         Some(self.id)
	///     }
	///
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.id = value;
	///     }
	/// }
	///
	/// let inspector = ModelInspector::<User>::new();
	/// assert_eq!(inspector.table_name(), "users");
	/// ```
	pub fn new() -> Self {
		Self {
			_phantom: PhantomData,
		}
	}

	/// Get the model's table name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::inspection::ModelInspector;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Deserialize, Serialize};
	///
	/// #[derive(Clone, Serialize, Deserialize)]
	/// struct Post {
	///     id: i64,
	/// }
	///
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// impl Model for Post {
	///     type PrimaryKey = i64;
	///     type Fields = PostFields;
	///     type Objects = reinhardt_db::orm::Manager<Self>;
	///
	///     fn table_name() -> &'static str {
	///         "blog_posts"
	///     }
	///
	///     fn new_fields() -> Self::Fields {
	///         PostFields
	///     }
	///
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         Some(self.id)
	///     }
	///
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.id = value;
	///     }
	/// }
	///
	/// let inspector = ModelInspector::<Post>::new();
	/// assert_eq!(inspector.table_name(), "blog_posts");
	/// ```
	pub fn table_name(&self) -> &'static str {
		M::table_name()
	}

	/// Get the model's primary key field name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::inspection::ModelInspector;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Deserialize, Serialize};
	///
	/// #[derive(Clone, Serialize, Deserialize)]
	/// struct Article {
	///     article_id: u32,
	/// }
	///
	/// # #[derive(Clone)]
	/// # struct ArticleFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for ArticleFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// impl Model for Article {
	///     type PrimaryKey = u32;
	///     type Fields = ArticleFields;
	///     type Objects = reinhardt_db::orm::Manager<Self>;
	///
	///     fn table_name() -> &'static str {
	///         "articles"
	///     }
	///
	///     fn primary_key_field() -> &'static str {
	///         "article_id"
	///     }
	///
	///     fn new_fields() -> Self::Fields {
	///         ArticleFields
	///     }
	///
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         Some(self.article_id)
	///     }
	///
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.article_id = value;
	///     }
	/// }
	///
	/// let inspector = ModelInspector::<Article>::new();
	/// assert_eq!(inspector.primary_key_field(), "article_id");
	/// ```
	pub fn primary_key_field(&self) -> &'static str {
		M::primary_key_field()
	}

	/// Get all field information from the model
	///
	/// Returns field metadata provided by the Model implementation.
	/// By default, returns an empty vector unless the model provides
	/// field metadata through the `field_metadata()` method.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::inspection::ModelInspector;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Deserialize, Serialize};
	///
	/// #[derive(Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: i32,
	///     name: String,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::model::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i32;
	///     type Fields = UserFields;
	///     type Objects = reinhardt_db::orm::Manager<Self>;
	///
	///     fn table_name() -> &'static str {
	///         "users"
	///     }
	///
	///     fn new_fields() -> Self::Fields {
	///         UserFields
	///     }
	///
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         Some(self.id)
	///     }
	///
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.id = value;
	///     }
	/// }
	///
	/// let inspector = ModelInspector::<User>::new();
	/// let fields = inspector.get_fields();
	/// // Returns empty by default, override field_metadata() to provide fields
	/// assert_eq!(fields.len(), 0);
	/// ```
	pub fn get_fields(&self) -> Vec<FieldInfo> {
		M::field_metadata()
	}

	/// Get information about a specific field by name
	///
	/// Returns None if the field does not exist.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::inspection::ModelInspector;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Deserialize, Serialize};
	///
	/// #[derive(Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: i32,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::model::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i32;
	///     type Fields = UserFields;
	///     type Objects = reinhardt_db::orm::Manager<Self>;
	///
	///     fn table_name() -> &'static str {
	///         "users"
	///     }
	///
	///     fn new_fields() -> Self::Fields {
	///         UserFields
	///     }
	///
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         Some(self.id)
	///     }
	///
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.id = value;
	///     }
	/// }
	///
	/// let inspector = ModelInspector::<User>::new();
	/// let field = inspector.get_field("nonexistent");
	/// assert!(field.is_none());
	/// ```
	pub fn get_field(&self, name: &str) -> Option<FieldInfo> {
		self.get_fields().into_iter().find(|f| f.name == name)
	}

	/// Get all relationship information from the model
	///
	/// Returns relationship metadata provided by the Model implementation.
	/// By default, returns an empty vector unless the model provides
	/// relationship metadata through the `relationship_metadata()` method.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::inspection::ModelInspector;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Deserialize, Serialize};
	///
	/// #[derive(Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: i32,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::model::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i32;
	///     type Fields = UserFields;
	///     type Objects = reinhardt_db::orm::Manager<Self>;
	///
	///     fn table_name() -> &'static str {
	///         "users"
	///     }
	///
	///     fn new_fields() -> Self::Fields {
	///         UserFields
	///     }
	///
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         Some(self.id)
	///     }
	///
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.id = value;
	///     }
	/// }
	///
	/// let inspector = ModelInspector::<User>::new();
	/// let relations = inspector.get_relations();
	/// // Returns empty by default, override relationship_metadata() to provide relations
	/// assert_eq!(relations.len(), 0);
	/// ```
	pub fn get_relations(&self) -> Vec<RelationInfo> {
		M::relationship_metadata()
	}

	/// Get all index information from the model
	///
	/// Returns index metadata provided by the Model implementation.
	/// By default, returns an empty vector unless the model provides
	/// index metadata through the `index_metadata()` method.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::inspection::ModelInspector;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Deserialize, Serialize};
	///
	/// #[derive(Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: i32,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::model::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i32;
	///     type Fields = UserFields;
	///     type Objects = reinhardt_db::orm::Manager<Self>;
	///
	///     fn table_name() -> &'static str {
	///         "users"
	///     }
	///
	///     fn new_fields() -> Self::Fields {
	///         UserFields
	///     }
	///
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         Some(self.id)
	///     }
	///
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.id = value;
	///     }
	/// }
	///
	/// let inspector = ModelInspector::<User>::new();
	/// let indexes = inspector.get_indexes();
	/// // Returns empty by default, override index_metadata() to provide indexes
	/// assert_eq!(indexes.len(), 0);
	/// ```
	pub fn get_indexes(&self) -> Vec<IndexInfo> {
		M::index_metadata()
	}

	/// Get all constraint information from the model
	///
	/// Returns constraint metadata provided by the Model implementation.
	/// By default, returns an empty vector unless the model provides
	/// constraint metadata through the `constraint_metadata()` method.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::inspection::ModelInspector;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Deserialize, Serialize};
	///
	/// #[derive(Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: i32,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::model::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i32;
	///     type Fields = UserFields;
	///     type Objects = reinhardt_db::orm::Manager<Self>;
	///
	///     fn table_name() -> &'static str {
	///         "users"
	///     }
	///
	///     fn new_fields() -> Self::Fields {
	///         UserFields
	///     }
	///
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         Some(self.id)
	///     }
	///
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.id = value;
	///     }
	/// }
	///
	/// let inspector = ModelInspector::<User>::new();
	/// let constraints = inspector.get_constraints();
	/// // Returns empty by default, override constraint_metadata() to provide constraints
	/// assert_eq!(constraints.len(), 0);
	/// ```
	pub fn get_constraints(&self) -> Vec<ConstraintInfo> {
		M::constraint_metadata()
	}
}

impl<M: Model> Default for ModelInspector<M> {
	fn default() -> Self {
		Self::new()
	}
}

/// Inspector for individual field metadata
///
/// Provides detailed information about a specific field's type,
/// validation rules, and database mapping.
pub struct FieldInspector {
	field_info: FieldInfo,
}

impl FieldInspector {
	/// Create a new FieldInspector from field information
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::{FieldInfo, FieldInspector};
	///
	/// let mut field = CharField::new(50);
	/// field.set_attributes_from_name("title");
	/// let field_info = FieldInfo::from_field(&field);
	/// let inspector = FieldInspector::new(field_info);
	///
	/// assert_eq!(inspector.name(), "title");
	/// ```
	pub fn new(field_info: FieldInfo) -> Self {
		Self { field_info }
	}

	/// Create a FieldInspector from a Field trait object
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{IntegerField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = IntegerField::new();
	/// field.set_attributes_from_name("count");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert_eq!(inspector.name(), "count");
	/// assert_eq!(inspector.field_type(), "reinhardt.orm.models.IntegerField");
	/// ```
	pub fn from_field<F: Field>(field: &F) -> Self {
		Self::new(FieldInfo::from_field(field))
	}

	/// Get the field name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{EmailField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = EmailField::new();
	/// field.set_attributes_from_name("email");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert_eq!(inspector.name(), "email");
	/// ```
	pub fn name(&self) -> &str {
		&self.field_info.name
	}

	/// Get the field type path
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{BooleanField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = BooleanField::new();
	/// field.set_attributes_from_name("is_active");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert_eq!(inspector.field_type(), "reinhardt.orm.models.BooleanField");
	/// ```
	pub fn field_type(&self) -> &str {
		&self.field_info.field_type
	}

	/// Check if the field allows NULL values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = CharField::with_null_blank(100);
	/// field.set_attributes_from_name("middle_name");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert!(inspector.is_nullable());
	/// ```
	pub fn is_nullable(&self) -> bool {
		self.field_info.nullable
	}

	/// Check if the field is the primary key
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{AutoField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = AutoField::new();
	/// field.set_attributes_from_name("id");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert!(inspector.is_primary_key());
	/// ```
	pub fn is_primary_key(&self) -> bool {
		self.field_info.primary_key
	}

	/// Check if the field must be unique
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = CharField::new(100);
	/// field.base.unique = true;
	/// field.set_attributes_from_name("slug");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert!(inspector.is_unique());
	/// ```
	pub fn is_unique(&self) -> bool {
		self.field_info.unique
	}

	/// Check if the field allows blank values (for forms)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = CharField::with_null_blank(100);
	/// field.set_attributes_from_name("bio");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert!(inspector.is_blank());
	/// ```
	pub fn is_blank(&self) -> bool {
		self.field_info.blank
	}

	/// Check if the field is editable in forms
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = CharField::new(100);
	/// field.set_attributes_from_name("username");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert!(inspector.is_editable());
	/// ```
	pub fn is_editable(&self) -> bool {
		self.field_info.editable
	}

	/// Get the default value if any
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{BooleanField, Field, FieldKwarg};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = BooleanField::with_default(true);
	/// field.set_attributes_from_name("is_active");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert_eq!(inspector.default_value(), Some(&FieldKwarg::Bool(true)));
	/// ```
	pub fn default_value(&self) -> Option<&FieldKwarg> {
		self.field_info.default.as_ref()
	}

	/// Get the database default value if any
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{IntegerField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = IntegerField::new();
	/// field.set_attributes_from_name("counter");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert!(inspector.db_default_value().is_none());
	/// ```
	pub fn db_default_value(&self) -> Option<&FieldKwarg> {
		self.field_info.db_default.as_ref()
	}

	/// Get the database column name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = CharField::new(100);
	/// field.set_attributes_from_name("first_name");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert_eq!(inspector.db_column_name(), "first_name");
	/// ```
	pub fn db_column_name(&self) -> &str {
		self.field_info.db_column_name()
	}

	/// Get field choices if any
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let choices = vec![
	///     ("M".to_string(), "Male".to_string()),
	///     ("F".to_string(), "Female".to_string()),
	/// ];
	/// let mut field = CharField::with_choices(1, choices.clone());
	/// field.set_attributes_from_name("gender");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert_eq!(inspector.choices(), Some(&choices));
	/// ```
	pub fn choices(&self) -> Option<&Vec<(String, String)>> {
		self.field_info.choices.as_ref()
	}

	/// Get a specific field attribute by name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field, FieldKwarg};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = CharField::new(255);
	/// field.set_attributes_from_name("url");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// assert_eq!(inspector.get_attribute("max_length"), Some(&FieldKwarg::Uint(255)));
	/// ```
	pub fn get_attribute(&self, name: &str) -> Option<&FieldKwarg> {
		self.field_info.attributes.get(name)
	}

	/// Get all field attributes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{DecimalField, Field};
	/// use reinhardt_db::orm::inspection::FieldInspector;
	///
	/// let mut field = DecimalField::new(10, 2);
	/// field.set_attributes_from_name("price");
	/// let inspector = FieldInspector::from_field(&field);
	///
	/// let attributes = inspector.attributes();
	/// assert!(attributes.contains_key("max_digits"));
	/// assert!(attributes.contains_key("decimal_places"));
	/// ```
	pub fn attributes(&self) -> &HashMap<String, FieldKwarg> {
		&self.field_info.attributes
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::Manager;
	use crate::orm::fields::{
		AutoField, BooleanField, CharField, DecimalField, EmailField, IntegerField,
	};
	use crate::orm::model::FieldSelector;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};

	#[derive(Clone, Debug, Deserialize, Serialize)]
	struct ConstraintModel {
		id: i64,
		email: String,
	}

	#[derive(Clone)]
	struct ConstraintModelFields;

	impl FieldSelector for ConstraintModelFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for ConstraintModel {
		type PrimaryKey = i64;
		type Fields = ConstraintModelFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"constraint_models"
		}

		fn new_fields() -> Self::Fields {
			ConstraintModelFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			Some(self.id)
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = value;
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut id = AutoField::new();
			id.set_attributes_from_name("id");
			let mut email = CharField::new(255);
			email.base.db_column = Some("email_addr".to_string());
			email.set_attributes_from_name("email");
			vec![FieldInfo::from_field(&id), FieldInfo::from_field(&email)]
		}
	}

	#[test]
	fn test_field_info_from_char_field() {
		let mut field = CharField::new(100);
		field.set_attributes_from_name("username");
		let info = FieldInfo::from_field(&field);

		assert_eq!(info.name, "username");
		assert_eq!(info.field_type, "reinhardt.orm.models.CharField");
		assert!(!info.nullable);
		assert!(!info.primary_key);
		assert!(info.editable);
	}

	#[test]
	fn test_field_info_from_auto_field() {
		let mut field = AutoField::new();
		field.set_attributes_from_name("id");
		let info = FieldInfo::from_field(&field);

		assert_eq!(info.name, "id");
		assert_eq!(info.field_type, "reinhardt.orm.models.AutoField");
		assert!(info.primary_key);
	}

	#[test]
	fn test_field_info_nullable_field() {
		let mut field = CharField::with_null_blank(200);
		field.set_attributes_from_name("bio");
		let info = FieldInfo::from_field(&field);

		assert!(info.nullable);
		assert!(info.blank);
	}

	#[test]
	fn test_field_info_with_choices() {
		let choices = vec![
			("A".to_string(), "Active".to_string()),
			("I".to_string(), "Inactive".to_string()),
		];
		let mut field = CharField::with_choices(1, choices.clone());
		field.set_attributes_from_name("status");
		let info = FieldInfo::from_field(&field);

		assert!(info.has_choices());
		assert_eq!(info.choices, Some(choices));
	}

	#[test]
	fn test_field_info_db_column_name() {
		let mut field = CharField::new(100);
		field.base.db_column = Some("usr_name".to_string());
		field.set_attributes_from_name("username");
		let info = FieldInfo::from_field(&field);

		assert_eq!(info.db_column_name(), "usr_name");
	}

	#[test]
	fn test_field_info_db_column_name_fallback() {
		let mut field = CharField::new(100);
		field.set_attributes_from_name("email");
		let info = FieldInfo::from_field(&field);

		assert_eq!(info.db_column_name(), "email");
	}

	#[test]
	fn test_field_inspector_name() {
		let mut field = IntegerField::new();
		field.set_attributes_from_name("count");
		let inspector = FieldInspector::from_field(&field);

		assert_eq!(inspector.name(), "count");
	}

	#[test]
	fn test_field_inspector_field_type() {
		let mut field = EmailField::new();
		field.set_attributes_from_name("contact_email");
		let inspector = FieldInspector::from_field(&field);

		assert_eq!(inspector.field_type(), "reinhardt.orm.models.EmailField");
	}

	#[test]
	fn test_field_inspector_is_nullable() {
		let mut nullable_field = CharField::with_null_blank(100);
		nullable_field.set_attributes_from_name("middle_name");
		let inspector = FieldInspector::from_field(&nullable_field);

		assert!(inspector.is_nullable());
	}

	#[test]
	fn test_field_inspector_is_primary_key() {
		let mut pk_field = AutoField::new();
		pk_field.set_attributes_from_name("id");
		let inspector = FieldInspector::from_field(&pk_field);

		assert!(inspector.is_primary_key());
	}

	#[test]
	fn test_field_inspector_is_unique() {
		let mut field = CharField::new(100);
		field.base.unique = true;
		field.set_attributes_from_name("slug");
		let inspector = FieldInspector::from_field(&field);

		assert!(inspector.is_unique());
	}

	#[test]
	fn test_field_inspector_is_blank() {
		let mut field = CharField::with_null_blank(500);
		field.set_attributes_from_name("description");
		let inspector = FieldInspector::from_field(&field);

		assert!(inspector.is_blank());
	}

	#[test]
	fn test_field_inspector_is_editable() {
		let mut field = CharField::new(100);
		field.set_attributes_from_name("title");
		let inspector = FieldInspector::from_field(&field);

		assert!(inspector.is_editable());
	}

	#[test]
	fn test_field_inspector_default_value() {
		let mut field = BooleanField::with_default(true);
		field.set_attributes_from_name("is_active");
		let inspector = FieldInspector::from_field(&field);

		assert_eq!(inspector.default_value(), Some(&FieldKwarg::Bool(true)));
	}

	#[test]
	fn test_field_inspector_db_column_name() {
		let mut field = CharField::new(100);
		field.base.db_column = Some("usr_email".to_string());
		field.set_attributes_from_name("user_email");
		let inspector = FieldInspector::from_field(&field);

		assert_eq!(inspector.db_column_name(), "usr_email");
	}

	#[test]
	fn test_field_inspector_choices() {
		let choices = vec![
			("S".to_string(), "Small".to_string()),
			("M".to_string(), "Medium".to_string()),
			("L".to_string(), "Large".to_string()),
		];
		let mut field = CharField::with_choices(1, choices.clone());
		field.set_attributes_from_name("size");
		let inspector = FieldInspector::from_field(&field);

		assert_eq!(inspector.choices(), Some(&choices));
	}

	#[test]
	fn test_field_inspector_get_attribute() {
		let mut field = DecimalField::new(10, 2);
		field.set_attributes_from_name("price");
		let inspector = FieldInspector::from_field(&field);

		assert_eq!(
			inspector.get_attribute("max_digits"),
			Some(&FieldKwarg::Uint(10))
		);
		assert_eq!(
			inspector.get_attribute("decimal_places"),
			Some(&FieldKwarg::Uint(2))
		);
	}

	#[test]
	fn test_field_inspector_attributes() {
		let mut field = CharField::new(255);
		field.set_attributes_from_name("url");
		let inspector = FieldInspector::from_field(&field);

		let attributes = inspector.attributes();
		assert!(attributes.contains_key("max_length"));
	}

	#[test]
	fn test_relation_info_creation() {
		let info = RelationInfo::new("posts", RelationshipType::OneToMany, "Post");

		assert_eq!(info.name, "posts");
		assert_eq!(info.relationship_type, RelationshipType::OneToMany);
		assert_eq!(info.related_model, "Post");
		assert!(info.foreign_key.is_none());
		assert!(info.back_populates.is_none());
	}

	#[test]
	fn test_relation_info_with_foreign_key() {
		let info = RelationInfo::new("author", RelationshipType::ManyToOne, "User")
			.with_foreign_key("author_id");

		assert_eq!(info.foreign_key, Some("author_id".to_string()));
	}

	#[test]
	fn test_relation_info_with_back_populates() {
		let info = RelationInfo::new("comments", RelationshipType::OneToMany, "Comment")
			.with_back_populates("post");

		assert_eq!(info.back_populates, Some("post".to_string()));
	}

	#[test]
	fn test_index_info_from_index() {
		let index = Index::new("email_idx", vec!["email".to_string()]);
		let info = IndexInfo::from_index(&index);

		assert_eq!(info.name, "email_idx");
		assert_eq!(info.fields.len(), 1);
		assert!(!info.unique);
		assert!(info.condition.is_none());
	}

	#[test]
	fn test_index_info_unique_index() {
		let index = Index::new("username_idx", vec!["username".to_string()]).unique();
		let info = IndexInfo::from_index(&index);

		assert!(info.unique);
	}

	#[test]
	fn test_constraint_info_from_check() {
		let constraint = CheckConstraint::new("age_check", "age >= 18");
		let info = ConstraintInfo::from_check(&constraint);

		assert_eq!(info.name, "age_check");
		assert_eq!(info.constraint_type, ConstraintType::Check);
		assert!(info.definition.contains("CHECK"));
	}

	#[rstest]
	fn test_constraint_info_from_unique() {
		let constraint = UniqueConstraint::new("email_unique", vec!["email_addr".to_string()]);
		let info = ConstraintInfo::from_unique(&constraint, &ConstraintModel::field_metadata());

		assert_eq!(info.name, "email_unique");
		assert_eq!(info.constraint_type, ConstraintType::Unique);
		assert!(info.definition.contains("UNIQUE"));
		assert_eq!(info.fields, vec!["email"]);
		assert_eq!(info.condition, None);
		assert!(!info.deferrable);
		assert_eq!(info.nulls_distinct, None);
	}

	#[test]
	fn test_model_inspector_table_name() {
		use serde::{Deserialize, Serialize};

		#[derive(Clone, Serialize, Deserialize)]
		struct TestModel {
			id: i32,
		}

		#[derive(Clone)]
		struct TestModelFields;
		impl crate::orm::model::FieldSelector for TestModelFields {
			fn with_alias(self, _alias: &str) -> Self {
				self
			}
		}

		impl Model for TestModel {
			type PrimaryKey = i32;
			type Fields = TestModelFields;
			type Objects = Manager<Self>;

			fn table_name() -> &'static str {
				"test_table"
			}

			fn new_fields() -> Self::Fields {
				TestModelFields
			}

			fn primary_key(&self) -> Option<Self::PrimaryKey> {
				Some(self.id)
			}

			fn set_primary_key(&mut self, value: Self::PrimaryKey) {
				self.id = value;
			}
		}

		let inspector = ModelInspector::<TestModel>::new();
		assert_eq!(inspector.table_name(), "test_table");
	}

	#[test]
	fn test_model_inspector_primary_key_field() {
		use serde::{Deserialize, Serialize};

		#[derive(Clone, Serialize, Deserialize)]
		struct Article {
			article_id: u32,
		}

		#[derive(Clone)]
		struct ArticleFields;
		impl crate::orm::model::FieldSelector for ArticleFields {
			fn with_alias(self, _alias: &str) -> Self {
				self
			}
		}

		impl Model for Article {
			type PrimaryKey = u32;
			type Fields = ArticleFields;
			type Objects = Manager<Self>;

			fn table_name() -> &'static str {
				"articles"
			}

			fn primary_key_field() -> &'static str {
				"article_id"
			}

			fn new_fields() -> Self::Fields {
				ArticleFields
			}

			fn primary_key(&self) -> Option<Self::PrimaryKey> {
				Some(self.article_id)
			}

			fn set_primary_key(&mut self, value: Self::PrimaryKey) {
				self.article_id = value;
			}
		}

		let inspector = ModelInspector::<Article>::new();
		assert_eq!(inspector.primary_key_field(), "article_id");
	}

	#[test]
	fn test_model_inspector_get_field_nonexistent() {
		use serde::{Deserialize, Serialize};

		#[derive(Clone, Serialize, Deserialize)]
		struct User {
			id: i32,
		}

		#[derive(Clone)]
		struct UserFields;
		impl crate::orm::model::FieldSelector for UserFields {
			fn with_alias(self, _alias: &str) -> Self {
				self
			}
		}

		impl Model for User {
			type PrimaryKey = i32;
			type Fields = UserFields;
			type Objects = Manager<Self>;

			fn table_name() -> &'static str {
				"users"
			}

			fn new_fields() -> Self::Fields {
				UserFields
			}

			fn primary_key(&self) -> Option<Self::PrimaryKey> {
				Some(self.id)
			}

			fn set_primary_key(&mut self, value: Self::PrimaryKey) {
				self.id = value;
			}
		}

		let inspector = ModelInspector::<User>::new();
		assert!(inspector.get_field("nonexistent").is_none());
	}

	#[test]
	fn test_model_inspector_get_fields_empty() {
		use serde::{Deserialize, Serialize};

		#[derive(Clone, Serialize, Deserialize)]
		struct Empty {
			id: i32,
		}

		#[derive(Clone)]
		struct EmptyFields;
		impl crate::orm::model::FieldSelector for EmptyFields {
			fn with_alias(self, _alias: &str) -> Self {
				self
			}
		}

		impl Model for Empty {
			type PrimaryKey = i32;
			type Fields = EmptyFields;
			type Objects = Manager<Self>;

			fn table_name() -> &'static str {
				"empty"
			}

			fn new_fields() -> Self::Fields {
				EmptyFields
			}

			fn primary_key(&self) -> Option<Self::PrimaryKey> {
				Some(self.id)
			}

			fn set_primary_key(&mut self, value: Self::PrimaryKey) {
				self.id = value;
			}
		}

		let inspector = ModelInspector::<Empty>::new();
		assert!(inspector.get_fields().is_empty());
	}

	#[test]
	fn test_model_inspector_get_relations_empty() {
		use serde::{Deserialize, Serialize};

		#[derive(Clone, Serialize, Deserialize)]
		struct Simple {
			id: i32,
		}

		#[derive(Clone)]
		struct SimpleFields;
		impl crate::orm::model::FieldSelector for SimpleFields {
			fn with_alias(self, _alias: &str) -> Self {
				self
			}
		}

		impl Model for Simple {
			type PrimaryKey = i32;
			type Fields = SimpleFields;
			type Objects = Manager<Self>;

			fn table_name() -> &'static str {
				"simple"
			}

			fn new_fields() -> Self::Fields {
				SimpleFields
			}

			fn primary_key(&self) -> Option<Self::PrimaryKey> {
				Some(self.id)
			}

			fn set_primary_key(&mut self, value: Self::PrimaryKey) {
				self.id = value;
			}
		}

		let inspector = ModelInspector::<Simple>::new();
		assert!(inspector.get_relations().is_empty());
	}
}
