use reinhardt_routers_macros::path;

fn main() {
	// Error: wildcard not at end of path
	let _path = path!("/files/*/download");
}
