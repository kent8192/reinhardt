use reinhardt_macros::settings;

struct ServiceSettings {
	port: u16,
}

#[settings(service: ServiceSettings { port: optional })]
struct ProjectSettings;

fn main() {}
