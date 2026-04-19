//! GraphQL schema fixtures
//!
//! This module provides fixtures for creating GraphQL schemas with various configurations.

use crate::prelude::*;
use async_graphql::{EmptySubscription, Schema};
use reinhardt_graphql::{Mutation, Query, QueryLimits, UserStorage};
use std::sync::Arc;

/// Creates a GraphQL schema connected to a test database.
///
/// This fixture wraps the generic `postgres_container` fixture from `reinhardt-test`
/// and creates a GraphQL schema with a real PostgreSQL database connection. The
/// returned tuple carries both the schema and the pool so dependent fixtures can
/// seed test data through the same container instance.
#[fixture]
async fn graphql_schema_fixture(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> (Schema<Query, Mutation, EmptySubscription>, Arc<PgPool>) {
	let (_container, pool, _port, _url) = postgres_container.await;

	// Build the schema with the in-memory `UserStorage` (current resolver backend)
	// and attach the real `PgPool` as schema data so future resolvers can use it.
	let limits = QueryLimits::default();
	let schema = Schema::build(Query, Mutation, EmptySubscription)
		.data(UserStorage::new())
		.data(pool.clone())
		.limit_depth(limits.max_depth)
		.limit_complexity(limits.max_complexity)
		.finish();

	(schema, pool)
}

/// Creates a GraphQL schema with DI context.
///
/// This fixture requires the `di` feature to be enabled.
#[cfg(feature = "di")]
#[fixture]
async fn graphql_di_fixture(
	#[future] injection_context_with_database: Arc<InjectionContext>,
) -> Schema<Query, Mutation, EmptySubscription> {
	use reinhardt_graphql::di::{GraphQLContextExt, SchemaBuilderExt};

	let context = injection_context_with_database.await;

	// Build schema with DI context
	let schema = create_schema().with_di_context(context.clone()).build();

	schema
}

/// Creates a GraphQL schema with subscription support.
///
/// This fixture requires the `subscription` feature to be enabled.
#[cfg(feature = "subscription")]
#[fixture]
async fn subscription_fixture(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> Schema<Query, Mutation, Subscription> {
	use reinhardt_graphql::subscription::{EventBroadcaster, SubscriptionRoot};

	let (_container, pool, _port, _url) = postgres_container.await;
	let broadcaster = EventBroadcaster::new();

	// Create subscription-enabled schema
	let schema = Schema::build(
		Query::default(),
		Mutation::default(),
		SubscriptionRoot::new(broadcaster),
	)
	.finish();

	schema
}

/// Creates a GraphQL schema with gRPC service.
///
/// This fixture requires the `graphql-grpc` feature to be enabled.
#[cfg(feature = "graphql-grpc")]
#[fixture]
async fn grpc_fixture(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> (
	Schema<Query, Mutation, EmptySubscription>,
	GraphQLGrpcService,
) {
	use reinhardt_graphql::grpc_service::GraphQLGrpcService;

	let (_container, pool, _port, _url) = postgres_container.await;
	let schema = create_schema();
	let grpc_service = GraphQLGrpcService::new(schema.clone());

	(schema, grpc_service)
}
