use reinhardt_testkit_macros::with_di_overrides;

mod support {
	#[derive(Clone)]
	pub struct SingletonCfg {
		pub key: &'static str,
	}

	pub struct RequestValue;
	pub struct SingletonFactory;
	pub struct RequestFactory;
	pub struct TransientFactory;
}

fn main() {
	let _future = async {
		let __scope = "caller scope";
		let __builder = "caller builder";
		let _result = with_di_overrides! {
			singleton support::SingletonCfg { key: "test" },
			request support::RequestValue,
			singleton support::SingletonFactory => move |_ctx| async move {
				let caller_bindings: (&'static str, &'static str) = (__scope, __builder);
				let _ = caller_bindings;
				Ok::<_, ::reinhardt_testkit::DiError>(support::SingletonFactory)
			},
			request support::RequestFactory => |_ctx| async {
				Ok::<_, ::reinhardt_testkit::DiError>(support::RequestFactory)
			},
			transient support::TransientFactory => |_ctx| async {
				Ok::<_, ::reinhardt_testkit::DiError>(support::TransientFactory)
			},
		};
		let _caller_bindings = (__scope, __builder);
	};
}
