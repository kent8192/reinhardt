//! Test: Duplicate labels should cause issues at usage time

use reinhardt_macros::installed_apps;

fn main() {
	// This will compile but create duplicate enum variants
	// which should cause an error
	installed_apps! {
		auth: "myproject.auth",
		auth: "myproject.sessions",
	}
}
