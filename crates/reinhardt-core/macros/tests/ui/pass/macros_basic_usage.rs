//! Test: Basic usage should compile successfully

use reinhardt_macros::installed_apps;

fn main() {
	installed_apps! {
		auth: "myproject.auth",
		contenttypes: "myproject.contenttypes",
	}

	// Should be able to use the generated enum
	let _app = InstalledApp::auth;
	let _apps = InstalledApp::all_apps();

	println!("Test passed!");
}
