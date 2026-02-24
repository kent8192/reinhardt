//! Tweet factory for examples-twitter tests.
//!
//! Provides factory functions for creating Tweet records in the database.

use chrono::Utc;
use reinhardt_query::prelude::{
	Alias, Expr, ExprTrait, Order, PostgresQueryBuilder, Query, QueryStatementBuilder, Value,
};
use rstest::*;
use sqlx::PgPool;
use uuid::Uuid;

use crate::apps::tweet::models::Tweet;

/// Factory for creating Tweet records in the database.
pub struct TweetFactory;

impl Default for TweetFactory {
	fn default() -> Self {
		Self::new()
	}
}

impl TweetFactory {
	/// Create a new TweetFactory.
	pub fn new() -> Self {
		Self
	}

	/// Create a tweet for the given user.
	pub async fn create(
		&self,
		pool: &PgPool,
		user_id: Uuid,
		content: &str,
	) -> Result<Tweet, sqlx::Error> {
		let id = Uuid::new_v4();
		let now = Utc::now();

		let sql = Query::insert()
			.into_table(Alias::new("tweet_tweet"))
			.columns([
				Alias::new("id"),
				Alias::new("user_id"),
				Alias::new("content"),
				Alias::new("like_count"),
				Alias::new("retweet_count"),
				Alias::new("created_at"),
				Alias::new("updated_at"),
			])
			.values_panic([
				Value::from(id),
				Value::from(user_id),
				Value::from(content),
				Value::from(0i32),
				Value::from(0i32),
				Value::from(now),
				Value::from(now),
			])
			.to_string(PostgresQueryBuilder);

		sqlx::query(&sql).execute(pool).await?;

		self.find_by_id(pool, id).await
	}

	/// Create a tweet with likes and retweets.
	pub async fn create_with_counts(
		&self,
		pool: &PgPool,
		user_id: Uuid,
		content: &str,
		like_count: i32,
		retweet_count: i32,
	) -> Result<Tweet, sqlx::Error> {
		let id = Uuid::new_v4();
		let now = Utc::now();

		let sql = Query::insert()
			.into_table(Alias::new("tweet_tweet"))
			.columns([
				Alias::new("id"),
				Alias::new("user_id"),
				Alias::new("content"),
				Alias::new("like_count"),
				Alias::new("retweet_count"),
				Alias::new("created_at"),
				Alias::new("updated_at"),
			])
			.values_panic([
				Value::from(id),
				Value::from(user_id),
				Value::from(content),
				Value::from(like_count),
				Value::from(retweet_count),
				Value::from(now),
				Value::from(now),
			])
			.to_string(PostgresQueryBuilder);

		sqlx::query(&sql).execute(pool).await?;

		self.find_by_id(pool, id).await
	}

	/// Create multiple tweets for a user.
	pub async fn create_many(
		&self,
		pool: &PgPool,
		user_id: Uuid,
		contents: &[&str],
	) -> Result<Vec<Tweet>, sqlx::Error> {
		let mut tweets = Vec::with_capacity(contents.len());
		for content in contents {
			let tweet = self.create(pool, user_id, content).await?;
			tweets.push(tweet);
		}
		Ok(tweets)
	}

	/// Find a tweet by ID.
	pub async fn find_by_id(&self, pool: &PgPool, id: Uuid) -> Result<Tweet, sqlx::Error> {
		let sql = Query::select()
			.columns([
				Alias::new("id"),
				Alias::new("user_id"),
				Alias::new("content"),
				Alias::new("like_count"),
				Alias::new("retweet_count"),
				Alias::new("created_at"),
				Alias::new("updated_at"),
			])
			.from(Alias::new("tweet_tweet"))
			.and_where(Expr::col(Alias::new("id")).eq(Expr::val(id)))
			.to_string(PostgresQueryBuilder);

		sqlx::query_as::<_, Tweet>(&sql).fetch_one(pool).await
	}

	/// Find all tweets by user ID.
	pub async fn find_by_user_id(
		&self,
		pool: &PgPool,
		user_id: Uuid,
	) -> Result<Vec<Tweet>, sqlx::Error> {
		let sql = Query::select()
			.columns([
				Alias::new("id"),
				Alias::new("user_id"),
				Alias::new("content"),
				Alias::new("like_count"),
				Alias::new("retweet_count"),
				Alias::new("created_at"),
				Alias::new("updated_at"),
			])
			.from(Alias::new("tweet_tweet"))
			.and_where(Expr::col(Alias::new("user_id")).eq(Expr::val(user_id)))
			.order_by(Alias::new("created_at"), Order::Desc)
			.to_string(PostgresQueryBuilder);

		sqlx::query_as::<_, Tweet>(&sql).fetch_all(pool).await
	}

	/// Count tweets by user ID.
	pub async fn count_by_user_id(&self, pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
		sqlx::query_scalar("SELECT COUNT(*) FROM tweet_tweet WHERE user_id = $1")
			.bind(user_id)
			.fetch_one(pool)
			.await
	}

	/// Delete a tweet by ID.
	pub async fn delete(&self, pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
		let sql = Query::delete()
			.from_table(Alias::new("tweet_tweet"))
			.and_where(Expr::col(Alias::new("id")).eq(Expr::val(id)))
			.to_string(PostgresQueryBuilder);

		sqlx::query(&sql).execute(pool).await?;
		Ok(())
	}
}

/// rstest fixture providing a TweetFactory.
#[fixture]
pub fn tweet_factory() -> TweetFactory {
	TweetFactory::new()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_utils::factories::user::UserFactory;
	use crate::test_utils::fixtures::database::twitter_db_pool;
	use crate::test_utils::fixtures::users::TestTwitterUser;

	#[rstest]
	#[tokio::test]
	async fn test_tweet_factory_create(#[future] twitter_db_pool: (PgPool, String)) {
		let (pool, _url) = twitter_db_pool.await;
		let user_factory = UserFactory::new();
		let tweet_factory = TweetFactory::new();

		let test_user = TestTwitterUser::new("tweetuser");
		let user = user_factory
			.create_from_test_user(&pool, &test_user)
			.await
			.expect("User creation should succeed");

		let tweet = tweet_factory
			.create(&pool, user.id(), "Hello, Twitter!")
			.await
			.expect("Tweet creation should succeed");

		assert_eq!(tweet.content(), "Hello, Twitter!");
		assert_eq!(tweet.like_count(), 0);
		assert_eq!(tweet.retweet_count(), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_tweet_factory_create_many(#[future] twitter_db_pool: (PgPool, String)) {
		let (pool, _url) = twitter_db_pool.await;
		let user_factory = UserFactory::new();
		let tweet_factory = TweetFactory::new();

		let test_user = TestTwitterUser::new("manytweets");
		let user = user_factory
			.create_from_test_user(&pool, &test_user)
			.await
			.expect("User creation should succeed");

		let contents = ["Tweet 1", "Tweet 2", "Tweet 3"];
		let tweets = tweet_factory
			.create_many(&pool, user.id(), &contents)
			.await
			.expect("Multiple tweet creation should succeed");

		assert_eq!(tweets.len(), 3);
	}

	#[rstest]
	#[tokio::test]
	async fn test_tweet_factory_find_by_user_id(#[future] twitter_db_pool: (PgPool, String)) {
		let (pool, _url) = twitter_db_pool.await;
		let user_factory = UserFactory::new();
		let tweet_factory = TweetFactory::new();

		let test_user = TestTwitterUser::new("findtweets");
		let user = user_factory
			.create_from_test_user(&pool, &test_user)
			.await
			.expect("User creation should succeed");

		tweet_factory
			.create(&pool, user.id(), "First tweet")
			.await
			.expect("Tweet creation should succeed");
		tweet_factory
			.create(&pool, user.id(), "Second tweet")
			.await
			.expect("Tweet creation should succeed");

		let tweets = tweet_factory
			.find_by_user_id(&pool, user.id())
			.await
			.expect("Find should succeed");

		assert_eq!(tweets.len(), 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_tweet_factory_delete(#[future] twitter_db_pool: (PgPool, String)) {
		let (pool, _url) = twitter_db_pool.await;
		let user_factory = UserFactory::new();
		let tweet_factory = TweetFactory::new();

		let test_user = TestTwitterUser::new("deletetweet");
		let user = user_factory
			.create_from_test_user(&pool, &test_user)
			.await
			.expect("User creation should succeed");

		let tweet = tweet_factory
			.create(&pool, user.id(), "To be deleted")
			.await
			.expect("Tweet creation should succeed");

		tweet_factory
			.delete(&pool, tweet.id())
			.await
			.expect("Delete should succeed");

		let count = tweet_factory
			.count_by_user_id(&pool, user.id())
			.await
			.expect("Count should succeed");

		assert_eq!(count, 0);
	}
}
