use reinhardt_auth::guard;

type UnsupportedGuard = guard!(HasPerm("blog\"edit"));

fn main() {}
