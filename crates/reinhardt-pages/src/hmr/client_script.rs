//! HMR client-side JavaScript that is injected into development pages.

/// JavaScript source for the HMR client.
///
/// This script establishes a WebSocket connection to the HMR server and handles
/// incoming messages to apply CSS hot updates or trigger full page reloads.
pub const HMR_CLIENT_SCRIPT: &str = r#"
(function() {
  "use strict";

  var HMR_WS_PORT = __HMR_WS_PORT__;
  // Mirror the page protocol: wss under HTTPS, plain ws under HTTP.
  // location.protocol is "https:" or "http:", replacing "http" with "ws" gives the correct scheme.
  var wsProtocol = window.location.protocol.replace("http", "ws");
  var wsUrl = wsProtocol + "//" + window.location.hostname + ":" + HMR_WS_PORT + "/hmr";
  var reconnectDelay = 1000;
  var maxReconnectDelay = 30000;
  var ws;

  function connect() {
    ws = new WebSocket(wsUrl);

    ws.onopen = function() {
      console.log("[HMR] Connected to development server");
      reconnectDelay = 1000;
    };

    ws.onmessage = function(event) {
      var msg;
      try {
        msg = JSON.parse(event.data);
      } catch (e) {
        console.warn("[HMR] Invalid message:", event.data);
        return;
      }

      switch (msg.type) {
        case "css_update":
          hotSwapCss(msg.path);
          break;
        case "full_reload":
          console.log("[HMR] Full reload:", msg.reason);
          window.location.reload();
          break;
        case "connected":
          console.log("[HMR] Server acknowledged connection");
          break;
        default:
          console.warn("[HMR] Unknown message type:", msg.type);
      }
    };

    ws.onclose = function() {
      console.log("[HMR] Connection lost, reconnecting in " + reconnectDelay + "ms...");
      setTimeout(function() {
        reconnectDelay = Math.min(reconnectDelay * 2, maxReconnectDelay);
        connect();
      }, reconnectDelay);
    };

    ws.onerror = function() {
      ws.close();
    };
  }

  function hotSwapCss(path) {
    var links = document.querySelectorAll('link[rel="stylesheet"]');
    var updated = false;
    var cacheBust = "?hmr=" + Date.now();

    for (var i = 0; i < links.length; i++) {
      var link = links[i];
      var href = link.getAttribute("href");
      if (href && href.split("?")[0].endsWith(path)) {
        link.href = href.split("?")[0] + cacheBust;
        updated = true;
        console.log("[HMR] CSS updated:", path);
      }
    }

    if (!updated) {
      console.log("[HMR] CSS file not found in page, reloading:", path);
      window.location.reload();
    }
  }

  connect();
})();
"#;

/// Generates an HTML `<script>` tag containing the HMR client, with the
/// WebSocket port placeholder replaced.
pub fn hmr_script_tag(ws_port: u16) -> String {
	let script = HMR_CLIENT_SCRIPT.replace("__HMR_WS_PORT__", &ws_port.to_string());
	format!("<script>{}</script>", script)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_hmr_client_script_is_valid_js() {
		// Assert
		assert!(HMR_CLIENT_SCRIPT.contains("WebSocket"));
		assert!(HMR_CLIENT_SCRIPT.contains("css_update"));
		assert!(HMR_CLIENT_SCRIPT.contains("full_reload"));
		assert!(HMR_CLIENT_SCRIPT.contains("connected"));
		assert!(HMR_CLIENT_SCRIPT.contains("__HMR_WS_PORT__"));
	}

	#[rstest]
	fn test_hmr_script_tag_replaces_port() {
		// Arrange
		let port = 35729;

		// Act
		let tag = hmr_script_tag(port);

		// Assert
		assert!(tag.starts_with("<script>"));
		assert!(tag.ends_with("</script>"));
		assert!(tag.contains("35729"));
		assert!(!tag.contains("__HMR_WS_PORT__"));
	}

	#[rstest]
	fn test_hmr_script_tag_custom_port() {
		// Arrange
		let port = 9000;

		// Act
		let tag = hmr_script_tag(port);

		// Assert
		assert!(tag.contains("9000"));
		assert!(!tag.contains("35729"));
	}

	#[rstest]
	fn test_hmr_client_script_has_reconnect_logic() {
		// Assert
		assert!(HMR_CLIENT_SCRIPT.contains("reconnectDelay"));
		assert!(HMR_CLIENT_SCRIPT.contains("maxReconnectDelay"));
		assert!(HMR_CLIENT_SCRIPT.contains("onclose"));
	}

	#[rstest]
	fn test_hmr_client_script_has_css_hot_swap() {
		// Assert
		assert!(HMR_CLIENT_SCRIPT.contains("hotSwapCss"));
		assert!(HMR_CLIENT_SCRIPT.contains("cacheBust"));
		assert!(HMR_CLIENT_SCRIPT.contains("stylesheet"));
	}

	// --- Boundary values ---

	#[rstest]
	fn test_hmr_script_tag_port_zero() {
		// Boundary: port 0 should be stringified as "0"
		let tag = hmr_script_tag(0);
		assert!(tag.contains("var HMR_WS_PORT = 0;"));
		assert!(!tag.contains("__HMR_WS_PORT__"));
	}

	#[rstest]
	fn test_hmr_script_tag_port_max() {
		// Boundary: port 65535 = maximum valid TCP port
		let tag = hmr_script_tag(65535);
		assert!(tag.contains("65535"));
		assert!(!tag.contains("__HMR_WS_PORT__"));
	}

	#[rstest]
	fn test_hmr_client_script_placeholder_appears_once() {
		// The placeholder must appear exactly once so replacement is unambiguous
		let count = HMR_CLIENT_SCRIPT.matches("__HMR_WS_PORT__").count();
		assert_eq!(
			count, 1,
			"__HMR_WS_PORT__ must appear exactly once in the template"
		);
	}

	#[rstest]
	fn test_hmr_script_tag_no_placeholder_after_replacement() {
		// After replacement, no placeholder remains regardless of the port value
		for port in [0u16, 1, 1024, 35729, 65535] {
			let tag = hmr_script_tag(port);
			assert!(
				!tag.contains("__HMR_WS_PORT__"),
				"placeholder still present for port {port}"
			);
		}
	}

	#[rstest]
	fn test_hmr_script_tag_wrapped_in_script_element() {
		// The output must be a well-formed <script> element
		let tag = hmr_script_tag(35729);
		assert!(tag.starts_with("<script>"));
		assert!(tag.ends_with("</script>"));
		// No attributes on the opening tag (no src= or type= that could break inline scripts)
		assert!(!tag.starts_with("<script "));
	}

	#[rstest]
	fn test_hmr_client_script_uses_ws_protocol_detection() {
		// The script must detect wss vs ws based on page protocol
		assert!(HMR_CLIENT_SCRIPT.contains("replace(\"http\", \"ws\")"));
	}

	#[rstest]
	fn test_hmr_client_script_handles_unknown_message_type() {
		// Unknown message types should be warned, not crash
		assert!(HMR_CLIENT_SCRIPT.contains("Unknown message type"));
	}
}
