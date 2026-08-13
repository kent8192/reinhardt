#[tokio::main]
async fn main() {
	if let Err(error) = reinhardt::commands::execute_from_command_line().await {
		eprintln!("{error}");
		std::process::exit(1);
	}
}
