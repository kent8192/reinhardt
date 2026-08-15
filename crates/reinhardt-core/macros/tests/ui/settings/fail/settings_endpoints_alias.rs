use std::collections::HashMap;

use reinhardt_macros::settings;

type Endpoints = HashMap<String, u16>;

#[settings(fragment = true, section = "test")]
struct AliasSettings {
	endpoints: Endpoints,
}

fn main() {}
