use reinhardt::settings;
use reinhardt::http::{Response, ViewResult};
use reinhardt::prelude::*;
use reinhardt::urls::prelude::UnifiedRouter;
use reinhardt::{get, routes};

#[get("/mounted"__MOUNTED_AUTH__)]
async fn mounted_endpoint() -> ViewResult<Response> {
	Ok(Response::ok())
}

#[get("/unmounted"__UNMOUNTED_AUTH__)]
async fn unmounted_endpoint() -> ViewResult<Response> {
	Ok(Response::ok())
}

#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new().endpoint(mounted_endpoint)
}

#[settings(fragment = true, section = "verification")]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct VerificationSettings {
	#[setting(required, secret)]
	pub secret: String,
	#[setting(required)]
	pub values: Vec<String>,
	#[setting(required)]
	pub secrets: std::collections::BTreeMap<u16, u32>,
}

#[settings(core: CoreSettings | contacts: ContactSettings | migrations: MigrationSettings | verification: VerificationSettings)]
pub struct ProjectSettings;

__MODEL__

__BROKEN__
