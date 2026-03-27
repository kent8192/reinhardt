use reinhardt_macros::settings;

#[settings(db: Foo | db: Bar)]
pub struct BadSettings;

fn main() {}
