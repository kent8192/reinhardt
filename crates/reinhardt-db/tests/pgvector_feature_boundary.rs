use std::{fs, path::PathBuf, process::Command};

#[test]
fn vector_module_requires_the_pgvector_feature() {
	let temporary_project = tempfile::Builder::new()
		.prefix("reinhardt-pgvector-feature-boundary-")
		.tempdir_in("/tmp")
		.unwrap();
	let manifest_path = temporary_project.path().join("Cargo.toml");
	let source_directory = temporary_project.path().join("src");
	let crate_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let crate_path = crate_path.to_str().unwrap();

	fs::create_dir(&source_directory).unwrap();
	fs::write(
		&manifest_path,
		format!(
			"[package]\nname = \"pgvector-feature-boundary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nreinhardt-db = {{ path = \"{crate_path}\", default-features = false, features = [\"orm\", \"postgres\"] }}\n"
		),
	)
	.unwrap();
	fs::write(
		source_directory.join("main.rs"),
		"use reinhardt_db::orm::vector::Vector;\n\nfn main() {\n    let _ = Vector::<3>::try_from(vec![1.0, 2.0, 3.0]);\n}\n",
	)
	.unwrap();

	let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
		.arg("check")
		.arg("--manifest-path")
		.arg(manifest_path)
		.env("CARGO_TARGET_DIR", temporary_project.path().join("target"))
		.output()
		.unwrap();

	assert!(!output.status.success());
	let stderr = String::from_utf8(output.stderr).unwrap();
	assert!(stderr.contains("could not find `vector` in `orm`"));
}

#[test]
fn non_pgvector_consumer_preserves_existing_public_shapes() {
	let temporary_project = tempfile::Builder::new()
		.prefix("reinhardt-non-pgvector-source-compat-")
		.tempdir_in("/tmp")
		.unwrap();
	let manifest_path = temporary_project.path().join("Cargo.toml");
	let source_directory = temporary_project.path().join("src");
	let db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let query_path = db_path.parent().unwrap().join("reinhardt-query");

	fs::create_dir(&source_directory).unwrap();
	fs::write(
		&manifest_path,
		format!(
			"[package]\nname = \"non-pgvector-source-compat\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nctor = \"0.8\"\nreinhardt-core = {{ path = {:?} }}\nreinhardt-db = {{ path = {:?}, default-features = false, features = [\"migrations\", \"orm\", \"postgres\"] }}\nreinhardt-query = {{ path = {:?}, default-features = false }}\nserde = {{ version = \"1\", features = [\"derive\"] }}\n",
			db_path.parent().unwrap().join("reinhardt-core"),
			db_path,
			query_path
		),
	)
	.unwrap();
	fs::write(
		source_directory.join("main.rs"),
		r#"use reinhardt_db::{
    backends::types::QueryValue,
    migrations::{FieldType, IndexDefinition, IndexType, Operation},
    migrations::introspection::IndexInfo as MigrationIndexInfo,
    orm::{DatabaseStorageKind, Model, inspection::IndexInfo as OrmIndexInfo},
};
use reinhardt_core::macros::model;
use reinhardt_query::{Value, types::PgBinOper};
use serde::{Deserialize, Serialize};

#[model(app_label = "compat", table_name = "legacy_documents")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct LegacyDocument {
    #[field(primary_key = true)]
    id: Option<i64>,
    #[field(max_length = 255, index = true)]
    title: String,
}

fn storage_kind(value: DatabaseStorageKind) {
    match value {
        DatabaseStorageKind::Bool | DatabaseStorageKind::I32 | DatabaseStorageKind::I64
        | DatabaseStorageKind::F32 | DatabaseStorageKind::F64
        | DatabaseStorageKind::Decimal | DatabaseStorageKind::String
        | DatabaseStorageKind::Bytes | DatabaseStorageKind::Json
        | DatabaseStorageKind::Uuid | DatabaseStorageKind::Date
        | DatabaseStorageKind::Time | DatabaseStorageKind::DateTime
        | DatabaseStorageKind::NaiveDateTime => {}
    }
}

fn query_value(value: QueryValue) {
    match value {
        QueryValue::Null | QueryValue::Bool(_) | QueryValue::Int(_) | QueryValue::Float(_)
        | QueryValue::String(_) | QueryValue::Bytes(_) | QueryValue::Timestamp(_)
        | QueryValue::NaiveTimestamp(_)
        | QueryValue::Uuid(_) | QueryValue::Json(_) | QueryValue::StringArray(_)
        | QueryValue::IntArray(_) | QueryValue::BigIntArray(_) | QueryValue::BoolArray(_)
        | QueryValue::FloatArray(_) | QueryValue::DoubleArray(_) | QueryValue::UuidArray(_)
        | QueryValue::Now => {}
    }
}

fn index_type(value: IndexType) {
    match value {
        IndexType::BTree | IndexType::Hash | IndexType::Gin | IndexType::Gist
        | IndexType::Brin | IndexType::Fulltext | IndexType::Spatial => {}
    }
}

fn field_type(value: FieldType) {
    match value {
        FieldType::BigInteger | FieldType::Integer | FieldType::SmallInteger
        | FieldType::TinyInt | FieldType::MediumInt | FieldType::Char(_)
        | FieldType::VarChar(_) | FieldType::Text | FieldType::TinyText
        | FieldType::MediumText | FieldType::LongText | FieldType::Date
        | FieldType::Time | FieldType::DateTime | FieldType::TimestampTz
        | FieldType::Decimal { .. } | FieldType::Float | FieldType::Double
        | FieldType::Real | FieldType::Boolean | FieldType::Binary | FieldType::Blob
        | FieldType::TinyBlob | FieldType::MediumBlob | FieldType::LongBlob
        | FieldType::Bytea | FieldType::Json | FieldType::Jsonb
        | FieldType::Array(_) | FieldType::HStore | FieldType::CIText
        | FieldType::Int4Range | FieldType::Int8Range | FieldType::NumRange
        | FieldType::DateRange | FieldType::TsRange | FieldType::TsTzRange
        | FieldType::TsVector | FieldType::TsQuery | FieldType::Uuid | FieldType::Year
        | FieldType::Enum { .. } | FieldType::Set { .. } | FieldType::ForeignKey { .. }
        | FieldType::OneToOne { .. } | FieldType::ManyToMany { .. }
        | FieldType::Custom(_) => {}
    }
}

fn query_builder_value(value: Value) {
    match value {
        Value::Bool(_) | Value::TinyInt(_) | Value::SmallInt(_) | Value::Int(_)
        | Value::BigInt(_) | Value::TinyUnsigned(_) | Value::SmallUnsigned(_)
        | Value::Unsigned(_) | Value::BigUnsigned(_) | Value::Float(_)
        | Value::Double(_) | Value::Char(_) | Value::String(_) | Value::Bytes(_)
        | Value::ChronoDate(_) | Value::ChronoTime(_) | Value::ChronoDateTime(_)
        | Value::ChronoDateTimeUtc(_) | Value::ChronoDateTimeLocal(_)
        | Value::ChronoDateTimeWithTimeZone(_) | Value::Uuid(_) | Value::Json(_)
        | Value::Decimal(_) | Value::BigDecimal(_)
        | Value::Array(_, _) => {}
    }
}

fn pg_operator(value: PgBinOper) {
    match value {
        PgBinOper::Contains | PgBinOper::Contained | PgBinOper::Overlap
        | PgBinOper::Concatenate | PgBinOper::JsonContainsKey
        | PgBinOper::JsonContainsAnyKey | PgBinOper::JsonContainsAllKeys
        | PgBinOper::JsonGetByIndex | PgBinOper::JsonGetAsText
        | PgBinOper::JsonGetPath | PgBinOper::JsonGetPathAsText => {}
        _ => {}
    }
}

fn operation(value: Operation) {
    match value {
        Operation::CreateTable { .. }
        | Operation::DropTable { .. }
        | Operation::AddColumn { .. }
        | Operation::DropColumn { .. }
        | Operation::AlterColumn { .. }
        | Operation::RenameTable { .. }
        | Operation::RenameColumn { .. }
        | Operation::AddConstraint { .. }
        | Operation::AddConstraintDefinition { .. }
        | Operation::AddConstraintRepair { .. }
        | Operation::RestoreConstraintOnRollback { .. }
        | Operation::DropConstraint { .. }
        | Operation::DropConstraintDefinition { .. }
        | Operation::CreateIndex { .. }
        | Operation::CreateIndexRepair { .. }
        | Operation::RestoreIndexOnRollback { .. }
        | Operation::DropIndex { .. }
        | Operation::RunSQL { .. }
        | Operation::RunRust { .. }
        | Operation::AlterTableComment { .. }
        | Operation::AlterUniqueTogether { .. }
        | Operation::AlterModelOptions { .. }
        | Operation::CreateInheritedTable { .. }
        | Operation::AddDiscriminatorColumn { .. }
        | Operation::MoveModel { .. }
        | Operation::CreateSchema { .. }
        | Operation::DropSchema { .. }
        | Operation::CreateExtension { .. }
        | Operation::BulkLoad { .. }
        | Operation::SetAutoIncrementValue { .. }
        | Operation::CreateCompositePrimaryKey { .. } => {}
    }
}

fn main() {
    let _ = LegacyDocument::index_metadata();
    let _ = IndexDefinition {
        name: "documents_title_idx".into(),
        fields: vec!["title".into()],
        unique: false,
    };
    let _ = MigrationIndexInfo {
        name: "documents_title_idx".into(),
        columns: vec!["title".into()],
        unique: false,
        index_type: Some("btree".into()),
    };
    let _ = OrmIndexInfo {
        name: "documents_title_idx".into(),
        fields: vec!["title".into()],
        unique: false,
        condition: None,
    };
    let _ = (
        storage_kind,
        query_value,
        index_type,
        field_type,
        query_builder_value,
        pg_operator,
        operation,
    );
}
"#,
	)
	.unwrap();

	let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
		.arg("check")
		.arg("--manifest-path")
		.arg(manifest_path)
		.env("CARGO_TARGET_DIR", temporary_project.path().join("target"))
		.output()
		.unwrap();

	assert!(
		output.status.success(),
		"legacy non-pgvector consumer failed to compile:\n{}",
		String::from_utf8_lossy(&output.stderr)
	);
}
