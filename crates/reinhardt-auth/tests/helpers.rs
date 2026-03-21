//! Test helpers for social authentication tests

#![cfg(feature = "social")]
// This file is a shared module included via #[path] in test files, not a standalone test.
// When compiled as its own test binary, all items appear unused.
#![allow(dead_code, unused_imports)]

#[path = "helpers/assertions.rs"]
pub(crate) mod assertions;
#[path = "helpers/mock_server.rs"]
pub(crate) mod mock_server;
#[path = "helpers/test_fixtures.rs"]
pub(crate) mod test_fixtures;
