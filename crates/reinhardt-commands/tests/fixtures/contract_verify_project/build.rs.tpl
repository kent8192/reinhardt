use std::env;
use std::path::Path;

fn main() {
	let mut features: Vec<_> = env::vars()
		.filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
		.map(|feature| feature.to_ascii_lowercase().replace('_', "-"))
		.collect();
	features.sort();
	println!("cargo:rustc-env=REINHARDT_ENABLED_FEATURES={}", features.join(","));
	if let Ok(target) = env::var("TARGET") {
		println!("cargo:rustc-env=REINHARDT_TARGET={target}");
	}
	if let Ok(profile) = env::var("PROFILE") {
		println!("cargo:rustc-env=REINHARDT_PROFILE={profile}");
	}
	if let Ok(rustc) = env::var("RUSTC") {
		if let Some(toolchain) = Path::new(&rustc)
			.ancestors()
			.nth(2)
			.and_then(Path::file_name)
			.and_then(|name| name.to_str())
		{
			println!("cargo:rustc-env=REINHARDT_RUSTUP_TOOLCHAIN={toolchain}");
		}
	}
	let replay = if env::var("TARGET").is_ok() && env::var("PROFILE").is_ok() {
		"exact"
	} else {
		"unsupported"
	};
	println!("cargo:rustc-env=REINHARDT_CARGO_REPLAY={replay}");
}
