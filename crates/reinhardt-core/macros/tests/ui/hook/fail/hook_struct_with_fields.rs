use reinhardt_macros::hook;

#[hook(on = runserver)]
struct MyHook {
    config: String,
}

fn main() {}
