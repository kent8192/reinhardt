use cfg_aliases::cfg_aliases;

fn main() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo::rustc-check-cfg=cfg(client)");
	println!("cargo::rustc-check-cfg=cfg(server)");

	cfg_aliases! {
		// Platform aliases for simpler conditional compilation
		client: { all(target_family = "wasm", target_os = "unknown") },
		server: { not(all(target_family = "wasm", target_os = "unknown")) },
	}
}
