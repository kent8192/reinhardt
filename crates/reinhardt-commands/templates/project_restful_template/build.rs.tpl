use std::env;
use std::path::PathBuf;

fn selected_profile() -> Option<String> {
	let mut path = PathBuf::from(env::var_os("OUT_DIR")?);
	while let Some(name) = path.file_name() {
		if name == "build" {
			return path
				.parent()
				.and_then(|parent| parent.file_name())
				.map(|profile| profile.to_string_lossy().into_owned());
		}
		path.pop();
	}
	None
}

fn main() {
	let mut features: Vec<_> = env::vars()
		.filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
		.map(|feature| feature.to_ascii_lowercase())
		.collect();
	features.sort();
	features.dedup();
	println!("cargo:rustc-env=REINHARDT_ENABLED_FEATURES={}", features.join(","));
	if let Ok(target) = env::var("TARGET") {
		println!("cargo:rustc-env=REINHARDT_TARGET={target}");
	}
	if let Some(profile) = selected_profile().or_else(|| env::var("PROFILE").ok()) {
		println!("cargo:rustc-env=REINHARDT_PROFILE={profile}");
	}
	if let Ok(flags) = env::var("CARGO_ENCODED_RUSTFLAGS") {
		println!("cargo:rustc-env=REINHARDT_ENCODED_RUSTFLAGS={flags}");
	}
	let replay = if env::var("TARGET").is_ok() && selected_profile().is_some() {
		"exact"
	} else {
		"unsupported"
	};
	println!("cargo:rustc-env=REINHARDT_CARGO_REPLAY={replay}");
	println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
}
