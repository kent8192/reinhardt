use reinhardt_pages::reactive::RetryPolicy;

#[derive(Clone)]
struct AppError;

fn main() {
	let prefix = String::from("transient");
	let _policy = RetryPolicy::<AppError>::exponential().when(|_| !prefix.is_empty());
}
