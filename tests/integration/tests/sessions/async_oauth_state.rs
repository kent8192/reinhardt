//! Redis-backed async OAuth state integration tests.

use reinhardt_auth::social::{ContextualStateData, SocialAuthError, StateData, StateStore};
use reinhardt_middleware::session::{AsyncSessionBackend, AtomicSessionBackend, SessionData};
use reinhardt_middleware::{AsyncSessionStateStore, RedisSessionBackend};
use serial_test::serial;
use std::time::Duration;
use testcontainers::{
	GenericImage,
	core::{ContainerPort, WaitFor},
	runners::AsyncRunner,
};

async fn setup_redis() -> testcontainers::ContainerAsync<GenericImage> {
	GenericImage::new("redis", "7-alpine")
		.with_exposed_port(ContainerPort::Tcp(6379))
		.with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
		.start()
		.await
		.expect("Redis container should start")
}

async fn wait_for_redis(backend: &RedisSessionBackend) {
	for _ in 0..20 {
		if backend.destroy("__reinhardt_readiness__").await.is_ok() {
			return;
		}
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
	panic!("Redis did not accept connections within the readiness window");
}

#[tokio::test]
#[serial(redis)]
async fn contextual_state_is_consumed_once_across_redis_backends() {
	let container = setup_redis().await;
	let port = container.get_host_port_ipv4(6379).await.unwrap();
	let redis_url = format!("redis://127.0.0.1:{port}/");
	let first_backend = RedisSessionBackend::new_from_url(&redis_url).unwrap();
	wait_for_redis(&first_backend).await;
	let first = AsyncSessionStateStore::new(first_backend);
	let second =
		AsyncSessionStateStore::new(RedisSessionBackend::new_from_url(&redis_url).unwrap());
	first
		.store_contextual(
			ContextualStateData::new(
				StateData::new("shared-state".to_string(), None, None),
				"github".to_string(),
				b"binding",
				b"context".to_vec(),
			)
			.unwrap(),
		)
		.await
		.unwrap();

	let (left, right) = tokio::join!(
		first.consume_contextual("shared-state"),
		second.consume_contextual("shared-state"),
	);
	let contexts: Vec<Vec<u8>> = [left, right]
		.into_iter()
		.filter_map(Result::ok)
		.map(|record| record.context().to_vec())
		.collect();

	assert_eq!(contexts, vec![b"context".to_vec()]);
	assert!(matches!(
		first.consume_contextual("shared-state").await,
		Err(SocialAuthError::InvalidState),
	));
}

#[tokio::test]
#[serial(redis)]
async fn atomic_take_omits_expired_redis_session() {
	let container = setup_redis().await;
	let port = container.get_host_port_ipv4(6379).await.unwrap();
	let redis_url = format!("redis://127.0.0.1:{port}/");
	let backend = RedisSessionBackend::new_from_url(&redis_url).unwrap();
	wait_for_redis(&backend).await;
	let session = SessionData::new(Duration::from_secs(60));
	let id = session.id.clone();
	backend.save(&session).await.unwrap();
	backend.touch(&id, Duration::from_secs(1)).await.unwrap();
	tokio::time::sleep(Duration::from_millis(1_100)).await;

	let result = AtomicSessionBackend::take(&backend, &id).await.unwrap();

	assert!(result.is_none());
}
