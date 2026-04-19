use reinhardt_macros::routes;

struct UnifiedRouter;

#[routes]
pub fn routes(#[inject] _router: UnifiedRouter) -> UnifiedRouter {
    UnifiedRouter
}

fn main() {}
