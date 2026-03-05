/// Database indexes similar to Django's indexes
use serde::{Deserialize, Serialize};

/// Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
	pub name: String,
	pub fields: Vec<String>,
	pub unique: bool,
	pub condition: Option<String>, // Partial index
	pub include: Vec<String>,      // Covering index
	pub opclass: Option<String>,   // Operator class
}

impl Index {
	/// Create a new database index on specified fields
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::indexes::Index;
	///
	/// let index = Index::new("user_email_idx", vec!["email".to_string()]);
	/// assert_eq!(index.name, "user_email_idx");
	/// assert_eq!(index.fields.len(), 1);
	/// assert!(!index.unique); // Not unique by default
	/// ```
	pub fn new(name: impl Into<String>, fields: Vec<String>) -> Self {
		Self {
			name: name.into(),
			fields,
			unique: false,
			condition: None,
			include: Vec::new(),
			opclass: None,
		}
	}
	/// Documentation for `unique`
	///
	pub fn unique(mut self) -> Self {
		self.unique = true;
		self
	}
	/// Documentation for `with_condition`
	pub fn with_condition(mut self, condition: String) -> Self {
		self.condition = Some(condition);
		self
	}
	/// Documentation for `include`
	///
	pub fn include(mut self, fields: Vec<String>) -> Self {
		self.include = fields;
		self
	}
	/// Documentation for `with_opclass`
	pub fn with_opclass(mut self, opclass: String) -> Self {
		self.opclass = Some(opclass);
		self
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self, table: &str) -> String {
		let unique = if self.unique { "UNIQUE " } else { "" };
		let fields = self.fields.join(", ");

		let mut sql = format!(
			"CREATE {}INDEX {} ON {} ({})",
			unique, self.name, table, fields
		);

		if let Some(ref opclass) = self.opclass {
			sql = format!(
				"CREATE {}INDEX {} ON {} ({} {})",
				unique, self.name, table, fields, opclass
			);
		}

		if !self.include.is_empty() {
			sql.push_str(&format!(" INCLUDE ({})", self.include.join(", ")));
		}

		if let Some(ref cond) = self.condition {
			sql.push_str(&format!(" WHERE {}", cond));
		}

		sql
	}
}

/// B-Tree index (default)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BTreeIndex {
	pub index: Index,
}

impl BTreeIndex {
	/// Create a new B-Tree index (default index type)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::indexes::BTreeIndex;
	///
	/// let btree = BTreeIndex::new("user_name_idx", vec!["name".to_string()]);
	/// assert_eq!(btree.index.name, "user_name_idx");
	/// ```
	pub fn new(name: impl Into<String>, fields: Vec<String>) -> Self {
		Self {
			index: Index::new(name, fields),
		}
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self, table: &str) -> String {
		self.index.to_sql(table)
	}
}

/// Hash index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashIndex {
	pub index: Index,
}

impl HashIndex {
	/// Create a new Hash index for equality comparisons
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::indexes::HashIndex;
	///
	/// let hash = HashIndex::new("user_email_hash", vec!["email".to_string()]);
	/// assert_eq!(hash.index.name, "user_email_hash");
	/// // Hash indexes are fast for equality but don't support range queries
	/// ```
	pub fn new(name: impl Into<String>, fields: Vec<String>) -> Self {
		let mut index = Index::new(name, fields);
		index.opclass = Some("USING HASH".to_string());
		Self { index }
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self, table: &str) -> String {
		format!(
			"CREATE INDEX {} ON {} USING HASH ({})",
			self.index.name,
			table,
			self.index.fields.join(", ")
		)
	}
}

/// GIN index (for arrays, JSONB, full-text search)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GinIndex {
	pub index: Index,
}

impl GinIndex {
	/// Create a new GIN index for arrays, JSONB, and full-text search
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::indexes::GinIndex;
	///
	/// let gin = GinIndex::new("post_tags_gin", vec!["tags".to_string()]);
	/// assert_eq!(gin.index.name, "post_tags_gin");
	/// // GIN indexes are ideal for array and JSONB column searches
	/// ```
	pub fn new(name: impl Into<String>, fields: Vec<String>) -> Self {
		let mut index = Index::new(name, fields);
		index.opclass = Some("USING GIN".to_string());
		Self { index }
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self, table: &str) -> String {
		format!(
			"CREATE INDEX {} ON {} USING GIN ({})",
			self.index.name,
			table,
			self.index.fields.join(", ")
		)
	}
}

/// GiST index (for geometric data, full-text search)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GistIndex {
	pub index: Index,
}

impl GistIndex {
	/// Create a new GiST index for geometric data and full-text search
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::indexes::GistIndex;
	///
	/// let gist = GistIndex::new("location_gist", vec!["coordinates".to_string()]);
	/// assert_eq!(gist.index.name, "location_gist");
	/// // GiST indexes support geometric and spatial queries
	/// ```
	pub fn new(name: impl Into<String>, fields: Vec<String>) -> Self {
		let mut index = Index::new(name, fields);
		index.opclass = Some("USING GIST".to_string());
		Self { index }
	}
	/// Documentation for `to_sql`
	///
	pub fn to_sql(&self, table: &str) -> String {
		format!(
			"CREATE INDEX {} ON {} USING GIST ({})",
			self.index.name,
			table,
			self.index.fields.join(", ")
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_basic_index() {
		let index = Index::new("idx_email", vec!["email".to_string()]);
		let sql = index.to_sql("users");
		assert_eq!(sql, "CREATE INDEX idx_email ON users (email)");
	}

	#[test]
	fn test_unique_index() {
		let index = Index::new("idx_unique_email", vec!["email".to_string()]).unique();
		let sql = index.to_sql("users");
		assert_eq!(sql, "CREATE UNIQUE INDEX idx_unique_email ON users (email)");
	}

	#[test]
	fn test_orm_indexes_composite() {
		let index = Index::new(
			"idx_user_email",
			vec!["user_id".to_string(), "email".to_string()],
		);
		let sql = index.to_sql("messages");
		assert_eq!(
			sql,
			"CREATE INDEX idx_user_email ON messages (user_id, email)"
		);
	}

	#[test]
	fn test_partial_index() {
		let index = Index::new("idx_active_users", vec!["email".to_string()])
			.with_condition("deleted_at IS NULL".to_string());
		let sql = index.to_sql("users");
		assert_eq!(
			sql,
			"CREATE INDEX idx_active_users ON users (email) WHERE deleted_at IS NULL"
		);
	}

	#[test]
	fn test_orm_indexes_covering() {
		let index = Index::new("idx_email_covering", vec!["email".to_string()])
			.include(vec!["name".to_string(), "created_at".to_string()]);
		let sql = index.to_sql("users");
		assert_eq!(
			sql,
			"CREATE INDEX idx_email_covering ON users (email) INCLUDE (name, created_at)"
		);
	}

	#[test]
	fn test_btree_index() {
		let index = BTreeIndex::new("idx_btree", vec!["id".to_string()]);
		let sql = index.to_sql("users");
		assert_eq!(sql, "CREATE INDEX idx_btree ON users (id)");
	}

	#[test]
	fn test_hash_index() {
		let index = HashIndex::new("idx_hash", vec!["email".to_string()]);
		let sql = index.to_sql("users");
		assert_eq!(sql, "CREATE INDEX idx_hash ON users USING HASH (email)");
	}

	#[test]
	fn test_gin_index() {
		let index = GinIndex::new("idx_gin", vec!["tags".to_string()]);
		let sql = index.to_sql("posts");
		assert_eq!(sql, "CREATE INDEX idx_gin ON posts USING GIN (tags)");
	}

	#[test]
	fn test_gist_index() {
		let index = GistIndex::new("idx_gist", vec!["location".to_string()]);
		let sql = index.to_sql("places");
		assert_eq!(sql, "CREATE INDEX idx_gist ON places USING GIST (location)");
	}
}
