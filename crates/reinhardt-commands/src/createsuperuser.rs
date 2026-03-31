//! Implementation of the `createsuperuser` management command.
//!
//! Creates a superuser account interactively or via CLI arguments.
//! Delegates actual user creation to the registered [`SuperuserCreator`]
//! from `reinhardt-auth`, which supports any user model decorated with
//! `#[user(...)]` + `#[model(...)]` macros.

use console::style;
use dialoguer::{Confirm, Input, Password};

fn validate_email(email: &str) -> bool {
	email.contains('@') && email.contains('.')
}

fn validate_username(username: &str) -> bool {
	!username.is_empty() && username.len() >= 3
}

/// Execute the `createsuperuser` management command.
///
/// Collects username, email, and password via interactive prompts or
/// CLI arguments, then delegates to the registered [`SuperuserCreator`].
pub(crate) async fn execute_createsuperuser(
	username: Option<String>,
	email: Option<String>,
	no_password: bool,
	noinput: bool,
	database: Option<String>,
	_verbosity: u8,
) -> Result<(), Box<dyn std::error::Error>> {
	println!("{}", style("Creating superuser account").cyan().bold());
	println!();

	// Get username
	let username = if let Some(username) = username {
		if noinput && !validate_username(&username) {
			eprintln!(
				"{}",
				style("Error: Username must be at least 3 characters").red()
			);
			std::process::exit(1);
		}
		username
	} else if noinput {
		eprintln!(
			"{}",
			style("Error: --username is required in non-interactive mode").red()
		);
		std::process::exit(1);
	} else {
		Input::<String>::new()
			.with_prompt("Username")
			.validate_with(|input: &String| -> Result<(), &str> {
				if validate_username(input) {
					Ok(())
				} else {
					Err("Username must be at least 3 characters")
				}
			})
			.interact_text()?
	};

	// Get email
	let email = if let Some(email) = email {
		if noinput && !validate_email(&email) {
			eprintln!("{}", style("Error: Invalid email address").red());
			std::process::exit(1);
		}
		email
	} else if noinput {
		eprintln!(
			"{}",
			style("Error: --email is required in non-interactive mode").red()
		);
		std::process::exit(1);
	} else {
		Input::<String>::new()
			.with_prompt("Email address")
			.validate_with(|input: &String| -> Result<(), &str> {
				if validate_email(input) {
					Ok(())
				} else {
					Err("Invalid email address")
				}
			})
			.interact_text()?
	};

	// Get password
	let password = if no_password {
		println!(
			"{}",
			style("Warning: Superuser created without password").yellow()
		);
		None
	} else if noinput {
		eprintln!(
			"{}",
			style("Error: Cannot set password in non-interactive mode without --no-password").red()
		);
		std::process::exit(1);
	} else {
		let password = Password::new()
			.with_prompt("Password")
			.with_confirmation("Password (again)", "Error: Passwords do not match")
			.validate_with(|input: &String| -> Result<(), &str> {
				if input.len() >= 8 {
					Ok(())
				} else {
					Err("Password must be at least 8 characters")
				}
			})
			.interact()?;
		Some(password)
	};

	println!();
	println!("{}", style("Superuser details:").green().bold());
	println!("  Username: {}", style(&username).yellow());
	println!("  Email:    {}", style(&email).yellow());
	if password.is_some() {
		println!("  Password: {}", style("(set)").green());
	} else {
		println!("  Password: {}", style("(not set)").red());
	}

	// Confirmation in interactive mode
	if !noinput {
		println!();
		let confirmed = Confirm::new()
			.with_prompt("Create superuser?")
			.default(true)
			.interact()?;

		if !confirmed {
			println!("{}", style("Superuser creation cancelled").yellow());
			return Ok(());
		}
	}

	// Warn if deprecated --database flag was used (#3186)
	if database.is_some() {
		eprintln!(
			"{}",
			style(
				"Warning: --database flag is deprecated. Database URL is resolved from reinhardt-conf settings."
			)
			.yellow()
		);
	}

	// Create the user via the registered SuperuserCreator
	println!();
	println!("{}", style("Creating user in database...").cyan());

	let creator = reinhardt_auth::get_superuser_creator().ok_or(
		"No SuperuserCreator registered. Ensure your user model has \
		 #[user(hasher = ..., username_field = \"...\", full = true)] and \
		 #[model(...)]. Auto-registration happens automatically.\n\
		 \n\
		 If implementing BaseUser manually, also implement SuperuserInit \
		 and call register_superuser_creator(superuser_creator_for::<YourUser>()) \
		 before execute_from_command_line().\n\
		 See reinhardt_auth::SuperuserInit documentation for details.",
	)?;

	match creator
		.create_superuser(&username, &email, password.as_deref())
		.await
	{
		Ok(()) => {
			println!(
				"{}",
				style("Superuser created successfully!").green().bold()
			);
			println!();
			println!("  Username: {}", style(&username).yellow());
			println!("  Email:    {}", style(&email).yellow());
		}
		Err(e) => {
			eprintln!("{}", style(format!("Error: {}", e)).red().bold());
			std::process::exit(1);
		}
	}

	Ok(())
}
