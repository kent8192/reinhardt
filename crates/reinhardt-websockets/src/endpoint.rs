pub trait WebSocketEndpointInfo {
    fn path() -> &'static str;
    fn name() -> Option<&'static str>;
}

pub struct WebSocketEndpointMetadata {
    pub path: &'static str,
    pub name: &'static str,
    pub fn_name: &'static str,
    pub module_path: &'static str,
}

inventory::collect!(WebSocketEndpointMetadata);

/// Substitute path parameters: "/ws/chat/{room_id}/" + [("room_id","42")] → "/ws/chat/42/"
pub fn substitute_ws_params(path: &str, params: &[(&str, &str)]) -> String {
    let mut result = path.to_string();
    for (name, value) in params {
        result = result.replace(&format!("{{{}}}", name), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_substitute_no_params() {
        let result = substitute_ws_params("/ws/notif/", &[]);
        assert_eq!(result, "/ws/notif/");
    }

    #[rstest]
    fn test_substitute_one_param() {
        let result = substitute_ws_params("/ws/chat/{room_id}/", &[("room_id", "42")]);
        assert_eq!(result, "/ws/chat/42/");
    }

    #[rstest]
    fn test_substitute_two_params() {
        let result = substitute_ws_params("/ws/{org}/{repo}/", &[("org", "acme"), ("repo", "app")]);
        assert_eq!(result, "/ws/acme/app/");
    }
}
