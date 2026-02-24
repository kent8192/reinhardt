//! Test: Single app should compile successfully

use reinhardt_macros::installed_apps;

fn main() {
	installed_apps! {
		auth: "myproject.auth",
	}

	let app = InstalledApp::auth;
	assert_eq!(app.path(), "myproject.auth");

	println!("Test passed!");
}
