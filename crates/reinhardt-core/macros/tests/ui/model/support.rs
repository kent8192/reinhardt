extern crate self as reinhardt;
extern crate self as reinhardt_core;

pub mod macros {
	pub use reinhardt_macros::Model;
}

pub mod apps {
	pub mod registry {
		#[derive(Debug, Clone, PartialEq, Eq)]
		pub struct RelationshipMetadata {
			pub from_model: &'static str,
			pub to_model: &'static str,
			pub relationship_type: RelationshipType,
			pub field_name: &'static str,
			pub related_name: Option<&'static str>,
			pub db_column: Option<&'static str>,
			pub through_table: Option<&'static str>,
		}

		#[derive(Debug, Clone, Copy, PartialEq, Eq)]
		pub enum RelationshipType {
			ForeignKey,
			ManyToMany,
			OneToOne,
		}

		#[linkme::distributed_slice]
		pub static RELATIONSHIPS: [RelationshipMetadata];
	}
}

pub mod exception {
	#[derive(Debug)]
	pub enum Error {
		Internal(String),
		Validation(String),
	}

	pub type Result<T> = core::result::Result<T, Error>;
}

pub mod model_info {
	pub trait InfoModel {
		type PrimaryKey;
	}

	#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
	#[serde(bound(
		serialize = "T::PrimaryKey: serde::Serialize",
		deserialize = "T::PrimaryKey: serde::Deserialize<'de>"
	))]
	pub struct RelationInfo<T: InfoModel> {
		pub id: T::PrimaryKey,
	}

	impl<T: InfoModel> RelationInfo<T> {
		pub const fn new(id: T::PrimaryKey) -> Self {
			Self { id }
		}

		pub fn into_id(self) -> T::PrimaryKey {
			self.id
		}
	}

	#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
	#[serde(bound(
		serialize = "Target::PrimaryKey: serde::Serialize",
		deserialize = "Target::PrimaryKey: serde::Deserialize<'de>"
	))]
	pub struct ManyToManyInfo<Source, Target: InfoModel> {
		pub target_ids: Vec<Target::PrimaryKey>,
		_source: core::marker::PhantomData<Source>,
	}

	impl<Source, Target: InfoModel> ManyToManyInfo<Source, Target> {
		pub fn new<I>(target_ids: I) -> Self
		where
			I: IntoIterator<Item = Target::PrimaryKey>,
		{
			Self {
				target_ids: target_ids.into_iter().collect(),
				_source: core::marker::PhantomData,
			}
		}

		pub const fn empty() -> Self {
			Self {
				target_ids: Vec::new(),
				_source: core::marker::PhantomData,
			}
		}
	}
}

pub mod db {
	pub mod associations {
		#[derive(Debug, Clone, Copy)]
		pub struct ForeignKeyField<T>(core::marker::PhantomData<T>);

		#[derive(Debug, Clone, Copy)]
		pub struct OneToOneField<T>(core::marker::PhantomData<T>);

		#[derive(Debug, Clone, Copy)]
		pub struct ManyToManyField<Source, Target>(core::marker::PhantomData<(Source, Target)>);

		impl<T> Default for ForeignKeyField<T> {
			fn default() -> Self {
				Self(core::marker::PhantomData)
			}
		}

		impl<T> PartialEq for ForeignKeyField<T> {
			fn eq(&self, _other: &Self) -> bool {
				true
			}
		}

		impl<T> Eq for ForeignKeyField<T> {}
	}

	pub mod orm {
		pub mod query {
			#[derive(Debug, Clone, Copy)]
			pub struct FilterValue;

			impl From<i64> for FilterValue {
				fn from(_value: i64) -> Self {
					Self
				}
			}
		}

		pub mod model {
			pub fn deserialize_primary_key_from_str<T>(value: &str) -> Result<T, serde_json::Error>
			where
				T: serde::de::DeserializeOwned,
			{
				serde_json::from_value(serde_json::Value::String(value.to_owned()))
					.or_else(|_| serde_json::from_str(value))
			}

			pub fn deserialize_primary_key_filter_value_from_str<T>(
				_value: &str,
			) -> Result<Option<super::query::FilterValue>, serde_json::Error>
			where
				T: serde::de::DeserializeOwned,
			{
				Ok(None)
			}
		}

		pub struct Manager<T>(core::marker::PhantomData<T>);

		impl<T> Default for Manager<T> {
			fn default() -> Self {
				Self(core::marker::PhantomData)
			}
		}

		impl<T> Manager<T> {
			pub fn filter(self, _condition: impl Sized) -> Self {
				self
			}

			pub async fn first_with_db(
				self,
				_db: &connection::DatabaseConnection,
			) -> crate::exception::Result<Option<T>> {
				Ok(None)
			}
		}

		pub mod connection {
			#[derive(Debug, Clone)]
			pub struct DatabaseConnection;
		}

		pub trait FieldSelector: Sized {
			fn with_alias(self, _alias: &str) -> Self {
				self
			}
		}

		pub trait Model {
			type PrimaryKey;
			type Fields;
			type Objects: Default;

			fn table_name() -> &'static str;
			fn new_fields() -> Self::Fields;
			fn app_label() -> &'static str;
			fn primary_key_field() -> &'static str;
			fn primary_key_filter_value(pk: Self::PrimaryKey) -> query::FilterValue;
			fn primary_key_filter_value_from_str(
				_value: &str,
			) -> crate::exception::Result<query::FilterValue> {
				Ok(query::FilterValue)
			}
			fn primary_key(&self) -> Option<Self::PrimaryKey>;
			fn set_primary_key(&mut self, value: Self::PrimaryKey);
			fn field_metadata() -> Vec<inspection::FieldInfo>;
			fn index_metadata() -> Vec<inspection::IndexInfo>;
			fn constraint_metadata() -> Vec<inspection::ConstraintInfo>;
			fn relationship_metadata() -> Vec<inspection::RelationInfo>;

			fn objects() -> Self::Objects
			where
				Self: Sized,
			{
				Self::Objects::default()
			}
		}

		pub trait IntoPrimaryKey<T: Model> {
			fn into_primary_key(self) -> T::PrimaryKey;
		}

		impl<T: Model> IntoPrimaryKey<T> for &T {
			fn into_primary_key(self) -> T::PrimaryKey {
				self.primary_key().unwrap()
			}
		}

		impl<T: Model<PrimaryKey = i64>> IntoPrimaryKey<T> for i64 {
			fn into_primary_key(self) -> T::PrimaryKey {
				self
			}
		}

		pub struct ForeignKeyAccessor<Source, Target> {
			_marker: core::marker::PhantomData<(Source, Target)>,
		}

		impl<Source, Target> ForeignKeyAccessor<Source, Target> {
			pub const fn new(_db_column: &'static str) -> Self {
				Self {
					_marker: core::marker::PhantomData,
				}
			}
		}

		pub mod relationship {
			#[derive(Debug, Clone, Copy, PartialEq, Eq)]
			pub enum RelationshipType {
				OneToOne,
				OneToMany,
				ManyToOne,
				ManyToMany,
			}
		}

		pub mod expressions {
			#[derive(Debug, Clone)]
			pub struct FieldRef<Model, Type> {
				pub name: &'static str,
				_marker: core::marker::PhantomData<(Model, Type)>,
			}

			impl<Model, Type> FieldRef<Model, Type> {
				pub const fn new(name: &'static str) -> Self {
					Self {
						name,
						_marker: core::marker::PhantomData,
					}
				}

				pub fn eq(self, _value: impl Into<String>) -> bool {
					true
				}
			}
		}

		pub mod query_fields {
			#[derive(Debug, Clone)]
			pub struct Field<Model, Type> {
				pub names: Vec<String>,
				pub alias: Option<String>,
				_marker: core::marker::PhantomData<(Model, Type)>,
			}

			impl<Model, Type> Field<Model, Type> {
				pub fn new<S: Into<String>>(names: Vec<S>) -> Self {
					Self {
						names: names.into_iter().map(Into::into).collect(),
						alias: None,
						_marker: core::marker::PhantomData,
					}
				}

				pub fn with_alias(mut self, alias: &str) -> Self {
					self.alias = Some(alias.to_string());
					self
				}
			}
		}

		pub mod fields {
			#[derive(Debug, Clone, PartialEq)]
			pub enum FieldKwarg {
				Bool(bool),
				Int(i64),
				Uint(u64),
				String(String),
			}
		}

		pub mod inspection {
			use super::fields::FieldKwarg;
			use std::collections::HashMap;

			#[derive(Debug, Clone, PartialEq)]
			pub struct FieldInfo {
				pub name: String,
				pub field_type: String,
				pub nullable: bool,
				pub primary_key: bool,
				pub unique: bool,
				pub blank: bool,
				pub editable: bool,
				pub default: Option<String>,
				pub db_default: Option<String>,
				pub db_column: Option<String>,
				pub choices: Option<Vec<String>>,
				pub attributes: HashMap<String, FieldKwarg>,
			}

			#[derive(Debug, Clone, PartialEq)]
			pub struct IndexInfo {
				pub name: String,
				pub fields: Vec<String>,
				pub unique: bool,
				pub condition: Option<String>,
			}

			#[derive(Debug, Clone, PartialEq)]
			pub enum ConstraintType {
				Check,
				Unique,
			}

			#[derive(Debug, Clone, PartialEq)]
			pub struct ConstraintInfo {
				pub name: String,
				pub constraint_type: ConstraintType,
				pub definition: String,
			}

			#[derive(Debug, Clone, PartialEq)]
			pub struct RelationInfo {
				pub name: String,
				pub relationship_type: super::relationship::RelationshipType,
				pub foreign_key: Option<String>,
				pub related_model: String,
				pub back_populates: Option<String>,
				pub through_table: Option<String>,
				pub source_field: Option<String>,
				pub target_field: Option<String>,
			}
		}

		pub mod registry {
			#[derive(Debug, Clone, PartialEq)]
			pub struct ModelInfo {
				pub app_label: String,
				pub model_name: String,
				pub type_path: String,
				pub table_name: String,
			}

			pub struct Registry;

			impl Registry {
				pub fn register(&self, _info: ModelInfo) {}
			}

			pub fn global_model_registry() -> Registry {
				Registry
			}
		}
	}

	pub mod migrations {
		#[derive(Debug, Clone, PartialEq)]
		pub enum FieldType {
			Integer,
			BigInteger,
			VarChar(u32),
			Boolean,
			TimestampTz,
			Date,
			Time,
			Float,
			Double,
			Uuid,
		}

		#[derive(Debug, Clone, PartialEq)]
		pub struct ConstraintDefinition {
			pub name: String,
			pub constraint_type: String,
			pub fields: Vec<String>,
			pub expression: Option<String>,
			pub foreign_key_info: Option<ForeignKeyInfo>,
		}

		#[derive(Debug, Clone, PartialEq)]
		pub struct ForeignKeyInfo {
			pub referenced_table: String,
			pub referenced_column: String,
			pub on_delete: ForeignKeyAction,
			pub on_update: ForeignKeyAction,
		}

		#[derive(Debug, Clone, PartialEq)]
		pub enum ForeignKeyAction {
			Cascade,
		}

		pub fn to_snake_case(value: &str) -> String {
			value.to_ascii_lowercase()
		}

		pub mod model_registry {
			use super::{ConstraintDefinition, FieldType, ForeignKeyInfo};

			#[derive(Debug, Clone, PartialEq)]
			pub struct FieldMetadata {
				pub field_type: FieldType,
			}

			impl FieldMetadata {
				pub const fn new(field_type: FieldType) -> Self {
					Self { field_type }
				}

				pub fn with_param(self, _key: &str, _value: &str) -> Self {
					self
				}

				pub fn with_nullable(self, _nullable: bool) -> Self {
					self
				}

				pub fn with_foreign_key(self, _foreign_key: ForeignKeyInfo) -> Self {
					self
				}
			}

			#[derive(Debug, Clone, PartialEq)]
			pub struct ManyToManyMetadata {
				pub field_name: String,
				pub to_model: String,
				pub related_name: Option<String>,
				pub through: Option<String>,
				pub source_field: Option<String>,
				pub target_field: Option<String>,
				pub db_constraint_prefix: Option<String>,
			}

			pub struct ModelMetadata;

			impl ModelMetadata {
				pub const fn new(_app_label: &str, _model_name: &str, _table_name: &str) -> Self {
					Self
				}

				pub fn add_field(&mut self, _name: String, _metadata: FieldMetadata) {}

				pub fn add_many_to_many(&mut self, _metadata: ManyToManyMetadata) {}

				pub fn add_constraint(&mut self, _constraint: ConstraintDefinition) {}
			}

			pub struct Registry;

			impl Registry {
				pub fn register_model(&self, _metadata: ModelMetadata) {}
			}

			pub fn global_registry() -> Registry {
				Registry
			}
		}
	}
}
