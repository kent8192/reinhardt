use reinhardt_macros::settings;

type PortList = Vec<u16>;

#[settings(fragment = true, section = "test")]
struct AliasSettings {
	ports: PortList,
}

fn main() {}
