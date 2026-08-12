use crate::database_selector::{DatabaseSelector, alias_looks_sensitive, resolve_database};
use crate::{CommandContext, CommandError, CommandResult};
use reinhardt_apps::registry::{
	RelationshipMetadata, RelationshipType, get_registered_relationships,
};
use reinhardt_conf::settings::policy::FieldRequirement;
use reinhardt_conf::settings::schema::{SettingsPathBuf, SettingsPathSegment};
use reinhardt_conf::{HasCommonSettings, MigrationSettings, SettingsResolutionMetadata};
use reinhardt_db::field_domain::{FieldDomain, ModelEnumValue};
use reinhardt_db::migrations::{
	ColumnDefinition, DatabaseMigrationRecorder, FieldState, FieldType, FilesystemSource,
	ForeignKeyAction, IndexDefinition, MigrationCatalog, MigrationKey, ModelState, ProjectState,
};
use serde::Serialize;
use std::collections::{BTreeSet, HashSet};
use std::io::Write;
use std::sync::Arc;

#[cfg(feature = "sqlite")]
use std::path::PathBuf;

const APPLICATION_CONTRACT_SCHEMA_URL: &str =
	"https://reinhardt-web.dev/schemas/application-contract/v0.json";

#[derive(Serialize)]
struct ApplicationContractV0 {
	#[serde(rename = "$schema")]
	schema: &'static str,
	schema_version: u8,
	models: Vec<ModelContract>,
	migrations: Vec<MigrationContract>,
	routes: Vec<RouteContract>,
	settings: Vec<SettingContract>,
}

#[derive(Serialize)]
struct ModelContract {
	app_label: String,
	model_name: String,
	table_name: String,
	fields: Vec<ModelFieldContract>,
	constraints: Vec<ConstraintContract>,
	indexes: Vec<IndexContract>,
	relationships: Vec<RelationshipContract>,
}

#[derive(Serialize)]
struct ModelFieldContract {
	name: String,
	#[serde(rename = "type")]
	field_type: ContractFieldType,
	nullable: bool,
	primary_key: bool,
	unique: bool,
	default: Option<String>,
	generated: Option<GeneratedFieldContract>,
}

#[derive(Serialize)]
struct GeneratedFieldContract {
	expression: String,
	storage: GeneratedStorageContract,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum GeneratedStorageContract {
	Stored,
	Virtual,
}

#[derive(Serialize)]
struct ConstraintContract {
	name: String,
	kind: ConstraintKindContract,
	fields: Vec<String>,
	expression: Option<String>,
	references: Option<ConstraintReferenceContract>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ConstraintKindContract {
	PrimaryKey,
	Unique,
	Check,
	ForeignKey,
	OneToOne,
	ManyToMany,
	Exclude,
	EnumDomain,
}

impl ConstraintKindContract {
	fn as_str(&self) -> &'static str {
		match self {
			Self::PrimaryKey => "primary_key",
			Self::Unique => "unique",
			Self::Check => "check",
			Self::ForeignKey => "foreign_key",
			Self::OneToOne => "one_to_one",
			Self::ManyToMany => "many_to_many",
			Self::Exclude => "exclude",
			Self::EnumDomain => "enum_domain",
		}
	}
}

#[derive(Serialize)]
struct ConstraintReferenceContract {
	table: String,
	columns: Vec<String>,
	on_delete: ForeignKeyActionContract,
	on_update: ForeignKeyActionContract,
}

#[derive(Serialize)]
struct IndexContract {
	name: String,
	fields: Vec<String>,
	unique: bool,
	predicate: Option<String>,
	method: Option<IndexMethodContract>,
	operator_class: Option<String>,
	expressions: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum IndexMethodContract {
	#[serde(rename = "btree")]
	BTree,
	Hash,
	Gin,
	Gist,
	Brin,
	Fulltext,
	Spatial,
	#[cfg(feature = "pgvector")]
	Hnsw {
		m: Option<u16>,
		ef_construction: Option<u16>,
	},
	#[cfg(feature = "pgvector")]
	#[serde(rename = "ivfflat")]
	IvfFlat {
		lists: Option<u32>,
	},
}

#[derive(Serialize)]
struct RelationshipContract {
	field: String,
	kind: RelationshipKindContract,
	target: RelationshipTargetContract,
	related_name: Option<String>,
	through_table: Option<String>,
	on_delete: Option<ForeignKeyActionContract>,
	on_update: Option<ForeignKeyActionContract>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum RelationshipKindContract {
	ForeignKey,
	OneToOne,
	ManyToMany,
}

impl RelationshipKindContract {
	fn as_str(&self) -> &'static str {
		match self {
			Self::ForeignKey => "foreign_key",
			Self::OneToOne => "one_to_one",
			Self::ManyToMany => "many_to_many",
		}
	}
}

#[derive(Serialize)]
struct RelationshipTargetContract {
	app_label: Option<String>,
	model_name: Option<String>,
	table_name: String,
	field_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ForeignKeyActionContract {
	Restrict,
	Cascade,
	SetNull,
	NoAction,
	SetDefault,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ContractEnumValue {
	String(String),
	Integer(i32),
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ContractFieldType {
	BigInteger,
	Integer,
	SmallInteger,
	TinyInt,
	MediumInt,
	Char {
		max_length: u32,
	},
	#[serde(rename = "varchar")]
	VarChar {
		max_length: u32,
	},
	Text,
	TinyText,
	MediumText,
	LongText,
	Date,
	Time,
	#[serde(rename = "datetime")]
	DateTime,
	TimestampTz,
	Decimal {
		precision: u32,
		scale: u32,
	},
	Float,
	Double,
	Real,
	Boolean,
	Binary,
	Blob,
	TinyBlob,
	MediumBlob,
	LongBlob,
	Bytea,
	Json,
	JsonBinary,
	Array {
		element: Box<ContractFieldType>,
	},
	#[serde(rename = "hstore")]
	HStore,
	#[serde(rename = "citext")]
	CiText,
	Int4Range,
	Int8Range,
	NumRange,
	DateRange,
	TsRange,
	TsTzRange,
	TsVector,
	TsQuery,
	#[cfg(feature = "pgvector")]
	Vector {
		dimensions: usize,
	},
	Uuid,
	Year,
	Enum {
		values: Vec<ContractEnumValue>,
	},
	Set {
		values: Vec<String>,
	},
	ForeignKey {
		to_table: String,
		to_field: String,
		on_delete: ForeignKeyActionContract,
	},
	OneToOne {
		to: String,
		on_delete: ForeignKeyActionContract,
		on_update: ForeignKeyActionContract,
	},
	ManyToMany {
		to: String,
		through: Option<String>,
	},
	Custom {
		name: String,
	},
}

#[derive(Serialize)]
struct MigrationIdentityContract {
	app_label: String,
	name: String,
}

#[derive(Serialize)]
struct MigrationContract {
	app_label: String,
	name: String,
	dependencies: Vec<MigrationIdentityContract>,
	replaces: Vec<MigrationIdentityContract>,
	applied: Option<bool>,
}

#[derive(Serialize)]
struct RouteContract {
	path: String,
	method: String,
	name: Option<String>,
	handler: String,
	authentication: AuthenticationContract,
	guard: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum AuthenticationContract {
	Protected,
	Optional,
	Public,
	None,
}

impl AuthenticationContract {
	fn as_str(&self) -> &'static str {
		match self {
			Self::Protected => "protected",
			Self::Optional => "optional",
			Self::Public => "public",
			Self::None => "none",
		}
	}
}

#[derive(Serialize)]
struct SettingContract {
	key_path: String,
	rust_type: String,
	required: bool,
	has_default: bool,
	secret: bool,
	present: bool,
}

impl ApplicationContractV0 {
	fn sort_canonical(&mut self) {
		for model in &mut self.models {
			model
				.fields
				.sort_by(|left, right| left.name.cmp(&right.name));
			model.constraints.sort_by(|left, right| {
				left.name
					.cmp(&right.name)
					.then_with(|| left.kind.as_str().cmp(right.kind.as_str()))
					.then_with(|| left.fields.cmp(&right.fields))
			});
			model.indexes.sort_by(|left, right| {
				left.name
					.cmp(&right.name)
					.then_with(|| left.fields.cmp(&right.fields))
			});
			model.relationships.sort_by(|left, right| {
				left.field
					.cmp(&right.field)
					.then_with(|| left.kind.as_str().cmp(right.kind.as_str()))
					.then_with(|| {
						relationship_target_key(&left.target)
							.cmp(&relationship_target_key(&right.target))
					})
			});
		}
		self.models.sort_by(|left, right| {
			left.app_label
				.cmp(&right.app_label)
				.then_with(|| left.model_name.cmp(&right.model_name))
				.then_with(|| left.table_name.cmp(&right.table_name))
		});
		for migration in &mut self.migrations {
			migration.dependencies.sort_by(migration_identity_cmp);
			migration.replaces.sort_by(migration_identity_cmp);
		}
		self.routes.sort_by(|left, right| {
			left.path
				.cmp(&right.path)
				.then_with(|| left.method.cmp(&right.method))
				.then_with(|| left.handler.cmp(&right.handler))
				.then_with(|| left.name.cmp(&right.name))
				.then_with(|| {
					left.authentication
						.as_str()
						.cmp(right.authentication.as_str())
				})
				.then_with(|| left.guard.cmp(&right.guard))
		});
		self.settings.sort_by(|left, right| {
			left.key_path
				.cmp(&right.key_path)
				.then_with(|| left.rust_type.cmp(&right.rust_type))
		});
	}
}

fn relationship_target_key(
	target: &RelationshipTargetContract,
) -> (&Option<String>, &Option<String>, &String, &Option<String>) {
	(
		&target.app_label,
		&target.model_name,
		&target.table_name,
		&target.field_name,
	)
}

fn migration_identity_cmp(
	left: &MigrationIdentityContract,
	right: &MigrationIdentityContract,
) -> std::cmp::Ordering {
	left.app_label
		.cmp(&right.app_label)
		.then_with(|| left.name.cmp(&right.name))
}

fn resolution_error(detail: impl Into<String>) -> CommandError {
	CommandError::ExecutionError(format!(
		"Failed to resolve application contract: {}",
		detail.into()
	))
}

fn foreign_key_action(action: ForeignKeyAction) -> ForeignKeyActionContract {
	match action {
		ForeignKeyAction::Restrict => ForeignKeyActionContract::Restrict,
		ForeignKeyAction::Cascade => ForeignKeyActionContract::Cascade,
		ForeignKeyAction::SetNull => ForeignKeyActionContract::SetNull,
		ForeignKeyAction::NoAction => ForeignKeyActionContract::NoAction,
		ForeignKeyAction::SetDefault => ForeignKeyActionContract::SetDefault,
	}
}

fn contract_field_type(field: &FieldState) -> CommandResult<ContractFieldType> {
	if let Some(FieldDomain::Enum { values, .. }) = &field.domain {
		return Ok(ContractFieldType::Enum {
			values: values
				.iter()
				.map(|value| match value {
					ModelEnumValue::String(value) => ContractEnumValue::String(value.clone()),
					ModelEnumValue::I32(value) => ContractEnumValue::Integer(*value),
				})
				.collect(),
		});
	}

	Ok(match &field.field_type {
		FieldType::BigInteger => ContractFieldType::BigInteger,
		FieldType::Integer => ContractFieldType::Integer,
		FieldType::SmallInteger => ContractFieldType::SmallInteger,
		FieldType::TinyInt => ContractFieldType::TinyInt,
		FieldType::MediumInt => ContractFieldType::MediumInt,
		FieldType::Char(max_length) => ContractFieldType::Char {
			max_length: *max_length,
		},
		FieldType::VarChar(max_length) => ContractFieldType::VarChar {
			max_length: *max_length,
		},
		FieldType::Text => ContractFieldType::Text,
		FieldType::TinyText => ContractFieldType::TinyText,
		FieldType::MediumText => ContractFieldType::MediumText,
		FieldType::LongText => ContractFieldType::LongText,
		FieldType::Date => ContractFieldType::Date,
		FieldType::Time => ContractFieldType::Time,
		FieldType::DateTime => ContractFieldType::DateTime,
		FieldType::TimestampTz => ContractFieldType::TimestampTz,
		FieldType::Decimal { precision, scale } => ContractFieldType::Decimal {
			precision: *precision,
			scale: *scale,
		},
		FieldType::Float => ContractFieldType::Float,
		FieldType::Double => ContractFieldType::Double,
		FieldType::Real => ContractFieldType::Real,
		FieldType::Boolean => ContractFieldType::Boolean,
		FieldType::Binary => ContractFieldType::Binary,
		FieldType::Blob => ContractFieldType::Blob,
		FieldType::TinyBlob => ContractFieldType::TinyBlob,
		FieldType::MediumBlob => ContractFieldType::MediumBlob,
		FieldType::LongBlob => ContractFieldType::LongBlob,
		FieldType::Bytea => ContractFieldType::Bytea,
		FieldType::Json => ContractFieldType::Json,
		FieldType::JsonBinary => ContractFieldType::JsonBinary,
		FieldType::Array(element) => ContractFieldType::Array {
			element: Box::new(contract_field_type(&FieldState::new(
				"element",
				*element.clone(),
				false,
			))?),
		},
		FieldType::HStore => ContractFieldType::HStore,
		FieldType::CIText => ContractFieldType::CiText,
		FieldType::Int4Range => ContractFieldType::Int4Range,
		FieldType::Int8Range => ContractFieldType::Int8Range,
		FieldType::NumRange => ContractFieldType::NumRange,
		FieldType::DateRange => ContractFieldType::DateRange,
		FieldType::TsRange => ContractFieldType::TsRange,
		FieldType::TsTzRange => ContractFieldType::TsTzRange,
		FieldType::TsVector => ContractFieldType::TsVector,
		FieldType::TsQuery => ContractFieldType::TsQuery,
		#[cfg(feature = "pgvector")]
		FieldType::Vector { dimensions } => ContractFieldType::Vector {
			dimensions: *dimensions,
		},
		FieldType::Uuid => ContractFieldType::Uuid,
		FieldType::Year => ContractFieldType::Year,
		FieldType::Enum { values } => ContractFieldType::Enum {
			values: values
				.iter()
				.cloned()
				.map(ContractEnumValue::String)
				.collect(),
		},
		FieldType::Set { values } => ContractFieldType::Set {
			values: values.clone(),
		},
		FieldType::ForeignKey {
			to_table,
			to_field,
			on_delete,
		} => ContractFieldType::ForeignKey {
			to_table: to_table.clone(),
			to_field: to_field.clone(),
			on_delete: foreign_key_action(*on_delete),
		},
		FieldType::OneToOne {
			to,
			on_delete,
			on_update,
		} => ContractFieldType::OneToOne {
			to: to.clone(),
			on_delete: foreign_key_action(*on_delete),
			on_update: foreign_key_action(*on_update),
		},
		FieldType::ManyToMany { to, through } => ContractFieldType::ManyToMany {
			to: to.clone(),
			through: through.clone(),
		},
		FieldType::Custom(name) => ContractFieldType::Custom { name: name.clone() },
	})
}

fn contract_field_type_for_model_field(field: &FieldState) -> CommandResult<ContractFieldType> {
	let mut resolved = field.clone();
	resolved.field_type =
		ColumnDefinition::from_field_state(field.name.clone(), field).type_definition;
	contract_field_type(&resolved)
}

fn generated_field(field: &FieldState) -> CommandResult<Option<GeneratedFieldContract>> {
	let Some(generated) = &field.generated else {
		return Ok(None);
	};
	let expression = generated
		.expr_tokens
		.clone()
		.or_else(|| generated.raw_sql.clone())
		.ok_or_else(|| {
			resolution_error(format!(
				"generated field `{}` has no serializable expression",
				field.name
			))
		})?;
	let storage = match generated.storage {
		reinhardt_db::migrations::GeneratedStorage::Stored => GeneratedStorageContract::Stored,
		reinhardt_db::migrations::GeneratedStorage::Virtual => GeneratedStorageContract::Virtual,
		_ => {
			return Err(resolution_error(format!(
				"generated field `{}` uses an unknown storage mode",
				field.name
			)));
		}
	};
	Ok(Some(GeneratedFieldContract {
		expression,
		storage,
	}))
}

fn constraint_kind(value: &str) -> CommandResult<ConstraintKindContract> {
	match value {
		"primary_key" => Ok(ConstraintKindContract::PrimaryKey),
		"unique" => Ok(ConstraintKindContract::Unique),
		"check" => Ok(ConstraintKindContract::Check),
		"foreign_key" => Ok(ConstraintKindContract::ForeignKey),
		"one_to_one" => Ok(ConstraintKindContract::OneToOne),
		"many_to_many" => Ok(ConstraintKindContract::ManyToMany),
		"exclude" => Ok(ConstraintKindContract::Exclude),
		"enum_domain" => Ok(ConstraintKindContract::EnumDomain),
		_ => Err(resolution_error(format!(
			"constraint type `{value}` is not supported by contract version 0"
		))),
	}
}

fn constraint_contract(
	constraint: &reinhardt_db::migrations::ConstraintDefinition,
) -> CommandResult<ConstraintContract> {
	let kind = constraint_kind(&constraint.constraint_type)?;
	let references = if matches!(
		kind,
		ConstraintKindContract::ForeignKey | ConstraintKindContract::OneToOne
	) {
		let info = constraint.foreign_key_info.as_ref().ok_or_else(|| {
			resolution_error(format!(
				"foreign key constraint `{}` has no reference metadata",
				constraint.name
			))
		})?;
		Some(ConstraintReferenceContract {
			table: info.referenced_table.clone(),
			columns: info.referenced_columns.clone(),
			on_delete: foreign_key_action(info.on_delete),
			on_update: foreign_key_action(info.on_update),
		})
	} else {
		None
	};
	Ok(ConstraintContract {
		name: constraint.name.clone(),
		kind,
		fields: constraint.fields.clone(),
		expression: constraint.expression.clone(),
		references,
	})
}

fn index_method(index: &IndexDefinition) -> Option<IndexMethodContract> {
	index.index_type().map(|method| match method {
		reinhardt_db::migrations::IndexType::BTree => IndexMethodContract::BTree,
		reinhardt_db::migrations::IndexType::Hash => IndexMethodContract::Hash,
		reinhardt_db::migrations::IndexType::Gin => IndexMethodContract::Gin,
		reinhardt_db::migrations::IndexType::Gist => IndexMethodContract::Gist,
		reinhardt_db::migrations::IndexType::Brin => IndexMethodContract::Brin,
		reinhardt_db::migrations::IndexType::Fulltext => IndexMethodContract::Fulltext,
		reinhardt_db::migrations::IndexType::Spatial => IndexMethodContract::Spatial,
		#[cfg(feature = "pgvector")]
		reinhardt_db::migrations::IndexType::Hnsw { m, ef_construction } => {
			IndexMethodContract::Hnsw { m, ef_construction }
		}
		#[cfg(feature = "pgvector")]
		reinhardt_db::migrations::IndexType::Ivfflat { lists } => IndexMethodContract::IvfFlat { lists },
	})
}

fn split_model_identity(identity: &str) -> (Option<String>, String) {
	identity.split_once('.').map_or_else(
		|| (None, identity.to_string()),
		|(app_label, model_name)| (Some(app_label.to_string()), model_name.to_string()),
	)
}

fn model_by_identity<'a>(
	state: &'a ProjectState,
	source_app: &str,
	identity: &str,
) -> Option<&'a ModelState> {
	let (app_label, model_name) = split_model_identity(identity);
	if let Some(app_label) = app_label {
		return state.models.get(&(app_label, model_name));
	}
	state
		.models
		.get(&(source_app.to_string(), model_name.clone()))
		.or_else(|| {
			let mut matches = state
				.models
				.values()
				.filter(|model| model.name == model_name);
			let model = matches.next()?;
			matches.next().is_none().then_some(model)
		})
}

fn inventory_relationship<'a>(
	relationships: &'a [RelationshipMetadata],
	qualified_source: &str,
	physical_field: &str,
	relationship_type: RelationshipType,
) -> Option<&'a RelationshipMetadata> {
	relationships.iter().find(|relationship| {
		relationship.from_model == qualified_source
			&& relationship.relationship_type == relationship_type
			&& (relationship.field_name == physical_field
				|| relationship.db_column == Some(physical_field))
	})
}

fn logical_target(
	state: &ProjectState,
	source_app: &str,
	identity: &str,
	physical_table: &str,
) -> (Option<String>, Option<String>) {
	let (declared_app, declared_model) = split_model_identity(identity);
	let resolved = match declared_app.as_ref() {
		Some(app) => state
			.models
			.get(&(app.clone(), declared_model.clone()))
			.or_else(|| {
				state
					.models
					.values()
					.find(|model| model.table_name == physical_table)
			}),
		None => state
			.models
			.values()
			.find(|model| model.table_name == physical_table)
			.or_else(|| model_by_identity(state, source_app, identity)),
	};
	match resolved {
		Some(model) => (Some(model.app_label.clone()), Some(model.name.clone())),
		None => (declared_app, Some(declared_model)),
	}
}

fn relationship_contracts(
	state: &ProjectState,
	model: &ModelState,
	relationships: &[RelationshipMetadata],
	skip_physical_foreign_keys: bool,
) -> CommandResult<Vec<RelationshipContract>> {
	let qualified_source = format!("{}.{}", model.app_label, model.name);
	let mut result = Vec::new();
	if !skip_physical_foreign_keys {
		for field in model.fields.values() {
			let Some(foreign_key) = &field.foreign_key else {
				continue;
			};
			let one_to_one = matches!(field.field_type, FieldType::OneToOne { .. })
				|| field
					.params
					.get("unique")
					.is_some_and(|value| value == "true");
			let relationship_type = if one_to_one {
				RelationshipType::OneToOne
			} else {
				RelationshipType::ForeignKey
			};
			let inventory = inventory_relationship(
				relationships,
				&qualified_source,
				&field.name,
				relationship_type,
			);
			let (app_label, model_name) = inventory.map_or((None, None), |relationship| {
				logical_target(
					state,
					&model.app_label,
					relationship.to_model,
					&foreign_key.referenced_table,
				)
			});
			result.push(RelationshipContract {
				field: field.name.clone(),
				kind: if one_to_one {
					RelationshipKindContract::OneToOne
				} else {
					RelationshipKindContract::ForeignKey
				},
				target: RelationshipTargetContract {
					app_label,
					model_name,
					table_name: foreign_key.referenced_table.clone(),
					field_name: Some(foreign_key.referenced_column.clone()),
				},
				related_name: inventory.and_then(|value| value.related_name.map(str::to_string)),
				through_table: None,
				on_delete: Some(foreign_key_action(foreign_key.on_delete)),
				on_update: Some(foreign_key_action(foreign_key.on_update)),
			});
		}
	}

	for many_to_many in &model.many_to_many_fields {
		let target_model = model_by_identity(state, &model.app_label, &many_to_many.to_model)
			.ok_or_else(|| {
				resolution_error(format!(
					"many-to-many field `{}.{}` has an unresolved target `{}`",
					qualified_source, many_to_many.field_name, many_to_many.to_model
				))
			})?;
		let inventory = inventory_relationship(
			relationships,
			&qualified_source,
			&many_to_many.field_name,
			RelationshipType::ManyToMany,
		);
		let (app_label, model_name) = inventory.map_or((None, None), |relationship| {
			logical_target(
				state,
				&model.app_label,
				relationship.to_model,
				&target_model.table_name,
			)
		});
		result.push(RelationshipContract {
			field: many_to_many.field_name.clone(),
			kind: RelationshipKindContract::ManyToMany,
			target: RelationshipTargetContract {
				app_label,
				model_name,
				table_name: target_model.table_name.clone(),
				field_name: None,
			},
			related_name: inventory
				.and_then(|value| value.related_name.map(str::to_string))
				.or_else(|| many_to_many.related_name.clone()),
			through_table: Some(many_to_many.through.clone().unwrap_or_else(|| {
				reinhardt_db::migrations::default_through_table(
					&model.table_name,
					&many_to_many.field_name,
				)
			})),
			on_delete: None,
			on_update: None,
		});
	}
	Ok(result)
}

fn model_contracts(state: &ProjectState) -> CommandResult<Vec<ModelContract>> {
	let relationships = get_registered_relationships();
	let through_tables = state
		.models
		.values()
		.flat_map(|model| {
			model.many_to_many_fields.iter().map(|field| {
				field.through.clone().unwrap_or_else(|| {
					reinhardt_db::migrations::default_through_table(
						&model.table_name,
						&field.field_name,
					)
				})
			})
		})
		.collect::<BTreeSet<_>>();
	state
		.models
		.values()
		.map(|model| {
			let fields = model
				.fields
				.values()
				.map(|field| {
					Ok(ModelFieldContract {
						name: field.name.clone(),
						field_type: contract_field_type_for_model_field(field)?,
						nullable: field.nullable,
						primary_key: field
							.params
							.get("primary_key")
							.is_some_and(|value| value == "true"),
						unique: field
							.params
							.get("unique")
							.is_some_and(|value| value == "true"),
						default: field.params.get("default").cloned(),
						generated: generated_field(field)?,
					})
				})
				.collect::<CommandResult<Vec<_>>>()?;
			let constraints = model
				.constraints
				.iter()
				.map(constraint_contract)
				.collect::<CommandResult<Vec<_>>>()?;
			let indexes = model
				.indexes
				.iter()
				.map(|index| IndexContract {
					name: index.name.clone(),
					fields: index.fields.clone(),
					unique: index.unique,
					predicate: index.where_clause.clone(),
					method: index_method(index),
					operator_class: index.operator_class().cloned(),
					expressions: index.expressions().cloned(),
				})
				.collect();
			Ok(ModelContract {
				app_label: model.app_label.clone(),
				model_name: model.name.clone(),
				table_name: model.table_name.clone(),
				fields,
				constraints,
				indexes,
				relationships: relationship_contracts(
					state,
					model,
					relationships,
					through_tables.contains(&model.table_name),
				)?,
			})
		})
		.collect()
}

fn migration_identity(key: &MigrationKey) -> MigrationIdentityContract {
	MigrationIdentityContract {
		app_label: key.app_label.clone(),
		name: key.name.clone(),
	}
}

async fn read_applied_migrations(
	catalog: &MigrationCatalog,
	selector: &DatabaseSelector,
	settings: Option<&dyn HasCommonSettings>,
	explicit_database: bool,
	stderr: &mut dyn Write,
) -> CommandResult<Option<HashSet<MigrationKey>>> {
	let applied = async {
		let database = resolve_database(selector, settings)?;
		#[cfg(feature = "sqlite")]
		let connection = if database.backend() == reinhardt_db::backends::DatabaseType::Sqlite {
			connect_sqlite_read_only(database.url(), settings).await?
		} else {
			database.connect().await?
		};
		#[cfg(not(feature = "sqlite"))]
		let connection = database.connect().await?;
		let recorder = DatabaseMigrationRecorder::new(connection);
		let snapshot = catalog
			.snapshot(&recorder, &[])
			.await
			.map_err(crate::squashmigrations::migration_error_to_command_error)?;
		Ok::<_, CommandError>(snapshot.applied.into_keys().collect::<HashSet<_>>())
	}
	.await;
	match applied {
		Ok(applied) => Ok(Some(applied)),
		Err(_) if !explicit_database => {
			writeln!(
				stderr,
				"Warning: migration applied state is unavailable for database alias `{}`; exporting null applied states.",
				selector.display_alias(),
			)?;
			Ok(None)
		}
		Err(_) => Err(CommandError::ExecutionError(format!(
			"Failed to read migration state for database alias `{}`.",
			selector.display_alias(),
		))),
	}
}

fn migration_contracts(
	catalog: &MigrationCatalog,
	applied: Option<&HashSet<MigrationKey>>,
) -> CommandResult<Vec<MigrationContract>> {
	catalog
		.raw_ordered_migrations()
		.map_err(crate::squashmigrations::migration_error_to_command_error)?
		.into_iter()
		.map(|(migration, dependencies)| {
			let key = MigrationKey::new(&migration.app_label, &migration.name);
			Ok(MigrationContract {
				app_label: migration.app_label.clone(),
				name: migration.name.clone(),
				dependencies: dependencies.iter().map(migration_identity).collect(),
				replaces: migration
					.replaces
					.iter()
					.map(|(app_label, name)| MigrationIdentityContract {
						app_label: app_label.clone(),
						name: name.clone(),
					})
					.collect(),
				applied: applied.map(|applied| applied.contains(&key)),
			})
		})
		.collect()
}

fn route_contracts() -> CommandResult<Vec<RouteContract>> {
	let router = reinhardt_urls::routers::get_router()
		.ok_or_else(|| resolution_error("no router is registered"))?;
	router
		.get_mounted_route_contracts()
		.map_err(resolution_error)?
		.into_iter()
		.map(|route| {
			let authentication = match format!("{:?}", route.metadata.authentication).as_str() {
				"Protected" => AuthenticationContract::Protected,
				"Optional" => AuthenticationContract::Optional,
				"Public" => AuthenticationContract::Public,
				"None" => AuthenticationContract::None,
				value => {
					return Err(resolution_error(format!(
						"mounted route `{}` uses unknown authentication metadata `{value}`",
						route.path
					)));
				}
			};
			Ok(RouteContract {
				path: route.path,
				method: route.method.as_str().to_ascii_uppercase(),
				name: route.name,
				handler: route.metadata.handler,
				authentication,
				guard: route.metadata.guard,
			})
		})
		.collect()
}

fn escape_literal_segment(segment: &str) -> String {
	if segment == "*" {
		return "\\*".to_string();
	}
	segment.replace('\\', "\\\\").replace('.', "\\.")
}

fn escape_settings_path(path: &SettingsPathBuf) -> String {
	path.segments()
		.iter()
		.map(|segment| match segment {
			SettingsPathSegment::Key(value) => escape_literal_segment(value),
			SettingsPathSegment::DynamicKey(value) => escape_literal_segment(value),
			SettingsPathSegment::AnyKey | SettingsPathSegment::AnyIndex => "*".to_string(),
		})
		.collect::<Vec<_>>()
		.join(".")
}

fn has_sensitive_dynamic_key(path: &SettingsPathBuf) -> bool {
	path.segments().iter().any(|segment| {
		matches!(segment, SettingsPathSegment::DynamicKey(value) if alias_looks_sensitive(value))
	})
}

fn setting_contracts(metadata: &SettingsResolutionMetadata) -> Vec<SettingContract> {
	metadata
		.fields()
		.iter()
		.filter(|field| !has_sensitive_dynamic_key(&field.path))
		.map(|field| SettingContract {
			key_path: escape_settings_path(&field.path),
			rust_type: field.rust_type.to_string(),
			required: field.policy.requirement == FieldRequirement::Required,
			has_default: field.policy.has_default,
			secret: field.secret,
			present: field.present,
		})
		.collect()
}

#[cfg(feature = "sqlite")]
fn contract_sqlite_path(
	url: &str,
	settings: Option<&dyn HasCommonSettings>,
) -> CommandResult<PathBuf> {
	if url == "sqlite::memory:" {
		return Err(CommandError::ExecutionError(
			"In-memory SQLite databases do not have a file path.".to_string(),
		));
	}

	let configured_base = settings
		.map(|settings| settings.core().base_dir.clone())
		.unwrap_or_else(|| PathBuf::from("."));
	let base_dir = if configured_base.is_absolute() {
		configured_base
	} else {
		std::env::current_dir()
			.map_err(|_| {
				CommandError::ExecutionError(
					"Failed to resolve project base directory.".to_string(),
				)
			})?
			.join(configured_base)
	};
	let value = url
		.strip_prefix("sqlite:///")
		.map(|value| {
			#[cfg(windows)]
			{
				if value.as_bytes().get(1) == Some(&b':') {
					value.to_string()
				} else {
					format!(r"\{value}")
				}
			}
			#[cfg(not(windows))]
			{
				if value.starts_with('/') {
					value.to_string()
				} else {
					format!("/{value}")
				}
			}
		})
		.or_else(|| url.strip_prefix("sqlite://").map(str::to_string))
		.or_else(|| url.strip_prefix("sqlite:").map(str::to_string))
		.ok_or_else(|| CommandError::ExecutionError("Invalid SQLite database URL.".to_string()))?;
	let value = value
		.split_once('?')
		.map_or(value.as_str(), |(path, _)| path);
	let path = PathBuf::from(value);
	let path = if path.is_absolute() {
		path
	} else {
		base_dir.join(path)
	};
	if !path.is_file() {
		return Err(CommandError::ExecutionError(
			"SQLite database file is unavailable.".to_string(),
		));
	}

	Ok(path)
}

#[cfg(feature = "sqlite")]
async fn connect_sqlite_read_only(
	url: &str,
	settings: Option<&dyn HasCommonSettings>,
) -> CommandResult<reinhardt_db::backends::DatabaseConnection> {
	if url == "sqlite::memory:" {
		return reinhardt_db::backends::DatabaseConnection::connect_sqlite(url)
			.await
			.map_err(|_| {
				CommandError::ExecutionError("Failed to connect to SQLite database.".to_string())
			});
	}

	let path = contract_sqlite_path(url, settings)?;
	let options = sqlx::sqlite::SqliteConnectOptions::new()
		.filename(&path)
		.read_only(true)
		.create_if_missing(false);
	let pool = sqlx::SqlitePool::connect_with(options).await.map_err(|_| {
		CommandError::ExecutionError("Failed to connect to SQLite database.".to_string())
	})?;
	Ok(reinhardt_db::backends::DatabaseConnection::from_sqlite_pool(pool))
}

fn serialize_v0(contract: &ApplicationContractV0) -> CommandResult<Vec<u8>> {
	let mut output = serde_json::to_vec_pretty(contract)?;
	output.push(b'\n');
	Ok(output)
}

fn write_v0(contract: &ApplicationContractV0, stdout: &mut dyn Write) -> CommandResult<()> {
	stdout.write_all(&serialize_v0(contract)?)?;
	Ok(())
}

pub(crate) async fn execute_contract_export(
	settings: Arc<dyn HasCommonSettings>,
	migration_settings: &MigrationSettings,
	metadata: &SettingsResolutionMetadata,
	database: Option<String>,
	database_url: Option<String>,
	stdout: &mut dyn Write,
	stderr: &mut dyn Write,
) -> CommandResult<()> {
	let state = ProjectState::try_from_global_registry()
		.map_err(crate::squashmigrations::migration_error_to_command_error)?;
	let models = model_contracts(&state)?;
	let mut context = CommandContext::new(Vec::new()).with_settings(settings.clone());
	crate::showmigrations::attach_migration_settings(&mut context, migration_settings);
	let source = FilesystemSource::new(crate::showmigrations::migration_source_path(&context));
	let dependency_context = crate::showmigrations::migration_dependency_context(&context);
	let catalog = MigrationCatalog::load_strict_with_context(&source, &dependency_context)
		.await
		.map_err(crate::squashmigrations::migration_error_to_command_error)?;
	let explicit_database = database.is_some() || database_url.is_some();
	let selector = DatabaseSelector {
		alias: database.unwrap_or_else(|| "default".to_string()),
		url_override: database_url,
	};
	let applied = read_applied_migrations(
		&catalog,
		&selector,
		Some(settings.as_ref()),
		explicit_database,
		stderr,
	)
	.await?;
	let mut contract = ApplicationContractV0 {
		schema: APPLICATION_CONTRACT_SCHEMA_URL,
		schema_version: 0,
		models,
		migrations: migration_contracts(&catalog, applied.as_ref())?,
		routes: route_contracts()?,
		settings: setting_contracts(metadata),
	};
	contract.sort_canonical();
	write_v0(&contract, stdout)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::database_selector::DatabaseSelector;
	use reinhardt_conf::settings::DatabaseConfig;
	use reinhardt_conf::settings::contacts::ContactSettings;
	use reinhardt_conf::settings::core_settings::CoreSettings;
	use reinhardt_conf::settings::fragment::HasSettings;
	use reinhardt_conf::settings::policy::{FieldPolicy, FieldRequirement};
	use reinhardt_conf::settings::schema::{
		ResolvedSettingsField, SettingsPathBuf, SettingsPathSegment,
	};
	use reinhardt_db::migrations::{FilesystemSource, ForeignKeyConstraintInfo, MigrationCatalog};
	use rstest::rstest;
	use std::collections::HashMap;

	struct TestSettings {
		core: CoreSettings,
		contacts: ContactSettings,
	}

	impl HasSettings<CoreSettings> for TestSettings {
		fn get_settings(&self) -> &CoreSettings {
			&self.core
		}
	}

	impl HasSettings<ContactSettings> for TestSettings {
		fn get_settings(&self) -> &ContactSettings {
			&self.contacts
		}
	}

	fn empty_contract() -> ApplicationContractV0 {
		ApplicationContractV0 {
			schema: "https://reinhardt-web.dev/schemas/application-contract/v0.json",
			schema_version: 0,
			models: Vec::new(),
			migrations: Vec::new(),
			routes: Vec::new(),
			settings: Vec::new(),
		}
	}

	#[test]
	fn schema_url_matches_published_id() {
		let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.ancestors()
			.nth(2)
			.expect("repository root");
		let schema: serde_json::Value = serde_json::from_str(
			&std::fs::read_to_string(
				root.join("website/static/schemas/application-contract/v0.json"),
			)
			.expect("read published schema"),
		)
		.expect("parse published schema");
		assert_eq!(schema["$id"], APPLICATION_CONTRACT_SCHEMA_URL);
		let docs =
			std::fs::read_to_string(root.join("website/content/docs/application-contract.md"))
				.expect("read application contract documentation");
		assert!(docs.contains(APPLICATION_CONTRACT_SCHEMA_URL));
	}

	#[test]
	fn field_kind_spellings_match_v0() {
		assert_eq!(
			serde_json::to_value(ContractFieldType::DateTime).unwrap()["kind"],
			"datetime"
		);
		assert_eq!(
			serde_json::to_value(ContractFieldType::HStore).unwrap()["kind"],
			"hstore"
		);
	}

	fn model(app_label: &str, model_name: &str, table_name: &str) -> ModelContract {
		ModelContract {
			app_label: app_label.to_string(),
			model_name: model_name.to_string(),
			table_name: table_name.to_string(),
			fields: Vec::new(),
			constraints: Vec::new(),
			indexes: Vec::new(),
			relationships: Vec::new(),
		}
	}

	fn field(name: &str) -> ModelFieldContract {
		ModelFieldContract {
			name: name.to_string(),
			field_type: ContractFieldType::Integer,
			nullable: false,
			primary_key: false,
			unique: false,
			default: None,
			generated: None,
		}
	}

	fn constraint(name: &str, kind: ConstraintKindContract, fields: &[&str]) -> ConstraintContract {
		ConstraintContract {
			name: name.to_string(),
			kind,
			fields: fields.iter().map(|field| (*field).to_string()).collect(),
			expression: None,
			references: None,
		}
	}

	fn index(name: &str, fields: &[&str]) -> IndexContract {
		IndexContract {
			name: name.to_string(),
			fields: fields.iter().map(|field| (*field).to_string()).collect(),
			unique: false,
			predicate: None,
			method: None,
			operator_class: None,
			expressions: None,
		}
	}

	fn relationship(
		field: &str,
		kind: RelationshipKindContract,
		table_name: &str,
	) -> RelationshipContract {
		RelationshipContract {
			field: field.to_string(),
			kind,
			target: RelationshipTargetContract {
				app_label: None,
				model_name: None,
				table_name: table_name.to_string(),
				field_name: None,
			},
			related_name: None,
			through_table: None,
			on_delete: None,
			on_update: None,
		}
	}

	fn route(path: &str, method: &str, handler: &str, name: &str) -> RouteContract {
		RouteContract {
			path: path.to_string(),
			method: method.to_string(),
			name: Some(name.to_string()),
			handler: handler.to_string(),
			authentication: AuthenticationContract::Public,
			guard: None,
		}
	}

	fn setting(key_path: &str, rust_type: &str) -> SettingContract {
		SettingContract {
			key_path: key_path.to_string(),
			rust_type: rust_type.to_string(),
			required: false,
			has_default: true,
			secret: false,
			present: true,
		}
	}

	fn resolved_setting(path: SettingsPathBuf) -> ResolvedSettingsField {
		ResolvedSettingsField {
			path,
			rust_type: "alloc::string::String",
			policy: FieldPolicy {
				name: "value",
				requirement: FieldRequirement::Optional,
				has_default: true,
			},
			secret: false,
			present: true,
		}
	}

	async fn empty_catalog() -> MigrationCatalog {
		let directory = tempfile::tempdir().expect("create migration directory");
		MigrationCatalog::load_strict(&FilesystemSource::new(directory.path()))
			.await
			.expect("load empty migration catalog")
	}

	#[rstest]
	fn literal_settings_path_segments_are_escaped() {
		assert_eq!(
			escape_settings_path(&SettingsPathBuf::from_segments([
				SettingsPathSegment::Key("tenants"),
				SettingsPathSegment::DynamicKey("production.eu".to_string()),
				SettingsPathSegment::Key("token"),
			])),
			"tenants.production\\.eu.token"
		);
		assert_eq!(
			escape_settings_path(&SettingsPathBuf::from_segments([
				SettingsPathSegment::Key("literal"),
				SettingsPathSegment::DynamicKey("*".to_string()),
			])),
			"literal.\\*"
		);
		assert_eq!(
			escape_settings_path(&SettingsPathBuf::from_segments([
				SettingsPathSegment::Key("literal"),
				SettingsPathSegment::AnyKey,
			])),
			"literal.*"
		);
		assert_eq!(
			escape_settings_path(&SettingsPathBuf::from_segments([
				SettingsPathSegment::Key("literal.segment"),
				SettingsPathSegment::Key("*"),
			])),
			"literal\\.segment.\\*"
		);
		assert_eq!(
			escape_settings_path(&SettingsPathBuf::from_segments([
				SettingsPathSegment::Key("tenants"),
				SettingsPathSegment::DynamicKey("production\\blue".to_string()),
				SettingsPathSegment::Key("token"),
			])),
			"tenants.production\\\\blue.token"
		);
	}

	#[test]
	fn setting_contracts_redact_sensitive_dynamic_keys() {
		let sentinel = "postgresql://operator:secret@db.example/private";
		let metadata = SettingsResolutionMetadata::from_fields(vec![
			resolved_setting(SettingsPathBuf::from_segments([
				SettingsPathSegment::Key("core"),
				SettingsPathSegment::Key("databases"),
				SettingsPathSegment::AnyKey,
				SettingsPathSegment::Key("name"),
			])),
			resolved_setting(SettingsPathBuf::from_segments([
				SettingsPathSegment::Key("core"),
				SettingsPathSegment::Key("databases"),
				SettingsPathSegment::DynamicKey(sentinel.to_string()),
				SettingsPathSegment::Key("name"),
			])),
			resolved_setting(SettingsPathBuf::from_segments([
				SettingsPathSegment::Key("core"),
				SettingsPathSegment::Key("databases"),
				SettingsPathSegment::DynamicKey("default".to_string()),
				SettingsPathSegment::Key("name"),
			])),
		]);

		let contracts = setting_contracts(&metadata);
		let key_paths = contracts
			.iter()
			.map(|contract| contract.key_path.as_str())
			.collect::<Vec<_>>();

		assert_eq!(
			key_paths,
			["core.databases.*.name", "core.databases.default.name"]
		);
		assert!(
			contracts
				.iter()
				.all(|contract| !contract.key_path.contains(sentinel))
		);
	}

	#[test]
	fn logical_target_prefers_physical_table_for_unqualified_identity() {
		let mut state = ProjectState::new();
		let mut source = ModelState::new("source", "User");
		source.table_name = "source_users".to_string();
		let mut target = ModelState::new("target", "User");
		target.table_name = "legacy_users".to_string();
		state.add_model(source);
		state.add_model(target);

		assert_eq!(
			logical_target(&state, "source", "User", "legacy_users"),
			(Some("target".to_string()), Some("User".to_string()))
		);
	}

	#[rstest]
	fn serialize_v0_uses_explicit_nulls_and_one_newline() {
		let mut contract = empty_contract();
		contract.routes = vec![RouteContract {
			path: "/health".to_string(),
			method: "GET".to_string(),
			name: None,
			handler: "health".to_string(),
			authentication: AuthenticationContract::Public,
			guard: None,
		}];

		let output = serialize_v0(&contract).expect("contract should serialize");
		let value: serde_json::Value =
			serde_json::from_slice(&output).expect("contract should be valid JSON");
		assert!(value["routes"][0]["name"].is_null());
		assert!(value["routes"][0]["guard"].is_null());
		assert!(output.ends_with(b"\n"));
		assert!(!output.ends_with(b"\n\n"));
	}

	#[rstest]
	fn canonical_order_sorts_every_projection() {
		let mut post = model("blog", "Post", "blog_posts");
		post.fields = vec![field("title"), field("id")];
		post.constraints = vec![
			constraint(
				"post_slug_unique",
				ConstraintKindContract::Unique,
				&["slug"],
			),
			constraint(
				"post_author_fk",
				ConstraintKindContract::ForeignKey,
				&["author_id"],
			),
		];
		post.indexes = vec![
			index("post_title_idx", &["title"]),
			index("post_author_idx", &["author_id"]),
		];
		post.relationships = vec![
			relationship("tags", RelationshipKindContract::ManyToMany, "tags"),
			relationship("author", RelationshipKindContract::ForeignKey, "users"),
		];
		let mut contract = empty_contract();
		contract.models = vec![post, model("accounts", "User", "accounts_users")];
		contract.routes = vec![
			route("/posts", "POST", "post_create", "post_create"),
			route("/health", "GET", "health", "health"),
			route("/posts", "GET", "post_list", "post_list"),
		];
		contract.settings = vec![
			setting("core.secret_key", "alloc::string::String"),
			setting("core.debug", "bool"),
		];

		contract.sort_canonical();

		assert_eq!(
			contract
				.models
				.iter()
				.map(|model| format!("{}.{}", model.app_label, model.model_name))
				.collect::<Vec<_>>(),
			["accounts.User", "blog.Post"]
		);
		let post = &contract.models[1];
		assert_eq!(
			post.fields
				.iter()
				.map(|field| field.name.as_str())
				.collect::<Vec<_>>(),
			["id", "title"]
		);
		assert_eq!(
			post.constraints
				.iter()
				.map(|constraint| constraint.name.as_str())
				.collect::<Vec<_>>(),
			["post_author_fk", "post_slug_unique"]
		);
		assert_eq!(
			post.indexes
				.iter()
				.map(|index| index.name.as_str())
				.collect::<Vec<_>>(),
			["post_author_idx", "post_title_idx"]
		);
		assert_eq!(
			post.relationships
				.iter()
				.map(|relationship| relationship.field.as_str())
				.collect::<Vec<_>>(),
			["author", "tags"]
		);
		assert_eq!(
			contract
				.routes
				.iter()
				.map(|route| (
					route.path.as_str(),
					route.method.as_str(),
					route.handler.as_str()
				))
				.collect::<Vec<_>>(),
			[
				("/health", "GET", "health"),
				("/posts", "GET", "post_list"),
				("/posts", "POST", "post_create"),
			]
		);
		assert_eq!(
			contract
				.settings
				.iter()
				.map(|setting| setting.key_path.as_str())
				.collect::<Vec<_>>(),
			["core.debug", "core.secret_key"]
		);
	}

	#[rstest]
	fn canonical_order_uses_every_secondary_key() {
		let mut contract = empty_contract();
		contract.models = vec![
			model("blog", "Post", "z_posts"),
			model("blog", "Post", "a_posts"),
		];
		contract.models[0].constraints = vec![
			constraint("same", ConstraintKindContract::Unique, &["z"]),
			constraint("same", ConstraintKindContract::Check, &["a"]),
		];
		contract.models[0].indexes = vec![index("same", &["z"]), index("same", &["a"])];
		contract.models[0].relationships = vec![
			relationship("same", RelationshipKindContract::ManyToMany, "z_target"),
			relationship("same", RelationshipKindContract::ForeignKey, "a_target"),
		];
		contract.routes = vec![
			route("/same", "POST", "z_handler", "same"),
			route("/same", "GET", "a_handler", "same"),
		];
		contract.settings = vec![setting("same", "z_type"), setting("same", "a_type")];

		contract.sort_canonical();

		assert_eq!(
			contract
				.models
				.iter()
				.map(|model| model.table_name.as_str())
				.collect::<Vec<_>>(),
			["a_posts", "z_posts"]
		);
		let tied = contract
			.models
			.iter()
			.find(|model| model.table_name == "z_posts")
			.expect("find model containing tied nested records");
		assert_eq!(
			serde_json::to_value(&tied.constraints[0].kind).expect("serialize kind"),
			"check"
		);
		assert_eq!(tied.indexes[0].fields, ["a"]);
		assert_eq!(
			serde_json::to_value(&tied.relationships[0].kind).expect("serialize kind"),
			"foreign_key"
		);
		assert_eq!(tied.relationships[0].target.table_name, "a_target");
		assert_eq!(
			contract
				.routes
				.iter()
				.map(|route| (route.method.as_str(), route.handler.as_str()))
				.collect::<Vec<_>>(),
			[("GET", "a_handler"), ("POST", "z_handler")]
		);
		assert_eq!(
			contract
				.settings
				.iter()
				.map(|setting| setting.rust_type.as_str())
				.collect::<Vec<_>>(),
			["a_type", "z_type"]
		);
	}

	#[test]
	fn canonical_order_uses_route_name_as_tiebreaker() {
		let mut contract = empty_contract();
		contract.routes = vec![
			route("/same", "GET", "same_handler", "z_name"),
			route("/same", "GET", "same_handler", "a_name"),
		];

		contract.sort_canonical();

		assert_eq!(
			contract
				.routes
				.iter()
				.map(|route| route.name.as_deref())
				.collect::<Vec<_>>(),
			[Some("a_name"), Some("z_name")]
		);
	}

	#[rstest]
	fn one_to_one_constraint_retains_reference_metadata() {
		let constraint = reinhardt_db::migrations::ConstraintDefinition {
			name: "profile_user_one_to_one".to_string(),
			constraint_type: "one_to_one".to_string(),
			fields: vec!["user_id".to_string()],
			expression: None,
			foreign_key_info: Some(ForeignKeyConstraintInfo {
				referenced_table: "users".to_string(),
				referenced_columns: vec!["id".to_string()],
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::NoAction,
			}),
		};

		let contract =
			constraint_contract(&constraint).expect("one-to-one constraint should resolve");
		let references = contract
			.references
			.expect("one-to-one constraint should retain references");
		assert_eq!(references.table, "users");
		assert_eq!(references.columns, ["id"]);
		assert_eq!(
			serde_json::to_value(&references.on_delete).expect("serialize delete action"),
			"cascade"
		);
		assert_eq!(
			serde_json::to_value(&references.on_update).expect("serialize update action"),
			"no_action"
		);
	}

	#[rstest]
	fn canonical_order_sorts_migration_members_without_reordering_migrations() {
		let mut contract = empty_contract();
		contract.migrations = vec![
			MigrationContract {
				app_label: "blog".to_string(),
				name: "0002_post".to_string(),
				dependencies: vec![
					MigrationIdentityContract {
						app_label: "blog".to_string(),
						name: "0001_initial".to_string(),
					},
					MigrationIdentityContract {
						app_label: "accounts".to_string(),
						name: "0001_initial".to_string(),
					},
				],
				replaces: vec![
					MigrationIdentityContract {
						app_label: "blog".to_string(),
						name: "0001_initial".to_string(),
					},
					MigrationIdentityContract {
						app_label: "accounts".to_string(),
						name: "0001_initial".to_string(),
					},
				],
				applied: None,
			},
			MigrationContract {
				app_label: "accounts".to_string(),
				name: "0001_initial".to_string(),
				dependencies: Vec::new(),
				replaces: Vec::new(),
				applied: None,
			},
		];

		contract.sort_canonical();

		assert_eq!(
			contract
				.migrations
				.iter()
				.map(|migration| format!("{}.{}", migration.app_label, migration.name))
				.collect::<Vec<_>>(),
			["blog.0002_post", "accounts.0001_initial"]
		);
		let migration = &contract.migrations[0];
		for identities in [&migration.dependencies, &migration.replaces] {
			assert_eq!(
				identities
					.iter()
					.map(|identity| format!("{}.{}", identity.app_label, identity.name))
					.collect::<Vec<_>>(),
				["accounts.0001_initial", "blog.0001_initial"]
			);
		}
	}

	#[rstest]
	#[tokio::test]
	async fn implicit_database_failure_warns_once_and_exports_null() {
		let catalog = empty_catalog().await;
		let settings = TestSettings {
			core: CoreSettings {
				databases: HashMap::new(),
				..CoreSettings::default()
			},
			contacts: ContactSettings::default(),
		};
		let selector = DatabaseSelector {
			alias: "default".to_string(),
			url_override: None,
		};
		let mut stderr = Vec::new();

		let applied =
			read_applied_migrations(&catalog, &selector, Some(&settings), false, &mut stderr)
				.await
				.expect("implicit database failure should be non-fatal");

		assert_eq!(applied, None);
		assert_eq!(
			String::from_utf8(stderr).expect("warning should be UTF-8"),
			"Warning: migration applied state is unavailable for database alias `default`; exporting null applied states.\n"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn explicit_database_failure_is_fatal() {
		let catalog = empty_catalog().await;
		let selector = DatabaseSelector {
			alias: "default".to_string(),
			url_override: Some("invalid://operator:secret@db.example/private".to_string()),
		};
		let mut stderr = Vec::new();

		let error = read_applied_migrations(&catalog, &selector, None, true, &mut stderr)
			.await
			.expect_err("explicit database failure should be fatal");

		assert_eq!(
			error.to_string(),
			"Execution error: Failed to read migration state for database alias `default`."
		);
		assert!(stderr.is_empty());
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn missing_sqlite_export_does_not_create_database_or_parent() {
		let directory = tempfile::tempdir().expect("create SQLite project directory");
		let database_path = directory.path().join("nested/missing.sqlite3");
		let mut databases = HashMap::new();
		databases.insert(
			"default".to_string(),
			DatabaseConfig::sqlite("nested/missing.sqlite3"),
		);
		let settings = TestSettings {
			core: CoreSettings {
				base_dir: directory.path().to_path_buf(),
				databases,
				..CoreSettings::default()
			},
			contacts: ContactSettings::default(),
		};
		let catalog = empty_catalog().await;
		let selector = DatabaseSelector {
			alias: "default".to_string(),
			url_override: None,
		};
		let mut stderr = Vec::new();

		let applied =
			read_applied_migrations(&catalog, &selector, Some(&settings), false, &mut stderr)
				.await
				.expect("missing implicit SQLite database should be non-fatal");

		assert_eq!(applied, None);
		assert!(!database_path.exists());
		assert!(!database_path.parent().expect("database parent").exists());
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn relative_sqlite_export_uses_project_base_directory() {
		let directory = tempfile::tempdir().expect("create SQLite project directory");
		let database_path = directory.path().join("relative.sqlite3");
		let pool = sqlx::SqlitePool::connect_with(
			sqlx::sqlite::SqliteConnectOptions::new()
				.filename(&database_path)
				.create_if_missing(true),
		)
		.await
		.expect("create SQLite fixture");
		pool.close().await;

		let mut databases = HashMap::new();
		databases.insert(
			"default".to_string(),
			DatabaseConfig::sqlite("relative.sqlite3"),
		);
		let settings = TestSettings {
			core: CoreSettings {
				base_dir: directory.path().to_path_buf(),
				databases,
				..CoreSettings::default()
			},
			contacts: ContactSettings::default(),
		};
		let catalog = empty_catalog().await;
		let selector = DatabaseSelector {
			alias: "default".to_string(),
			url_override: None,
		};
		let mut stderr = Vec::new();

		let applied =
			read_applied_migrations(&catalog, &selector, Some(&settings), false, &mut stderr)
				.await
				.expect("relative SQLite database should be readable");

		assert_eq!(applied, Some(HashSet::new()));
		assert!(database_path.is_file());
		assert!(stderr.is_empty());
	}

	#[rstest]
	#[tokio::test]
	async fn contract_output_never_contains_secret_sentinel() {
		let sentinel = "not-a-secret-contract-sentinel-5985";
		let catalog = empty_catalog().await;
		let selector = DatabaseSelector {
			alias: format!("postgresql://operator:{sentinel}@db.example/private"),
			url_override: None,
		};
		let mut stderr = Vec::new();
		let mut stdout = Vec::new();

		let applied = read_applied_migrations(&catalog, &selector, None, false, &mut stderr)
			.await
			.expect("implicit database failure should be non-fatal");
		assert_eq!(applied, None);
		write_v0(&empty_contract(), &mut stdout).expect("write contract output");

		for captured in [&stdout, &stderr] {
			assert!(!String::from_utf8_lossy(captured).contains(sentinel));
		}
		assert_eq!(
			String::from_utf8(stderr).expect("warning should be UTF-8"),
			"Warning: migration applied state is unavailable for database alias `[REDACTED]`; exporting null applied states.\n"
		);
	}
}
