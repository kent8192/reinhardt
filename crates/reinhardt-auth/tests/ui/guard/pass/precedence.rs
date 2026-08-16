use reinhardt_auth::{guard, AllowAny, IsAuthenticated};

type IsAdminUser = AllowAny;
type IsActiveUser = AllowAny;

type GuardType = guard!(IsAuthenticated & !IsAdminUser | IsActiveUser);

fn main() {
	let _: GuardType = Default::default();
}
