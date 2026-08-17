use reinhardt_macros::settings;

type Ports = Vec<u16>;

#[settings(fragment = true, section = "test")]
struct AliasSettings {
	ports: Ports,
}

fn main() {}
