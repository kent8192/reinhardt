use reinhardt as framework;

type ShellDatabase = framework::db::orm::DatabaseConnection;
type ShellDi = std::sync::Arc<framework::di::InjectionContext>;

#[test]
fn generated_shell_aliases_compile_with_shell_and_backend_features() {
	let database_type = std::any::type_name::<ShellDatabase>();
	let di_type = std::any::type_name::<ShellDi>();

	assert_eq!(
		database_type,
		"reinhardt_db::orm::connection::DatabaseConnection"
	);
	assert_eq!(
		di_type,
		"alloc::sync::Arc<reinhardt_di::context::InjectionContext>"
	);
}
