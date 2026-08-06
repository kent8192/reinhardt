#![cfg(all(feature = "orm", feature = "sqlite"))]

use reinhardt_db::orm::connection::DatabaseConnection;
use reinhardt_db::orm::inspection::RelationInfo;
use reinhardt_db::orm::model::FieldSelector;
use reinhardt_db::orm::relationship::RelationshipType;
use reinhardt_db::orm::{Manager, ManyToManyAccessor, Model};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct User {
	id: Option<i64>,
	name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Group {
	id: Option<i64>,
	name: String,
}

#[derive(Clone, Debug)]
struct EmptyFields;

impl FieldSelector for EmptyFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for User {
	type PrimaryKey = i64;
	type Fields = EmptyFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"users"
	}

	fn new_fields() -> Self::Fields {
		EmptyFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}

	fn relationship_metadata() -> Vec<RelationInfo> {
		vec![
			RelationInfo::new("groups", RelationshipType::ManyToMany, "Group")
				.with_through_table("users_groups")
				.with_source_field("users_id")
				.with_target_field("groups_id"),
		]
	}
}

impl Model for Group {
	type PrimaryKey = i64;
	type Fields = EmptyFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"groups"
	}

	fn new_fields() -> Self::Fields {
		EmptyFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

async fn sqlite_fixture() -> (DatabaseConnection, User, [Group; 3]) {
	let db = DatabaseConnection::connect("sqlite::memory:")
		.await
		.expect("in-memory SQLite connection should be available");
	for statement in [
		"CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
		"CREATE TABLE groups (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
		"CREATE TABLE users_groups (users_id INTEGER NOT NULL, groups_id INTEGER NOT NULL, PRIMARY KEY (users_id, groups_id))",
		"INSERT INTO users (id, name) VALUES (1, 'Ada')",
		"INSERT INTO groups (id, name) VALUES (1, 'Readers'), (2, 'Writers'), (3, 'Editors')",
	] {
		db.execute(statement, Vec::new())
			.await
			.expect("SQLite fixture statement should succeed");
	}

	(
		db,
		User {
			id: Some(1),
			name: "Ada".to_owned(),
		},
		[
			Group {
				id: Some(1),
				name: "Readers".to_owned(),
			},
			Group {
				id: Some(2),
				name: "Writers".to_owned(),
			},
			Group {
				id: Some(3),
				name: "Editors".to_owned(),
			},
		],
	)
}

fn sorted_group_ids(groups: &[Group]) -> Vec<i64> {
	let mut ids = groups
		.iter()
		.map(|group| group.id.expect("fixture groups should have primary keys"))
		.collect::<Vec<_>>();
	ids.sort_unstable();
	ids
}

#[tokio::test]
async fn lifecycle_binds_values_and_preserves_exact_relationship_state() {
	let (db, user, [readers, writers, editors]) = sqlite_fixture().await;
	let accessor = ManyToManyAccessor::<User, Group>::new(&user, "groups", db.clone());

	accessor
		.add(&readers)
		.await
		.expect("first relationship should be added");
	assert_eq!(accessor.count().await.unwrap(), 1);
	assert_eq!(sorted_group_ids(&accessor.all().await.unwrap()), vec![1]);

	accessor
		.add(&writers)
		.await
		.expect("second relationship should be added");
	assert_eq!(accessor.count().await.unwrap(), 2);
	assert_eq!(sorted_group_ids(&accessor.all().await.unwrap()), vec![1, 2]);

	accessor
		.remove(&readers)
		.await
		.expect("selected relationship should be removed");
	assert_eq!(accessor.count().await.unwrap(), 1);
	assert_eq!(sorted_group_ids(&accessor.all().await.unwrap()), vec![2]);

	accessor
		.set(&[readers.clone(), writers.clone(), editors.clone()])
		.await
		.expect("relationship set should be replaced atomically");
	assert_eq!(accessor.count().await.unwrap(), 3);
	assert_eq!(
		sorted_group_ids(&accessor.all().await.unwrap()),
		vec![1, 2, 3]
	);

	let second_page =
		ManyToManyAccessor::<User, Group>::new(&user, "groups", db.clone()).paginate(2, 10);
	assert_eq!(
		sorted_group_ids(&second_page.all().await.unwrap()),
		Vec::<i64>::new()
	);

	accessor
		.clear()
		.await
		.expect("all relationships should be cleared");
	assert_eq!(accessor.count().await.unwrap(), 0);
	assert_eq!(
		sorted_group_ids(&accessor.all().await.unwrap()),
		Vec::<i64>::new()
	);
}

#[tokio::test]
async fn add_rejects_a_target_without_a_primary_key() {
	let (db, user, _) = sqlite_fixture().await;
	let accessor = ManyToManyAccessor::<User, Group>::new(&user, "groups", db);
	let unpersisted = Group {
		id: None,
		name: "unpersisted".to_string(),
	};

	let error = accessor.add(&unpersisted).await.unwrap_err();

	assert_eq!(error, "Target model has no primary key");
}

#[tokio::test]
async fn construction_panics_with_the_exact_message_for_a_missing_source_key() {
	let (db, _, _) = sqlite_fixture().await;
	let unpersisted = User {
		id: None,
		name: "unpersisted".to_string(),
	};

	let panic = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		ManyToManyAccessor::<User, Group>::new(&unpersisted, "groups", db)
	})) {
		Ok(_) => panic!("an unpersisted source should panic"),
		Err(panic) => panic,
	};
	let message = if let Some(message) = panic.downcast_ref::<&str>() {
		(*message).to_owned()
	} else if let Some(message) = panic.downcast_ref::<String>() {
		message.clone()
	} else {
		panic!("unexpected panic payload type");
	};

	assert_eq!(message, "Source model must have primary key");
}

#[tokio::test]
async fn set_rolls_back_when_a_later_target_has_no_primary_key() {
	let (db, user, [readers, writers, editors]) = sqlite_fixture().await;
	let accessor = ManyToManyAccessor::<User, Group>::new(&user, "groups", db);
	accessor.add(&readers).await.unwrap();
	accessor.add(&writers).await.unwrap();
	let unpersisted = Group {
		id: None,
		name: "unpersisted".to_string(),
	};

	let error = accessor.set(&[editors, unpersisted]).await.unwrap_err();

	assert_eq!(error, "Target model has no primary key");
	assert_eq!(sorted_group_ids(&accessor.all().await.unwrap()), vec![1, 2]);
}

#[tokio::test]
async fn filter_by_target_returns_the_exact_related_source() {
	let (db, user, [_, writers, _]) = sqlite_fixture().await;
	let accessor = ManyToManyAccessor::<User, Group>::new(&user, "groups", db.clone());
	accessor.add(&writers).await.unwrap();

	let related_users = ManyToManyAccessor::<User, Group>::filter_by_target(
		&User::objects(),
		"groups",
		&writers,
		db,
	)
	.await
	.unwrap();

	assert_eq!(related_users, vec![user]);
}
