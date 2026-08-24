use super::{
	AlterTableOptions, InterleaveSpec, MySqlAlgorithm, MySqlLock, PartitionDef, PartitionOptions,
	PartitionType, PartitionValues,
};
use crate::migrations::{
	ColumnDefinition, Constraint, DeferrableOption, FieldType, ForeignKeyAction, IndexType,
	Operation,
};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

/// Helper function to convert FieldType to TokenStream (for recursive Array handling)
fn field_type_to_tokens(field_type: &FieldType) -> TokenStream {
	match field_type {
		// Integer types
		FieldType::BigInteger => quote! { FieldType::BigInteger },
		FieldType::Integer => quote! { FieldType::Integer },
		FieldType::SmallInteger => quote! { FieldType::SmallInteger },
		FieldType::TinyInt => quote! { FieldType::TinyInt },
		FieldType::MediumInt => quote! { FieldType::MediumInt },

		// String types
		FieldType::Char(len) => quote! { FieldType::Char(#len) },
		FieldType::VarChar(len) => quote! { FieldType::VarChar(#len) },
		FieldType::Text => quote! { FieldType::Text },
		FieldType::TinyText => quote! { FieldType::TinyText },
		FieldType::MediumText => quote! { FieldType::MediumText },
		FieldType::LongText => quote! { FieldType::LongText },

		// Date/Time types
		FieldType::Date => quote! { FieldType::Date },
		FieldType::Time => quote! { FieldType::Time },
		FieldType::DateTime => quote! { FieldType::DateTime },
		FieldType::TimestampTz => quote! { FieldType::TimestampTz },

		// Numeric types
		FieldType::Decimal { precision, scale } => {
			quote! { FieldType::Decimal { precision: #precision, scale: #scale } }
		}
		FieldType::Float => quote! { FieldType::Float },
		FieldType::Double => quote! { FieldType::Double },
		FieldType::Real => quote! { FieldType::Real },

		// Boolean
		FieldType::Boolean => quote! { FieldType::Boolean },

		// Binary types
		FieldType::Binary => quote! { FieldType::Binary },
		FieldType::Blob => quote! { FieldType::Blob },
		FieldType::TinyBlob => quote! { FieldType::TinyBlob },
		FieldType::MediumBlob => quote! { FieldType::MediumBlob },
		FieldType::LongBlob => quote! { FieldType::LongBlob },
		FieldType::Bytea => quote! { FieldType::Bytea },

		// JSON types
		FieldType::Json => quote! { FieldType::Json },
		FieldType::JsonBinary => quote! { FieldType::JsonBinary },

		// PostgreSQL-specific types
		FieldType::Array(inner) => {
			let inner_token = field_type_to_tokens(inner);
			quote! { FieldType::Array(Box::new(#inner_token)) }
		}
		FieldType::HStore => quote! { FieldType::HStore },
		FieldType::CIText => quote! { FieldType::CIText },
		FieldType::Int4Range => quote! { FieldType::Int4Range },
		FieldType::Int8Range => quote! { FieldType::Int8Range },
		FieldType::NumRange => quote! { FieldType::NumRange },
		FieldType::DateRange => quote! { FieldType::DateRange },
		FieldType::TsRange => quote! { FieldType::TsRange },
		FieldType::TsTzRange => quote! { FieldType::TsTzRange },
		FieldType::TsVector => quote! { FieldType::TsVector },
		FieldType::TsQuery => quote! { FieldType::TsQuery },

		// UUID and Year
		FieldType::Uuid => quote! { FieldType::Uuid },
		FieldType::Year => quote! { FieldType::Year },

		// Collection types
		FieldType::Enum { values } => {
			quote! { FieldType::Enum { values: vec![#(#values.to_string()),*] } }
		}
		FieldType::Set { values } => {
			quote! { FieldType::Set { values: vec![#(#values.to_string()),*] } }
		}

		// Relationship types
		FieldType::OneToOne {
			to,
			on_delete,
			on_update,
		} => {
			quote! {
				FieldType::OneToOne {
					to: #to.to_string(),
					on_delete: #on_delete,
					on_update: #on_update,
				}
			}
		}
		FieldType::ManyToMany { to, through } => {
			let through_token = match through {
				Some(t) => quote! { Some(#t.to_string()) },
				None => quote! { None },
			};
			quote! {
				FieldType::ManyToMany {
					to: #to.to_string(),
					through: #through_token,
				}
			}
		}

		// Custom types
		FieldType::Custom(s) => quote! { FieldType::Custom(#s.to_string()) },

		// Foreign key
		FieldType::ForeignKey {
			to_table,
			to_field,
			on_delete,
		} => {
			quote! {
				FieldType::ForeignKey {
					to_table: #to_table.to_string(),
					to_field: #to_field.to_string(),
					on_delete: #on_delete,
				}
			}
		}
	}
}

impl ToTokens for ForeignKeyAction {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variant = match self {
			ForeignKeyAction::Restrict => quote! { ForeignKeyAction::Restrict },
			ForeignKeyAction::Cascade => quote! { ForeignKeyAction::Cascade },
			ForeignKeyAction::SetNull => quote! { ForeignKeyAction::SetNull },
			ForeignKeyAction::NoAction => quote! { ForeignKeyAction::NoAction },
			ForeignKeyAction::SetDefault => quote! { ForeignKeyAction::SetDefault },
		};
		tokens.extend(variant);
	}
}

impl ToTokens for MySqlAlgorithm {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variant = match self {
			MySqlAlgorithm::Instant => quote! { MySqlAlgorithm::Instant },
			MySqlAlgorithm::Inplace => quote! { MySqlAlgorithm::Inplace },
			MySqlAlgorithm::Copy => quote! { MySqlAlgorithm::Copy },
			MySqlAlgorithm::Default => quote! { MySqlAlgorithm::Default },
		};
		tokens.extend(variant);
	}
}

impl ToTokens for MySqlLock {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variant = match self {
			MySqlLock::None => quote! { MySqlLock::None },
			MySqlLock::Shared => quote! { MySqlLock::Shared },
			MySqlLock::Exclusive => quote! { MySqlLock::Exclusive },
			MySqlLock::Default => quote! { MySqlLock::Default },
		};
		tokens.extend(variant);
	}
}

impl ToTokens for AlterTableOptions {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let algorithm_token = match &self.algorithm {
			Some(algo) => quote! { Some(#algo) },
			None => quote! { None },
		};
		let lock_token = match &self.lock {
			Some(lock) => quote! { Some(#lock) },
			None => quote! { None },
		};
		tokens.extend(quote! {
			AlterTableOptions {
				algorithm: #algorithm_token,
				lock: #lock_token,
			}
		});
	}
}

impl ToTokens for DeferrableOption {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variant = match self {
			DeferrableOption::Immediate => quote! { DeferrableOption::Immediate },
			DeferrableOption::Deferred => quote! { DeferrableOption::Deferred },
		};
		tokens.extend(variant);
	}
}

impl ToTokens for PartitionType {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variant = match self {
			PartitionType::Range => quote! { PartitionType::Range },
			PartitionType::List => quote! { PartitionType::List },
			PartitionType::Hash => quote! { PartitionType::Hash },
			PartitionType::Key => quote! { PartitionType::Key },
		};
		tokens.extend(variant);
	}
}

impl ToTokens for PartitionValues {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variant = match self {
			PartitionValues::LessThan(value) => {
				quote! { PartitionValues::LessThan(#value.to_string()) }
			}
			PartitionValues::In(values) => {
				quote! { PartitionValues::In(vec![#(#values.to_string()),*]) }
			}
			PartitionValues::ModuloCount(count) => {
				quote! { PartitionValues::ModuloCount(#count) }
			}
		};
		tokens.extend(variant);
	}
}

impl ToTokens for PartitionDef {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let name = &self.name;
		let values = &self.values;
		tokens.extend(quote! {
			PartitionDef {
				name: #name.to_string(),
				values: #values,
			}
		});
	}
}

impl ToTokens for PartitionOptions {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let partition_type = &self.partition_type;
		let column = &self.column;
		let partitions = &self.partitions;
		tokens.extend(quote! {
			PartitionOptions {
				partition_type: #partition_type,
				column: #column.to_string(),
				partitions: vec![#(#partitions),*],
			}
		});
	}
}

impl ToTokens for InterleaveSpec {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let parent_table = &self.parent_table;
		let parent_columns = &self.parent_columns;
		tokens.extend(quote! {
			InterleaveSpec {
				parent_table: #parent_table.to_string(),
				parent_columns: vec![#(#parent_columns.to_string()),*],
			}
		});
	}
}

impl ToTokens for Constraint {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Constraint::PrimaryKey { name, columns } => {
				let columns_iter = columns.iter();
				tokens.extend(quote! {
					Constraint::PrimaryKey {
						name: #name.to_string(),
						columns: vec![#(#columns_iter.to_string()),*],
					}
				});
			}
			Constraint::ForeignKey {
				name,
				columns,
				referenced_table,
				referenced_columns,
				on_delete,
				on_update,
				deferrable,
			} => {
				let columns_iter = columns.iter();
				let ref_columns_iter = referenced_columns.iter();
				let deferrable_tokens = match deferrable {
					Some(d) => quote! { Some(#d) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Constraint::ForeignKey {
						name: #name.to_string(),
						columns: vec![#(#columns_iter.to_string()),*],
						referenced_table: #referenced_table.to_string(),
						referenced_columns: vec![#(#ref_columns_iter.to_string()),*],
						on_delete: #on_delete,
						on_update: #on_update,
						deferrable: #deferrable_tokens,
					}
				});
			}
			Constraint::Unique { name, columns } => {
				let columns_iter = columns.iter();
				tokens.extend(quote! {
					Constraint::Unique {
						name: #name.to_string(),
						columns: vec![#(#columns_iter.to_string()),*],
					}
				});
			}
			Constraint::Check { name, expression } => {
				tokens.extend(quote! {
					Constraint::Check {
						name: #name.to_string(),
						expression: #expression.to_string(),
					}
				});
			}
			Constraint::OneToOne {
				name,
				column,
				referenced_table,
				referenced_column,
				on_delete,
				on_update,
				deferrable,
			} => {
				let deferrable_tokens = match deferrable {
					Some(d) => quote! { Some(#d) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Constraint::OneToOne {
						name: #name.to_string(),
						column: #column.to_string(),
						referenced_table: #referenced_table.to_string(),
						referenced_column: #referenced_column.to_string(),
						on_delete: #on_delete,
						on_update: #on_update,
						deferrable: #deferrable_tokens,
					}
				});
			}
			Constraint::ManyToMany {
				name,
				through_table,
				source_column,
				target_column,
				target_table,
			} => {
				tokens.extend(quote! {
					Constraint::ManyToMany {
						name: #name.to_string(),
						through_table: #through_table.to_string(),
						source_column: #source_column.to_string(),
						target_column: #target_column.to_string(),
						target_table: #target_table.to_string(),
					}
				});
			}
			Constraint::Exclude { .. } => {
				// Exclude constraints are PostgreSQL-specific
				// For code generation, we output a placeholder comment
				tokens.extend(quote! {
					// Exclude constraints require raw SQL generation
				});
			}
		}
	}
}

impl ToTokens for Operation {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Operation::CreateTable {
				name,
				columns,
				constraints,
				without_rowid,
				interleave_in_parent,
				partition,
			} => {
				let columns_tokens = columns.iter();
				let constraints_tokens = constraints.iter();
				let without_rowid_tokens = match without_rowid {
					Some(true) => quote! { Some(true) },
					Some(false) => quote! { Some(false) },
					None => quote! { None },
				};
				let interleave_tokens = match interleave_in_parent {
					Some(spec) => quote! { Some(#spec) },
					None => quote! { None },
				};
				let partition_tokens = match partition {
					Some(opts) => quote! { Some(#opts) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::CreateTable {
						name: #name.to_string(),
						columns: vec![#(#columns_tokens),*],
						constraints: vec![#(#constraints_tokens),*],
						without_rowid: #without_rowid_tokens,
						interleave_in_parent: #interleave_tokens,
						partition: #partition_tokens,
					}
				});
			}
			Operation::DropTable { name } => {
				tokens.extend(quote! {
					Operation::DropTable {
						name: #name.to_string(),
					}
				});
			}
			Operation::AddColumn {
				table,
				column,
				mysql_options,
			} => {
				let mysql_opts_token = match mysql_options {
					Some(opts) => quote! { Some(#opts) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::AddColumn {
						table: #table.to_string(),
						column: #column,
						mysql_options: #mysql_opts_token,
					}
				});
			}
			Operation::DropColumn { table, column } => {
				tokens.extend(quote! {
					Operation::DropColumn {
						table: #table.to_string(),
						column: #column.to_string(),
					}
				});
			}
			Operation::AlterColumn {
				table,
				column,
				old_definition,
				new_definition,
				mysql_options,
			} => {
				let old_def_token = match old_definition {
					Some(def) => quote! { Some(#def) },
					None => quote! { None },
				};
				let mysql_opts_token = match mysql_options {
					Some(opts) => quote! { Some(#opts) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::AlterColumn {
						table: #table.to_string(),
						column: #column.to_string(),
						old_definition: #old_def_token,
						new_definition: #new_definition,
						mysql_options: #mysql_opts_token,
					}
				});
			}
			Operation::RenameTable { old_name, new_name } => {
				tokens.extend(quote! {
					Operation::RenameTable {
						old_name: #old_name.to_string(),
						new_name: #new_name.to_string(),
					}
				});
			}
			Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} => {
				tokens.extend(quote! {
					Operation::RenameColumn {
						table: #table.to_string(),
						old_name: #old_name.to_string(),
						new_name: #new_name.to_string(),
					}
				});
			}
			Operation::AddConstraint {
				table,
				constraint_sql,
			} => {
				tokens.extend(quote! {
					Operation::AddConstraint {
						table: #table.to_string(),
						constraint_sql: #constraint_sql.to_string(),
					}
				});
			}
			Operation::DropConstraint {
				table,
				constraint_name,
			} => {
				tokens.extend(quote! {
					Operation::DropConstraint {
						table: #table.to_string(),
						constraint_name: #constraint_name.to_string(),
					}
				});
			}
			Operation::CreateIndex {
				table,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => {
				let columns_iter = columns.iter();
				let index_type_token = match index_type {
					Some(it) => {
						let variant = match it {
							IndexType::BTree => quote! { IndexType::BTree },
							IndexType::Hash => quote! { IndexType::Hash },
							IndexType::Gin => quote! { IndexType::Gin },
							IndexType::Gist => quote! { IndexType::Gist },
							IndexType::Brin => quote! { IndexType::Brin },
							IndexType::Fulltext => quote! { IndexType::Fulltext },
							IndexType::Spatial => quote! { IndexType::Spatial },
						};
						quote! { Some(#variant) }
					}
					None => quote! { None },
				};
				let where_clause_token = match where_clause {
					Some(s) => quote! { Some(#s.to_string()) },
					None => quote! { None },
				};
				let expressions_token = match expressions {
					Some(exprs) => {
						let exprs_iter = exprs.iter();
						quote! { Some(vec![#(#exprs_iter.to_string()),*]) }
					}
					None => quote! { None },
				};
				let mysql_options_token = match mysql_options {
					Some(opts) => quote! { Some(#opts) },
					None => quote! { None },
				};
				let operator_class_token = match operator_class {
					Some(oc) => quote! { Some(#oc.to_string()) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::CreateIndex {
						table: #table.to_string(),
						columns: vec![#(#columns_iter.to_string()),*],
						unique: #unique,
						index_type: #index_type_token,
						where_clause: #where_clause_token,
						concurrently: #concurrently,
						expressions: #expressions_token,
						mysql_options: #mysql_options_token,
						operator_class: #operator_class_token,
					}
				});
			}
			Operation::DropIndex { table, columns } => {
				let columns_iter = columns.iter();
				tokens.extend(quote! {
					Operation::DropIndex {
						table: #table.to_string(),
						columns: vec![#(#columns_iter.to_string()),*],
					}
				});
			}
			Operation::DropNamedIndex {
				table,
				name,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => {
				let columns_iter = columns.iter();
				let index_type_token = match index_type {
					Some(it) => {
						let variant = match it {
							IndexType::BTree => quote! { IndexType::BTree },
							IndexType::Hash => quote! { IndexType::Hash },
							IndexType::Gin => quote! { IndexType::Gin },
							IndexType::Gist => quote! { IndexType::Gist },
							IndexType::Brin => quote! { IndexType::Brin },
							IndexType::Fulltext => quote! { IndexType::Fulltext },
							IndexType::Spatial => quote! { IndexType::Spatial },
						};
						quote! { Some(#variant) }
					}
					None => quote! { None },
				};
				let where_clause_token = match where_clause {
					Some(value) => quote! { Some(#value.to_string()) },
					None => quote! { None },
				};
				let expressions_token = match expressions {
					Some(values) => {
						let values_iter = values.iter();
						quote! { Some(vec![#(#values_iter.to_string()),*]) }
					}
					None => quote! { None },
				};
				let mysql_options_token = match mysql_options {
					Some(options) => quote! { Some(#options) },
					None => quote! { None },
				};
				let operator_class_token = match operator_class {
					Some(value) => quote! { Some(#value.to_string()) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::DropNamedIndex {
						table: #table.to_string(),
						name: #name.to_string(),
						columns: vec![#(#columns_iter.to_string()),*],
						unique: #unique,
						index_type: #index_type_token,
						where_clause: #where_clause_token,
						concurrently: #concurrently,
						expressions: #expressions_token,
						mysql_options: #mysql_options_token,
						operator_class: #operator_class_token,
					}
				});
			}
			Operation::RunSQL { sql, reverse_sql } => {
				let reverse_sql_token = match reverse_sql {
					Some(s) => quote! { Some(#s.to_string()) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::RunSQL {
						sql: #sql.to_string(),
						reverse_sql: #reverse_sql_token,
					}
				});
			}
			Operation::RunRust { code, reverse_code } => {
				let reverse_code_token = match reverse_code {
					Some(s) => quote! { Some(#s.to_string()) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::RunRust {
						code: #code.to_string(),
						reverse_code: #reverse_code_token,
					}
				});
			}
			Operation::AlterTableComment { table, comment } => {
				let comment_token = match comment {
					Some(s) => quote! { Some(#s.to_string()) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::AlterTableComment {
						table: #table.to_string(),
						comment: #comment_token,
					}
				});
			}
			Operation::AlterUniqueTogether {
				table,
				unique_together,
			} => {
				let unique_together_tokens = unique_together.iter().map(|fields| {
					let fields_iter = fields.iter();
					quote! { vec![#(#fields_iter.to_string()),*] }
				});
				tokens.extend(quote! {
					Operation::AlterUniqueTogether {
						table: #table.to_string(),
						unique_together: vec![#(#unique_together_tokens),*],
					}
				});
			}
			Operation::AlterModelOptions { table, options } => {
				let keys = options.keys();
				let values = options.values();
				tokens.extend(quote! {
					Operation::AlterModelOptions {
						table: #table.to_string(),
						options: {
							let mut map = std::collections::HashMap::new();
							#(map.insert(#keys.to_string(), #values.to_string());)*
							map
						},
					}
				});
			}
			Operation::CreateInheritedTable {
				name,
				columns,
				base_table,
				join_column,
			} => {
				let columns_tokens = columns.iter();
				tokens.extend(quote! {
					Operation::CreateInheritedTable {
						name: #name.to_string(),
						columns: vec![#(#columns_tokens),*],
						base_table: #base_table.to_string(),
						join_column: #join_column.to_string(),
					}
				});
			}
			Operation::AddDiscriminatorColumn {
				table,
				column_name,
				default_value,
			} => {
				tokens.extend(quote! {
					Operation::AddDiscriminatorColumn {
						table: #table.to_string(),
						column_name: #column_name.to_string(),
						default_value: #default_value.to_string(),
					}
				});
			}
			Operation::MoveModel {
				model_name,
				from_app,
				to_app,
				rename_table,
				old_table_name,
				new_table_name,
			} => {
				let old_table_token = match old_table_name {
					Some(s) => quote! { Some(#s.to_string()) },
					None => quote! { None },
				};
				let new_table_token = match new_table_name {
					Some(s) => quote! { Some(#s.to_string()) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::MoveModel {
						model_name: #model_name.to_string(),
						from_app: #from_app.to_string(),
						to_app: #to_app.to_string(),
						rename_table: #rename_table,
						old_table_name: #old_table_token,
						new_table_name: #new_table_token,
					}
				});
			}
			Operation::CreateSchema {
				name,
				if_not_exists,
			} => {
				tokens.extend(quote! {
					Operation::CreateSchema {
						name: #name.to_string(),
						if_not_exists: #if_not_exists,
					}
				});
			}
			Operation::DropSchema {
				name,
				cascade,
				if_exists,
			} => {
				tokens.extend(quote! {
					Operation::DropSchema {
						name: #name.to_string(),
						cascade: #cascade,
						if_exists: #if_exists,
					}
				});
			}
			Operation::CreateExtension {
				name,
				if_not_exists,
				schema,
			} => {
				let schema_token = match schema {
					Some(s) => quote! { Some(#s.to_string()) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::CreateExtension {
						name: #name.to_string(),
						if_not_exists: #if_not_exists,
						schema: #schema_token,
					}
				});
			}
			Operation::BulkLoad {
				table,
				source,
				format,
				options,
			} => {
				// Manually construct tokens for BulkLoadSource
				let source_tokens = match source {
					super::BulkLoadSource::File(path) => {
						quote! { BulkLoadSource::File(#path.to_string()) }
					}
					super::BulkLoadSource::Stdin => {
						quote! { BulkLoadSource::Stdin }
					}
					super::BulkLoadSource::Program(cmd) => {
						quote! { BulkLoadSource::Program(#cmd.to_string()) }
					}
				};

				// Manually construct tokens for BulkLoadFormat
				let format_tokens = match format {
					super::BulkLoadFormat::Text => quote! { BulkLoadFormat::Text },
					super::BulkLoadFormat::Csv => quote! { BulkLoadFormat::Csv },
					super::BulkLoadFormat::Binary => quote! { BulkLoadFormat::Binary },
				};

				// Manually construct tokens for BulkLoadOptions
				let delimiter_token = match &options.delimiter {
					Some(d) => quote! { Some(#d) },
					None => quote! { None },
				};
				let null_string_token = match &options.null_string {
					Some(s) => quote! { Some(#s.to_string()) },
					None => quote! { None },
				};
				let header = options.header;
				let columns_token = match &options.columns {
					Some(cols) => {
						let cols_iter = cols.iter();
						quote! { Some(vec![#(#cols_iter.to_string()),*]) }
					}
					None => quote! { None },
				};
				let local = options.local;
				let quote_token = match &options.quote {
					Some(q) => quote! { Some(#q) },
					None => quote! { None },
				};
				let escape_token = match &options.escape {
					Some(e) => quote! { Some(#e) },
					None => quote! { None },
				};
				let line_terminator_token = match &options.line_terminator {
					Some(lt) => quote! { Some(#lt.to_string()) },
					None => quote! { None },
				};
				let encoding_token = match &options.encoding {
					Some(e) => quote! { Some(#e.to_string()) },
					None => quote! { None },
				};

				tokens.extend(quote! {
					Operation::BulkLoad {
						table: #table.to_string(),
						source: #source_tokens,
						format: #format_tokens,
						options: BulkLoadOptions {
							delimiter: #delimiter_token,
							null_string: #null_string_token,
							header: #header,
							columns: #columns_token,
							local: #local,
							quote: #quote_token,
							escape: #escape_token,
							line_terminator: #line_terminator_token,
							encoding: #encoding_token,
						},
					}
				});
			}
			Operation::SetAutoIncrementValue {
				table,
				column,
				value,
			} => {
				tokens.extend(quote! {
					Operation::SetAutoIncrementValue {
						table: #table.to_string(),
						column: #column.to_string(),
						value: #value,
					}
				});
			}
			Operation::CreateCompositePrimaryKey {
				table,
				columns,
				constraint_name,
			} => {
				let columns_iter = columns.iter();
				let constraint_token = match constraint_name {
					Some(n) => quote! { Some(#n.to_string()) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::CreateCompositePrimaryKey {
						table: #table.to_string(),
						columns: vec![#(#columns_iter.to_string()),*],
						constraint_name: #constraint_token,
					}
				});
			}
		}
	}
}

impl ToTokens for ColumnDefinition {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let name = &self.name;
		let not_null = self.not_null;
		let unique = self.unique;
		let primary_key = self.primary_key;
		let auto_increment = self.auto_increment;

		let default_token = match &self.default {
			Some(s) => quote! { Some(#s.to_string()) },
			None => quote! { None },
		};

		// Generate FieldType token based on the actual type
		let field_type_token = match &self.type_definition {
			// Integer types
			FieldType::BigInteger => quote! { FieldType::BigInteger },
			FieldType::Integer => quote! { FieldType::Integer },
			FieldType::SmallInteger => quote! { FieldType::SmallInteger },
			FieldType::TinyInt => quote! { FieldType::TinyInt },
			FieldType::MediumInt => quote! { FieldType::MediumInt },

			// String types
			FieldType::Char(len) => quote! { FieldType::Char(#len) },
			FieldType::VarChar(len) => quote! { FieldType::VarChar(#len) },
			FieldType::Text => quote! { FieldType::Text },
			FieldType::TinyText => quote! { FieldType::TinyText },
			FieldType::MediumText => quote! { FieldType::MediumText },
			FieldType::LongText => quote! { FieldType::LongText },

			// Date/Time types
			FieldType::Date => quote! { FieldType::Date },
			FieldType::Time => quote! { FieldType::Time },
			FieldType::DateTime => quote! { FieldType::DateTime },
			FieldType::TimestampTz => quote! { FieldType::TimestampTz },

			// Numeric types
			FieldType::Decimal { precision, scale } => {
				quote! { FieldType::Decimal { precision: #precision, scale: #scale } }
			}
			FieldType::Float => quote! { FieldType::Float },
			FieldType::Double => quote! { FieldType::Double },
			FieldType::Real => quote! { FieldType::Real },

			// Boolean
			FieldType::Boolean => quote! { FieldType::Boolean },

			// Binary types
			FieldType::Binary => quote! { FieldType::Binary },
			FieldType::Blob => quote! { FieldType::Blob },
			FieldType::TinyBlob => quote! { FieldType::TinyBlob },
			FieldType::MediumBlob => quote! { FieldType::MediumBlob },
			FieldType::LongBlob => quote! { FieldType::LongBlob },
			FieldType::Bytea => quote! { FieldType::Bytea },

			// JSON types
			FieldType::Json => quote! { FieldType::Json },
			FieldType::JsonBinary => quote! { FieldType::JsonBinary },

			// PostgreSQL-specific types
			FieldType::Array(inner) => {
				// Generate the inner field type token recursively
				let inner_token = field_type_to_tokens(inner);
				quote! { FieldType::Array(Box::new(#inner_token)) }
			}
			FieldType::HStore => quote! { FieldType::HStore },
			FieldType::CIText => quote! { FieldType::CIText },
			FieldType::Int4Range => quote! { FieldType::Int4Range },
			FieldType::Int8Range => quote! { FieldType::Int8Range },
			FieldType::NumRange => quote! { FieldType::NumRange },
			FieldType::DateRange => quote! { FieldType::DateRange },
			FieldType::TsRange => quote! { FieldType::TsRange },
			FieldType::TsTzRange => quote! { FieldType::TsTzRange },
			FieldType::TsVector => quote! { FieldType::TsVector },
			FieldType::TsQuery => quote! { FieldType::TsQuery },

			// UUID and Year
			FieldType::Uuid => quote! { FieldType::Uuid },
			FieldType::Year => quote! { FieldType::Year },

			// Collection types
			FieldType::Enum { values } => {
				quote! { FieldType::Enum { values: vec![#(#values.to_string()),*] } }
			}
			FieldType::Set { values } => {
				quote! { FieldType::Set { values: vec![#(#values.to_string()),*] } }
			}

			// Relationship types
			FieldType::OneToOne {
				to,
				on_delete,
				on_update,
			} => {
				quote! {
					FieldType::OneToOne {
						to: #to.to_string(),
						on_delete: #on_delete,
						on_update: #on_update,
					}
				}
			}
			FieldType::ManyToMany { to, through } => {
				let through_token = match through {
					Some(t) => quote! { Some(#t.to_string()) },
					None => quote! { None },
				};
				quote! {
					FieldType::ManyToMany {
						to: #to.to_string(),
						through: #through_token,
					}
				}
			}

			// Custom types
			FieldType::Custom(s) => quote! { FieldType::Custom(#s.to_string()) },

			// Foreign key
			FieldType::ForeignKey {
				to_table,
				to_field,
				on_delete,
			} => {
				quote! {
					FieldType::ForeignKey {
						to_table: #to_table.to_string(),
						to_field: #to_field.to_string(),
						on_delete: #on_delete,
					}
				}
			}
		};

		tokens.extend(quote! {
			ColumnDefinition {
				name: #name.to_string(),
				type_definition: #field_type_token,
				not_null: #not_null,
				unique: #unique,
				primary_key: #primary_key,
				auto_increment: #auto_increment,
				default: #default_token,
			}
		});
	}
}

impl ToTokens for super::BulkLoadSource {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variant = match self {
			super::BulkLoadSource::File(path) => {
				quote! { BulkLoadSource::File(#path.to_string()) }
			}
			super::BulkLoadSource::Stdin => {
				quote! { BulkLoadSource::Stdin }
			}
			super::BulkLoadSource::Program(cmd) => {
				quote! { BulkLoadSource::Program(#cmd.to_string()) }
			}
		};
		tokens.extend(variant);
	}
}

impl ToTokens for super::BulkLoadFormat {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variant = match self {
			super::BulkLoadFormat::Text => quote! { BulkLoadFormat::Text },
			super::BulkLoadFormat::Csv => quote! { BulkLoadFormat::Csv },
			super::BulkLoadFormat::Binary => quote! { BulkLoadFormat::Binary },
		};
		tokens.extend(variant);
	}
}

impl ToTokens for super::BulkLoadOptions {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let delimiter = match self.delimiter {
			Some(c) => quote! { Some(#c) },
			None => quote! { None },
		};

		let null_string = match &self.null_string {
			Some(s) => quote! { Some(#s.to_string()) },
			None => quote! { None },
		};

		let header = self.header;

		let columns = match &self.columns {
			Some(cols) => quote! { Some(vec![#(#cols.to_string()),*]) },
			None => quote! { None },
		};

		let local = self.local;

		let quote_char = match self.quote {
			Some(c) => quote! { Some(#c) },
			None => quote! { None },
		};

		let escape = match self.escape {
			Some(c) => quote! { Some(#c) },
			None => quote! { None },
		};

		let line_terminator = match &self.line_terminator {
			Some(s) => quote! { Some(#s.to_string()) },
			None => quote! { None },
		};

		let encoding = match &self.encoding {
			Some(s) => quote! { Some(#s.to_string()) },
			None => quote! { None },
		};

		tokens.extend(quote! {
			BulkLoadOptions {
				delimiter: #delimiter,
				null_string: #null_string,
				header: #header,
				columns: #columns,
				local: #local,
				quote: #quote_char,
				escape: #escape,
				line_terminator: #line_terminator,
				encoding: #encoding,
			}
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::{BulkLoadFormat, BulkLoadOptions, BulkLoadSource};
	use quote::{ToTokens, quote};
	use std::collections::HashMap;

	fn normalized_tokens<T: ToTokens>(value: &T) -> String {
		let expression: syn::Expr = syn::parse2(value.to_token_stream())
			.expect("generated migration tokens must parse as a Rust expression");
		quote!(#expression).to_string()
	}

	fn normalized_expected(tokens: proc_macro2::TokenStream) -> String {
		let expression: syn::Expr =
			syn::parse2(tokens).expect("expected tokens must parse as a Rust expression");
		quote!(#expression).to_string()
	}

	fn assert_tokens<T: ToTokens>(value: &T, expected: proc_macro2::TokenStream) {
		assert_eq!(normalized_tokens(value), normalized_expected(expected));
	}

	fn column(name: &str, type_definition: FieldType) -> ColumnDefinition {
		ColumnDefinition::new(name, type_definition)
	}

	#[test]
	fn foreign_key_actions_preserve_the_selected_variant() {
		let cases = [
			(
				ForeignKeyAction::Restrict,
				quote!(ForeignKeyAction::Restrict),
			),
			(ForeignKeyAction::Cascade, quote!(ForeignKeyAction::Cascade)),
			(ForeignKeyAction::SetNull, quote!(ForeignKeyAction::SetNull)),
			(
				ForeignKeyAction::NoAction,
				quote!(ForeignKeyAction::NoAction),
			),
			(
				ForeignKeyAction::SetDefault,
				quote!(ForeignKeyAction::SetDefault),
			),
		];

		for (value, expected) in cases {
			assert_eq!(normalized_tokens(&value), normalized_expected(expected));
		}
	}

	#[test]
	fn mysql_algorithms_preserve_the_selected_variant() {
		let cases = [
			(MySqlAlgorithm::Instant, quote!(MySqlAlgorithm::Instant)),
			(MySqlAlgorithm::Inplace, quote!(MySqlAlgorithm::Inplace)),
			(MySqlAlgorithm::Copy, quote!(MySqlAlgorithm::Copy)),
			(MySqlAlgorithm::Default, quote!(MySqlAlgorithm::Default)),
		];

		for (value, expected) in cases {
			assert_tokens(&value, expected);
		}
	}

	#[test]
	fn mysql_locks_preserve_the_selected_variant() {
		let cases = [
			(MySqlLock::None, quote!(MySqlLock::None)),
			(MySqlLock::Shared, quote!(MySqlLock::Shared)),
			(MySqlLock::Exclusive, quote!(MySqlLock::Exclusive)),
			(MySqlLock::Default, quote!(MySqlLock::Default)),
		];

		for (value, expected) in cases {
			assert_tokens(&value, expected);
		}
	}

	#[test]
	fn deferrable_options_preserve_the_selected_variant() {
		let cases = [
			(
				DeferrableOption::Immediate,
				quote!(DeferrableOption::Immediate),
			),
			(
				DeferrableOption::Deferred,
				quote!(DeferrableOption::Deferred),
			),
		];

		for (value, expected) in cases {
			assert_tokens(&value, expected);
		}
	}

	#[test]
	fn partition_types_preserve_the_selected_variant() {
		let cases = [
			(PartitionType::Range, quote!(PartitionType::Range)),
			(PartitionType::List, quote!(PartitionType::List)),
			(PartitionType::Hash, quote!(PartitionType::Hash)),
			(PartitionType::Key, quote!(PartitionType::Key)),
		];

		for (value, expected) in cases {
			assert_tokens(&value, expected);
		}
	}

	#[test]
	fn partition_values_preserve_the_selected_data() {
		let cases = [
			(
				PartitionValues::LessThan("100".to_string()),
				quote!(PartitionValues::LessThan("100".to_string())),
			),
			(
				PartitionValues::In(vec!["eu".to_string(), "us".to_string()]),
				quote!(PartitionValues::In(vec![
					"eu".to_string(),
					"us".to_string()
				])),
			),
			(
				PartitionValues::ModuloCount(8),
				quote!(PartitionValues::ModuloCount(8u32)),
			),
		];

		for (value, expected) in cases {
			assert_tokens(&value, expected);
		}
	}

	#[test]
	fn table_options_and_partition_specs_preserve_nested_values() {
		let option_cases = [
			(
				AlterTableOptions::new(),
				quote!(AlterTableOptions {
					algorithm: None,
					lock: None,
				}),
			),
			(
				AlterTableOptions::new()
					.with_algorithm(MySqlAlgorithm::Inplace)
					.with_lock(MySqlLock::Shared),
				quote!(AlterTableOptions {
					algorithm: Some(MySqlAlgorithm::Inplace),
					lock: Some(MySqlLock::Shared),
				}),
			),
		];

		for (value, expected) in option_cases {
			assert_tokens(&value, expected);
		}

		let partition = PartitionOptions::new(
			PartitionType::Range,
			"id",
			vec![
				PartitionDef::new("before_100", PartitionValues::LessThan("100".to_string())),
				PartitionDef::new(
					"after_100",
					PartitionValues::LessThan("MAXVALUE".to_string()),
				),
			],
		);
		assert_tokens(
			&partition,
			quote!(PartitionOptions {
				partition_type: PartitionType::Range,
				column: "id".to_string(),
				partitions: vec![
					PartitionDef {
						name: "before_100".to_string(),
						values: PartitionValues::LessThan("100".to_string()),
					},
					PartitionDef {
						name: "after_100".to_string(),
						values: PartitionValues::LessThan("MAXVALUE".to_string()),
					}
				],
			}),
		);

		let interleave = InterleaveSpec {
			parent_table: "accounts".to_string(),
			parent_columns: vec!["tenant_id".to_string(), "id".to_string()],
		};
		assert_tokens(
			&interleave,
			quote!(InterleaveSpec {
				parent_table: "accounts".to_string(),
				parent_columns: vec!["tenant_id".to_string(), "id".to_string()],
			}),
		);
	}

	fn assert_column_type_tokens(
		field_type: FieldType,
		expected_field_type: proc_macro2::TokenStream,
	) {
		let value = column("value", field_type);
		assert_tokens(
			&value,
			quote!(ColumnDefinition {
				name: "value".to_string(),
				type_definition: #expected_field_type,
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
			}),
		);
	}

	#[test]
	fn column_definitions_preserve_integer_and_string_types() {
		let cases = [
			(FieldType::BigInteger, quote!(FieldType::BigInteger)),
			(FieldType::Integer, quote!(FieldType::Integer)),
			(FieldType::SmallInteger, quote!(FieldType::SmallInteger)),
			(FieldType::TinyInt, quote!(FieldType::TinyInt)),
			(FieldType::MediumInt, quote!(FieldType::MediumInt)),
			(FieldType::Char(12), quote!(FieldType::Char(12u32))),
			(FieldType::VarChar(255), quote!(FieldType::VarChar(255u32))),
			(FieldType::Text, quote!(FieldType::Text)),
			(FieldType::TinyText, quote!(FieldType::TinyText)),
			(FieldType::MediumText, quote!(FieldType::MediumText)),
			(FieldType::LongText, quote!(FieldType::LongText)),
		];

		for (field_type, expected) in cases {
			assert_column_type_tokens(field_type, expected);
		}
	}

	#[test]
	fn column_definitions_preserve_temporal_numeric_boolean_and_binary_types() {
		let cases = [
			(FieldType::Date, quote!(FieldType::Date)),
			(FieldType::Time, quote!(FieldType::Time)),
			(FieldType::DateTime, quote!(FieldType::DateTime)),
			(FieldType::TimestampTz, quote!(FieldType::TimestampTz)),
			(
				FieldType::Decimal {
					precision: 10,
					scale: 2,
				},
				quote!(FieldType::Decimal {
					precision: 10u32,
					scale: 2u32
				}),
			),
			(FieldType::Float, quote!(FieldType::Float)),
			(FieldType::Double, quote!(FieldType::Double)),
			(FieldType::Real, quote!(FieldType::Real)),
			(FieldType::Boolean, quote!(FieldType::Boolean)),
			(FieldType::Binary, quote!(FieldType::Binary)),
			(FieldType::Blob, quote!(FieldType::Blob)),
			(FieldType::TinyBlob, quote!(FieldType::TinyBlob)),
			(FieldType::MediumBlob, quote!(FieldType::MediumBlob)),
			(FieldType::LongBlob, quote!(FieldType::LongBlob)),
			(FieldType::Bytea, quote!(FieldType::Bytea)),
		];

		for (field_type, expected) in cases {
			assert_column_type_tokens(field_type, expected);
		}
	}

	#[test]
	fn column_definitions_preserve_json_and_postgres_types() {
		let cases = [
			(FieldType::Json, quote!(FieldType::Json)),
			(FieldType::JsonBinary, quote!(FieldType::JsonBinary)),
			(
				FieldType::Array(Box::new(FieldType::Array(Box::new(FieldType::Integer)))),
				quote!(FieldType::Array(Box::new(FieldType::Array(Box::new(
					FieldType::Integer
				))))),
			),
			(FieldType::HStore, quote!(FieldType::HStore)),
			(FieldType::CIText, quote!(FieldType::CIText)),
			(FieldType::Int4Range, quote!(FieldType::Int4Range)),
			(FieldType::Int8Range, quote!(FieldType::Int8Range)),
			(FieldType::NumRange, quote!(FieldType::NumRange)),
			(FieldType::DateRange, quote!(FieldType::DateRange)),
			(FieldType::TsRange, quote!(FieldType::TsRange)),
			(FieldType::TsTzRange, quote!(FieldType::TsTzRange)),
			(FieldType::TsVector, quote!(FieldType::TsVector)),
			(FieldType::TsQuery, quote!(FieldType::TsQuery)),
		];

		for (field_type, expected) in cases {
			assert_column_type_tokens(field_type, expected);
		}
	}

	#[test]
	fn column_definitions_preserve_collections_relationships_and_custom_types() {
		let cases = [
			(FieldType::Uuid, quote!(FieldType::Uuid)),
			(FieldType::Year, quote!(FieldType::Year)),
			(
				FieldType::Enum {
					values: vec!["draft".to_string(), "published".to_string()],
				},
				quote!(FieldType::Enum {
					values: vec!["draft".to_string(), "published".to_string()]
				}),
			),
			(
				FieldType::Set {
					values: vec!["reader".to_string(), "writer".to_string()],
				},
				quote!(FieldType::Set {
					values: vec!["reader".to_string(), "writer".to_string()]
				}),
			),
			(
				FieldType::OneToOne {
					to: "accounts.User".to_string(),
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Restrict,
				},
				quote!(FieldType::OneToOne {
					to: "accounts.User".to_string(),
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Restrict,
				}),
			),
			(
				FieldType::ManyToMany {
					to: "accounts.Group".to_string(),
					through: Some("membership".to_string()),
				},
				quote!(FieldType::ManyToMany {
					to: "accounts.Group".to_string(),
					through: Some("membership".to_string()),
				}),
			),
			(
				FieldType::ManyToMany {
					to: "accounts.Role".to_string(),
					through: None,
				},
				quote!(FieldType::ManyToMany {
					to: "accounts.Role".to_string(),
					through: None,
				}),
			),
			(
				FieldType::ForeignKey {
					to_table: "accounts".to_string(),
					to_field: "id".to_string(),
					on_delete: ForeignKeyAction::SetNull,
				},
				quote!(FieldType::ForeignKey {
					to_table: "accounts".to_string(),
					to_field: "id".to_string(),
					on_delete: ForeignKeyAction::SetNull,
				}),
			),
			(
				FieldType::Custom("citext_domain".to_string()),
				quote!(FieldType::Custom("citext_domain".to_string())),
			),
		];

		for (field_type, expected) in cases {
			assert_column_type_tokens(field_type, expected);
		}
	}

	#[test]
	fn column_definitions_preserve_flags_and_defaults() {
		let fully_populated = ColumnDefinition {
			name: "id".to_string(),
			type_definition: FieldType::BigInteger,
			not_null: true,
			unique: true,
			primary_key: true,
			auto_increment: true,
			default: Some("42".to_string()),
		};
		assert_tokens(
			&fully_populated,
			quote!(ColumnDefinition {
				name: "id".to_string(),
				type_definition: FieldType::BigInteger,
				not_null: true,
				unique: true,
				primary_key: true,
				auto_increment: true,
				default: Some("42".to_string()),
			}),
		);

		let minimal = column("name", FieldType::VarChar(255));
		assert_tokens(
			&minimal,
			quote!(ColumnDefinition {
				name: "name".to_string(),
				type_definition: FieldType::VarChar(255u32),
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
			}),
		);
	}

	#[test]
	fn constraints_preserve_every_supported_variant() {
		let constraints = [
			(
				Constraint::PrimaryKey {
					name: "pk_booking".to_string(),
					columns: vec!["room_id".to_string(), "period".to_string()],
				},
				quote!(Constraint::PrimaryKey {
					name: "pk_booking".to_string(),
					columns: vec!["room_id".to_string(), "period".to_string()],
				}),
			),
			(
				Constraint::ForeignKey {
					name: "fk_booking_room".to_string(),
					columns: vec!["room_id".to_string()],
					referenced_table: "rooms".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Restrict,
					deferrable: Some(DeferrableOption::Deferred),
				},
				quote!(Constraint::ForeignKey {
					name: "fk_booking_room".to_string(),
					columns: vec!["room_id".to_string()],
					referenced_table: "rooms".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Restrict,
					deferrable: Some(DeferrableOption::Deferred),
				}),
			),
			(
				Constraint::ForeignKey {
					name: "fk_booking_room_optional".to_string(),
					columns: vec!["room_id".to_string()],
					referenced_table: "rooms".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::SetNull,
					on_update: ForeignKeyAction::SetDefault,
					deferrable: None,
				},
				quote!(Constraint::ForeignKey {
					name: "fk_booking_room_optional".to_string(),
					columns: vec!["room_id".to_string()],
					referenced_table: "rooms".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::SetNull,
					on_update: ForeignKeyAction::SetDefault,
					deferrable: None,
				}),
			),
			(
				Constraint::Unique {
					name: "uq_booking".to_string(),
					columns: vec!["room_id".to_string(), "starts_at".to_string()],
				},
				quote!(Constraint::Unique {
					name: "uq_booking".to_string(),
					columns: vec!["room_id".to_string(), "starts_at".to_string()],
				}),
			),
			(
				Constraint::Check {
					name: "ck_booking_end".to_string(),
					expression: "ends_at > starts_at".to_string(),
				},
				quote!(Constraint::Check {
					name: "ck_booking_end".to_string(),
					expression: "ends_at > starts_at".to_string(),
				}),
			),
			(
				Constraint::OneToOne {
					name: "fk_profile_user".to_string(),
					column: "user_id".to_string(),
					referenced_table: "users".to_string(),
					referenced_column: "id".to_string(),
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::NoAction,
					deferrable: Some(DeferrableOption::Immediate),
				},
				quote!(Constraint::OneToOne {
					name: "fk_profile_user".to_string(),
					column: "user_id".to_string(),
					referenced_table: "users".to_string(),
					referenced_column: "id".to_string(),
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::NoAction,
					deferrable: Some(DeferrableOption::Immediate),
				}),
			),
			(
				Constraint::OneToOne {
					name: "fk_profile_user_optional".to_string(),
					column: "user_id".to_string(),
					referenced_table: "users".to_string(),
					referenced_column: "id".to_string(),
					on_delete: ForeignKeyAction::Restrict,
					on_update: ForeignKeyAction::Cascade,
					deferrable: None,
				},
				quote!(Constraint::OneToOne {
					name: "fk_profile_user_optional".to_string(),
					column: "user_id".to_string(),
					referenced_table: "users".to_string(),
					referenced_column: "id".to_string(),
					on_delete: ForeignKeyAction::Restrict,
					on_update: ForeignKeyAction::Cascade,
					deferrable: None,
				}),
			),
			(
				Constraint::ManyToMany {
					name: "memberships".to_string(),
					through_table: "memberships".to_string(),
					source_column: "user_id".to_string(),
					target_column: "group_id".to_string(),
					target_table: "groups".to_string(),
				},
				quote!(Constraint::ManyToMany {
					name: "memberships".to_string(),
					through_table: "memberships".to_string(),
					source_column: "user_id".to_string(),
					target_column: "group_id".to_string(),
					target_table: "groups".to_string(),
				}),
			),
		];

		for (constraint, expected) in constraints {
			assert_tokens(&constraint, expected);
		}

		let exclude = Constraint::Exclude {
			name: "no_overlap".to_string(),
			elements: vec![
				("room".to_string(), "=".to_string()),
				("period".to_string(), "&&".to_string()),
			],
			using: Some("gist".to_string()),
			where_clause: Some("cancelled = false".to_string()),
		};
		assert!(exclude.to_token_stream().is_empty());
	}

	#[test]
	fn schema_operations_preserve_table_column_and_constraint_data() {
		let populated_column = ColumnDefinition {
			name: "id".to_string(),
			type_definition: FieldType::BigInteger,
			not_null: true,
			unique: true,
			primary_key: true,
			auto_increment: true,
			default: Some("1".to_string()),
		};
		let create = Operation::CreateTable {
			name: "bookings".to_string(),
			columns: vec![populated_column.clone()],
			constraints: vec![Constraint::PrimaryKey {
				name: "pk_bookings".to_string(),
				columns: vec!["id".to_string()],
			}],
			without_rowid: Some(true),
			interleave_in_parent: Some(InterleaveSpec {
				parent_table: "accounts".to_string(),
				parent_columns: vec!["tenant_id".to_string(), "id".to_string()],
			}),
			partition: Some(PartitionOptions::new(
				PartitionType::Range,
				"id",
				vec![PartitionDef::new(
					"before_100",
					PartitionValues::LessThan("100".to_string()),
				)],
			)),
		};
		assert_tokens(
			&create,
			quote!(Operation::CreateTable {
				name: "bookings".to_string(),
				columns: vec![ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::BigInteger,
					not_null: true,
					unique: true,
					primary_key: true,
					auto_increment: true,
					default: Some("1".to_string()),
				}],
				constraints: vec![Constraint::PrimaryKey {
					name: "pk_bookings".to_string(),
					columns: vec!["id".to_string()],
				}],
				without_rowid: Some(true),
				interleave_in_parent: Some(InterleaveSpec {
					parent_table: "accounts".to_string(),
					parent_columns: vec!["tenant_id".to_string(), "id".to_string()],
				}),
				partition: Some(PartitionOptions {
					partition_type: PartitionType::Range,
					column: "id".to_string(),
					partitions: vec![PartitionDef {
						name: "before_100".to_string(),
						values: PartitionValues::LessThan("100".to_string()),
					}],
				}),
			}),
		);

		let minimal_create = Operation::CreateTable {
			name: "audit_log".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		assert_tokens(
			&minimal_create,
			quote!(Operation::CreateTable {
				name: "audit_log".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			}),
		);
		let rowid_create = Operation::CreateTable {
			name: "with_rowid".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: Some(false),
			interleave_in_parent: None,
			partition: None,
		};
		assert_tokens(
			&rowid_create,
			quote!(Operation::CreateTable {
				name: "with_rowid".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: Some(false),
				interleave_in_parent: None,
				partition: None,
			}),
		);

		let add_column = Operation::AddColumn {
			table: "bookings".to_string(),
			column: populated_column.clone(),
			mysql_options: Some(
				AlterTableOptions::new()
					.with_algorithm(MySqlAlgorithm::Instant)
					.with_lock(MySqlLock::None),
			),
		};
		assert_tokens(
			&add_column,
			quote!(Operation::AddColumn {
				table: "bookings".to_string(),
				column: ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::BigInteger,
					not_null: true,
					unique: true,
					primary_key: true,
					auto_increment: true,
					default: Some("1".to_string()),
				},
				mysql_options: Some(AlterTableOptions {
					algorithm: Some(MySqlAlgorithm::Instant),
					lock: Some(MySqlLock::None),
				}),
			}),
		);
		assert_tokens(
			&Operation::AddColumn {
				table: "bookings".to_string(),
				column: column("status", FieldType::Text),
				mysql_options: None,
			},
			quote!(Operation::AddColumn {
				table: "bookings".to_string(),
				column: ColumnDefinition {
					name: "status".to_string(),
					type_definition: FieldType::Text,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			}),
		);

		assert_tokens(
			&Operation::DropTable {
				name: "bookings".to_string(),
			},
			quote!(Operation::DropTable {
				name: "bookings".to_string(),
			}),
		);
		assert_tokens(
			&Operation::DropColumn {
				table: "bookings".to_string(),
				column: "status".to_string(),
			},
			quote!(Operation::DropColumn {
				table: "bookings".to_string(),
				column: "status".to_string(),
			}),
		);
		assert_tokens(
			&Operation::AlterColumn {
				table: "bookings".to_string(),
				column: "status".to_string(),
				old_definition: Some(column("status", FieldType::Text)),
				new_definition: column("status", FieldType::VarChar(32)),
				mysql_options: Some(AlterTableOptions::new().with_algorithm(MySqlAlgorithm::Copy)),
			},
			quote!(Operation::AlterColumn {
				table: "bookings".to_string(),
				column: "status".to_string(),
				old_definition: Some(ColumnDefinition {
					name: "status".to_string(),
					type_definition: FieldType::Text,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				}),
				new_definition: ColumnDefinition {
					name: "status".to_string(),
					type_definition: FieldType::VarChar(32u32),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: Some(AlterTableOptions {
					algorithm: Some(MySqlAlgorithm::Copy),
					lock: None,
				}),
			}),
		);
		assert_tokens(
			&Operation::AlterColumn {
				table: "bookings".to_string(),
				column: "status".to_string(),
				old_definition: None,
				new_definition: column("status", FieldType::Text),
				mysql_options: None,
			},
			quote!(Operation::AlterColumn {
				table: "bookings".to_string(),
				column: "status".to_string(),
				old_definition: None,
				new_definition: ColumnDefinition {
					name: "status".to_string(),
					type_definition: FieldType::Text,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			}),
		);
		assert_tokens(
			&Operation::RenameTable {
				old_name: "bookings".to_string(),
				new_name: "reservations".to_string(),
			},
			quote!(Operation::RenameTable {
				old_name: "bookings".to_string(),
				new_name: "reservations".to_string(),
			}),
		);
		assert_tokens(
			&Operation::RenameColumn {
				table: "bookings".to_string(),
				old_name: "status".to_string(),
				new_name: "state".to_string(),
			},
			quote!(Operation::RenameColumn {
				table: "bookings".to_string(),
				old_name: "status".to_string(),
				new_name: "state".to_string(),
			}),
		);
		assert_tokens(
			&Operation::AddConstraint {
				table: "bookings".to_string(),
				constraint_sql: "CHECK (id > 0)".to_string(),
			},
			quote!(Operation::AddConstraint {
				table: "bookings".to_string(),
				constraint_sql: "CHECK (id > 0)".to_string(),
			}),
		);
		assert_tokens(
			&Operation::DropConstraint {
				table: "bookings".to_string(),
				constraint_name: "ck_booking_id".to_string(),
			},
			quote!(Operation::DropConstraint {
				table: "bookings".to_string(),
				constraint_name: "ck_booking_id".to_string(),
			}),
		);
	}

	#[test]
	fn index_and_data_operations_preserve_optional_values() {
		let create_index = Operation::CreateIndex {
			table: "bookings".to_string(),
			columns: vec!["room_id".to_string()],
			unique: true,
			index_type: Some(IndexType::Gin),
			where_clause: Some("cancelled = false".to_string()),
			concurrently: true,
			expressions: Some(vec!["lower(reference)".to_string()]),
			mysql_options: Some(AlterTableOptions::new().with_lock(MySqlLock::Shared)),
			operator_class: Some("gin_trgm_ops".to_string()),
		};
		assert_tokens(
			&create_index,
			quote!(Operation::CreateIndex {
				table: "bookings".to_string(),
				columns: vec!["room_id".to_string()],
				unique: true,
				index_type: Some(IndexType::Gin),
				where_clause: Some("cancelled = false".to_string()),
				concurrently: true,
				expressions: Some(vec!["lower(reference)".to_string()]),
				mysql_options: Some(AlterTableOptions {
					algorithm: None,
					lock: Some(MySqlLock::Shared),
				}),
				operator_class: Some("gin_trgm_ops".to_string()),
			}),
		);
		assert_tokens(
			&Operation::CreateIndex {
				table: "bookings".to_string(),
				columns: vec!["created_at".to_string()],
				unique: false,
				index_type: None,
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			},
			quote!(Operation::CreateIndex {
				table: "bookings".to_string(),
				columns: vec!["created_at".to_string()],
				unique: false,
				index_type: None,
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			}),
		);
		assert_tokens(
			&Operation::DropIndex {
				table: "bookings".to_string(),
				columns: vec!["room_id".to_string()],
			},
			quote!(Operation::DropIndex {
				table: "bookings".to_string(),
				columns: vec!["room_id".to_string()],
			}),
		);

		for (operation, expected) in [
			(
				Operation::RunSQL {
					sql: "DELETE FROM bookings".to_string(),
					reverse_sql: Some("INSERT INTO bookings DEFAULT VALUES".to_string()),
				},
				quote!(Operation::RunSQL {
					sql: "DELETE FROM bookings".to_string(),
					reverse_sql: Some("INSERT INTO bookings DEFAULT VALUES".to_string()),
				}),
			),
			(
				Operation::RunSQL {
					sql: "VACUUM".to_string(),
					reverse_sql: None,
				},
				quote!(Operation::RunSQL {
					sql: "VACUUM".to_string(),
					reverse_sql: None,
				}),
			),
			(
				Operation::RunRust {
					code: "apply()".to_string(),
					reverse_code: Some("rollback()".to_string()),
				},
				quote!(Operation::RunRust {
					code: "apply()".to_string(),
					reverse_code: Some("rollback()".to_string()),
				}),
			),
			(
				Operation::RunRust {
					code: "rebuild()".to_string(),
					reverse_code: None,
				},
				quote!(Operation::RunRust {
					code: "rebuild()".to_string(),
					reverse_code: None,
				}),
			),
			(
				Operation::AlterTableComment {
					table: "bookings".to_string(),
					comment: Some("Reservation records".to_string()),
				},
				quote!(Operation::AlterTableComment {
					table: "bookings".to_string(),
					comment: Some("Reservation records".to_string()),
				}),
			),
			(
				Operation::AlterTableComment {
					table: "bookings".to_string(),
					comment: None,
				},
				quote!(Operation::AlterTableComment {
					table: "bookings".to_string(),
					comment: None,
				}),
			),
		] {
			assert_tokens(&operation, expected);
		}

		assert_tokens(
			&Operation::AlterUniqueTogether {
				table: "bookings".to_string(),
				unique_together: vec![vec!["room_id".to_string(), "starts_at".to_string()]],
			},
			quote!(Operation::AlterUniqueTogether {
				table: "bookings".to_string(),
				unique_together: vec![vec!["room_id".to_string(), "starts_at".to_string()]],
			}),
		);
		let options = HashMap::from([("verbose".to_string(), "true".to_string())]);
		assert_tokens(
			&Operation::AlterModelOptions {
				table: "bookings".to_string(),
				options,
			},
			quote!(Operation::AlterModelOptions {
				table: "bookings".to_string(),
				options: {
					let mut map = std::collections::HashMap::new();
					map.insert("verbose".to_string(), "true".to_string());
					map
				},
			}),
		);
	}

	#[test]
	fn inheritance_schema_extension_and_bulk_load_operations_preserve_data() {
		assert_tokens(
			&Operation::CreateInheritedTable {
				name: "premium_booking".to_string(),
				columns: vec![column("priority", FieldType::Integer)],
				base_table: "bookings".to_string(),
				join_column: "booking_id".to_string(),
			},
			quote!(Operation::CreateInheritedTable {
				name: "premium_booking".to_string(),
				columns: vec![ColumnDefinition {
					name: "priority".to_string(),
					type_definition: FieldType::Integer,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				}],
				base_table: "bookings".to_string(),
				join_column: "booking_id".to_string(),
			}),
		);
		assert_tokens(
			&Operation::AddDiscriminatorColumn {
				table: "bookings".to_string(),
				column_name: "kind".to_string(),
				default_value: "standard".to_string(),
			},
			quote!(Operation::AddDiscriminatorColumn {
				table: "bookings".to_string(),
				column_name: "kind".to_string(),
				default_value: "standard".to_string(),
			}),
		);
		assert_tokens(
			&Operation::MoveModel {
				model_name: "Booking".to_string(),
				from_app: "reservations".to_string(),
				to_app: "archive".to_string(),
				rename_table: true,
				old_table_name: Some("bookings".to_string()),
				new_table_name: Some("archived_bookings".to_string()),
			},
			quote!(Operation::MoveModel {
				model_name: "Booking".to_string(),
				from_app: "reservations".to_string(),
				to_app: "archive".to_string(),
				rename_table: true,
				old_table_name: Some("bookings".to_string()),
				new_table_name: Some("archived_bookings".to_string()),
			}),
		);
		assert_tokens(
			&Operation::MoveModel {
				model_name: "Booking".to_string(),
				from_app: "reservations".to_string(),
				to_app: "archive".to_string(),
				rename_table: false,
				old_table_name: None,
				new_table_name: None,
			},
			quote!(Operation::MoveModel {
				model_name: "Booking".to_string(),
				from_app: "reservations".to_string(),
				to_app: "archive".to_string(),
				rename_table: false,
				old_table_name: None,
				new_table_name: None,
			}),
		);

		for (operation, expected) in [
			(
				Operation::CreateSchema {
					name: "reporting".to_string(),
					if_not_exists: true,
				},
				quote!(Operation::CreateSchema {
					name: "reporting".to_string(),
					if_not_exists: true,
				}),
			),
			(
				Operation::DropSchema {
					name: "reporting".to_string(),
					cascade: true,
					if_exists: false,
				},
				quote!(Operation::DropSchema {
					name: "reporting".to_string(),
					cascade: true,
					if_exists: false,
				}),
			),
			(
				Operation::CreateExtension {
					name: "pg_trgm".to_string(),
					if_not_exists: true,
					schema: Some("extensions".to_string()),
				},
				quote!(Operation::CreateExtension {
					name: "pg_trgm".to_string(),
					if_not_exists: true,
					schema: Some("extensions".to_string()),
				}),
			),
			(
				Operation::CreateExtension {
					name: "hstore".to_string(),
					if_not_exists: false,
					schema: None,
				},
				quote!(Operation::CreateExtension {
					name: "hstore".to_string(),
					if_not_exists: false,
					schema: None,
				}),
			),
		] {
			assert_tokens(&operation, expected);
		}

		let bulk_load = Operation::BulkLoad {
			table: "events".to_string(),
			source: BulkLoadSource::File("/tmp/events.csv".to_string()),
			format: BulkLoadFormat::Csv,
			options: BulkLoadOptions {
				delimiter: Some(','),
				null_string: Some("NULL".to_string()),
				header: true,
				columns: Some(vec!["id".to_string(), "name".to_string()]),
				local: false,
				quote: Some('"'),
				escape: Some('\\'),
				line_terminator: Some("\n".to_string()),
				encoding: Some("UTF-8".to_string()),
			},
		};
		assert_tokens(
			&bulk_load,
			quote!(Operation::BulkLoad {
				table: "events".to_string(),
				source: BulkLoadSource::File("/tmp/events.csv".to_string()),
				format: BulkLoadFormat::Csv,
				options: BulkLoadOptions {
					delimiter: Some(','),
					null_string: Some("NULL".to_string()),
					header: true,
					columns: Some(vec!["id".to_string(), "name".to_string()]),
					local: false,
					quote: Some('"'),
					escape: Some('\\'),
					line_terminator: Some("\n".to_string()),
					encoding: Some("UTF-8".to_string()),
				},
			}),
		);

		let source_cases = [
			(
				BulkLoadSource::File("/tmp/events.csv".to_string()),
				quote!(BulkLoadSource::File("/tmp/events.csv".to_string())),
			),
			(BulkLoadSource::Stdin, quote!(BulkLoadSource::Stdin)),
			(
				BulkLoadSource::Program("gzip -dc events.csv.gz".to_string()),
				quote!(BulkLoadSource::Program(
					"gzip -dc events.csv.gz".to_string()
				)),
			),
		];
		for (source, expected) in source_cases {
			assert_tokens(&source, expected);
		}

		let format_cases = [
			(BulkLoadFormat::Text, quote!(BulkLoadFormat::Text)),
			(BulkLoadFormat::Csv, quote!(BulkLoadFormat::Csv)),
			(BulkLoadFormat::Binary, quote!(BulkLoadFormat::Binary)),
		];
		for (format, expected) in format_cases {
			assert_tokens(&format, expected);
		}

		let no_option_values = BulkLoadOptions {
			delimiter: None,
			null_string: None,
			header: false,
			columns: None,
			local: true,
			quote: None,
			escape: None,
			line_terminator: None,
			encoding: None,
		};
		assert_tokens(
			&no_option_values,
			quote!(BulkLoadOptions {
				delimiter: None,
				null_string: None,
				header: false,
				columns: None,
				local: true,
				quote: None,
				escape: None,
				line_terminator: None,
				encoding: None,
			}),
		);
		assert_tokens(
			&Operation::SetAutoIncrementValue {
				table: "events".to_string(),
				column: "id".to_string(),
				value: 500i64,
			},
			quote!(Operation::SetAutoIncrementValue {
				table: "events".to_string(),
				column: "id".to_string(),
				value: 500i64,
			}),
		);
		assert_tokens(
			&Operation::CreateCompositePrimaryKey {
				table: "event_tags".to_string(),
				columns: vec!["event_id".to_string(), "tag_id".to_string()],
				constraint_name: Some("pk_event_tags".to_string()),
			},
			quote!(Operation::CreateCompositePrimaryKey {
				table: "event_tags".to_string(),
				columns: vec!["event_id".to_string(), "tag_id".to_string()],
				constraint_name: Some("pk_event_tags".to_string()),
			}),
		);
		assert_tokens(
			&Operation::CreateCompositePrimaryKey {
				table: "event_tags".to_string(),
				columns: vec!["event_id".to_string(), "tag_id".to_string()],
				constraint_name: None,
			},
			quote!(Operation::CreateCompositePrimaryKey {
				table: "event_tags".to_string(),
				columns: vec!["event_id".to_string(), "tag_id".to_string()],
				constraint_name: None,
			}),
		);
	}
}
