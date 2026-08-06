use super::{
	AlterTableOptions, InterleaveSpec, MySqlAlgorithm, MySqlLock, PartitionDef, PartitionOptions,
	PartitionType, PartitionValues,
};
use crate::migrations::{
	ColumnDefinition, Constraint, DeferrableOption, FieldType, ForeignKeyAction,
	GeneratedColumnDefinition, IndexType, Operation,
};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use reinhardt_query::prelude::{
	ColumnType as QueryColumnType, GeneratedStorage, SchemaBinOper, SchemaExpr, SchemaFunc, Value,
};

impl ToTokens for crate::field_domain::FieldDomain {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Self::Enum { repr, values } => {
				let repr = match repr {
					crate::field_domain::ModelEnumRepr::String => {
						quote! { ModelEnumRepr::String }
					}
					crate::field_domain::ModelEnumRepr::I32 => quote! { ModelEnumRepr::I32 },
				};
				let values = values.iter().map(|value| match value {
					crate::field_domain::ModelEnumValue::String(value) => {
						quote! { ModelEnumValue::String(#value.to_string()) }
					}
					crate::field_domain::ModelEnumValue::I32(value) => {
						quote! { ModelEnumValue::I32(#value) }
					}
				});
				tokens.extend(quote! {
					FieldDomain::Enum {
						repr: #repr,
						values: vec![#(#values),*],
					}
				});
			}
		}
	}
}

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
		FieldType::Jsonb => quote! { FieldType::Jsonb },

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
		#[cfg(feature = "pgvector")]
		FieldType::Vector { dimensions } => {
			quote! { FieldType::Vector { dimensions: #dimensions } }
		}

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
		let algorithm_token = self
			.algorithm
			.as_ref()
			.map(|algorithm| quote! { .with_algorithm(#algorithm) });
		let lock_token = self.lock.as_ref().map(|lock| quote! { .with_lock(#lock) });
		tokens.extend(quote! { AlterTableOptions::new() #algorithm_token #lock_token });
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
			PartitionOptions::new(#partition_type, #column, vec![#(#partitions),*])
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
			Constraint::EnumDomain {
				name,
				column,
				domain,
			} => {
				tokens.extend(quote! {
					Constraint::EnumDomain {
						name: #name.to_string(),
						column: #column.to_string(),
						domain: #domain,
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
			Operation::DropColumn {
				table,
				column,
				old_definition,
			} => {
				let old_def_token = match old_definition {
					Some(def) => quote! { Some(#def) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::DropColumn {
						table: #table.to_string(),
						column: #column.to_string(),
						old_definition: #old_def_token,
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
			Operation::AddConstraintDefinition { table, constraint } => {
				tokens.extend(quote! {
					Operation::AddConstraintDefinition {
						table: #table.to_string(),
						constraint: #constraint,
					}
				});
			}
			Operation::AddConstraintRepair {
				table,
				constraint_sql,
			} => {
				tokens.extend(quote! {
					Operation::AddConstraintRepair {
						table: #table.to_string(),
						constraint_sql: #constraint_sql.to_string(),
					}
				});
			}
			Operation::RestoreConstraintOnRollback {
				table,
				constraint_sql,
			} => {
				tokens.extend(quote! {
					Operation::RestoreConstraintOnRollback {
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
			Operation::DropConstraintDefinition { table, constraint } => {
				tokens.extend(quote! {
					Operation::DropConstraintDefinition {
						table: #table.to_string(),
						constraint: #constraint,
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
			}
			| Operation::CreateIndexRepair {
				table,
				name: _,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			}
			| Operation::RestoreIndexOnRollback {
				table,
				name: _,
				columns,
				unique,
				index_type,
				where_clause,
				concurrently,
				expressions,
				mysql_options,
				operator_class,
			} => {
				let variant = match self {
					Operation::CreateIndex { .. } => format_ident!("CreateIndex"),
					Operation::CreateIndexRepair { .. } => format_ident!("CreateIndexRepair"),
					Operation::RestoreIndexOnRollback { .. } => {
						format_ident!("RestoreIndexOnRollback")
					}
					_ => unreachable!(),
				};
				let columns_iter = columns.iter();
				let name_field = match self {
					Operation::CreateIndex { .. } => quote! {},
					Operation::CreateIndexRepair { name, .. }
					| Operation::RestoreIndexOnRollback { name, .. } => match name {
						Some(name) => quote! { name: Some(#name.to_string()), },
						None => quote! { name: None, },
					},
					_ => unreachable!(),
				};
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
							#[cfg(feature = "pgvector")]
							IndexType::Hnsw { m, ef_construction } => {
								let m = optional_u16_to_tokens(*m);
								let ef_construction = optional_u16_to_tokens(*ef_construction);
								quote! {
									IndexType::Hnsw {
										m: #m,
										ef_construction: #ef_construction,
									}
								}
							}
							#[cfg(feature = "pgvector")]
							IndexType::Ivfflat { lists } => {
								let lists = optional_u32_to_tokens(*lists);
								quote! { IndexType::Ivfflat { lists: #lists } }
							}
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
						Operation::#variant {
							table: #table.to_string(),
							#name_field
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
			#[cfg(feature = "pgvector")]
			Operation::CreateNamedIndex {
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
					Some(IndexType::Hnsw { m, ef_construction }) => {
						let m = optional_u16_to_tokens(*m);
						let ef_construction = optional_u16_to_tokens(*ef_construction);
						quote! {
							Some(IndexType::Hnsw {
								m: #m,
								ef_construction: #ef_construction,
							})
						}
					}
					Some(IndexType::Ivfflat { lists }) => {
						let lists = optional_u32_to_tokens(*lists);
						quote! { Some(IndexType::Ivfflat { lists: #lists }) }
					}
					Some(IndexType::BTree) => quote! { Some(IndexType::BTree) },
					Some(IndexType::Hash) => quote! { Some(IndexType::Hash) },
					Some(IndexType::Gin) => quote! { Some(IndexType::Gin) },
					Some(IndexType::Gist) => quote! { Some(IndexType::Gist) },
					Some(IndexType::Brin) => quote! { Some(IndexType::Brin) },
					Some(IndexType::Fulltext) => quote! { Some(IndexType::Fulltext) },
					Some(IndexType::Spatial) => quote! { Some(IndexType::Spatial) },
					None => quote! { None },
				};
				let where_clause_token = match where_clause {
					Some(value) => quote! { Some(#value.to_string()) },
					None => quote! { None },
				};
				let expressions_token = match expressions {
					Some(values) => {
						let values = values.iter();
						quote! { Some(vec![#(#values.to_string()),*]) }
					}
					None => quote! { None },
				};
				let mysql_options_token = match mysql_options {
					Some(value) => quote! { Some(#value) },
					None => quote! { None },
				};
				let operator_class_token = match operator_class {
					Some(value) => quote! { Some(#value.to_string()) },
					None => quote! { None },
				};
				tokens.extend(quote! {
					Operation::CreateNamedIndex {
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
			Operation::DropIndex { table, columns } => {
				let columns_iter = columns.iter();
				tokens.extend(quote! {
					Operation::DropIndex {
						table: #table.to_string(),
						columns: vec![#(#columns_iter.to_string()),*],
					}
				});
			}
			#[cfg(feature = "pgvector")]
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
					#[cfg(feature = "pgvector")]
					Some(IndexType::Hnsw { m, ef_construction }) => {
						let m = optional_u16_to_tokens(*m);
						let ef_construction = optional_u16_to_tokens(*ef_construction);
						quote! {
							Some(IndexType::Hnsw {
								m: #m,
								ef_construction: #ef_construction,
							})
						}
					}
					#[cfg(feature = "pgvector")]
					Some(IndexType::Ivfflat { lists }) => {
						let lists = optional_u32_to_tokens(*lists);
						quote! { Some(IndexType::Ivfflat { lists: #lists }) }
					}
					Some(IndexType::BTree) => quote! { Some(IndexType::BTree) },
					Some(IndexType::Hash) => quote! { Some(IndexType::Hash) },
					Some(IndexType::Gin) => quote! { Some(IndexType::Gin) },
					Some(IndexType::Gist) => quote! { Some(IndexType::Gist) },
					Some(IndexType::Brin) => quote! { Some(IndexType::Brin) },
					Some(IndexType::Fulltext) => quote! { Some(IndexType::Fulltext) },
					Some(IndexType::Spatial) => quote! { Some(IndexType::Spatial) },
					None => quote! { None },
				};
				let where_clause_token = match where_clause {
					Some(value) => quote! { Some(#value.to_string()) },
					None => quote! { None },
				};
				let expressions_token = match expressions {
					Some(values) => {
						let values = values.iter();
						quote! { Some(vec![#(#values.to_string()),*]) }
					}
					None => quote! { None },
				};
				let mysql_options_token = match mysql_options {
					Some(value) => quote! { Some(#value) },
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
				let mut entries: Vec<_> = options.iter().collect();
				entries.sort_unstable_by_key(|(key, _)| *key);
				let entries = entries.iter().map(|(key, value)| {
					quote! { map.insert(#key.to_string(), #value.to_string()); }
				});
				tokens.extend(quote! {
					Operation::AlterModelOptions {
						table: #table.to_string(),
						options: {
							let mut map = std::collections::HashMap::new();
							#(#entries)*
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
		let generated_token = match &self.generated {
			Some(generated) => quote! { Some(#generated) },
			None => quote! { None },
		};
		let domain_token = match &self.domain {
			Some(domain) => quote! { Some(#domain) },
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
			FieldType::Jsonb => quote! { FieldType::Jsonb },

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
			#[cfg(feature = "pgvector")]
			FieldType::Vector { dimensions } => {
				quote! { FieldType::Vector { dimensions: #dimensions } }
			}

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
				generated: #generated_token,
				domain: #domain_token,
			}
		});
	}
}

impl ToTokens for GeneratedColumnDefinition {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let storage_token = match self.storage {
			GeneratedStorage::Stored => quote! { GeneratedStorage::Stored },
			GeneratedStorage::Virtual => quote! { GeneratedStorage::Virtual },
			_ => panic!("unsupported generated-column storage: {:?}", self.storage),
		};

		let canonical_expr = self.typed_expr();
		let canonical_expr_tokens = canonical_expr.as_ref().map(schema_expr_to_tokens);
		let expr_token = match &canonical_expr_tokens {
			Some(expr_stream) => quote! { Some(Box::new(#expr_stream)) },
			None => quote! { None },
		};
		let expr_tokens_token = match &canonical_expr_tokens {
			Some(expr_stream) => {
				let expr_tokens = expr_stream.to_string();
				quote! { Some(#expr_tokens.to_string()) }
			}
			None => quote! { None },
		};
		let raw_sql_token = match &self.raw_sql {
			Some(raw_sql) => quote! { Some(#raw_sql.to_string()) },
			None => quote! { None },
		};

		tokens.extend(quote! {
			GeneratedColumnDefinition {
				expr: #expr_token,
				expr_tokens: #expr_tokens_token,
				raw_sql: #raw_sql_token,
				storage: #storage_token,
			}
		});
	}
}

fn schema_expr_to_tokens(expr: &SchemaExpr) -> TokenStream {
	match expr {
		SchemaExpr::Column(iden) => {
			let name = iden.to_string();
			quote! { SchemaExpr::col(#name) }
		}
		SchemaExpr::Value(value) => {
			let value = schema_value_to_tokens(value);
			quote! { SchemaExpr::val(#value) }
		}
		SchemaExpr::Binary { left, op, right } => {
			let left = schema_expr_to_tokens(left);
			let op = schema_bin_oper_to_tokens(*op);
			let right = schema_expr_to_tokens(right);
			quote! { #left.binary(#op, #right) }
		}
		SchemaExpr::Function { func, args } => {
			let args = args.iter().map(schema_expr_to_tokens);
			match func {
				SchemaFunc::Concat => quote! { SchemaExpr::concat([#(#args),*]) },
				SchemaFunc::Coalesce => quote! { SchemaExpr::coalesce([#(#args),*]) },
				_ => panic!("unsupported generated-column schema function: {:?}", func),
			}
		}
		SchemaExpr::Cast { expr, ty } => {
			let expr = schema_expr_to_tokens(expr);
			let ty = query_column_type_to_tokens(ty);
			quote! { #expr.cast(#ty) }
		}
		_ => panic!("unsupported generated-column schema expression: {:?}", expr),
	}
}

fn schema_bin_oper_to_tokens(op: SchemaBinOper) -> TokenStream {
	match op {
		SchemaBinOper::Add => quote! { SchemaBinOper::Add },
		SchemaBinOper::Sub => quote! { SchemaBinOper::Sub },
		SchemaBinOper::Mul => quote! { SchemaBinOper::Mul },
		SchemaBinOper::Div => quote! { SchemaBinOper::Div },
		_ => panic!("unsupported generated-column binary operator: {:?}", op),
	}
}

fn schema_value_to_tokens(value: &Value) -> TokenStream {
	match value {
		Value::Bool(Some(value)) => quote! { #value },
		Value::Bool(None) => quote! { Option::<bool>::None },
		Value::TinyInt(Some(value)) => quote! { #value },
		Value::TinyInt(None) => quote! { Option::<i8>::None },
		Value::SmallInt(Some(value)) => quote! { #value },
		Value::SmallInt(None) => quote! { Option::<i16>::None },
		Value::Int(Some(value)) => quote! { #value },
		Value::Int(None) => quote! { Option::<i32>::None },
		Value::BigInt(Some(value)) => quote! { #value },
		Value::BigInt(None) => quote! { Option::<i64>::None },
		Value::TinyUnsigned(Some(value)) => quote! { #value },
		Value::TinyUnsigned(None) => quote! { Option::<u8>::None },
		Value::SmallUnsigned(Some(value)) => quote! { #value },
		Value::SmallUnsigned(None) => quote! { Option::<u16>::None },
		Value::Unsigned(Some(value)) => quote! { #value },
		Value::Unsigned(None) => quote! { Option::<u32>::None },
		Value::BigUnsigned(Some(value)) => quote! { #value },
		Value::BigUnsigned(None) => quote! { Option::<u64>::None },
		Value::Float(Some(value)) => quote! { #value },
		Value::Float(None) => quote! { Option::<f32>::None },
		Value::Double(Some(value)) => quote! { #value },
		Value::Double(None) => quote! { Option::<f64>::None },
		Value::Char(Some(value)) => quote! { #value },
		Value::Char(None) => quote! { Option::<char>::None },
		Value::String(Some(value)) => {
			let value = value.as_str();
			quote! { #value }
		}
		Value::String(None) => quote! { Option::<String>::None },
		_ => panic!("unsupported generated-column literal value: {:?}", value),
	}
}

fn query_column_type_to_tokens(ty: &QueryColumnType) -> TokenStream {
	match ty {
		QueryColumnType::Char(len) => {
			let len = optional_u32_to_tokens(*len);
			quote! { ColumnType::Char(#len) }
		}
		QueryColumnType::String(len) => {
			let len = optional_u32_to_tokens(*len);
			quote! { ColumnType::String(#len) }
		}
		QueryColumnType::Text => quote! { ColumnType::Text },
		QueryColumnType::TinyInteger => quote! { ColumnType::TinyInteger },
		QueryColumnType::SmallInteger => quote! { ColumnType::SmallInteger },
		QueryColumnType::Integer => quote! { ColumnType::Integer },
		QueryColumnType::BigInteger => quote! { ColumnType::BigInteger },
		QueryColumnType::Float => quote! { ColumnType::Float },
		QueryColumnType::Double => quote! { ColumnType::Double },
		QueryColumnType::Decimal(Some((precision, scale))) => {
			quote! { ColumnType::Decimal(Some((#precision, #scale))) }
		}
		QueryColumnType::Decimal(None) => quote! { ColumnType::Decimal(None) },
		QueryColumnType::Boolean => quote! { ColumnType::Boolean },
		QueryColumnType::Date => quote! { ColumnType::Date },
		QueryColumnType::Time => quote! { ColumnType::Time },
		QueryColumnType::DateTime => quote! { ColumnType::DateTime },
		QueryColumnType::Timestamp => quote! { ColumnType::Timestamp },
		QueryColumnType::TimestampWithTimeZone => quote! { ColumnType::TimestampWithTimeZone },
		QueryColumnType::Binary(len) => {
			let len = optional_u32_to_tokens(*len);
			quote! { ColumnType::Binary(#len) }
		}
		QueryColumnType::VarBinary(len) => quote! { ColumnType::VarBinary(#len) },
		QueryColumnType::Blob => quote! { ColumnType::Blob },
		QueryColumnType::Uuid => quote! { ColumnType::Uuid },
		QueryColumnType::Json => quote! { ColumnType::Json },
		QueryColumnType::Jsonb => quote! { ColumnType::Jsonb },
		QueryColumnType::Array(inner) => {
			let inner = query_column_type_to_tokens(inner);
			quote! { ColumnType::Array(Box::new(#inner)) }
		}
		#[cfg(feature = "pgvector")]
		QueryColumnType::Vector(dimensions) => quote! { ColumnType::Vector(#dimensions) },
		QueryColumnType::Custom(name) => quote! { ColumnType::Custom(#name.to_string()) },
		_ => panic!("unsupported generated-column cast type: {:?}", ty),
	}
}

fn optional_u32_to_tokens(value: Option<u32>) -> TokenStream {
	match value {
		Some(value) => quote! { Some(#value) },
		None => quote! { None },
	}
}

#[cfg(feature = "pgvector")]
fn optional_u16_to_tokens(value: Option<u16>) -> TokenStream {
	match value {
		Some(value) => quote! { Some(#value) },
		None => quote! { None },
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
	use quote::ToTokens;

	#[test]
	fn drop_constraint_definition_tokens_preserve_typed_constraint() {
		let operation = Operation::DropConstraintDefinition {
			table: "jobs".to_string(),
			constraint: Constraint::EnumDomain {
				name: "jobs_status_check".to_string(),
				column: "status".to_string(),
				domain: crate::field_domain::FieldDomain::Enum {
					repr: crate::field_domain::ModelEnumRepr::String,
					values: vec![crate::field_domain::ModelEnumValue::String(
						"queued".to_string(),
					)],
				},
			},
		};

		let tokens = operation.to_token_stream().to_string();

		assert!(tokens.contains("DropConstraintDefinition"), "{tokens}");
		assert!(tokens.contains("Constraint :: EnumDomain"), "{tokens}");
		assert!(tokens.contains("ModelEnumValue :: String"), "{tokens}");
	}

	#[test]
	fn column_definition_tokens_preserve_model_enum_domain() {
		let column = ColumnDefinition::new("job_status", FieldType::VarChar(32)).with_domain(
			crate::field_domain::FieldDomain::Enum {
				repr: crate::field_domain::ModelEnumRepr::String,
				values: vec![
					crate::field_domain::ModelEnumValue::String("queued".to_string()),
					crate::field_domain::ModelEnumValue::String("running".to_string()),
				],
			},
		);

		let tokens = column.to_token_stream().to_string();

		assert!(tokens.contains("domain : Some (FieldDomain :: Enum"));
		assert!(tokens.contains("ModelEnumRepr :: String"));
		assert!(tokens.contains("ModelEnumValue :: String (\"queued\" . to_string ())"));
	}

	#[test]
	fn field_domain_tokens_preserve_i32_min() {
		let domain = crate::field_domain::FieldDomain::Enum {
			repr: crate::field_domain::ModelEnumRepr::I32,
			values: vec![crate::field_domain::ModelEnumValue::I32(i32::MIN)],
		};

		let tokens = domain.to_token_stream().to_string();

		assert!(
			tokens.contains("ModelEnumValue :: I32 (- 2147483648i32)"),
			"tokens must preserve i32::MIN: {tokens}"
		);
	}

	#[test]
	fn generated_schema_expr_tokens_emit_option_cast_lengths() {
		let expr = SchemaExpr::col("name").cast(QueryColumnType::String(Some(64)));
		let tokens = schema_expr_to_tokens(&expr).to_string();

		assert!(
			tokens.contains("ColumnType :: String (Some (64u32))"),
			"tokens must preserve optional cast length: {tokens}"
		);
		assert_eq!(
			crate::migrations::ast_parser::parse_schema_expr_tokens(&tokens),
			Some(expr)
		);
	}

	#[test]
	fn generated_schema_expr_tokens_reparse_null_literals() {
		let expressions = [
			SchemaExpr::Value(Value::Bool(None)),
			SchemaExpr::Value(Value::Int(None)),
			SchemaExpr::Value(Value::Unsigned(None)),
			SchemaExpr::Value(Value::Double(None)),
			SchemaExpr::Value(Value::String(None)),
		];

		for expr in expressions {
			let tokens = schema_expr_to_tokens(&expr).to_string();
			assert_eq!(
				crate::migrations::ast_parser::parse_schema_expr_tokens(&tokens),
				Some(expr),
				"tokens must reparse: {tokens}"
			);
		}
	}

	#[test]
	fn generated_schema_expr_tokens_reparse_suffixed_literals() {
		let expressions = [
			SchemaExpr::Value(Value::TinyInt(Some(1))),
			SchemaExpr::Value(Value::SmallInt(Some(1))),
			SchemaExpr::Value(Value::Unsigned(Some(1))),
			SchemaExpr::Value(Value::BigUnsigned(Some(1))),
			SchemaExpr::Value(Value::Float(Some(1.5))),
			SchemaExpr::Value(Value::Double(Some(1.5))),
		];

		for expr in expressions {
			let tokens = schema_expr_to_tokens(&expr).to_string();
			assert_eq!(
				crate::migrations::ast_parser::parse_schema_expr_tokens(&tokens),
				Some(expr),
				"tokens must preserve suffixed literal types: {tokens}"
			);
		}
	}

	#[test]
	fn alter_model_options_tokens_are_order_independent() {
		let mut first_options = std::collections::HashMap::new();
		first_options.insert("managed".to_string(), "true".to_string());
		first_options.insert("verbose_name".to_string(), "Job".to_string());
		let mut second_options = std::collections::HashMap::new();
		second_options.insert("verbose_name".to_string(), "Job".to_string());
		second_options.insert("managed".to_string(), "true".to_string());

		let first = Operation::AlterModelOptions {
			table: "jobs".to_string(),
			options: first_options,
		};
		let second = Operation::AlterModelOptions {
			table: "jobs".to_string(),
			options: second_options,
		};

		assert_eq!(
			first.to_token_stream().to_string(),
			second.to_token_stream().to_string()
		);
	}
}
