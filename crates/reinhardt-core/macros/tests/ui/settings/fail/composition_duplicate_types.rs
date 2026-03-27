use reinhardt_macros::settings;

#[settings(primary: Foo | replica: Foo)]
pub struct BadSettings;

fn main() {}
