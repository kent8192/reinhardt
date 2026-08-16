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

fn dependency_table_name(section: &str) -> Option<&str> {
	let section = section.strip_prefix('[')?.strip_suffix(']')?;
	for marker in ["dependencies.", "dev-dependencies.", "build-dependencies."] {
		if let Some(name) = section.strip_prefix(marker) {
			return Some(name);
		}
	}
	for marker in [
		".dependencies.",
		".dev-dependencies.",
		".build-dependencies.",
	] {
		if let Some((_, name)) = section.rsplit_once(marker) {
			return Some(name);
		}
	}
	None
}

fn declared_feature_names() -> Vec<String> {
	let Some(manifest_dir) = env::var_os("CARGO_MANIFEST_DIR") else {
		return Vec::new();
	};
	let Ok(manifest) = std::fs::read_to_string(PathBuf::from(manifest_dir).join("Cargo.toml"))
	else {
		return Vec::new();
	};
	let mut section = "";
	let mut features = Vec::new();
	let mut optional_dependencies = Vec::new();
	let mut suppressed_dependencies = Vec::new();
	let mut feature_array_depth = 0;
	for line in manifest.lines() {
		let line = strip_toml_comment(line).trim();
		if feature_array_depth > 0 {
			collect_suppressed_dependencies(line, &mut suppressed_dependencies);
			feature_array_depth += toml_array_depth(line);
			continue;
		}
		if line.starts_with('[') {
			section = line;
			continue;
		}
		let is_features = section == "[features]";
		let table_dependency = dependency_table_name(section);
		let is_dependency = section == "[dependencies]"
			|| section == "[dev-dependencies]"
			|| section == "[build-dependencies]"
			|| section.ends_with(".dependencies]")
			|| section.ends_with(".dev-dependencies]")
			|| section.ends_with(".build-dependencies]");
		let Some((name, value)) = line.split_once('=') else {
			continue;
		};
		let name = name
			.trim()
			.trim_matches(|character| character == '"' || character == '\'');
		let compact_value = value
			.chars()
			.filter(|character| !character.is_ascii_whitespace())
			.collect::<String>();
		let optional_dependency = is_dependency && compact_value.contains("optional=true");
		if is_features && !name.is_empty() {
			features.push(name.to_owned());
			collect_suppressed_dependencies(value, &mut suppressed_dependencies);
			feature_array_depth = toml_array_depth(value);
		}
		if optional_dependency && !name.is_empty() {
			optional_dependencies.push(name.to_owned());
		}
		if name == "optional"
			&& compact_value == "true"
			&& let Some(dependency) = table_dependency
		{
			optional_dependencies.push(dependency.trim_matches(['"', '\'']).to_owned());
		}
	}
	features.extend(
		optional_dependencies
			.into_iter()
			.filter(|dependency| !suppressed_dependencies.contains(dependency)),
	);
	features.sort();
	features.dedup();
	features
}

fn collect_suppressed_dependencies(value: &str, dependencies: &mut Vec<String>) {
	dependencies.extend(
		value
			.split(['"', '\''])
			.filter_map(|item| item.trim().strip_prefix("dep:"))
			.map(str::to_owned),
	);
}

fn toml_array_depth(value: &str) -> i32 {
	let mut depth = 0;
	let mut quote = None;
	let mut escaped = false;
	for character in value.chars() {
		match quote {
			Some('"') if escaped => escaped = false,
			Some('"') if character == '\\' => escaped = true,
			Some(active) if character == active => quote = None,
			None if character == '"' || character == '\'' => quote = Some(character),
			None if character == '[' => depth += 1,
			None if character == ']' => depth -= 1,
			_ => {}
		}
	}
	depth
}

fn strip_toml_comment(line: &str) -> &str {
	let mut quote = None;
	let mut escaped = false;
	for (index, character) in line.char_indices() {
		match quote {
			Some('"') if escaped => escaped = false,
			Some('"') if character == '\\' => escaped = true,
			Some(active) if character == active => quote = None,
			None if character == '"' || character == '\'' => quote = Some(character),
			None if character == '#' => return &line[..index],
			_ => {}
		}
	}
	line
}

fn cargo_feature_name(env_name: &str, declared: &[String]) -> Option<String> {
	let mut matches = declared
		.iter()
		.filter(|feature| feature.to_ascii_uppercase().replace('-', "_") == env_name);
	let Some(feature) = matches.next() else {
		return Some(env_name.to_ascii_lowercase());
	};
	if matches.next().is_some() {
		None
	} else {
		Some(feature.clone())
	}
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

fn cargo_invocation_has_target() -> Option<bool> {
	let command = cargo_process_command_line()?;
	Some(
		command
			.split_whitespace()
			.any(|argument| argument == "--target" || argument.strip_prefix("--target=").is_some()),
	)
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
	Some(command.split_whitespace().any(|argument| {
		UNSUPPORTED_CARGO_FLAGS.iter().any(|flag| {
			argument == *flag
				|| argument
					.strip_prefix(flag)
					.is_some_and(|suffix| suffix.starts_with('='))
		})
	}))
}

fn main() {
	let declared = declared_feature_names();
	let mut feature_names_supported = true;
	let mut features = Vec::new();
	for feature in
		env::vars().filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
	{
		match cargo_feature_name(&feature, &declared) {
			Some(feature) => features.push(feature),
			None => feature_names_supported = false,
		}
	}
	features.sort();
	features.dedup();
	println!(
		"cargo:rustc-env=REINHARDT_ENABLED_FEATURES={}",
		features.join(",")
	);
	match cargo_invocation_has_target() {
		Some(true) => {
			println!("cargo:rustc-env=REINHARDT_TARGET_EXPLICIT=true");
			if let Ok(target) = env::var("TARGET") {
				println!("cargo:rustc-env=REINHARDT_TARGET={target}");
			}
		}
		Some(false) => println!("cargo:rustc-env=REINHARDT_TARGET_EXPLICIT=false"),
		None => println!("cargo:rustc-env=REINHARDT_TARGET_EXPLICIT=unknown"),
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
		&& feature_names_supported
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
	println!("cargo::rustc-check-cfg=cfg(wasm)");
	println!("cargo::rustc-check-cfg=cfg(native)");

	cfg_aliases! {
		wasm: { all(target_family = "wasm", target_os = "unknown") },
		native: { not(all(target_family = "wasm", target_os = "unknown")) },
	};
}
