pub mod server_fn;

#[app_config(name = "polls", label = "polls")]
pub struct PollsConfig;

pub mod urls {
	pub mod server_router;
}
