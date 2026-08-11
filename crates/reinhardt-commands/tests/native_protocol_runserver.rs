//! Reachability test for the generated native HTTP/WebSocket/gRPC launch path.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};

fn fixture_source(relative: &str) -> &'static str {
	match relative {
		"Cargo.toml" => include_str!("fixtures/native_protocol_project/Cargo.toml.tpl"),
		"build.rs" => include_str!("fixtures/native_protocol_project/build.rs.tpl"),
		"proto/services.proto" => {
			include_str!("fixtures/native_protocol_project/proto/services.proto")
		}
		"src/lib.rs" => include_str!("fixtures/native_protocol_project/src/lib.rs.tpl"),
		"src/bin/manage.rs" => {
			include_str!("fixtures/native_protocol_project/src/bin/manage.rs.tpl")
		}
		"src/bin/probe.rs" => include_str!("fixtures/native_protocol_project/src/bin/probe.rs.tpl"),
		_ => panic!("unknown fixture file {relative}"),
	}
}

fn materialize_fixture(root: &Path) {
	let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../..")
		.canonicalize()
		.expect("resolve workspace root");
	let workspace_root = workspace_root.to_string_lossy();
	for relative in [
		"Cargo.toml",
		"build.rs",
		"proto/services.proto",
		"src/lib.rs",
		"src/bin/manage.rs",
		"src/bin/probe.rs",
	] {
		let destination = root.join(relative);
		if let Some(parent) = destination.parent() {
			std::fs::create_dir_all(parent).expect("create fixture directory");
		}
		let content = fixture_source(relative).replace("{{ workspace_root }}", &workspace_root);
		std::fs::write(destination, content).expect("write fixture file");
	}
}

async fn unused_local_address() -> SocketAddr {
	TcpListener::bind("127.0.0.1:0")
		.await
		.expect("allocate local port")
		.local_addr()
		.expect("read local address")
}

async fn wait_for_listener(address: SocketAddr, child: &mut Child) {
	timeout(Duration::from_secs(300), async {
		loop {
			if let Ok(Some(status)) = child.try_wait() {
				panic!("generated manage process exited before binding HTTP: {status}");
			}
			if tokio::net::TcpStream::connect(address).await.is_ok() {
				return;
			}
			sleep(Duration::from_millis(100)).await;
		}
	})
	.await
	.expect("native protocol fixture did not bind its HTTP listener");
}

fn shared_target_dir() -> PathBuf {
	std::env::current_exe()
		.expect("resolve test executable")
		.parent()
		.and_then(Path::parent)
		.and_then(Path::parent)
		.expect("resolve shared Cargo target directory")
		.to_path_buf()
}

async fn spawn_manage(root: &Path, http: SocketAddr, grpc: SocketAddr) -> Child {
	Command::new(env!("CARGO"))
		.current_dir(root)
		.env("CARGO_TARGET_DIR", shared_target_dir())
		.args([
			"run",
			"--manifest-path",
			"Cargo.toml",
			"--bin",
			"manage",
			"--",
			"runserver",
			&http.to_string(),
			"--grpc-address",
			&grpc.to_string(),
			"--noreload",
		])
		.kill_on_drop(true)
		.spawn()
		.expect("spawn generated manage runserver")
}

#[tokio::test]
#[ignore = "compiles an isolated generated project; run with --ignored for the full socket proof"]
async fn generated_manage_serves_two_apps_on_native_protocols() {
	let temp = TempDir::new().expect("create fixture directory");
	let root: PathBuf = temp.path().to_path_buf();
	materialize_fixture(&root);

	let http = unused_local_address().await;
	let grpc = unused_local_address().await;
	let mut server = spawn_manage(&root, http, grpc).await;
	wait_for_listener(http, &mut server).await;

	let probe = Command::new(env!("CARGO"))
		.current_dir(&root)
		.env("CARGO_TARGET_DIR", shared_target_dir())
		.args([
			"run",
			"--quiet",
			"--manifest-path",
			"Cargo.toml",
			"--bin",
			"probe",
			"--",
			&http.to_string(),
			&grpc.to_string(),
		])
		.output();
	let output = timeout(Duration::from_secs(180), probe)
		.await
		.expect("native protocol probe timed out")
		.expect("spawn native protocol probe");
	assert!(
		output.status.success(),
		"native protocol probe failed:\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	for expected in [
		"HTTP_A=app-a",
		"HTTP_B=app-b",
		"WS=Text(\"app-a:ping\")",
		"GRPC_A=app-a:ping",
		"GRPC_B=app-b:ping",
	] {
		assert!(
			stdout.contains(expected),
			"probe output missing {expected}: {stdout}"
		);
	}

	server.kill().await.expect("stop generated manage server");
	let _ = server.wait().await;
}
