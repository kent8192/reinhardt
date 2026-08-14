use cfg_aliases::cfg_aliases;
use std::env;

fn main() {
	let mut features: Vec<_> = env::vars()
		.filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
		.map(|feature| feature.to_ascii_lowercase().replace('_', "-"))
		.collect();
	features.sort();
	features.dedup();
	println!(
		"cargo:rustc-env=REINHARDT_ENABLED_FEATURES={}",
		features.join(",")
	);
	if let Ok(target) = env::var("TARGET") {
		println!("cargo:rustc-env=REINHARDT_TARGET={target}");
	}
	if let Ok(profile) = env::var("PROFILE") {
		println!("cargo:rustc-env=REINHARDT_PROFILE={profile}");
	}
	if let Ok(flags) = env::var("CARGO_ENCODED_RUSTFLAGS") {
		println!("cargo:rustc-env=REINHARDT_ENCODED_RUSTFLAGS={flags}");
	}
	let replay = if env::var("TARGET").is_ok() && env::var("PROFILE").is_ok() {
		"exact"
	} else {
		"unsupported"
	};
	println!("cargo:rustc-env=REINHARDT_CARGO_REPLAY={replay}");
	println!("cargo:rustc-cfg=with_reinhardt");

	println!("cargo:rerun-if-changed=build.rs");

	// Declare custom cfg to avoid warnings in Rust 2024 edition
	println!("cargo::rustc-check-cfg=cfg(with_reinhardt)");
	println!("cargo::rustc-check-cfg=cfg(wasm)");
	println!("cargo::rustc-check-cfg=cfg(native)");

	cfg_aliases! {
		wasm: { all(target_family = "wasm", target_os = "unknown") },
		native: { not(all(target_family = "wasm", target_os = "unknown")) },
	};
}
