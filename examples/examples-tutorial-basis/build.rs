use cfg_aliases::cfg_aliases;
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

fn declared_feature_names() -> Vec<String> {
	let Some(manifest_dir) = env::var_os("CARGO_MANIFEST_DIR") else {
		return Vec::new();
	};
	let Ok(manifest) = std::fs::read_to_string(PathBuf::from(manifest_dir).join("Cargo.toml")) else {
		return Vec::new();
	};
	let mut in_features = false;
	let mut features = Vec::new();
	for line in manifest.lines() {
		let line = line.split('#').next().unwrap_or_default().trim();
		if line.starts_with('[') {
			in_features = line == "[features]";
			continue;
		}
		if in_features {
			let Some((name, _)) = line.split_once('=') else {
				continue;
			};
			let name = name
				.trim()
				.trim_matches(|character| character == '"' || character == '\'');
			if !name.is_empty() {
				features.push(name.to_owned());
			}
		}
	}
	features
}

fn cargo_feature_name(env_name: &str, declared: &[String]) -> String {
	declared
		.iter()
		.find(|feature| feature.to_ascii_uppercase().replace('-', "_") == env_name)
		.cloned()
		.unwrap_or_else(|| env_name.to_ascii_lowercase())
}

#[cfg(unix)]
fn cargo_process_command_line() -> Option<String> {
	let pid = std::process::id().to_string();
	let parent = std::process::Command::new("ps")
		.args(["-o", "ppid=", "-p", &pid])
		.output()
		.ok()?;
	if !parent.status.success() {
		return None;
	}
	let parent = String::from_utf8(parent.stdout).ok()?;
	let command = std::process::Command::new("ps")
		.args(["-o", "command=", "-p", parent.trim()])
		.output()
		.ok()?;
	if !command.status.success() {
		return None;
	}
	String::from_utf8(command.stdout).ok()
}

#[cfg(windows)]
fn cargo_process_command_line() -> Option<String> {
	let script = format!(
		"$process = Get-CimInstance Win32_Process -Filter 'ProcessId = {}'; if ($null -ne $process) {{ (Get-CimInstance Win32_Process -Filter \"ProcessId = $($process.ParentProcessId)\").CommandLine }}",
		std::process::id()
	);
	let output = std::process::Command::new("powershell.exe")
		.args(["-NoProfile", "-NonInteractive", "-Command", &script])
		.output()
		.ok()?;
	if !output.status.success() {
		return None;
	}
	String::from_utf8(output.stdout).ok()
}

#[cfg(not(any(unix, windows)))]
fn cargo_process_command_line() -> Option<String> {
	None
}

const UNSUPPORTED_CARGO_FLAGS: &[&str] = &[
	"--config",
	"--ignore-rust-version",
	"--locked",
	"--offline",
	"--frozen",
	"--lockfile-path",
];

fn cargo_invocation_has_unsupported_flag() -> Option<bool> {
	let command = cargo_process_command_line()?;
	Some(
		command
			.split_whitespace()
			.any(|argument| {
				UNSUPPORTED_CARGO_FLAGS.iter().any(|flag| {
					argument == *flag
						|| argument
							.strip_prefix(flag)
							.is_some_and(|suffix| suffix.starts_with('='))
				})
			}),
	)
}

fn main() {
	let declared = declared_feature_names();
	let mut features: Vec<_> = env::vars()
		.filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
		.map(|feature| cargo_feature_name(&feature, &declared))
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
	if let Some(profile) = selected_profile().or_else(|| env::var("PROFILE").ok()) {
		println!("cargo:rustc-env=REINHARDT_PROFILE={profile}");
	}
	if let Ok(flags) = env::var("CARGO_ENCODED_RUSTFLAGS") {
		println!("cargo:rustc-env=REINHARDT_ENCODED_RUSTFLAGS={flags}");
	}
	for (source, target) in [
		("RUSTC_WRAPPER", "REINHARDT_RUSTC_WRAPPER"),
		(
			"RUSTC_WORKSPACE_WRAPPER",
			"REINHARDT_RUSTC_WORKSPACE_WRAPPER",
		),
		("RUSTC_LINKER", "REINHARDT_RUSTC_LINKER"),
	] {
		if let Ok(value) = env::var(source) {
			println!("cargo:rustc-env={target}={value}");
		}
	}
	let replay = if env::var("TARGET").is_ok()
		&& selected_profile().is_some()
		&& cargo_invocation_has_unsupported_flag() == Some(false)
	{
		"exact"
	} else {
		"unsupported"
	};
	println!("cargo:rustc-env=REINHARDT_CARGO_REPLAY={replay}");
	println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
	println!("cargo:rerun-if-env-changed=RUSTC_WRAPPER");
	println!("cargo:rerun-if-env-changed=RUSTC_WORKSPACE_WRAPPER");
	println!("cargo:rerun-if-env-changed=RUSTC_LINKER");
	println!("cargo:rerun-if-changed=Cargo.toml");
	println!("cargo:rustc-cfg=with_reinhardt");

	println!("cargo:rerun-if-changed=build.rs");

	// Declare custom cfg to avoid warnings in Rust 2024 edition
	println!("cargo::rustc-check-cfg=cfg(with_reinhardt)");
	println!("cargo::rustc-check-cfg=cfg(client)");
	println!("cargo::rustc-check-cfg=cfg(server)");
	println!("cargo::rustc-check-cfg=cfg(wasm)");
	println!("cargo::rustc-check-cfg=cfg(native)");

	cfg_aliases! {
		// Platform aliases for simpler conditional compilation
		// Use `#[cfg(client)]` instead of `#[cfg(target_arch = "wasm32")]`
		client: { target_arch = "wasm32" },
		// Use `#[cfg(server)]` instead of `#[cfg(not(target_arch = "wasm32"))]`
		server: { not(target_arch = "wasm32") },
		// Compatibility aliases used by framework macro expansions.
		wasm: { target_arch = "wasm32" },
		native: { not(target_arch = "wasm32") },
	};
}
