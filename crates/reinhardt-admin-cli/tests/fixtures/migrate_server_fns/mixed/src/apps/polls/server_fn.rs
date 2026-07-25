use reinhardt::server_fn;

#[server_fn]
pub async fn automatic() {}

#[server_fn(auto_register = false)]
pub async fn manual() {}
