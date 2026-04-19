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
