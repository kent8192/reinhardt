use reinhardt_macros::settings;

#[settings(fragment = true, section = "database")]
struct SchemaDatabaseSettings {
	value: String,
}

#[settings(SchemaDatabaseSettings)]
struct BadSettings;

fn main() {}
