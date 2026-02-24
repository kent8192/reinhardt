//! DM factory for examples-twitter tests.
//!
//! Provides factory functions for creating DMRoom and DMMessage records.

use chrono::Utc;
use reinhardt_query::prelude::{
	Alias, Expr, ExprTrait, IntoValue, Order, PostgresQueryBuilder, Query, QueryStatementBuilder,
	Value,
};
use rstest::*;
use sqlx::PgPool;
use uuid::Uuid;

use crate::apps::dm::models::{DMMessage, DMRoom};

/// Factory for creating DMRoom records in the database.
pub struct DMRoomFactory;

impl Default for DMRoomFactory {
	fn default() -> Self {
		Self::new()
	}
}

impl DMRoomFactory {
	/// Create a new DMRoomFactory.
	pub fn new() -> Self {
		Self
	}

	/// Create a 1-on-1 DM room between two users.
	pub async fn create_direct(
		&self,
		pool: &PgPool,
		user1_id: Uuid,
		user2_id: Uuid,
	) -> Result<DMRoom, sqlx::Error> {
		let id = Uuid::new_v4();
		let now = Utc::now();

		// Insert room
		let sql = Query::insert()
			.into_table(Alias::new("dm_room"))
			.columns([
				Alias::new("id"),
				Alias::new("name"),
				Alias::new("is_group"),
				Alias::new("created_at"),
				Alias::new("updated_at"),
			])
			.values_panic([
				Value::from(id),
				Option::<String>::None.into_value(),
				Value::from(false),
				Value::from(now),
				Value::from(now),
			])
			.to_string(PostgresQueryBuilder);

		sqlx::query(&sql).execute(pool).await?;

		// Add members to the room
		self.add_member(pool, id, user1_id).await?;
		self.add_member(pool, id, user2_id).await?;

		self.find_by_id(pool, id).await
	}

	/// Create a group DM room with multiple users.
	pub async fn create_group(
		&self,
		pool: &PgPool,
		name: &str,
		member_ids: &[Uuid],
	) -> Result<DMRoom, sqlx::Error> {
		let id = Uuid::new_v4();
		let now = Utc::now();

		// Insert room
		let sql = Query::insert()
			.into_table(Alias::new("dm_room"))
			.columns([
				Alias::new("id"),
				Alias::new("name"),
				Alias::new("is_group"),
				Alias::new("created_at"),
				Alias::new("updated_at"),
			])
			.values_panic([
				Value::from(id),
				Some(name.to_string()).into_value(),
				Value::from(true),
				Value::from(now),
				Value::from(now),
			])
			.to_string(PostgresQueryBuilder);

		sqlx::query(&sql).execute(pool).await?;

		// Add members to the room
		for member_id in member_ids {
			self.add_member(pool, id, *member_id).await?;
		}

		self.find_by_id(pool, id).await
	}

	/// Add a member to a room.
	pub async fn add_member(
		&self,
		pool: &PgPool,
		room_id: Uuid,
		user_id: Uuid,
	) -> Result<(), sqlx::Error> {
		let sql = Query::insert()
			.into_table(Alias::new("dm_room_members"))
			.columns([Alias::new("dmroom_id"), Alias::new("user_id")])
			.values_panic([Value::from(room_id), Value::from(user_id)])
			.to_string(PostgresQueryBuilder);

		sqlx::query(&sql).execute(pool).await?;
		Ok(())
	}

	/// Find a room by ID.
	pub async fn find_by_id(&self, pool: &PgPool, id: Uuid) -> Result<DMRoom, sqlx::Error> {
		let sql = Query::select()
			.columns([
				Alias::new("id"),
				Alias::new("name"),
				Alias::new("is_group"),
				Alias::new("created_at"),
				Alias::new("updated_at"),
			])
			.from(Alias::new("dm_room"))
			.and_where(Expr::col(Alias::new("id")).eq(Expr::val(id)))
			.to_string(PostgresQueryBuilder);

		sqlx::query_as::<_, DMRoom>(&sql).fetch_one(pool).await
	}

	/// Find rooms for a user.
	pub async fn find_by_member(
		&self,
		pool: &PgPool,
		user_id: Uuid,
	) -> Result<Vec<DMRoom>, sqlx::Error> {
		let sql = "SELECT r.id, r.name, r.is_group, r.created_at, r.updated_at \
		           FROM dm_room r \
		           INNER JOIN dm_room_members m ON r.id = m.dmroom_id \
		           WHERE m.user_id = $1 \
		           ORDER BY r.updated_at DESC";

		sqlx::query_as::<_, DMRoom>(sql)
			.bind(user_id)
			.fetch_all(pool)
			.await
	}

	/// Count rooms for a user.
	pub async fn count_by_member(&self, pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
		sqlx::query_scalar("SELECT COUNT(*) FROM dm_room_members WHERE user_id = $1")
			.bind(user_id)
			.fetch_one(pool)
			.await
	}

	/// Delete a room by ID (also deletes messages and members).
	pub async fn delete(&self, pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
		// Delete messages first (FK constraint)
		let sql = Query::delete()
			.from_table(Alias::new("dm_message"))
			.and_where(Expr::col(Alias::new("room_id")).eq(Expr::val(id)))
			.to_string(PostgresQueryBuilder);
		sqlx::query(&sql).execute(pool).await?;

		// Delete members
		let sql = Query::delete()
			.from_table(Alias::new("dm_room_members"))
			.and_where(Expr::col(Alias::new("dmroom_id")).eq(Expr::val(id)))
			.to_string(PostgresQueryBuilder);
		sqlx::query(&sql).execute(pool).await?;

		// Delete room
		let sql = Query::delete()
			.from_table(Alias::new("dm_room"))
			.and_where(Expr::col(Alias::new("id")).eq(Expr::val(id)))
			.to_string(PostgresQueryBuilder);
		sqlx::query(&sql).execute(pool).await?;

		Ok(())
	}
}

/// Factory for creating DMMessage records in the database.
pub struct DMMessageFactory;

impl Default for DMMessageFactory {
	fn default() -> Self {
		Self::new()
	}
}

impl DMMessageFactory {
	/// Create a new DMMessageFactory.
	pub fn new() -> Self {
		Self
	}

	/// Create a message in a room.
	pub async fn create(
		&self,
		pool: &PgPool,
		room_id: Uuid,
		sender_id: Uuid,
		content: &str,
	) -> Result<DMMessage, sqlx::Error> {
		let id = Uuid::new_v4();
		let now = Utc::now();

		let sql = Query::insert()
			.into_table(Alias::new("dm_message"))
			.columns([
				Alias::new("id"),
				Alias::new("room_id"),
				Alias::new("sender_id"),
				Alias::new("content"),
				Alias::new("is_read"),
				Alias::new("created_at"),
				Alias::new("updated_at"),
			])
			.values_panic([
				Value::from(id),
				Value::from(room_id),
				Value::from(sender_id),
				Value::from(content),
				Value::from(false),
				Value::from(now),
				Value::from(now),
			])
			.to_string(PostgresQueryBuilder);

		sqlx::query(&sql).execute(pool).await?;

		self.find_by_id(pool, id).await
	}

	/// Create multiple messages in a room.
	pub async fn create_many(
		&self,
		pool: &PgPool,
		room_id: Uuid,
		sender_id: Uuid,
		contents: &[&str],
	) -> Result<Vec<DMMessage>, sqlx::Error> {
		let mut messages = Vec::with_capacity(contents.len());
		for content in contents {
			let message = self.create(pool, room_id, sender_id, content).await?;
			messages.push(message);
		}
		Ok(messages)
	}

	/// Find a message by ID.
	pub async fn find_by_id(&self, pool: &PgPool, id: Uuid) -> Result<DMMessage, sqlx::Error> {
		let sql = Query::select()
			.columns([
				Alias::new("id"),
				Alias::new("room_id"),
				Alias::new("sender_id"),
				Alias::new("content"),
				Alias::new("is_read"),
				Alias::new("created_at"),
				Alias::new("updated_at"),
			])
			.from(Alias::new("dm_message"))
			.and_where(Expr::col(Alias::new("id")).eq(Expr::val(id)))
			.to_string(PostgresQueryBuilder);

		sqlx::query_as::<_, DMMessage>(&sql).fetch_one(pool).await
	}

	/// Find messages in a room.
	pub async fn find_by_room(
		&self,
		pool: &PgPool,
		room_id: Uuid,
		limit: Option<i32>,
	) -> Result<Vec<DMMessage>, sqlx::Error> {
		let mut query = Query::select()
			.columns([
				Alias::new("id"),
				Alias::new("room_id"),
				Alias::new("sender_id"),
				Alias::new("content"),
				Alias::new("is_read"),
				Alias::new("created_at"),
				Alias::new("updated_at"),
			])
			.from(Alias::new("dm_message"))
			.and_where(Expr::col(Alias::new("room_id")).eq(Expr::val(room_id)))
			.order_by(Alias::new("created_at"), Order::Desc)
			.to_owned();

		if let Some(l) = limit {
			query.limit(l as u64);
		}

		let sql = query.to_string(PostgresQueryBuilder);
		sqlx::query_as::<_, DMMessage>(&sql).fetch_all(pool).await
	}

	/// Count messages in a room.
	pub async fn count_by_room(&self, pool: &PgPool, room_id: Uuid) -> Result<i64, sqlx::Error> {
		sqlx::query_scalar("SELECT COUNT(*) FROM dm_message WHERE room_id = $1")
			.bind(room_id)
			.fetch_one(pool)
			.await
	}

	/// Mark a message as read.
	pub async fn mark_as_read(&self, pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
		let sql = Query::update()
			.table(Alias::new("dm_message"))
			.value(Alias::new("is_read"), true)
			.and_where(Expr::col(Alias::new("id")).eq(Expr::val(id)))
			.to_string(PostgresQueryBuilder);

		sqlx::query(&sql).execute(pool).await?;
		Ok(())
	}

	/// Mark all messages in a room as read for a user.
	pub async fn mark_room_as_read(
		&self,
		pool: &PgPool,
		room_id: Uuid,
		user_id: Uuid,
	) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE dm_message SET is_read = true WHERE room_id = $1 AND sender_id != $2")
			.bind(room_id)
			.bind(user_id)
			.execute(pool)
			.await?;
		Ok(())
	}

	/// Delete a message by ID.
	pub async fn delete(&self, pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
		let sql = Query::delete()
			.from_table(Alias::new("dm_message"))
			.and_where(Expr::col(Alias::new("id")).eq(Expr::val(id)))
			.to_string(PostgresQueryBuilder);

		sqlx::query(&sql).execute(pool).await?;
		Ok(())
	}
}

/// rstest fixture providing a DMRoomFactory.
#[fixture]
pub fn dm_room_factory() -> DMRoomFactory {
	DMRoomFactory::new()
}

/// rstest fixture providing a DMMessageFactory.
#[fixture]
pub fn dm_message_factory() -> DMMessageFactory {
	DMMessageFactory::new()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_utils::factories::user::UserFactory;
	use crate::test_utils::fixtures::database::twitter_db_pool;
	use crate::test_utils::fixtures::users::TestTwitterUser;

	#[rstest]
	#[tokio::test]
	async fn test_dm_room_factory_create_direct(#[future] twitter_db_pool: (PgPool, String)) {
		let (pool, _url) = twitter_db_pool.await;
		let user_factory = UserFactory::new();
		let room_factory = DMRoomFactory::new();

		let user1 = user_factory
			.create_from_test_user(&pool, &TestTwitterUser::new("dmuser1"))
			.await
			.expect("User1 creation should succeed");
		let user2 = user_factory
			.create_from_test_user(&pool, &TestTwitterUser::new("dmuser2"))
			.await
			.expect("User2 creation should succeed");

		let room = room_factory
			.create_direct(&pool, user1.id(), user2.id())
			.await
			.expect("Room creation should succeed");

		assert!(!room.is_group());
		assert!(room.name().is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_dm_room_factory_create_group(#[future] twitter_db_pool: (PgPool, String)) {
		let (pool, _url) = twitter_db_pool.await;
		let user_factory = UserFactory::new();
		let room_factory = DMRoomFactory::new();

		let user1 = user_factory
			.create_from_test_user(&pool, &TestTwitterUser::new("groupuser1"))
			.await
			.expect("User1 creation should succeed");
		let user2 = user_factory
			.create_from_test_user(&pool, &TestTwitterUser::new("groupuser2"))
			.await
			.expect("User2 creation should succeed");
		let user3 = user_factory
			.create_from_test_user(&pool, &TestTwitterUser::new("groupuser3"))
			.await
			.expect("User3 creation should succeed");

		let room = room_factory
			.create_group(&pool, "Test Group", &[user1.id(), user2.id(), user3.id()])
			.await
			.expect("Room creation should succeed");

		assert!(room.is_group());
		assert_eq!(room.name().as_deref(), Some("Test Group"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_dm_message_factory_create(#[future] twitter_db_pool: (PgPool, String)) {
		let (pool, _url) = twitter_db_pool.await;
		let user_factory = UserFactory::new();
		let room_factory = DMRoomFactory::new();
		let message_factory = DMMessageFactory::new();

		let user1 = user_factory
			.create_from_test_user(&pool, &TestTwitterUser::new("msguser1"))
			.await
			.expect("User1 creation should succeed");
		let user2 = user_factory
			.create_from_test_user(&pool, &TestTwitterUser::new("msguser2"))
			.await
			.expect("User2 creation should succeed");

		let room = room_factory
			.create_direct(&pool, user1.id(), user2.id())
			.await
			.expect("Room creation should succeed");

		let message = message_factory
			.create(&pool, room.id(), user1.id(), "Hello!")
			.await
			.expect("Message creation should succeed");

		assert_eq!(message.content(), "Hello!");
		assert!(!message.is_read());
	}

	#[rstest]
	#[tokio::test]
	async fn test_dm_message_factory_mark_as_read(#[future] twitter_db_pool: (PgPool, String)) {
		let (pool, _url) = twitter_db_pool.await;
		let user_factory = UserFactory::new();
		let room_factory = DMRoomFactory::new();
		let message_factory = DMMessageFactory::new();

		let user1 = user_factory
			.create_from_test_user(&pool, &TestTwitterUser::new("readuser1"))
			.await
			.expect("User1 creation should succeed");
		let user2 = user_factory
			.create_from_test_user(&pool, &TestTwitterUser::new("readuser2"))
			.await
			.expect("User2 creation should succeed");

		let room = room_factory
			.create_direct(&pool, user1.id(), user2.id())
			.await
			.expect("Room creation should succeed");

		let message = message_factory
			.create(&pool, room.id(), user1.id(), "Read this!")
			.await
			.expect("Message creation should succeed");

		assert!(!message.is_read());

		message_factory
			.mark_as_read(&pool, message.id())
			.await
			.expect("Mark as read should succeed");

		let updated_message = message_factory
			.find_by_id(&pool, message.id())
			.await
			.expect("Find should succeed");

		assert!(updated_message.is_read());
	}
}
