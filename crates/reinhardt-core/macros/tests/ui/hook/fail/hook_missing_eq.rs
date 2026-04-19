use reinhardt_macros::hook;

#[hook(on runserver)]
struct MyHook;

fn main() {}
