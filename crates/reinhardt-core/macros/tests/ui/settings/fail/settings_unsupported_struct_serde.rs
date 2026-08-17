use reinhardt_macros::settings;

#[settings(fragment = true, section = "deny")]
#[serde(deny_unknown_fields)]
struct DenyUnknown {
	value: String,
}

#[settings(fragment = true, section = "try_from")]
#[serde(try_from = "String")]
struct TryFromSettings {
	value: String,
}

#[settings(fragment = true, section = "from")]
#[serde(from = "String")]
struct FromSettings {
	value: String,
}

#[settings(fragment = true, section = "into")]
#[serde(into = "String")]
struct IntoSettings {
	value: String,
}

#[settings(fragment = true, section = "transparent")]
#[serde(transparent)]
struct TransparentSettings {
	value: String,
}

fn main() {}
