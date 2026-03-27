use reinhardt_macros::settings;

#[settings(core: CoreSettings | !CoreSettings)]
pub struct BadSettings;

fn main() {}
