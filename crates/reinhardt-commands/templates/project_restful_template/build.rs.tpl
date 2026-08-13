use std::env;

fn main() {
	let mut features: Vec<_> = env::vars()
		.filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
		.map(|feature| feature.to_ascii_lowercase().replace('_', "-"))
		.collect();
	features.sort();
	features.dedup();
	println!("cargo:rustc-env=REINHARDT_ENABLED_FEATURES={}", features.join(","));
	if let Ok(target) = env::var("TARGET") {
		println!("cargo:rustc-env=REINHARDT_TARGET={target}");
	}
	if let Ok(profile) = env::var("PROFILE") {
		println!("cargo:rustc-env=REINHARDT_PROFILE={profile}");
	}
	if let Ok(flags) = env::var("CARGO_ENCODED_RUSTFLAGS") {
		println!("cargo:rustc-env=REINHARDT_ENCODED_RUSTFLAGS={flags}");
	}
	println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
}
