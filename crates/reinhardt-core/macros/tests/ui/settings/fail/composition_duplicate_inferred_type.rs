use reinhardt_macros::settings;

#[settings(CacheSettings | my_cache: CacheSettings)]
pub struct BadSettings;

fn main() {}
