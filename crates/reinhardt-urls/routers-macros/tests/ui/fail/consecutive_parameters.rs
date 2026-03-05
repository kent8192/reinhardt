use reinhardt_routers_macros::path;

fn main() {
	// Error: consecutive parameters without separator
	let _path = path!("/users/{id}{name}/");
}
