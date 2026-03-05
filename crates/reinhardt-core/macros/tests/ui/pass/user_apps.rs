//! Test: User-defined apps should compile successfully

use reinhardt_macros::installed_apps;

fn main() {
	installed_apps! {
		auth: "myproject.auth",
		myapp: "apps.myapp",
		custom: "custom.app",
	}

	let apps = InstalledApp::all_apps();
	assert_eq!(apps.len(), 3);
	assert!(apps.contains(&"apps.myapp".to_string()));

	println!("Test passed!");
}
