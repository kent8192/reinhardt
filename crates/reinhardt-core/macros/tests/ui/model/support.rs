extern crate self as reinhardt;
extern crate self as reinhardt_core;
extern crate self as reinhardt_db;

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

pub mod validators {
	#[derive(Debug, Clone)]
	pub enum ValidationError {
		Custom(String),
	}

	#[derive(Debug, Clone, Default)]
	pub struct ValidationErrors;
}

pub mod model_info {
	pub trait InfoModel {
		type PrimaryKey;

		fn table_name() -> &'static str {
			""
		}
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

	impl<T> Default for RelationInfo<T>
	where
		T: InfoModel,
		T::PrimaryKey: Default,
	{
		fn default() -> Self {
			Self::new(T::PrimaryKey::default())
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

pub mod model_form {
	pub trait ModelFormPolicy: Send + Sync + 'static {
		fn allows(field: &str) -> bool;
	}

	pub struct AllEditableModelFields;

	impl ModelFormPolicy for AllEditableModelFields {
		fn allows(_field: &str) -> bool {
			true
		}
	}

	pub trait ModelFormSchema {
		type Model;
		fn fields() -> &'static [ModelFormFieldDescriptor];
		fn default_boolean_is_true(_field: &str) -> bool {
			false
		}
		fn relation_target_matches<T: 'static>(_field: &str) -> bool {
			false
		}
	}

	pub trait ModelFormTableName {
		fn table_name() -> &'static str;
	}

	pub trait ModelFormPayload<P: ModelFormPolicy>: Sized {
		fn supplied_fields(&self) -> Vec<&'static str>;
		fn forbidden_fields(&self) -> &[&'static str];
		fn get_json(&self, field: &str) -> Option<serde_json::Value>;
		fn set_json(
			&mut self,
			field: &str,
			value: serde_json::Value,
		) -> Result<(), ModelFormPayloadError>;
	}

	pub trait NativeModelFormPayload: Sized {
		fn from_native_form_value(value: serde_json::Value) -> Result<Self, serde_json::Error>;
	}

	/// Mirrors the native unchecked-checkbox normalization required by model-form fixtures.
	pub fn normalize_native_model_form_value<S, P>(
		mut value: serde_json::Value,
	) -> Result<serde_json::Value, serde_json::Error>
	where
		S: ModelFormSchema,
		P: ModelFormPolicy,
	{
		if let serde_json::Value::Object(values) = &mut value {
			for descriptor in S::fields() {
				if descriptor.editable
					&& P::allows(descriptor.name)
					&& matches!(descriptor.kind, ModelFormFieldKind::Boolean)
					&& !descriptor.nullable
					&& !descriptor.has_default
					&& !values.contains_key(descriptor.name)
				{
					values.insert(descriptor.name.to_owned(), serde_json::Value::Bool(false));
				}
			}
		}
		Ok(value)
	}
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub enum ModelFormFieldKind {
		Text {
			min_length: Option<usize>,
			max_length: Option<usize>,
			multiline: bool,
		},
		Email {
			max_length: Option<usize>,
		},
		Url {
			max_length: Option<usize>,
		},
		Integer {
			min: Option<i64>,
			max: Option<i64>,
		},
		Float {
			min: Option<f64>,
			max: Option<f64>,
		},
		Decimal,
		Boolean,
		Date,
		Time,
		DateTime,
		NaiveDateTime,
		Uuid,
		Json,
	}

	pub trait ModelFormPrimaryKey {
		const FIELD_KIND: ModelFormFieldKind;
	}

	pub trait ModelFormPrimaryKeyFields {
		fn primary_key_fields() -> &'static [&'static str];

		fn primary_key_field_kind() -> Option<ModelFormFieldKind> {
			None
		}
	}

	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct ModelFormFieldDescriptor {
		pub name: &'static str,
		pub kind: ModelFormFieldKind,
		pub required: bool,
		pub has_default: bool,
		pub nullable: bool,
		pub editable: bool,
		pub generated_relation_id: bool,
		pub trim: bool,
	}

	#[derive(Debug, Clone, PartialEq, Eq)]
	pub enum ModelFormPayloadError {
		UnknownField { field: String },
		ForbiddenField { field: String },
		InvalidValue { field: String, message: String },
	}
}

pub mod db {
	pub mod m2m_naming {
		pub fn default_through_table(source_table: &str, field_name: &str) -> String {
			format!(
				"{}_{}",
				source_table.to_lowercase(),
				field_name.to_lowercase()
			)
		}

		pub fn default_m2m_columns(source_table: &str, target_table: &str) -> (String, String) {
			let source = source_table.to_lowercase();
			let target = target_table.to_lowercase();
			if source == target {
				(format!("from_{}_id", source), format!("to_{}_id", target))
			} else {
				(format!("{}_id", source), format!("{}_id", target))
			}
		}
	}

	use serde::{Deserialize, Deserializer, Serialize, Serializer};
	use std::ops::{Deref, DerefMut};

	#[repr(transparent)]
	#[derive(Debug, Clone, PartialEq, Eq, Default)]
	pub struct Json<T>(pub T);

	impl<T> Json<T> {
		pub const fn new(value: T) -> Self {
			Self(value)
		}
	}

	impl<T> Deref for Json<T> {
		type Target = T;

		fn deref(&self) -> &Self::Target {
			&self.0
		}
	}

	impl<T> DerefMut for Json<T> {
		fn deref_mut(&mut self) -> &mut Self::Target {
			&mut self.0
		}
	}

	impl<T> Serialize for Json<T>
	where
		T: Serialize,
	{
		fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: Serializer,
		{
			self.0.serialize(serializer)
		}
	}

	impl<'de, T> Deserialize<'de> for Json<T>
	where
		T: Deserialize<'de>,
	{
		fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
		where
			D: Deserializer<'de>,
		{
			T::deserialize(deserializer).map(Self)
		}
	}

	pub mod associations {
		#[derive(Debug, Clone, Copy)]
		pub struct ForeignKeyField<T>(core::marker::PhantomData<T>);

		#[derive(Debug, Clone, Copy)]
		pub struct OneToOneField<T>(core::marker::PhantomData<T>);

		#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
		#[serde(bound = "")]
		pub struct ManyToManyField<Source, Target>(core::marker::PhantomData<(Source, Target)>);

		impl<T> Default for ForeignKeyField<T> {
			fn default() -> Self {
				Self(core::marker::PhantomData)
			}
		}

		impl<T> Default for OneToOneField<T> {
			fn default() -> Self {
				Self(core::marker::PhantomData)
			}
		}

		impl<Source, Target> Default for ManyToManyField<Source, Target> {
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

		impl<T> PartialEq for OneToOneField<T> {
			fn eq(&self, _other: &Self) -> bool {
				true
			}
		}

		impl<T> Eq for OneToOneField<T> {}

		impl<Source, Target> PartialEq for ManyToManyField<Source, Target> {
			fn eq(&self, _other: &Self) -> bool {
				true
			}
		}

		impl<Source, Target> Eq for ManyToManyField<Source, Target> {}

		pub struct ManyToManyAccessor<Source, Target>(core::marker::PhantomData<(Source, Target)>);

		impl<Source, Target> ManyToManyAccessor<Source, Target> {
			pub fn new(_source: &Source, _field_name: &str) -> Self {
				Self(core::marker::PhantomData)
			}
		}
	}

	pub mod orm {
		pub use super::associations::ManyToManyAccessor;
		pub use serde;

		pub mod naming {
			pub fn generated_unique_constraint_names(
				table: &str,
				fields: &[String],
				_reserved: &[String],
			) -> Vec<(String, String)> {
				fields
					.iter()
					.map(|field| (format!("{table}_{field}_uniq"), field.clone()))
					.collect()
			}

			pub fn foreign_key_constraint_name(table: &str, column: &str) -> String {
				format!("fk_{table}_{column}")
			}

			pub fn enum_domain_constraint_name(table: &str, column: &str) -> String {
				format!("{table}_{column}_model_enum_check")
			}
		}

		pub type FixtureFields = serde_json::Map<String, serde_json::Value>;
		pub type FixtureValue = serde_json::Value;

		pub mod query {
			#[derive(Debug)]
			pub enum FilterValue {
				Typed(Result<super::DatabaseValue, super::FieldCodecError>),
				String(String),
				Uuid(uuid::Uuid),
				Timestamp(chrono::DateTime<chrono::Utc>),
				Integer(i64),
				Unsupported,
			}

			impl Default for FilterValue {
				fn default() -> Self {
					Self::Unsupported
				}
			}

			impl From<i64> for FilterValue {
				fn from(value: i64) -> Self {
					Self::Integer(value)
				}
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

			pub async fn first_with_db<E>(self, _db: &mut E) -> crate::exception::Result<Option<T>>
			where
				E: connection::OrmExecutor,
			{
				Ok(None)
			}
		}

		pub mod connection {
			#[derive(Debug, Clone)]
			pub struct DatabaseConnection;

			pub trait OrmExecutor: Send {}

			impl OrmExecutor for DatabaseConnection {}
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
			fn primary_key_column() -> &'static str {
				Self::primary_key_field()
			}
			fn latest_by_fields() -> &'static [&'static str] {
				&[]
			}
			fn primary_key_filter_value(_pk: Self::PrimaryKey) -> query::FilterValue {
				query::FilterValue::default()
			}
			fn primary_key_filter_value_from_str(
				_value: &str,
			) -> crate::exception::Result<query::FilterValue> {
				Ok(query::FilterValue::default())
			}
			fn primary_key(&self) -> Option<Self::PrimaryKey>;
			fn set_primary_key(&mut self, value: Self::PrimaryKey);
			fn field_is_none(&self, field_name: &str) -> bool;
			fn encode_database_fields(
				&self,
			) -> Result<std::collections::BTreeMap<String, DatabaseValue>, FieldCodecError> {
				Ok(std::collections::BTreeMap::new())
			}
			fn decode_database_field(
				_field_name: &str,
				value: DatabaseValue,
			) -> Result<model::ModelFieldJsonValue, FieldCodecError> {
				value.into_json_value()
			}
			fn validate_fixture_fields(
				_fields: &FixtureFields,
			) -> core::result::Result<(), String> {
				Ok(())
			}
			fn field_metadata() -> Vec<inspection::FieldInfo>;
			fn index_metadata() -> Vec<inspection::IndexInfo>;
			fn constraint_metadata() -> Vec<inspection::ConstraintInfo>;
			fn constraint_fields(_constraint: &str) -> Option<Vec<&'static str>> {
				None
			}
			fn relationship_metadata() -> Vec<inspection::RelationInfo>;
			fn generated_field_names() -> &'static [&'static str];
			fn primary_key_uses_zero_sentinel() -> bool {
				false
			}
			fn primary_key_database_value(
				pk: &Self::PrimaryKey,
			) -> Result<DatabaseValue, FieldCodecError>
			where
				Self::PrimaryKey: DatabaseField,
			{
				<Self::PrimaryKey as DatabaseField>::encode_database(pk)
					.map(DatabaseScalar::into_database_value)
			}

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
			#[derive(Debug, Clone, Copy)]
			pub enum GeneratedModelField {}

			#[derive(Debug, Clone, Copy)]
			pub enum UnverifiedModelField {}

			#[derive(Debug, Clone)]
			pub struct FieldRef<Model, Type, Origin = UnverifiedModelField> {
				logical_name: &'static str,
				column_name: &'static str,
				_marker: core::marker::PhantomData<(Model, Type, Origin)>,
			}

			impl<Model, Type> FieldRef<Model, Type, UnverifiedModelField> {
				pub const fn new(name: &'static str) -> Self {
					Self {
						logical_name: name,
						column_name: name,
						_marker: core::marker::PhantomData,
					}
				}
			}

			impl<Model, Type> FieldRef<Model, Type, GeneratedModelField> {
				pub const unsafe fn from_model_field(name: &'static str) -> Self {
					Self {
						logical_name: name,
						column_name: name,
						_marker: core::marker::PhantomData,
					}
				}

				pub const unsafe fn from_generated_model_field_with_names(
					logical_name: &'static str,
					column_name: &'static str,
				) -> Self {
					Self {
						logical_name,
						column_name,
						_marker: core::marker::PhantomData,
					}
				}
			}

			impl<Model, Type, Origin> FieldRef<Model, Type, Origin> {
				pub const fn logical_name(&self) -> &'static str {
					self.logical_name
				}

				pub const fn name(&self) -> &'static str {
					self.column_name
				}

				pub fn eq(self, _value: impl Into<Type>) -> bool {
					true
				}
			}

			#[derive(Debug, Clone, Copy)]
			pub struct OrderingField<Model> {
				_marker: core::marker::PhantomData<Model>,
			}

			impl<Model> OrderingField<Model> {
				pub const unsafe fn from_model_field(_name: &'static str) -> Self {
					Self {
						_marker: core::marker::PhantomData,
					}
				}
			}

			#[derive(Debug, Clone)]
			pub struct UniqueFieldRef<Model, Type> {
				_marker: core::marker::PhantomData<(Model, Type)>,
			}

			impl<Model, Type> UniqueFieldRef<Model, Type> {
				pub const unsafe fn from_model_field(_name: &'static str) -> Self {
					Self {
						_marker: core::marker::PhantomData,
					}
				}

				pub const unsafe fn from_model_field_with_names(
					_logical_name: &'static str,
					_column_name: &'static str,
				) -> Self {
					Self {
						_marker: core::marker::PhantomData,
					}
				}

				pub const unsafe fn from_model_field_with_getter(
					_name: &'static str,
					_getter: fn(&Model) -> Option<Type>,
				) -> Self {
					Self {
						_marker: core::marker::PhantomData,
					}
				}

				pub const unsafe fn from_model_field_with_names_and_getter(
					_logical_name: &'static str,
					_column_name: &'static str,
					_getter: fn(&Model) -> Option<Type>,
				) -> Self {
					Self {
						_marker: core::marker::PhantomData,
					}
				}
			}
		}

		pub mod relations {
			use std::borrow::Cow;

			use super::Model;

			#[derive(Debug, Clone, Copy)]
			pub enum GeneratedRelationPath {}

			#[derive(Debug, Clone, Copy)]
			pub enum UnverifiedRelationPath {}

			#[derive(Debug, Clone, Copy)]
			pub enum GeneratedRelatedField {}

			#[derive(Debug, Clone, Copy)]
			pub enum UnverifiedRelatedField {}

			pub trait RelationFieldOrigin<FieldOrigin> {
				type RelatedFieldOrigin;
			}

			impl<FieldOrigin> RelationFieldOrigin<FieldOrigin> for UnverifiedRelationPath {
				type RelatedFieldOrigin = UnverifiedRelatedField;
			}

			impl RelationFieldOrigin<super::expressions::GeneratedModelField> for GeneratedRelationPath {
				type RelatedFieldOrigin = GeneratedRelatedField;
			}

			impl RelationFieldOrigin<super::expressions::UnverifiedModelField> for GeneratedRelationPath {
				type RelatedFieldOrigin = UnverifiedRelatedField;
			}

			#[derive(Debug, Clone, Copy, PartialEq, Eq)]
			pub enum RelationJoinKind {
				Inner,
				Left,
			}

			#[derive(Debug, Clone, Copy, PartialEq, Eq)]
			pub enum RelationMultiplicity {
				Single,
				Multiple,
			}

			#[derive(Debug, Clone, PartialEq, Eq)]
			pub struct RelationStep {
				pub name: Cow<'static, str>,
				pub source_table: Cow<'static, str>,
				pub target_table: Cow<'static, str>,
				pub source_column: Cow<'static, str>,
				pub target_column: Cow<'static, str>,
				pub default_join_kind: RelationJoinKind,
				pub multiplicity: RelationMultiplicity,
			}

			pub trait RelationDescriptor {
				type Source: Model;
				type Target: Model;

				fn steps() -> Vec<RelationStep>;
			}

			pub trait RelationPathLike {
				type Root: Model;
				type Target: Model;

				fn steps(&self) -> &[RelationStep];
				fn join_kind(&self) -> RelationJoinKind;
				fn join_kind_override(&self) -> Option<RelationJoinKind> {
					None
				}
				fn leaf_alias(&self) -> &str;
				fn is_multi_valued(&self) -> bool {
					self.steps()
						.iter()
						.any(|step| step.multiplicity == RelationMultiplicity::Multiple)
				}
			}

			pub struct RelationPath<Root: Model, Target: Model, Origin = UnverifiedRelationPath> {
				steps: Vec<RelationStep>,
				step_aliases: Vec<String>,
				join_kind_override: Option<RelationJoinKind>,
				_marker: core::marker::PhantomData<(Root, Target, Origin)>,
			}

			impl<Root: Model, Target: Model, Origin> RelationPath<Root, Target, Origin> {
				fn from_steps(steps: Vec<RelationStep>) -> Self {
					let step_aliases = step_aliases(&steps);
					Self {
						steps,
						step_aliases,
						join_kind_override: None,
						_marker: core::marker::PhantomData,
					}
				}

				fn into_unverified(self) -> RelationPath<Root, Target> {
					RelationPath {
						steps: self.steps,
						step_aliases: self.step_aliases,
						join_kind_override: self.join_kind_override,
						_marker: core::marker::PhantomData,
					}
				}

				pub fn into_typed(self) -> <Target as RelationTarget>::Path<Root, Origin>
				where
					Target: RelationTarget,
				{
					Target::wrap_relation_path(self)
				}

				pub fn then<D, Next>(self) -> RelationPath<Root, Next>
				where
					D: RelationDescriptor<Source = Target, Target = Next>,
					Next: Model,
				{
					let mut steps = self.steps;
					steps.extend(D::steps());
					RelationPath::from_steps(steps)
				}

				pub unsafe fn extend_generated_descriptor<D, Next>(
					self,
				) -> RelationPath<Root, Next, Origin>
				where
					D: RelationDescriptor<Source = Target, Target = Next>,
					Next: Model,
				{
					let mut steps = self.steps;
					steps.extend(D::steps());
					RelationPath::from_steps(steps)
				}

				pub fn optional(mut self) -> Self {
					self.join_kind_override = Some(RelationJoinKind::Left);
					self
				}

				pub fn field<Value, FieldOrigin>(
					self,
					field: super::expressions::FieldRef<Target, Value, FieldOrigin>,
				) -> RelatedFieldRef<
					Root,
					Target,
					Value,
					<Origin as RelationFieldOrigin<FieldOrigin>>::RelatedFieldOrigin,
				>
				where
					Origin: RelationFieldOrigin<FieldOrigin>,
				{
					RelatedFieldRef::from_name(self, field.name())
				}
			}

			impl<Root: Model, Target: Model> RelationPath<Root, Target, UnverifiedRelationPath> {
				pub fn new(steps: &'static [RelationStep]) -> Self {
					Self::from_steps(steps.to_vec())
				}

				pub fn from_descriptor<D>() -> Self
				where
					D: RelationDescriptor<Source = Root, Target = Target>,
				{
					Self::from_steps(D::steps())
				}
			}

			impl<Root: Model, Target: Model> RelationPath<Root, Target, GeneratedRelationPath> {
				pub unsafe fn from_generated_steps(steps: Vec<RelationStep>) -> Self {
					Self::from_steps(steps)
				}
			}

			impl<Root: Model, Target: Model, Origin> RelationPathLike for RelationPath<Root, Target, Origin> {
				type Root = Root;
				type Target = Target;

				fn steps(&self) -> &[RelationStep] {
					&self.steps
				}

				fn join_kind(&self) -> RelationJoinKind {
					self.join_kind_override.unwrap_or(RelationJoinKind::Inner)
				}

				fn join_kind_override(&self) -> Option<RelationJoinKind> {
					self.join_kind_override
				}

				fn leaf_alias(&self) -> &str {
					self.step_aliases
						.last()
						.map(String::as_str)
						.unwrap_or_else(|| Target::table_name())
				}
			}

			fn step_aliases(steps: &[RelationStep]) -> Vec<String> {
				let mut aliases = Vec::new();
				let mut source_alias = String::new();
				for (index, step) in steps.iter().enumerate() {
					let alias = if index == 0 {
						step.name.to_string()
					} else {
						format!("{}__{}", source_alias, step.name)
					};
					source_alias = alias.clone();
					aliases.push(alias);
				}
				aliases
			}

			pub struct RelatedFieldRef<
				Root: Model,
				Target: Model,
				Value,
				Origin = UnverifiedRelatedField,
			> {
				field: &'static str,
				_path: RelationPath<Root, Target>,
				_marker: core::marker::PhantomData<(Value, Origin)>,
			}

			impl<Root: Model, Target: Model, Value>
				RelatedFieldRef<Root, Target, Value, UnverifiedRelatedField>
			{
				pub fn new<PathOrigin>(
					path: RelationPath<Root, Target, PathOrigin>,
					field: &'static str,
				) -> Self {
					Self {
						field,
						_path: path.into_unverified(),
						_marker: core::marker::PhantomData,
					}
				}
			}

			impl<Root: Model, Target: Model, Value, Origin> RelatedFieldRef<Root, Target, Value, Origin> {
				fn from_name<PathOrigin>(
					path: RelationPath<Root, Target, PathOrigin>,
					field: &'static str,
				) -> Self {
					Self {
						field,
						_path: path.into_unverified(),
						_marker: core::marker::PhantomData,
					}
				}

				pub fn name(&self) -> &'static str {
					self.field
				}

				pub fn eq(self, _value: impl Into<String>) -> bool {
					true
				}

				pub fn icontains(self, _value: impl Into<String>) -> bool {
					true
				}

				pub fn is_null(self) -> bool {
					true
				}
			}

			pub trait RelationTarget: Model {
				type Path<Root: Model, Origin>: RelationPathLike<Root = Root, Target = Self>;

				fn wrap_relation_path<Root: Model, Origin>(
					path: RelationPath<Root, Self, Origin>,
				) -> Self::Path<Root, Origin>
				where
					Self: Sized;
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

		#[derive(Debug, Clone, Copy, PartialEq, Eq)]
		pub enum DatabaseStorageKind {
			Bool,
			I32,
			I64,
			F32,
			F64,
			Decimal,
			String,
			Bytes,
			Json,
			Vector(usize),
			Uuid,
			Date,
			Time,
			DateTime,
			NaiveDateTime,
		}

		#[derive(Debug, Clone, Copy, PartialEq, Eq)]
		pub enum ModelEnumRepr {
			String,
			I32,
		}

		#[derive(Debug, Clone, PartialEq, Eq)]
		pub enum ModelEnumValue {
			String(String),
			I32(i32),
		}

		#[derive(Debug, Clone, PartialEq, Eq)]
		pub enum FieldDomain {
			Enum {
				repr: ModelEnumRepr,
				values: Vec<ModelEnumValue>,
			},
		}

		#[derive(Debug, Clone)]
		pub struct FieldCodecContext;

		impl FieldCodecContext {
			pub fn new(
				_model: impl Into<String>,
				_field: impl Into<String>,
				_column: impl Into<String>,
			) -> Self {
				Self
			}
		}

		#[derive(Debug, Clone)]
		pub struct FieldCodecError;

		#[derive(Debug, Clone)]
		pub struct DatabaseValue(serde_json::Value);

		impl DatabaseValue {
			pub fn into_json_value(self) -> Result<serde_json::Value, FieldCodecError> {
				Ok(self.0)
			}
		}

		pub trait DatabaseScalar: Clone {
			const STORAGE_KIND: DatabaseStorageKind;
			fn into_database_value(self) -> DatabaseValue;
			fn from_database_value(value: DatabaseValue) -> Result<Self, FieldCodecError>;
		}

		pub trait DatabaseField:
			Clone + serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static
		{
			type Storage: DatabaseScalar;
			const MAX_STRING_VALUE_CHARS: Option<usize> = None;

			fn encode_database(&self) -> Result<Self::Storage, FieldCodecError>;
			fn decode_database(
				value: Self::Storage,
				_context: &FieldCodecContext,
			) -> Result<Self, FieldCodecError>;
			fn validate_database_context(
				&self,
				_context: &FieldCodecContext,
			) -> Result<(), FieldCodecError> {
				Ok(())
			}
			fn domain() -> Option<FieldDomain> {
				None
			}
		}

		pub trait IntoFieldValue<T> {
			fn into_field_value(self) -> Result<DatabaseValue, FieldCodecError>;
		}

		impl<T: DatabaseField> IntoFieldValue<T> for T {
			fn into_field_value(self) -> Result<DatabaseValue, FieldCodecError> {
				self.encode_database()
					.map(DatabaseScalar::into_database_value)
			}
		}

		pub enum FilterOperator {
			Eq,
		}

		pub enum FilterValue {
			Typed(Result<DatabaseValue, FieldCodecError>),
		}

		pub struct Filter;

		impl Filter {
			pub fn new(
				_field: impl Into<String>,
				_operator: FilterOperator,
				_value: FilterValue,
			) -> Self {
				Self
			}
		}

		macro_rules! scalar_codec {
			($type:ty, $kind:ident) => {
				impl DatabaseScalar for $type {
					const STORAGE_KIND: DatabaseStorageKind = DatabaseStorageKind::$kind;

					fn into_database_value(self) -> DatabaseValue {
						DatabaseValue(serde_json::to_value(self).unwrap())
					}

					fn from_database_value(value: DatabaseValue) -> Result<Self, FieldCodecError> {
						serde_json::from_value(value.0).map_err(|_| FieldCodecError)
					}
				}

				impl DatabaseField for $type {
					type Storage = Self;

					fn encode_database(&self) -> Result<Self::Storage, FieldCodecError> {
						Ok(self.clone())
					}

					fn decode_database(
						value: Self::Storage,
						_context: &FieldCodecContext,
					) -> Result<Self, FieldCodecError> {
						Ok(value)
					}
				}
			};
		}

		scalar_codec!(bool, Bool);
		scalar_codec!(i32, I32);
		scalar_codec!(i64, I64);
		scalar_codec!(String, String);
		scalar_codec!(chrono::DateTime<chrono::Utc>, DateTime);
		scalar_codec!(chrono::NaiveDateTime, DateTime);

		impl<S: DatabaseScalar> DatabaseScalar for Option<S> {
			const STORAGE_KIND: DatabaseStorageKind = S::STORAGE_KIND;

			fn into_database_value(self) -> DatabaseValue {
				self.map_or_else(
					|| DatabaseValue(serde_json::Value::Null),
					DatabaseScalar::into_database_value,
				)
			}

			fn from_database_value(value: DatabaseValue) -> Result<Self, FieldCodecError> {
				if value.0.is_null() {
					Ok(None)
				} else {
					S::from_database_value(value).map(Some)
				}
			}
		}

		impl<T: DatabaseField> DatabaseField for Option<T> {
			type Storage = Option<T::Storage>;

			fn encode_database(&self) -> Result<Self::Storage, FieldCodecError> {
				self.as_ref()
					.map(DatabaseField::encode_database)
					.transpose()
			}

			fn decode_database(
				value: Self::Storage,
				context: &FieldCodecContext,
			) -> Result<Self, FieldCodecError> {
				value
					.map(|value| T::decode_database(value, context))
					.transpose()
			}
		}

		impl<T> DatabaseField for super::Json<T>
		where
			T: Clone + serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
		{
			type Storage = String;

			fn encode_database(&self) -> Result<Self::Storage, FieldCodecError> {
				serde_json::to_string(&self.0).map_err(|_| FieldCodecError)
			}

			fn decode_database(
				value: Self::Storage,
				_context: &FieldCodecContext,
			) -> Result<Self, FieldCodecError> {
				serde_json::from_str(&value)
					.map(super::Json)
					.map_err(|_| FieldCodecError)
			}
		}

		pub mod model {
			pub type ModelFieldJsonValue = serde_json::Value;

			pub fn deserialize_primary_key_from_str<T>(
				value: &str,
			) -> std::result::Result<T, serde_json::Error>
			where
				T: serde::de::DeserializeOwned,
			{
				serde_json::from_value(serde_json::Value::String(value.to_owned()))
					.or_else(|_| serde_json::from_str(value))
			}

			pub fn deserialize_primary_key_from_database_str<M>(
				value: &str,
			) -> std::result::Result<M::PrimaryKey, serde_json::Error>
			where
				M: super::Model,
				M::PrimaryKey: serde::de::DeserializeOwned,
			{
				deserialize_primary_key_from_str(value)
			}

			pub fn serialize_decoded_database_field<T: serde::Serialize>(
				value: T,
			) -> Result<ModelFieldJsonValue, super::FieldCodecError> {
				serde_json::to_value(value).map_err(|_| super::FieldCodecError)
			}
		}

		pub mod inspection {
			use super::fields::FieldKwarg;
			use super::{DatabaseStorageKind, FieldDomain};
			use std::collections::HashMap;

			pub fn database_storage_field_type(
				storage_kind: DatabaseStorageKind,
				_max_length: Option<u32>,
			) -> crate::db::migrations::FieldType {
				match storage_kind {
					DatabaseStorageKind::DateTime => crate::db::migrations::FieldType::TimestampTz,
					DatabaseStorageKind::NaiveDateTime => {
						crate::db::migrations::FieldType::DateTime
					}
					_ => crate::db::migrations::FieldType::Json,
				}
			}

			pub fn database_field_type_path(storage_kind: DatabaseStorageKind) -> &'static str {
				match storage_kind {
					DatabaseStorageKind::Bool => "reinhardt.orm.models.BooleanField",
					DatabaseStorageKind::I32 => "reinhardt.orm.models.IntegerField",
					DatabaseStorageKind::I64 => "reinhardt.orm.models.BigIntegerField",
					DatabaseStorageKind::F32 | DatabaseStorageKind::F64 => {
						"reinhardt.orm.models.FloatField"
					}
					DatabaseStorageKind::Decimal => "reinhardt.orm.models.DecimalField",
					DatabaseStorageKind::String => "reinhardt.orm.models.CharField",
					DatabaseStorageKind::Bytes => "reinhardt.orm.models.BinaryField",
					DatabaseStorageKind::Json => "reinhardt.orm.models.JsonField",
					DatabaseStorageKind::Vector(_) => "reinhardt.orm.models.VectorField",
					DatabaseStorageKind::Uuid => "reinhardt.orm.models.UuidField",
					DatabaseStorageKind::Date => "reinhardt.orm.models.DateField",
					DatabaseStorageKind::Time => "reinhardt.orm.models.TimeField",
					DatabaseStorageKind::DateTime | DatabaseStorageKind::NaiveDateTime => {
						"reinhardt.orm.models.DateTimeField"
					}
				}
			}

			#[derive(Debug, Clone, PartialEq)]
			pub struct FieldInfo {
				pub name: String,
				pub field_type: String,
				pub storage_kind: Option<DatabaseStorageKind>,
				pub domain: Option<FieldDomain>,
				pub nullable: bool,
				pub primary_key: bool,
				pub unique: bool,
				pub blank: bool,
				pub editable: bool,
				pub default: Option<FieldKwarg>,
				pub db_default: Option<FieldKwarg>,
				pub db_column: Option<String>,
				pub choices: Option<Vec<String>>,
				pub attributes: HashMap<String, FieldKwarg>,
			}

			#[derive(Debug, Clone, PartialEq)]
			pub enum IndexMetadataType {
				Hnsw {
					m: Option<u16>,
					ef_construction: Option<u16>,
				},
				Ivfflat {
					lists: Option<u32>,
				},
			}

			#[derive(Debug, Clone, PartialEq)]
			pub struct IndexInfo {
				pub name: String,
				pub fields: Vec<String>,
				pub unique: bool,
				pub condition: Option<String>,
				pub index_type: Option<IndexMetadataType>,
				pub operator_class: Option<String>,
				pub expressions: Option<Vec<String>>,
			}

			impl IndexInfo {
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
						index_type: None,
						operator_class: None,
						expressions: None,
					}
				}
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
				pub fields: Vec<String>,
				pub condition: Option<String>,
				pub deferrable: bool,
				pub nulls_distinct: Option<bool>,
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

		pub mod fixtures {
			pub struct FixtureRegistry;

			pub fn __deserialize_fixture_projection<T>(
				_fields: &super::FixtureFields,
			) -> core::result::Result<T, String>
			where
				T: serde::de::DeserializeOwned,
			{
				Err("fixture projection validation is unavailable in UI test support".to_string())
			}

			impl FixtureRegistry {
				pub fn register_model<T>(&self)
				where
					T: super::Model + 'static,
				{
					let _ = core::any::type_name::<T>();
				}
			}

			pub fn global_fixture_registry() -> FixtureRegistry {
				FixtureRegistry
			}
		}
	}

	pub mod migrations {
		pub mod operations {
			pub fn default_index_name(table: &str, suffix: &str) -> String {
				format!("idx_{table}_{suffix}")
			}

			#[derive(Debug, Clone, Copy, PartialEq, Eq)]
			pub enum IndexType {
				Hnsw {
					m: Option<u16>,
					ef_construction: Option<u16>,
				},
				Ivfflat {
					lists: Option<u32>,
				},
			}
		}

		#[derive(Debug, Clone, PartialEq)]
		pub enum FieldType {
			Integer,
			BigInteger,
			VarChar(u32),
			Boolean,
			TimestampTz,
			DateTime,
			Date,
			Time,
			Float,
			Double,
			Uuid,
			Json,
			Jsonb,
			Vector { dimensions: usize },
		}

		#[derive(Debug, Clone, PartialEq)]
		pub struct IndexDefinition {
			pub name: String,
			pub fields: Vec<String>,
			pub unique: bool,
			pub where_clause: Option<String>,
			pub index_type: Option<operations::IndexType>,
			pub operator_class: Option<String>,
			pub expressions: Option<Vec<String>>,
		}

		impl IndexDefinition {
			pub fn new(name: impl Into<String>, fields: Vec<String>, unique: bool) -> Self {
				Self {
					name: name.into(),
					fields,
					unique,
					where_clause: None,
					index_type: None,
					operator_class: None,
					expressions: None,
				}
			}
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
			Restrict,
			Cascade,
			SetNull,
			NoAction,
			SetDefault,
		}

		pub fn to_snake_case(value: &str) -> String {
			value.to_ascii_lowercase()
		}

		pub mod model_registry {
			use super::{ConstraintDefinition, FieldType, ForeignKeyInfo, IndexDefinition};

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

				pub fn with_domain_opt(self, _domain: Option<crate::db::orm::FieldDomain>) -> Self {
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

				pub fn add_index(&mut self, _index: IndexDefinition) {}

				pub fn add_enum_domain_constraint(
					&mut self,
					_column: impl Into<String>,
					_domain: crate::db::orm::FieldDomain,
				) {
				}
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

pub use db::m2m_naming;
