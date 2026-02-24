use reinhardt_macros::permission_required;

#[permission_required("admin.delete_all")]
async fn dangerous_operation() -> Result<(), ()> {
	Ok(())
}

fn main() {}
