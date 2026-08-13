//! Wire contracts used by normalized query SSR and hydration.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Reserved resource identifier for the request-scoped normalized entity table.
pub(crate) const ENTITY_TABLE_HYDRATION_ID: &str = "pages.query-entities:v1";

/// Version of the normalized entity table currently understood by the client.
pub(crate) const ENTITY_TABLE_VERSION: u8 = 1;

/// Deduplicated entities reachable from one SSR query client.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct EntityHydrationEnvelope {
	pub(crate) version: u8,
	pub(crate) entities: BTreeMap<String, Vec<EntityHydrationRow>>,
}

/// One entity row in an [`EntityHydrationEnvelope`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct EntityHydrationRow {
	pub(crate) id: serde_json::Value,
	pub(crate) value: serde_json::Value,
}

/// Discriminant for a normalized query hydration snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NormalizedHydrationKind {
	Success,
	Error,
}

/// State payload for a normalized query hydration snapshot.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum NormalizedQueryHydrationState<E> {
	Success { projection: serde_json::Value },
	Error(E),
}

/// Versioned recipe snapshot for a normalized query.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct NormalizedQueryHydrationSnapshot<E> {
	pub(crate) version: u8,
	pub(crate) kind: NormalizedHydrationKind,
	pub(crate) schema: String,
	pub(crate) state: NormalizedQueryHydrationState<E>,
	pub(crate) refetch_error: Option<E>,
	pub(crate) is_fetching: bool,
	pub(crate) is_stale: bool,
}
