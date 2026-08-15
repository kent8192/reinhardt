use reinhardt_auth_macros::guard;

// The fixture mirrors the external crate path emitted by `guard!`; the generated
// type is referenced through this public module, so suppress its fixture-only
// visibility warning.
#[allow(unreachable_pub)]
mod reinhardt_auth {
	pub mod guard {
		use core::marker::PhantomData;

		pub struct All<T>(PhantomData<T>);
		pub struct Guard<T>(PhantomData<T>);

		pub trait Marker {
			fn marker() -> &'static str;
		}

		impl Marker for Guard<All<(super::super::IsAuthenticated, super::super::IsAdminUser)>> {
			fn marker() -> &'static str {
				"and"
			}
		}
	}
}

struct IsAuthenticated;
struct IsAdminUser;

type GeneratedGuard = guard!(IsAuthenticated & IsAdminUser);

#[test]
fn guard_macro_expands_to_expected_type() {
	assert_eq!(
		<GeneratedGuard as reinhardt_auth::guard::Marker>::marker(),
		"and"
	);
}
