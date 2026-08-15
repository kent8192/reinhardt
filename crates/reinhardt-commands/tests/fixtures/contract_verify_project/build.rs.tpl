use std::env;
use std::path::Path;
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
	let mut features: Vec<_> = env::vars()
		.filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
		.map(|feature| feature.to_ascii_lowercase())
		.collect();
	features.sort();
	println!("cargo:rustc-env=REINHARDT_ENABLED_FEATURES={}", features.join(","));
	if let Ok(target) = env::var("TARGET") {
		println!("cargo:rustc-env=REINHARDT_TARGET={target}");
	}
	if let Some(profile) = selected_profile().or_else(|| env::var("PROFILE").ok()) {
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
}
