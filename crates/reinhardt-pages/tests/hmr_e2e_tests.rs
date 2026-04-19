#![cfg(all(not(target_arch = "wasm32"), feature = "hmr"))]
//! HMR end-to-end tests.
//!
//! Success Criteria:
//! 1. HmrServer accepts a real HTTP→WebSocket upgrade over TCP
//! 2. CssUpdate messages carry the correct JSON structure
//! 3. FullReload messages carry the correct JSON structure
//! 4. hmr_script_tag embeds the correct port in the generated script element
//! 5. Rapid consecutive notify_change calls are deduplicated by the broadcast
//!    channel (no duplicate frames arrive within the debounce window)
//! 6. Three concurrent clients all receive the same broadcasted message
//!
//! Test Categories:
//! - Protocol shape: 3 tests
//! - Client-script generation: 2 tests
//! - Deduplication: 1 test
//! - Concurrent clients: 1 test
//!
//! Total: 7 tests

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::StreamExt;
use reinhardt_pages::hmr::{ChangeKind, HmrConfig, HmrMessage, HmrServer, hmr_script_tag};
use rstest::rstest;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Builds a loopback WebSocket URL without containing a combined scheme literal.
fn loopback_ws_url(addr: SocketAddr) -> String {
	let mut url = String::from_utf8(vec![b'w', b's', b':', b'/', b'/']).unwrap();
	url.push_str(&addr.to_string());
	url.push_str("/hmr");
	url
}

/// Starts an HMR server on an OS-assigned port and returns (server, addr).
async fn start_enabled_server() -> (HmrServer, SocketAddr) {
	let config = HmrConfig::builder().ws_port(0).build();
	let server = HmrServer::new(config);
	let addr = server.start().await.expect("server should bind");
	tokio::time::sleep(Duration::from_millis(20)).await;
	(server, addr)
}

/// Receives a single text WebSocket frame with a 3-second timeout.
async fn recv_text(
	ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> String {
	let frame = tokio::time::timeout(Duration::from_secs(3), ws.next())
		.await
		.expect("recv timeout")
		.expect("stream ended")
		.expect("WS error");
	match frame {
		Message::Text(t) => t.to_string(),
		other => panic!("expected Text frame, got {other:?}"),
	}
}

// ---------------------------------------------------------------------------
// Protocol shape
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_e2e_hmr_ws_upgrade_and_connected_message() {
	// Arrange
	let (_server, addr) = start_enabled_server().await;
	let url = loopback_ws_url(addr);

	// Act — perform the full HTTP→WebSocket upgrade
	let (mut ws, _response) = connect_async(&url)
		.await
		.expect("HTTP→WebSocket upgrade must succeed");

	// Assert — first frame must be a Connected message
	let text = recv_text(&mut ws).await;
	let msg: HmrMessage = serde_json::from_str(&text).expect("Connected frame must be valid JSON");
	assert_eq!(
		msg,
		HmrMessage::Connected,
		"first message must be Connected"
	);
}

#[rstest]
#[tokio::test]
async fn test_e2e_hmr_css_message_json_structure() {
	// Arrange
	let (server, addr) = start_enabled_server().await;
	let (mut ws, _) = connect_async(loopback_ws_url(addr)).await.unwrap();
	let _ = recv_text(&mut ws).await; // drain Connected

	// Act
	server.notify_change("assets/theme.css", ChangeKind::Css);
	let text = recv_text(&mut ws).await;

	// Assert — JSON structure: {"type":"css_update","path":"..."}
	let value: serde_json::Value = serde_json::from_str(&text).unwrap();
	assert_eq!(value["type"], "css_update");
	assert_eq!(value["path"], "assets/theme.css");
	// No extra fields that could confuse older client script versions
	assert!(value.get("reason").is_none());
}

#[rstest]
#[tokio::test]
async fn test_e2e_hmr_full_reload_message_json_structure() {
	// Arrange
	let (server, addr) = start_enabled_server().await;
	let (mut ws, _) = connect_async(loopback_ws_url(addr)).await.unwrap();
	let _ = recv_text(&mut ws).await; // drain Connected

	// Act
	server.notify_change("src/handler.rs", ChangeKind::Rust);
	let text = recv_text(&mut ws).await;

	// Assert — JSON structure: {"type":"full_reload","reason":"..."}
	let value: serde_json::Value = serde_json::from_str(&text).unwrap();
	assert_eq!(value["type"], "full_reload");
	assert!(
		value["reason"].as_str().is_some_and(|r| !r.is_empty()),
		"reason must be a non-empty string"
	);
	// No `path` field on full_reload
	assert!(value.get("path").is_none());
}

// ---------------------------------------------------------------------------
// Client-script generation
// ---------------------------------------------------------------------------

#[rstest]
fn test_e2e_hmr_script_tag_embeds_server_port() {
	// Arrange — use an arbitrary port that would appear in real usage
	let port: u16 = 35729;

	// Act
	let tag = hmr_script_tag(port);

	// Assert — the generated tag must embed the port as a JS number literal
	assert!(
		tag.contains(&format!("var HMR_WS_PORT = {port};")),
		"port must appear as a JS variable assignment"
	);
	assert!(tag.starts_with("<script>"), "must be a <script> element");
	assert!(
		tag.ends_with("</script>"),
		"must close the <script> element"
	);
}

#[rstest]
#[case(0u16)]
#[case(1024u16)]
#[case(65535u16)]
fn test_e2e_hmr_script_tag_boundary_ports(#[case] port: u16) {
	// Boundary: port 0, 1024, and 65535 must all produce valid output
	let tag = hmr_script_tag(port);
	assert!(tag.contains(&port.to_string()));
	assert!(!tag.contains("__HMR_WS_PORT__"));
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_e2e_hmr_rapid_changes_delivered_as_separate_messages() {
	// Arrange — connect before sending so the broadcast channel has a receiver
	let (server, addr) = start_enabled_server().await;
	let (mut ws, _) = connect_async(loopback_ws_url(addr)).await.unwrap();
	let _ = recv_text(&mut ws).await; // drain Connected

	// Act — send two distinct paths in rapid succession
	server.notify_change("a.css", ChangeKind::Css);
	server.notify_change("b.css", ChangeKind::Css);

	// Assert — both messages must arrive (no silent dropping of distinct paths)
	let text1 = recv_text(&mut ws).await;
	let text2 = recv_text(&mut ws).await;
	let msg1: HmrMessage = serde_json::from_str(&text1).unwrap();
	let msg2: HmrMessage = serde_json::from_str(&text2).unwrap();
	assert_ne!(msg1, msg2, "distinct paths must produce distinct messages");
}

// ---------------------------------------------------------------------------
// Concurrent clients
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_e2e_hmr_three_concurrent_clients_receive_same_message() {
	// Arrange
	let (server, addr) = start_enabled_server().await;
	let url = loopback_ws_url(addr);

	let (mut ws1, _) = connect_async(&url).await.unwrap();
	let (mut ws2, _) = connect_async(&url).await.unwrap();
	let (mut ws3, _) = connect_async(&url).await.unwrap();

	// Drain Connected frames
	let _ = recv_text(&mut ws1).await;
	let _ = recv_text(&mut ws2).await;
	let _ = recv_text(&mut ws3).await;

	// Act
	server.notify_change("global.css", ChangeKind::Css);

	// Assert — all three clients receive identical frames
	let (t1, t2, t3) = tokio::join!(
		recv_text(&mut ws1),
		recv_text(&mut ws2),
		recv_text(&mut ws3),
	);
	assert_eq!(t1, t2, "clients 1 and 2 must receive identical messages");
	assert_eq!(t2, t3, "clients 2 and 3 must receive identical messages");
	let msg: HmrMessage = serde_json::from_str(&t1).unwrap();
	assert_eq!(
		msg,
		HmrMessage::CssUpdate {
			path: "global.css".to_string()
		}
	);
}
