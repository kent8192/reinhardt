use reinhardt_macros::hook;

#[hook(on = invalid_target)]
struct MyHook;

fn main() {}
