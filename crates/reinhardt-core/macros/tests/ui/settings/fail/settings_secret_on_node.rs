use reinhardt_macros::settings;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct NestedSettings {
	value: String,
}

#[settings(fragment = true, section = "root")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RootSettings {
	#[setting(secret, node)]
	nested: NestedSettings,
}

fn main() {}
