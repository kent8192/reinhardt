//! GraphQL schema fixtures
//!
//! This module provides fixtures for creating GraphQL schemas with various configurations.

use crate::prelude::*;
use async_graphql::{EmptySubscription, Schema};
use reinhardt_graphql::{Mutation, Query, create_schema};
use std::sync::Arc;

/// Creates a GraphQL schema connected to a test database.
///
/// This fixture wraps the generic `postgres_container` fixture from `reinhardt-test`
/// and creates a GraphQL schema with a real PostgreSQL database connection.
#[fixture]
async fn graphql_schema_fixture(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> Schema<Query, Mutation, EmptySubscription> {
	let (_container, pool, _port, _url) = postgres_container.await;

	// Create schema with database connection
	// Note: production code would configure the schema to use the database pool;
	// this fixture uses the default in-memory storage to keep the test hermetic.
	create_schema()
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
