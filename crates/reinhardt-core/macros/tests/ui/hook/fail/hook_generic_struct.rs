use reinhardt_macros::hook;

#[hook(on = runserver)]
struct MyHook<T>;

fn main() {}
