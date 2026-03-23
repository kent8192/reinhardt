use reinhardt_macros::settings;

#[settings(CacheSettings | cache: SomeOtherType)]
pub struct BadSettings;

fn main() {}
